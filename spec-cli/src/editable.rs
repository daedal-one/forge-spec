//! Lossless indexing and rewriting for one forge-spec Markdown document.
//!
//! The semantic parser remains authoritative for meaning. This representation
//! exists solely to locate the smallest source spans that a typed mutation may
//! replace while preserving every untouched byte.

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::model::document::SpecDocument;
use crate::parse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterSpan {
    pub key: String,
    /// The complete key/value node, including its trailing line ending.
    pub range: Range<usize>,
    /// The first physical line for style inspection.
    pub first_line: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingSpan {
    pub level: u8,
    pub title: String,
    pub path: Vec<String>,
    pub heading: Range<usize>,
    /// Markdown governed by this heading, excluding the heading itself.
    pub section: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedBlockSpan {
    pub id: String,
    pub range: Range<usize>,
    pub body: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseSpan {
    pub id: String,
    pub block: String,
    pub range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSpan {
    pub reference: String,
    pub range: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct EditableDocument {
    pub source_path: PathBuf,
    pub original: Vec<u8>,
    pub text: String,
    pub bom: bool,
    pub line_ending: LineEnding,
    pub frontmatter: BTreeMap<String, FrontmatterSpan>,
    pub headings: Vec<HeadingSpan>,
    pub blocks: Vec<TypedBlockSpan>,
    pub clauses: Vec<ClauseSpan>,
    pub references: Vec<ReferenceSpan>,
    pub semantic: SpecDocument,
    yaml_range: Range<usize>,
    body_range: Range<usize>,
}

impl EditableDocument {
    pub fn load(path: &Path) -> Result<Self> {
        let original =
            std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Self::from_bytes(path, original)
    }

    pub fn from_bytes(path: &Path, original: Vec<u8>) -> Result<Self> {
        let text = String::from_utf8(original.clone())
            .with_context(|| format!("{} is not UTF-8", path.display()))?;
        Self::from_text(path, text, original)
    }

    pub fn from_text(path: &Path, text: String, original: Vec<u8>) -> Result<Self> {
        let bom = text.starts_with('\u{feff}');
        let line_ending = if text.contains("\r\n") {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        };
        let semantic = parse::parse_content(path, &text)?;
        let (yaml_range, body_range) = document_ranges(&text)?;
        let frontmatter = index_frontmatter(&text, yaml_range.clone());
        let headings = index_headings(&text, body_range.clone());
        let (blocks, clauses) = index_blocks_and_clauses(&text, &semantic);
        let references = index_references(&text, body_range.clone());

        Ok(Self {
            source_path: path.to_path_buf(),
            original,
            text,
            bom,
            line_ending,
            frontmatter,
            headings,
            blocks,
            clauses,
            references,
            semantic,
            yaml_range,
            body_range,
        })
    }

    pub fn id(&self) -> String {
        self.semantic.id_str()
    }

    pub fn reparse(&mut self, text: String) -> Result<()> {
        *self = Self::from_text(&self.source_path, text, self.original.clone())?;
        Ok(())
    }

    pub fn replace_frontmatter_scalar(&mut self, key: &str, value: &str) -> Result<()> {
        self.replace_frontmatter_value(key, &yaml_scalar(value))
    }

    pub fn replace_frontmatter_bool(&mut self, key: &str, value: bool) -> Result<()> {
        self.replace_frontmatter_value(key, if value { "true" } else { "false" })
    }

    pub fn replace_frontmatter_list(&mut self, key: &str, values: &[String]) -> Result<()> {
        let eol = self.line_ending.as_str();
        let rendered = if let Some(span) = self.frontmatter.get(key) {
            let first = &self.text[span.first_line.clone()];
            let after = first
                .split_once(':')
                .map(|(_, value)| value.trim())
                .unwrap_or("");
            if after.starts_with('[') {
                let items = values
                    .iter()
                    .map(|value| yaml_flow_scalar(value))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{key}: [{items}]{eol}")
            } else {
                render_block_list(key, values, eol)
            }
        } else if values.is_empty() {
            format!("{key}: []{eol}")
        } else {
            render_block_list(key, values, eol)
        };
        self.replace_or_append_frontmatter(key, rendered)
    }

    pub fn remove_frontmatter_key(&mut self, key: &str) -> Result<()> {
        let Some(span) = self.frontmatter.get(key) else {
            return Ok(());
        };
        self.replace_range(span.range.clone(), "")
    }

    pub fn frontmatter_list(&self, key: &str) -> Result<Vec<String>> {
        let (yaml, _, _) = parse::frontmatter::split_frontmatter(&self.text)?;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
        let Some(node) = value.get(key) else {
            return Ok(Vec::new());
        };
        let sequence = node
            .as_sequence()
            .ok_or_else(|| anyhow::anyhow!("frontmatter key '{key}' is not a list"))?;
        sequence
            .iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    anyhow::anyhow!("frontmatter list '{key}' contains a non-string")
                })
            })
            .collect()
    }

    pub fn add_frontmatter_list_item(&mut self, key: &str, value: &str) -> Result<()> {
        if self.frontmatter_list(key)?.iter().any(|item| item == value) {
            return Ok(());
        }
        let Some(span) = self.frontmatter.get(key).cloned() else {
            return self.replace_frontmatter_list(key, &[value.to_string()]);
        };
        let first = &self.text[span.first_line.clone()];
        let line = first.trim_end_matches(['\r', '\n']);
        let after = line
            .split_once(':')
            .map(|(_, value)| value.trim())
            .unwrap_or("");
        if after.starts_with('[') {
            let close = first.rfind(']').ok_or_else(|| {
                anyhow::anyhow!("frontmatter list '{key}' has no closing bracket")
            })?;
            let absolute = span.first_line.start + close;
            let separator = if after == "[]" { "" } else { ", " };
            return self.replace_range(
                absolute..absolute,
                &format!("{separator}{}", yaml_flow_scalar(value)),
            );
        }
        if !after.is_empty() {
            bail!("frontmatter key '{key}' is not a list");
        }
        let mut insertion = String::new();
        if span.range.end > 0 && !self.text[..span.range.end].ends_with('\n') {
            insertion.push_str(self.line_ending.as_str());
        }
        insertion.push_str("  - ");
        insertion.push_str(&yaml_scalar(value));
        insertion.push_str(self.line_ending.as_str());
        self.replace_range(span.range.end..span.range.end, &insertion)
    }

    pub fn remove_frontmatter_list_item(&mut self, key: &str, value: &str) -> Result<()> {
        let Some(span) = self.frontmatter.get(key).cloned() else {
            return Ok(());
        };
        let current = self.frontmatter_list(key)?;
        if !current.iter().any(|item| item == value) {
            return Ok(());
        }
        let first = &self.text[span.first_line.clone()];
        let line = first.trim_end_matches(['\r', '\n']);
        let after = line
            .split_once(':')
            .map(|(_, value)| value.trim())
            .unwrap_or("");
        if after.starts_with('[') {
            let values = current
                .into_iter()
                .filter(|item| item != value)
                .collect::<Vec<_>>();
            return self.replace_flow_list_contents(&span, &values);
        }
        let ranges = block_list_item_ranges(&self.text, &span, value);
        let remaining = current.iter().filter(|item| *item != value).count();
        let mut replacements = ranges
            .into_iter()
            .map(|range| (range, String::new()))
            .collect::<Vec<_>>();
        if remaining == 0 {
            replacements.push((
                span.first_line.clone(),
                format!("{key}: []{}", self.line_ending.as_str()),
            ));
        }
        self.replace_ranges(replacements)
    }

    pub fn replace_frontmatter_list_item(&mut self, key: &str, old: &str, new: &str) -> Result<()> {
        if old == new {
            return Ok(());
        }
        let Some(span) = self.frontmatter.get(key).cloned() else {
            return Ok(());
        };
        let current = self.frontmatter_list(key)?;
        if !current.iter().any(|item| item == old) {
            return Ok(());
        }
        let first = &self.text[span.first_line.clone()];
        let line = first.trim_end_matches(['\r', '\n']);
        let after = line
            .split_once(':')
            .map(|(_, value)| value.trim())
            .unwrap_or("");
        if after.starts_with('[') {
            let values = current
                .into_iter()
                .map(|item| if item == old { new.to_string() } else { item })
                .collect::<Vec<_>>();
            return self.replace_flow_list_contents(&span, &values);
        }
        let ranges = block_list_item_ranges(&self.text, &span, old);
        let replacements = ranges
            .into_iter()
            .map(|range| {
                let line = &self.text[range.clone()];
                let eol = if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                let without_eol = line.trim_end_matches(['\r', '\n']);
                let marker = without_eol
                    .find("- ")
                    .ok_or_else(|| anyhow::anyhow!("invalid block list item"))?;
                let prefix = &without_eol[..marker + 2];
                let comment = without_eol[marker + 2..]
                    .find(" #")
                    .map(|index| &without_eol[marker + 2 + index..])
                    .unwrap_or("");
                Ok((range, format!("{prefix}{}{comment}{eol}", yaml_scalar(new))))
            })
            .collect::<Result<Vec<_>>>()?;
        self.replace_ranges(replacements)
    }

    pub fn replace_title(&mut self, title: &str) -> Result<()> {
        let matches: Vec<_> = self
            .headings
            .iter()
            .filter(|heading| heading.level == 1)
            .collect();
        if matches.len() != 1 {
            bail!(
                "title selector requires exactly one level-one heading (found {})",
                matches.len()
            );
        }
        let heading = matches[0];
        let current = &self.text[heading.heading.clone()];
        let eol = if current.ends_with("\r\n") {
            "\r\n"
        } else if current.ends_with('\n') {
            "\n"
        } else {
            ""
        };
        self.replace_range(heading.heading.clone(), &format!("# {title}{eol}"))
    }

    pub fn replace_section(&mut self, path: &[String], markdown: &str) -> Result<()> {
        let matches: Vec<_> = self
            .headings
            .iter()
            .filter(|heading| heading.path == path)
            .collect();
        if matches.len() != 1 {
            bail!(
                "heading selector '{}' is {}",
                path.join(" > "),
                if matches.is_empty() {
                    "missing"
                } else {
                    "ambiguous"
                }
            );
        }
        let range = matches[0].section.clone();
        let replacement = normalize_fragment(markdown, self.line_ending, true);
        self.replace_range(range, &replacement)
    }

    pub fn replace_block(&mut self, id: &str, markdown: &str) -> Result<()> {
        let matches: Vec<_> = self.blocks.iter().filter(|block| block.id == id).collect();
        if matches.len() != 1 {
            bail!("typed block '{id}' is missing or ambiguous");
        }
        let range = matches[0].body.clone();
        self.replace_range(
            range,
            &normalize_fragment(markdown, self.line_ending, false),
        )
    }

    pub fn add_block(
        &mut self,
        heading_path: &[String],
        kind: &str,
        id: &str,
        level: Option<&str>,
        markdown: &str,
    ) -> Result<()> {
        if self.blocks.iter().any(|block| block.id == id) {
            bail!("typed block id '{id}' already exists");
        }
        let matches: Vec<_> = self
            .headings
            .iter()
            .filter(|heading| heading.path == heading_path)
            .collect();
        if matches.len() != 1 {
            bail!(
                "heading selector '{}' is missing or ambiguous",
                heading_path.join(" > ")
            );
        }
        let eol = self.line_ending.as_str();
        let level = level
            .map(|value| format!(" level=\"{value}\""))
            .unwrap_or_default();
        let body = normalize_fragment(markdown, self.line_ending, false);
        let block = format!("{eol}:::{{{kind} id=\"{id}\"{level}}}{eol}{body}:::{eol}");
        let insertion = matches[0].section.end;
        self.replace_range(insertion..insertion, &block)
    }

    pub fn remove_block(&mut self, id: &str) -> Result<()> {
        let matches: Vec<_> = self.blocks.iter().filter(|block| block.id == id).collect();
        if matches.len() != 1 {
            bail!("typed block '{id}' is missing or ambiguous");
        }
        self.replace_range(matches[0].range.clone(), "")
    }

    pub fn replace_clause(&mut self, block: &str, clause: &str, markdown: &str) -> Result<()> {
        let matches: Vec<_> = self
            .clauses
            .iter()
            .filter(|entry| entry.block == block && entry.id == clause)
            .collect();
        if matches.len() != 1 {
            bail!("clause '{block}#{clause}' is missing or ambiguous");
        }
        let current = &self.text[matches[0].range.clone()];
        let prefix = current
            .find("{#")
            .map(|index| &current[..index])
            .unwrap_or("- ");
        let eol = if current.ends_with("\r\n") {
            "\r\n"
        } else if current.ends_with('\n') {
            "\n"
        } else {
            ""
        };
        let replacement = format!("{prefix}{{#{clause}}} {}{eol}", markdown.trim());
        self.replace_range(matches[0].range.clone(), &replacement)
    }

    pub fn add_clause(&mut self, block: &str, clause: &str, markdown: &str) -> Result<()> {
        let matches: Vec<_> = self
            .blocks
            .iter()
            .filter(|entry| entry.id == block)
            .collect();
        if matches.len() != 1 {
            bail!("typed block '{block}' is missing or ambiguous");
        }
        if self.clauses.iter().any(|entry| entry.id == clause) {
            bail!("clause id '{clause}' already exists in the document");
        }
        let eol = self.line_ending.as_str();
        let insertion = matches[0].body.end;
        self.replace_range(
            insertion..insertion,
            &format!("- {{#{clause}}} {}{eol}", markdown.trim()),
        )
    }

    pub fn remove_clause(&mut self, block: &str, clause: &str) -> Result<()> {
        let matches: Vec<_> = self
            .clauses
            .iter()
            .filter(|entry| entry.block == block && entry.id == clause)
            .collect();
        if matches.len() != 1 {
            bail!("clause '{block}#{clause}' is missing or ambiguous");
        }
        self.replace_range(matches[0].range.clone(), "")
    }

    pub fn replace_reference(&mut self, old: &str, new: &str) -> Result<usize> {
        let ranges: Vec<_> = self
            .references
            .iter()
            .filter(|reference| reference.reference == old)
            .map(|reference| reference.range.clone())
            .collect();
        let count = ranges.len();
        for range in ranges.into_iter().rev() {
            self.replace_range(range, new)?;
        }
        Ok(count)
    }

    pub fn replace_reference_prefix(&mut self, old: &str, new: &str) -> Result<usize> {
        let replacements: Vec<_> = self
            .references
            .iter()
            .filter(|reference| {
                reference.reference == old
                    || reference
                        .reference
                        .strip_prefix(old)
                        .is_some_and(|suffix| suffix.starts_with('#'))
            })
            .map(|reference| {
                let replacement = format!("{new}{}", &reference.reference[old.len()..]);
                (reference.range.clone(), replacement)
            })
            .collect();
        let count = replacements.len();
        for (range, replacement) in replacements.into_iter().rev() {
            self.replace_range(range, &replacement)?;
        }
        Ok(count)
    }

    pub fn body_range(&self) -> Range<usize> {
        self.body_range.clone()
    }

    fn replace_or_append_frontmatter(&mut self, key: &str, replacement: String) -> Result<()> {
        if let Some(span) = self.frontmatter.get(key) {
            self.replace_range(span.range.clone(), &replacement)
        } else {
            self.replace_range(self.yaml_range.end..self.yaml_range.end, &replacement)
        }
    }

    fn replace_frontmatter_value(&mut self, key: &str, value: &str) -> Result<()> {
        let comment = self
            .frontmatter
            .get(key)
            .and_then(|span| inline_comment_suffix(&self.text[span.first_line.clone()]))
            .unwrap_or("");
        self.replace_or_append_frontmatter(
            key,
            format!("{key}: {value}{comment}{}", self.line_ending.as_str()),
        )
    }

    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
        self.replace_ranges(vec![(range, replacement.to_string())])
    }

    fn replace_ranges(&mut self, mut replacements: Vec<(Range<usize>, String)>) -> Result<()> {
        replacements.sort_by_key(|(range, _)| range.start);
        let mut updated = String::with_capacity(self.text.len());
        let mut cursor = 0;
        for (range, replacement) in replacements {
            if range.end > self.text.len() || range.start < cursor || range.start > range.end {
                bail!(
                    "invalid or overlapping editable span {}..{}",
                    range.start,
                    range.end
                );
            }
            updated.push_str(&self.text[cursor..range.start]);
            updated.push_str(&replacement);
            cursor = range.end;
        }
        updated.push_str(&self.text[cursor..]);
        self.reparse(updated)
    }

    fn replace_flow_list_contents(
        &mut self,
        span: &FrontmatterSpan,
        values: &[String],
    ) -> Result<()> {
        let first = &self.text[span.first_line.clone()];
        let open = first.find('[').ok_or_else(|| {
            anyhow::anyhow!("frontmatter list '{}' has no opening bracket", span.key)
        })?;
        let close = first.rfind(']').ok_or_else(|| {
            anyhow::anyhow!("frontmatter list '{}' has no closing bracket", span.key)
        })?;
        let rendered = values
            .iter()
            .map(|value| yaml_flow_scalar(value))
            .collect::<Vec<_>>()
            .join(", ");
        self.replace_range(
            span.first_line.start + open + 1..span.first_line.start + close,
            &rendered,
        )
    }
}

