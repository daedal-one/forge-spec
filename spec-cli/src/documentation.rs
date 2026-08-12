//! Configured, repository-relative Markdown documentation index.
//!
//! Generic documentation is navigable project knowledge, not a specification
//! entity. Its identity is a repository-relative path plus an optional
//! hierarchical Markdown heading.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

use crate::model::config::{DocumentationCollectionConfig, SpecConfig};
use crate::model::reference::SpecReference;
use crate::parse::references::parse_spec_url;

const HEADING_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`');

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DocumentationTarget {
    File,
    Heading { segments: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DocumentationReference {
    pub path: String,
    pub target: DocumentationTarget,
}

impl DocumentationReference {
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            target: DocumentationTarget::File,
        }
    }

    pub fn heading(path: impl Into<String>, segments: Vec<String>) -> Self {
        Self {
            path: path.into(),
            target: DocumentationTarget::Heading { segments },
        }
    }
}

impl std::fmt::Display for DocumentationReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "spec:doc:{}", self.path)?;
        if let DocumentationTarget::Heading { segments } = &self.target {
            formatter.write_str("#heading=")?;
            for (index, segment) in segments.iter().enumerate() {
                if index > 0 {
                    formatter.write_str("/")?;
                }
                write!(
                    formatter,
                    "{}",
                    utf8_percent_encode(segment, HEADING_SEGMENT_ENCODE_SET)
                )?;
            }
        }
        Ok(())
    }
}

pub fn decode_heading_segments(value: &str) -> Option<Vec<String>> {
    if value.is_empty() {
        return None;
    }
    value
        .split('/')
        .map(|segment| {
            if segment.is_empty() {
                return None;
            }
            percent_decode_str(segment)
                .decode_utf8()
                .ok()
                .map(|decoded| decoded.into_owned())
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentationHeading {
    pub title: String,
    pub segments: Vec<String>,
    pub level: u8,
    pub line: usize,
    pub end_line: usize,
    pub fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentationLinkTarget {
    Forge(SpecReference),
    Markdown {
        path: String,
        fragment: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentationLink {
    pub target: DocumentationLinkTarget,
    pub label: String,
    pub authored: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentationDocument {
    pub collection_id: String,
    pub collection_title: String,
    pub path: String,
    pub source_path: PathBuf,
    pub title: String,
    pub summary: Option<String>,
    pub headings: Vec<DocumentationHeading>,
    pub links: Vec<DocumentationLink>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationBacklink {
    pub source_kind: String,
    pub source: String,
    pub label: String,
    pub line: usize,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationIssue {
    pub code: String,
    pub message: String,
    pub file: PathBuf,
    pub line: Option<usize>,
}

impl DocumentationIssue {
    fn new(code: &str, message: impl Into<String>, file: PathBuf) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            file,
            line: None,
        }
    }

    fn at_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct DocumentationIndex {
    pub documents: Vec<DocumentationDocument>,
    pub issues: Vec<DocumentationIssue>,
    path_index: BTreeMap<String, usize>,
    backlinks: BTreeMap<String, Vec<DocumentationBacklink>>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredDocumentation {
    pub source_path: PathBuf,
    pub repository_path: String,
    pub collection: DocumentationCollectionConfig,
}

impl DocumentationIndex {
    pub fn load(specs_dir: &Path, config: &SpecConfig) -> Result<Self> {
        let repository_root = specs_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| specs_dir.to_path_buf());
        Self::load_from_root(&repository_root, &config.documentation)
    }

    pub fn load_from_root(
        repository_root: &Path,
        collections: &[DocumentationCollectionConfig],
    ) -> Result<Self> {
        let repository_root = canonical_or_absolute(repository_root)?;
        let (discovered, issues) = discover_documentation(&repository_root, collections)?;
        let mut documents = Vec::new();
        for entry in discovered {
            let content = std::fs::read_to_string(&entry.source_path).with_context(|| {
                format!("reading documentation {}", entry.source_path.display())
            })?;
            documents.push(parse_documentation(
                &entry.source_path,
                &entry.repository_path,
                &entry.collection,
                &content,
            ));
        }
        Ok(Self::from_documents(&repository_root, documents, issues))
    }

    pub(crate) fn from_documents(
        repository_root: &Path,
        mut documents: Vec<DocumentationDocument>,
        issues: Vec<DocumentationIssue>,
    ) -> Self {
        documents.sort_by(|left, right| left.path.cmp(&right.path));
        let path_index = documents
            .iter()
            .enumerate()
            .map(|(index, document)| (document.path.clone(), index))
            .collect();
        let mut index = Self {
            documents,
            issues,
            path_index,
            backlinks: BTreeMap::new(),
        };
        index.validate_and_build_backlinks(repository_root);
        index
    }

    pub fn get(&self, path: &str) -> Option<&DocumentationDocument> {
        self.path_index
            .get(path)
            .map(|index| &self.documents[*index])
    }

    pub fn contains_source_path(&self, path: &Path) -> bool {
        self.documents
            .iter()
            .any(|document| same_path(&document.source_path, path))
    }

    pub fn resolve(
        &self,
        reference: &DocumentationReference,
    ) -> Option<(&DocumentationDocument, Option<&DocumentationHeading>)> {
        let document = self.get(&reference.path)?;
        match &reference.target {
            DocumentationTarget::File => Some((document, None)),
            DocumentationTarget::Heading { segments } => {
                let mut matches = document
                    .headings
                    .iter()
                    .filter(|heading| heading.segments == *segments);
                let heading = matches.next()?;
                matches
                    .next()
                    .is_none()
                    .then_some((document, Some(heading)))
            }
        }
    }

    pub fn backlinks(&self, target: &str) -> &[DocumentationBacklink] {
        self.backlinks.get(target).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn backlinks_with_prefix(&self, prefix: &str) -> Vec<DocumentationBacklink> {
        self.backlinks
            .iter()
            .filter(|(target, _)| *target == prefix || target.starts_with(&format!("{prefix}#")))
            .flat_map(|(_, backlinks)| backlinks.iter().cloned())
            .collect()
    }

    pub fn canonical_target(&self, link: &DocumentationLink) -> Option<String> {
        match &link.target {
            DocumentationLinkTarget::Forge(reference) => Some(reference.to_string()),
            DocumentationLinkTarget::Markdown { path, fragment } => {
                let document = self.get(path)?;
                if let Some(fragment) = fragment {
                    let mut matches = document
                        .headings
                        .iter()
                        .filter(|heading| heading.fragment == *fragment);
                    let heading = matches.next()?;
                    if matches.next().is_some() {
                        return None;
                    }
                    Some(
                        DocumentationReference::heading(path.clone(), heading.segments.clone())
                            .to_string(),
                    )
                } else {
                    Some(DocumentationReference::file(path.clone()).to_string())
                }
            }
        }
    }

    pub fn add_specification_backlink(
        &mut self,
        target: &SpecReference,
        source: &str,
        label: &str,
        line: usize,
    ) {
        let key = target.to_string();
        self.backlinks
            .entry(key.clone())
            .or_default()
            .push(DocumentationBacklink {
                source_kind: "specification".to_string(),
                source: source.to_string(),
                label: label.to_string(),
                line,
                target: key,
            });
        self.sort_backlinks();
    }

    pub fn with_override(&self, source_path: &Path, content: &str) -> Result<Self> {
        let mut index = self.clone();
        let Some(position) = index
            .documents
            .iter()
            .position(|document| same_path(&document.source_path, source_path))
        else {
            return Ok(index);
        };
        let current = index.documents[position].clone();
        let config = DocumentationCollectionConfig {
            id: current.collection_id,
            title: current.collection_title,
            root: String::new(),
            include: Vec::new(),
            exclude: Vec::new(),
        };
        index.documents[position] =
            parse_documentation(source_path, &current.path, &config, content);
        index.path_index = index
            .documents
            .iter()
            .enumerate()
            .map(|(entry, document)| (document.path.clone(), entry))
            .collect();
        index.backlinks.clear();
        index
            .issues
            .retain(|issue| !same_path(&issue.file, source_path));
        let repository_root = source_path
            .ancestors()
            .find(|ancestor| ancestor.join(".specs/_config.toml").is_file())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| source_path.parent().unwrap_or(source_path).to_path_buf());
        index.validate_and_build_backlinks(&repository_root);
        Ok(index)
    }

    fn validate_and_build_backlinks(&mut self, repository_root: &Path) {
        let documents = self.documents.clone();
        for document in &documents {
            for link in &document.links {
                match &link.target {
                    DocumentationLinkTarget::Forge(SpecReference::Documentation(reference)) => {
                        self.validate_doc_target(document, link, reference);
                    }
                    DocumentationLinkTarget::Markdown { path, fragment } => {
                        let Some(target_document) = self.get(path) else {
                            self.issues.push(
                                DocumentationIssue::new(
                                    "R029",
                                    format!("Markdown link target '{}' is not enrolled", path),
                                    document.source_path.clone(),
                                )
                                .at_line(link.line),
                            );
                            continue;
                        };
                        let target = if let Some(fragment) = fragment {
                            let matches = target_document
                                .headings
                                .iter()
                                .filter(|heading| heading.fragment == *fragment)
                                .collect::<Vec<_>>();
                            if matches.len() != 1 {
                                self.issues.push(
                                    DocumentationIssue::new(
                                        "R029",
                                        format!(
                                            "Markdown heading fragment '#{}' does not resolve uniquely in '{}'",
                                            fragment, path
                                        ),
                                        document.source_path.clone(),
                                    )
                                    .at_line(link.line),
                                );
                                continue;
                            }
                            DocumentationReference::heading(
                                path.clone(),
                                matches[0].segments.clone(),
                            )
                        } else {
                            DocumentationReference::file(path.clone())
                        };
                        self.insert_doc_backlink(document, link, &target);
                    }
                    DocumentationLinkTarget::Forge(reference) => {
                        let key = reference.to_string();
                        self.backlinks.entry(key.clone()).or_default().push(
                            DocumentationBacklink {
                                source_kind: "documentation".to_string(),
                                source: document.path.clone(),
                                label: link.label.clone(),
                                line: link.line,
                                target: key,
                            },
                        );
                    }
                }
            }
        }
        self.issues.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| left.line.cmp(&right.line))
                .then_with(|| left.code.cmp(&right.code))
        });
        self.sort_backlinks();

        for document in &self.documents {
            if !document.source_path.starts_with(repository_root) {
                self.issues.push(DocumentationIssue::new(
                    "R026",
                    format!("documentation '{}' escaped the repository", document.path),
                    document.source_path.clone(),
                ));
            }
        }
    }

    fn validate_doc_target(
        &mut self,
        source: &DocumentationDocument,
        link: &DocumentationLink,
        target: &DocumentationReference,
    ) {
        let Some(document) = self.get(&target.path) else {
            self.issues.push(
                DocumentationIssue::new(
                    "R027",
                    format!("documentation target '{}' is not enrolled", target.path),
                    source.source_path.clone(),
                )
                .at_line(link.line),
            );
            return;
        };
        if let DocumentationTarget::Heading { segments } = &target.target {
            let count = document
                .headings
                .iter()
                .filter(|heading| heading.segments == *segments)
                .count();
            if count != 1 {
                self.issues.push(
                    DocumentationIssue::new(
                        "R028",
                        format!(
                            "documentation heading '{}' does not resolve uniquely in '{}'",
                            segments.join(" / "),
                            target.path
                        ),
                        source.source_path.clone(),
                    )
                    .at_line(link.line),
                );
                return;
            }
        }
        self.insert_doc_backlink(source, link, target);
    }

    fn insert_doc_backlink(
        &mut self,
        source: &DocumentationDocument,
        link: &DocumentationLink,
        target: &DocumentationReference,
    ) {
        let key = target.to_string();
        self.backlinks
            .entry(key.clone())
            .or_default()
            .push(DocumentationBacklink {
                source_kind: "documentation".to_string(),
                source: source.path.clone(),
                label: link.label.clone(),
                line: link.line,
                target: key,
            });
    }

    fn sort_backlinks(&mut self) {
        for backlinks in self.backlinks.values_mut() {
            backlinks.sort_by(|left, right| {
                left.source
                    .cmp(&right.source)
                    .then_with(|| left.line.cmp(&right.line))
                    .then_with(|| left.target.cmp(&right.target))
            });
            backlinks.dedup();
        }
    }
}

pub fn discover_documentation(
    repository_root: &Path,
    collections: &[DocumentationCollectionConfig],
) -> Result<(Vec<DiscoveredDocumentation>, Vec<DocumentationIssue>)> {
    let repository_root = canonical_or_absolute(repository_root)?;
    let mut discovered = Vec::new();
    let mut issues = Vec::new();
    let mut claimed = BTreeMap::<String, String>::new();
    let mut collection_ids = BTreeSet::new();

    for collection in collections {
        if !valid_collection_id(&collection.id) {
            issues.push(DocumentationIssue::new(
                "R026",
                format!("invalid documentation collection id '{}'", collection.id),
                repository_root.join(".specs/_config.toml"),
            ));
            continue;
        }
        if !collection_ids.insert(collection.id.clone()) {
            issues.push(DocumentationIssue::new(
                "R026",
                format!("duplicate documentation collection id '{}'", collection.id),
                repository_root.join(".specs/_config.toml"),
            ));
            continue;
        }
        if collection.title.trim().is_empty() || collection.include.is_empty() {
            issues.push(DocumentationIssue::new(
                "R026",
                format!(
                    "documentation collection '{}' requires a title and at least one include pattern",
                    collection.id
                ),
                repository_root.join(".specs/_config.toml"),
            ));
            continue;
        }
        if !safe_relative_path(&collection.root)
            || collection
                .include
                .iter()
                .chain(&collection.exclude)
                .any(|pattern| !safe_pattern(pattern))
        {
            issues.push(DocumentationIssue::new(
                "R026",
                format!(
                    "documentation collection '{}' contains an unsafe root or pattern",
                    collection.id
                ),
                repository_root.join(".specs/_config.toml"),
            ));
            continue;
        }

        let collection_root = repository_root.join(&collection.root);
        let canonical_root = match collection_root.canonicalize() {
            Ok(path) if path.starts_with(&repository_root) && path.is_dir() => path,
            _ => {
                issues.push(DocumentationIssue::new(
                    "R026",
                    format!(
                        "documentation collection '{}' root '{}' does not resolve inside the repository",
                        collection.id, collection.root
                    ),
                    repository_root.join(".specs/_config.toml"),
                ));
                continue;
            }
        };

        let includes = compile_patterns(&collection.include)?;
        let excludes = compile_patterns(&collection.exclude)?;
        let has_wildcard = collection
            .include
            .iter()
            .any(|pattern| pattern.contains('*') || pattern.contains('?'));
        let mut collection_paths = if has_wildcard {
            WalkDir::new(&canonical_root)
                .follow_links(false)
                .into_iter()
                .filter_entry(include_walk_entry)
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_file())
                .filter_map(|entry| {
                    let relative = entry.path().strip_prefix(&canonical_root).ok()?;
                    let relative = path_string(relative);
                    eligible_documentation_path(&relative, &includes, &excludes)
                        .then(|| entry.into_path())
                })
                .collect::<Vec<_>>()
        } else {
            collection
                .include
                .iter()
                .filter_map(|relative| {
                    if !eligible_documentation_path(relative, &includes, &excludes) {
                        return None;
                    }
                    let path = canonical_root.join(relative).canonicalize().ok()?;
                    (path.is_file() && path.starts_with(&canonical_root)).then_some(path)
                })
                .collect::<Vec<_>>()
        };
        collection_paths.sort();
        collection_paths.dedup();

        for source_path in collection_paths {
            let repository_path = path_string(
                source_path
                    .strip_prefix(&repository_root)
                    .context("documentation path escaped repository root")?,
            );
            if let Some(previous) = claimed.insert(repository_path.clone(), collection.id.clone()) {
                issues.push(DocumentationIssue::new(
                    "R026",
                    format!(
                        "documentation path '{}' belongs to overlapping collections '{}' and '{}'",
                        repository_path, previous, collection.id
                    ),
                    repository_root.join(".specs/_config.toml"),
                ));
                continue;
            }
            discovered.push(DiscoveredDocumentation {
                source_path,
                repository_path,
                collection: collection.clone(),
            });
        }
    }
    discovered.sort_by(|left, right| left.repository_path.cmp(&right.repository_path));
    Ok((discovered, issues))
}

fn eligible_documentation_path(path: &str, includes: &[Regex], excludes: &[Regex]) -> bool {
    path.ends_with(".md")
        && !path.ends_with(".spec.md")
        && matches_any(includes, path)
        && !matches_any(excludes, path)
}

pub fn matching_collections(
    repository_path: &str,
    collections: &[DocumentationCollectionConfig],
) -> Result<Vec<DocumentationCollectionConfig>> {
    let mut matches = Vec::new();
    for collection in collections {
        if !safe_relative_path(&collection.root)
            || collection.include.is_empty()
            || collection
                .include
                .iter()
                .chain(&collection.exclude)
                .any(|pattern| !safe_pattern(pattern))
        {
            continue;
        }
        let root = if collection.root == "." {
            ""
        } else {
            collection.root.trim_end_matches('/')
        };
        let relative = if root.is_empty() {
            repository_path
        } else if let Some(relative) = repository_path.strip_prefix(&format!("{root}/")) {
            relative
        } else {
            continue;
        };
        if !relative.ends_with(".md") || relative.ends_with(".spec.md") {
            continue;
        }
        let includes = compile_patterns(&collection.include)?;
        let excludes = compile_patterns(&collection.exclude)?;
        if matches_any(&includes, relative) && !matches_any(&excludes, relative) {
            matches.push(collection.clone());
        }
    }
    Ok(matches)
}

pub fn parse_documentation(
    source_path: &Path,
    repository_path: &str,
    collection: &DocumentationCollectionConfig,
    content: &str,
) -> DocumentationDocument {
    let parser = Parser::new_ext(content, Options::all());
    let line_offsets = line_offsets(content);
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut active_heading: Option<(u8, usize, String)> = None;
    let mut heading_stack = Vec::<String>::new();
    let mut active_link: Option<(String, usize, String)> = None;
    let mut active_paragraph = false;
    let mut paragraph_text = String::new();
    let mut summary = None;
    let mut title = None;

    for (event, range) in parser.into_offset_iter() {
        let line = offset_line(&line_offsets, range.start);
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                active_heading = Some((heading_level(level), line, String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start_line, raw_title)) = active_heading.take() {
                    let heading_title = normalize_inline_text(&raw_title);
                    if !heading_title.is_empty() {
                        heading_stack.truncate(level.saturating_sub(1) as usize);
                        while heading_stack.len() < level.saturating_sub(1) as usize {
                            heading_stack.push(String::new());
                        }
                        heading_stack.push(heading_title.clone());
                        let segments = heading_stack
                            .iter()
                            .filter(|segment| !segment.is_empty())
                            .cloned()
                            .collect::<Vec<_>>();
                        title.get_or_insert_with(|| heading_title.clone());
                        headings.push(DocumentationHeading {
                            title: heading_title.clone(),
                            segments,
                            level,
                            line: start_line,
                            end_line: start_line,
                            fragment: github_fragment(&heading_title),
                        });
                    }
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                active_link = Some((dest_url.to_string(), line, String::new()));
            }
            Event::End(TagEnd::Link) => {
                if let Some((authored, link_line, label)) = active_link.take() {
                    if let Some(target) = link_target(repository_path, &authored) {
                        links.push(DocumentationLink {
                            target,
                            label: normalize_inline_text(&label),
                            authored,
                            line: link_line,
                        });
                    }
                }
            }
            Event::Start(Tag::Paragraph) if active_heading.is_none() => {
                active_paragraph = true;
                paragraph_text.clear();
            }
            Event::End(TagEnd::Paragraph) if active_paragraph => {
                active_paragraph = false;
                let text = normalize_inline_text(&paragraph_text);
                if summary.is_none() && !text.is_empty() {
                    summary = Some(text);
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, _, heading_text)) = active_heading.as_mut() {
                    heading_text.push_str(&text);
                }
                if let Some((_, _, label)) = active_link.as_mut() {
                    label.push_str(&text);
                }
                if active_paragraph {
                    paragraph_text.push_str(&text);
                    paragraph_text.push(' ');
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, _, heading_text)) = active_heading.as_mut() {
                    heading_text.push(' ');
                }
                if let Some((_, _, label)) = active_link.as_mut() {
                    label.push(' ');
                }
                if active_paragraph {
                    paragraph_text.push(' ');
                }
            }
            _ => {}
        }
    }

    for index in 0..headings.len() {
        let start_level = headings[index].level;
        let end_line = headings
            .iter()
            .skip(index + 1)
            .find(|heading| heading.level <= start_level)
            .map(|heading| heading.line.saturating_sub(1))
            .unwrap_or_else(|| content.lines().count().max(1));
        headings[index].end_line = end_line.max(headings[index].line);
    }
    disambiguate_fragments(&mut headings);

    DocumentationDocument {
        collection_id: collection.id.clone(),
        collection_title: collection.title.clone(),
        path: repository_path.to_string(),
        source_path: source_path.to_path_buf(),
        title: title.unwrap_or_else(|| title_from_path(repository_path)),
        summary,
        headings,
        links,
        body: content.to_string(),
    }
}

fn link_target(current_path: &str, authored: &str) -> Option<DocumentationLinkTarget> {
    if let Some(reference) = parse_spec_url(authored) {
        return Some(DocumentationLinkTarget::Forge(reference));
    }
    if authored.starts_with('/') || authored.starts_with("//") || has_url_scheme(authored) {
        return None;
    }
    let (raw_path, fragment) = authored
        .split_once('#')
        .map(|(path, fragment)| (path, Some(fragment)))
        .unwrap_or((authored, None));
    let raw_path = raw_path.split('?').next().unwrap_or(raw_path);
    let decoded_path = percent_decode_str(raw_path).decode_utf8().ok()?;
    let target_path = if decoded_path.is_empty() {
        current_path.to_string()
    } else {
        let base = Path::new(current_path)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        normalize_relative_path(&base.join(decoded_path.as_ref()))?
    };
    if !target_path.ends_with(".md") || target_path.ends_with(".spec.md") {
        return None;
    }
    let fragment = fragment
        .filter(|value| !value.is_empty())
        .and_then(|value| percent_decode_str(value).decode_utf8().ok())
        .map(|value| value.into_owned());
    Some(DocumentationLinkTarget::Markdown {
        path: target_path,
        fragment,
    })
}

fn safe_relative_path(value: &str) -> bool {
    if value.is_empty() || Path::new(value).is_absolute() {
        return false;
    }
    !Path::new(value).components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn safe_pattern(pattern: &str) -> bool {
    !pattern.is_empty()
        && !pattern.starts_with('/')
        && !pattern.split('/').any(|component| component == "..")
}

fn valid_collection_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'-' | b'_'))
        })
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|pattern| Regex::new(&glob_regex(pattern)).context("compiling documentation pattern"))
        .collect()
}

fn glob_regex(pattern: &str) -> String {
    let chars = pattern.chars().collect::<Vec<_>>();
    let mut output = String::from("^");
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '*' if chars.get(index + 1) == Some(&'*') => {
                index += 1;
                if chars.get(index + 1) == Some(&'/') {
                    index += 1;
                    output.push_str("(?:.*/)?");
                } else {
                    output.push_str(".*");
                }
            }
            '*' => output.push_str("[^/]*"),
            '?' => output.push_str("[^/]"),
            character => output.push_str(&regex::escape(&character.to_string())),
        }
        index += 1;
    }
    output.push('$');
    output
}

fn matches_any(patterns: &[Regex], path: &str) -> bool {
    patterns.iter().any(|pattern| pattern.is_match(path))
}

fn include_walk_entry(entry: &DirEntry) -> bool {
    entry.depth() == 0 || entry.file_name() != ".git"
}

fn normalize_relative_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn github_fragment(title: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for character in title.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() || character == '_' || character == '-' {
            output.push(character);
            previous_dash = character == '-';
        } else if character.is_whitespace() && !output.is_empty() && !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }
    while output.ends_with('-') {
        output.pop();
    }
    output
}

fn disambiguate_fragments(headings: &mut [DocumentationHeading]) {
    let mut counts = BTreeMap::<String, usize>::new();
    for heading in headings {
        let base = heading.fragment.clone();
        let count = counts.entry(base.clone()).or_default();
        if *count > 0 {
            heading.fragment = format!("{base}-{count}");
        }
        *count += 1;
    }
}

fn title_from_path(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path);
    let readable = stem
        .replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    if readable.is_empty() {
        path.to_string()
    } else {
        readable
    }
}

fn normalize_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn line_offsets(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(content.match_indices('\n').map(|(index, _)| index + 1))
        .collect()
}

fn offset_line(offsets: &[usize], offset: usize) -> usize {
    offsets
        .partition_point(|candidate| *candidate <= offset)
        .saturating_sub(1)
        + 1
}

fn path_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::CurDir => None,
            Component::ParentDir => Some("..".to_string()),
            Component::RootDir | Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn canonical_or_absolute(path: &Path) -> Result<PathBuf> {
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(_) if path.is_absolute() => Ok(path.to_path_buf()),
        Err(_) => Ok(std::env::current_dir()?.join(path)),
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn has_url_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'+' | b'-' | b'.'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection(root: &str) -> DocumentationCollectionConfig {
        DocumentationCollectionConfig {
            id: "guides".to_string(),
            title: "Guides".to_string(),
            root: root.to_string(),
            include: vec!["**/*.md".to_string()],
            exclude: vec!["generated/**".to_string()],
        }
    }

    #[test]
    fn documentation_reference_round_trips_heading_segments() {
        let reference = DocumentationReference::heading(
            "docs/architecture.md",
            vec![
                "System design".to_string(),
                "Request / response".to_string(),
            ],
        );
        assert_eq!(
            reference.to_string(),
            "spec:doc:docs/architecture.md#heading=System%20design/Request%20%2F%20response"
        );
        assert_eq!(
            decode_heading_segments("System%20design/Request%20%2F%20response").unwrap(),
            ["System design", "Request / response"]
        );
    }

    #[test]
    fn parses_heading_hierarchy_summary_and_links() {
        let parsed = parse_documentation(
            Path::new("/repo/docs/architecture.md"),
            "docs/architecture.md",
            &collection("docs"),
            "# System design\n\nAn overview.\n\n## Request flow\n\nSee [operations](./ops.md#deploy).\n",
        );
        assert_eq!(parsed.title, "System design");
        assert_eq!(parsed.summary.as_deref(), Some("An overview."));
        assert_eq!(
            parsed.headings[1].segments,
            ["System design", "Request flow"]
        );
        assert_eq!(parsed.headings[1].fragment, "request-flow");
        assert!(matches!(
            &parsed.links[0].target,
            DocumentationLinkTarget::Markdown { path, fragment }
                if path == "docs/ops.md" && fragment.as_deref() == Some("deploy")
        ));
    }

    #[test]
    fn discovers_only_configured_markdown_and_builds_backlinks() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("docs/generated")).unwrap();
        std::fs::write(
            temp.path().join("docs/architecture.md"),
            "# Architecture\n\nSee [operations](./operations.md#deploy).\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("docs/operations.md"),
            "# Operations\n\n## Deploy\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("docs/generated/ignored.md"),
            "# Generated\n",
        )
        .unwrap();
        std::fs::write(temp.path().join("outside.md"), "# Outside\n").unwrap();

        let index = DocumentationIndex::load_from_root(temp.path(), &[collection("docs")]).unwrap();
        assert_eq!(index.documents.len(), 2);
        assert!(index.issues.is_empty(), "{:?}", index.issues);
        let target = DocumentationReference::heading(
            "docs/operations.md",
            vec!["Operations".to_string(), "Deploy".to_string()],
        );
        assert_eq!(index.backlinks(&target.to_string()).len(), 1);
    }

    #[test]
    fn reports_overlaps_and_dangling_markdown_links() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("docs")).unwrap();
        std::fs::write(
            temp.path().join("docs/a.md"),
            "# A\n\nSee [missing](./missing.md).\n",
        )
        .unwrap();
        let mut second = collection("docs");
        second.id = "other".to_string();
        let index =
            DocumentationIndex::load_from_root(temp.path(), &[collection("docs"), second]).unwrap();
        assert!(index.issues.iter().any(|issue| issue.code == "R026"));
        assert!(index.issues.iter().any(|issue| issue.code == "R029"));
    }

    #[test]
    fn glob_star_and_double_star_have_distinct_scope() {
        let one = Regex::new(&glob_regex("*.md")).unwrap();
        let many = Regex::new(&glob_regex("**/*.md")).unwrap();
        assert!(one.is_match("README.md"));
        assert!(!one.is_match("guides/start.md"));
        assert!(many.is_match("README.md"));
        assert!(many.is_match("guides/start.md"));
    }
}