fn document_ranges(text: &str) -> Result<(Range<usize>, Range<usize>)> {
    let bom_len = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let opening_end = line_end(text, bom_len)
        .ok_or_else(|| anyhow::anyhow!("frontmatter opening delimiter has no line ending"))?;
    if text[bom_len..opening_end].trim_end_matches(['\r', '\n']) != "---" {
        bail!("file does not start with YAML frontmatter delimiter '---'");
    }
    let mut cursor = opening_end;
    let yaml_start = cursor;
    while cursor < text.len() {
        let end = line_end(text, cursor).unwrap_or(text.len());
        if text[cursor..end].trim_end_matches(['\r', '\n']) == "---" {
            return Ok((yaml_start..cursor, end..text.len()));
        }
        cursor = end;
    }
    bail!("no closing '---' delimiter for frontmatter")
}

fn index_frontmatter(text: &str, yaml: Range<usize>) -> BTreeMap<String, FrontmatterSpan> {
    let mut starts = Vec::<(String, usize, usize)>::new();
    let mut cursor = yaml.start;
    while cursor < yaml.end {
        let end = line_end(text, cursor).unwrap_or(yaml.end).min(yaml.end);
        let line = text[cursor..end].trim_end_matches(['\r', '\n']);
        if !line.starts_with([' ', '\t']) {
            if let Some((key, _)) = line.split_once(':') {
                if !key.is_empty()
                    && key
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
                {
                    starts.push((key.to_string(), cursor, end));
                }
            }
        }
        cursor = end;
    }
    let mut result = BTreeMap::new();
    for index in 0..starts.len() {
        let (key, start, first_end) = &starts[index];
        let raw_end = starts
            .get(index + 1)
            .map(|(_, start, _)| *start)
            .unwrap_or(yaml.end);
        let mut end = raw_end;
        let node = &text[*start..raw_end];
        let mut offset = *start;
        for line in node.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if offset >= *first_end && (trimmed.is_empty() || trimmed.starts_with('#')) {
                end = end.min(offset);
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                end = raw_end;
            }
            offset += line.len();
        }
        result.insert(
            key.clone(),
            FrontmatterSpan {
                key: key.clone(),
                range: *start..end,
                first_line: *start..*first_end,
            },
        );
    }
    result
}

fn index_headings(text: &str, body: Range<usize>) -> Vec<HeadingSpan> {
    let body_text = &text[body.clone()];
    let mut raw = Vec::<(u8, String, Range<usize>)>::new();
    let mut active: Option<(u8, String, usize)> = None;
    let mut title = String::new();
    for (event, range) in Parser::new_ext(body_text, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                active = Some((heading_level(level), String::new(), range.start));
                title.clear();
            }
            Event::Text(value) | Event::Code(value) if active.is_some() => title.push_str(&value),
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, _, start)) = active.take() {
                    let absolute_start = body.start + start;
                    let absolute_event_end = body.start + range.end;
                    let heading_content_end = text[absolute_start..absolute_event_end]
                        .trim_end_matches(['\r', '\n'])
                        .len()
                        + absolute_start;
                    let end = heading_content_end
                        .checked_sub(1)
                        .and_then(|last| line_end(text, last))
                        .unwrap_or(heading_content_end);
                    raw.push((level, title.trim().to_string(), absolute_start..end));
                }
            }
            _ => {}
        }
    }
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut result = Vec::new();
    for index in 0..raw.len() {
        let (level, title, heading) = &raw[index];
        while stack
            .last()
            .is_some_and(|(parent_level, _)| parent_level >= level)
        {
            stack.pop();
        }
        stack.push((*level, title.clone()));
        let path = stack.iter().map(|(_, title)| title.clone()).collect();
        let section_end = raw[index + 1..]
            .iter()
            .find(|(next_level, _, _)| next_level <= level)
            .map(|(_, _, range)| range.start)
            .unwrap_or(body.end);
        result.push(HeadingSpan {
            level: *level,
            title: title.clone(),
            path,
            heading: heading.clone(),
            section: heading.end..section_end,
        });
    }
    result
}

fn index_blocks_and_clauses(
    text: &str,
    semantic: &SpecDocument,
) -> (Vec<TypedBlockSpan>, Vec<ClauseSpan>) {
    let offsets = line_offsets(text);
    let mut blocks = Vec::new();
    let mut clauses = Vec::new();
    for block in &semantic.blocks {
        let start = line_offset(&offsets, block.start_line);
        let end = line_offset(&offsets, block.end_line + 1).min(text.len());
        let body_start = line_offset(&offsets, block.start_line + 1);
        let body_end = line_offset(&offsets, block.end_line).min(text.len());
        blocks.push(TypedBlockSpan {
            id: block.id.clone(),
            range: start..end,
            body: body_start..body_end,
        });
        for clause in &block.clauses {
            let clause_start = line_offset(&offsets, clause.line);
            let clause_end = line_offset(&offsets, clause.line + 1).min(text.len());
            clauses.push(ClauseSpan {
                id: clause.id.clone(),
                block: block.id.clone(),
                range: clause_start..clause_end,
            });
        }
    }
    (blocks, clauses)
}

fn index_references(text: &str, body: Range<usize>) -> Vec<ReferenceSpan> {
    let mut result = Vec::new();
    let bytes = text.as_bytes();
    let mut cursor = body.start;
    while cursor + 5 <= body.end {
        let Some(relative) = text[cursor..body.end].find("spec:") else {
            break;
        };
        let start = cursor + relative;
        let mut end = start + 5;
        while end < body.end {
            let ch = bytes[end] as char;
            if ch.is_ascii_whitespace() || matches!(ch, ')' | ']' | '>' | '"' | '\'' | '`') {
                break;
            }
            end += 1;
        }
        result.push(ReferenceSpan {
            reference: text[start..end].to_string(),
            range: start..end,
        });
        cursor = end;
    }
    result
}

fn line_offsets(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(index, _)| index + 1))
        .chain(std::iter::once(text.len()))
        .collect()
}

fn line_offset(offsets: &[usize], line: usize) -> usize {
    offsets
        .get(line.saturating_sub(1))
        .copied()
        .unwrap_or_else(|| *offsets.last().unwrap_or(&0))
}

fn line_end(text: &str, start: usize) -> Option<usize> {
    text.as_bytes()[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| start + offset + 1)
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn yaml_scalar(value: &str) -> String {
    if !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, ' ' | '-' | '_' | '.' | '/' | ':' | '#' | '@' | '+')
        })
        && value.trim() == value
        && !matches!(value, "null" | "true" | "false" | "~")
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

fn inline_comment_suffix(line: &str) -> Option<&str> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (_, value) = line.split_once(':')?;
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut escaped = false;
    for (index, ch) in value.char_indices() {
        if double_quoted && escaped {
            escaped = false;
            continue;
        }
        if double_quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !double_quoted => single_quoted = !single_quoted,
            '"' if !single_quoted => double_quoted = !double_quoted,
            '#' if !single_quoted
                && !double_quoted
                && value[..index]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace) =>
            {
                let start = value[..index].trim_end_matches([' ', '\t']).len();
                return Some(&value[start..]);
            }
            _ => {}
        }
    }
    None
}

fn yaml_flow_scalar(value: &str) -> String {
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '#' | '@' | '+')
    }) && !value.is_empty()
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

fn render_block_list(key: &str, values: &[String], eol: &str) -> String {
    let mut output = format!("{key}:{eol}");
    for value in values {
        output.push_str("  - ");
        output.push_str(&yaml_scalar(value));
        output.push_str(eol);
    }
    output
}

fn block_list_item_ranges(text: &str, span: &FrontmatterSpan, wanted: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = span.first_line.end;
    while cursor < span.range.end {
        let end = line_end(text, cursor)
            .unwrap_or(span.range.end)
            .min(span.range.end);
        let line = text[cursor..end].trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();
        if let Some(value) = trimmed.strip_prefix("- ") {
            if serde_yaml::from_str::<String>(value).ok().as_deref() == Some(wanted) {
                ranges.push(cursor..end);
            }
        }
        cursor = end;
    }
    ranges
}

fn normalize_fragment(markdown: &str, line_ending: LineEnding, surround: bool) -> String {
    let mut normalized = markdown.replace("\r\n", "\n");
    if line_ending == LineEnding::CrLf {
        normalized = normalized.replace('\n', "\r\n");
    }
    let eol = line_ending.as_str();
    if surround && !normalized.starts_with(eol) {
        normalized.insert_str(0, eol);
    }
    if !normalized.ends_with(eol) {
        normalized.push_str(eol);
    }
    if surround && !normalized.ends_with(&format!("{eol}{eol}")) {
        normalized.push_str(eol);
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> EditableDocument {
        let text = "\u{feff}---\r\nid: REQ:auth/session\r\ntype: requirement\r\nstatus: accepted\r\nsummary: >\r\n  Kept summary style.\r\nowners: [carlo]\r\nlevel: MUST\r\nrefines: []\r\n---\r\n# Session\r\n\r\n## Policy\r\n\r\n:::{requirement id=\"session\" level=\"MUST\"}\r\n- {#c-lifetime} Old clause.\r\n:::\r\n".to_string();
        EditableDocument::from_text(
            Path::new("session.spec.md"),
            text.clone(),
            text.into_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn scalar_edit_preserves_bom_crlf_and_unselected_bytes() {
        let mut document = document();
        let body = document.text[document.body_range()].to_string();
        document
            .replace_frontmatter_scalar("status", "deprecated")
            .unwrap();
        assert!(document.bom);
        assert_eq!(document.line_ending, LineEnding::CrLf);
        assert_eq!(&document.text[document.body_range()], body);
        assert!(document
            .text
            .contains("summary: >\r\n  Kept summary style.\r\n"));
    }

    #[test]
    fn scalar_edit_preserves_comments_between_keys() {
        let text = "---\nid: REQ:auth/session\ntype: requirement\nstatus: accepted # lifecycle\n# keep this comment\nsummary: Original.\nowners: [carlo]\nlevel: MUST\nrefines: []\n---\n# Session\n".to_string();
        let mut document = EditableDocument::from_text(
            Path::new("session.spec.md"),
            text.clone(),
            text.into_bytes(),
        )
        .unwrap();
        document
            .replace_frontmatter_scalar("status", "draft")
            .unwrap();
        assert!(document
            .text
            .contains("status: draft # lifecycle\n# keep this comment\nsummary:"));
    }

    #[test]
    fn block_list_mutations_preserve_comments() {
        let text = "---\nid: REQ:auth/session\ntype: requirement\nstatus: accepted\nsummary: Original.\nowners:\n  - carlo # primary\n  # retain this note\nlevel: MUST\nrefines: []\n---\n# Session\n".to_string();
        let original = text.as_bytes().to_vec();
        let mut document = EditableDocument::from_text(
            Path::new("session.spec.md"),
            text.clone(),
            original.clone(),
        )
        .unwrap();
        document
            .add_frontmatter_list_item("owners", "maya")
            .unwrap();
        document
            .replace_frontmatter_list_item("owners", "carlo", "carlo-admin")
            .unwrap();
        assert!(document.text.contains("- carlo-admin # primary"));
        assert!(document.text.contains("# retain this note"));
        assert!(document.text.contains("- maya"));
        document
            .remove_frontmatter_list_item("owners", "maya")
            .unwrap();
        assert!(!document.text.contains("- maya"));
        assert!(document.text.contains("# retain this note"));
        document
            .remove_frontmatter_list_item("owners", "carlo-admin")
            .unwrap();
        assert!(document.text.contains("owners: []\n  # retain this note"));
        assert_eq!(document.original, original);
    }

    #[test]
    fn clause_selector_is_stable_and_precise() {
        let mut document = document();
        document
            .replace_clause("session", "c-lifetime", "New clause.")
            .unwrap();
        assert!(document.text.contains("- {#c-lifetime} New clause.\r\n"));
        assert!(document
            .text
            .contains("summary: >\r\n  Kept summary style.\r\n"));
    }

    #[test]
    fn heading_paths_reject_missing_selectors() {
        let mut document = document();
        let error = document
            .replace_section(&["Session".into(), "Missing".into()], "Nope")
            .unwrap_err();
        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn section_replacement_preserves_standard_heading_spacing() {
        let mut document = document();
        document
            .replace_section(&["Session".into(), "Policy".into()], "New policy.")
            .unwrap();
        assert!(document
            .text
            .contains("## Policy\r\n\r\nNew policy.\r\n\r\n"));
        assert!(!document.text.contains("## Policy\r\n\r\n\r\n"));
    }
}
