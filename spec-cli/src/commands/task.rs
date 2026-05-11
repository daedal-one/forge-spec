//! Task-progress commands: `todo`, `start`, `done`, `block`, `reset`, `defer`.
//!
//! These commands read and rewrite the `progress:` field (and its companions
//! `assignee:`, `eta:`, `blocked_by:`) on TASK-typed `.spec.md` files, leaving
//! the rest of the frontmatter and body untouched. Frontmatter rewriting is
//! line-oriented rather than YAML-roundtrip to preserve hand-written
//! formatting, comments, and field ordering.

use std::path::Path;

use anyhow::{Context as _, Result, bail};

use crate::model::frontmatter::{Progress, TypeSpecificFields};
use crate::model::registry::SpecRegistry;

#[derive(Debug, Clone, Copy)]
enum StateFilter {
    Open,
    All,
    Specific(Progress),
}

pub fn todo(specs_dir: &Path, state: Option<&str>, under: Option<&str>, all: bool) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let filter = match (state, all) {
        (_, true) => StateFilter::All,
        (Some("all"), _) => StateFilter::All,
        (Some(s), _) => match Progress::from_str_val(s) {
            Some(p) => StateFilter::Specific(p),
            None => bail!(
                "unknown progress state '{s}' — expected pending|in-progress|done|blocked|deferred|wontdo|all"
            ),
        },
        (None, false) => StateFilter::Open,
    };

    let mut rows: Vec<(Progress, String, String)> = Vec::new();

    for doc in &registry.documents {
        let TypeSpecificFields::Task {
            progress, refines, ..
        } = &doc.type_fields
        else {
            continue;
        };

        if let Some(under_id) = under {
            let matches = refines.iter().any(|r| {
                let parent = r.split('#').next().unwrap_or(r);
                parent == under_id || r == under_id
            });
            if !matches {
                continue;
            }
        }

        let keep = match filter {
            StateFilter::Open => progress.is_open(),
            StateFilter::All => true,
            StateFilter::Specific(target) => *progress == target,
        };
        if !keep {
            continue;
        }

        let summary = doc
            .universal
            .summary
            .as_deref()
            .map(str::trim)
            .unwrap_or("(no summary)")
            .to_string();
        rows.push((*progress, doc.id_str(), summary));
    }

    rows.sort_by(|a, b| {
        progress_order(a.0)
            .cmp(&progress_order(b.0))
            .then_with(|| a.1.cmp(&b.1))
    });

    if rows.is_empty() {
        println!("(no tasks match)");
        return Ok(());
    }

    let id_width = rows.iter().map(|(_, id, _)| id.len()).max().unwrap_or(0);
    for (progress, id, summary) in rows {
        println!(
            "  {:<13}  {:<id_width$}  {}",
            progress.as_str(),
            id,
            summary,
            id_width = id_width,
        );
    }
    Ok(())
}

pub fn start(specs_dir: &Path, id: &str) -> Result<()> {
    set_progress(specs_dir, id, Progress::InProgress, None)
}

pub fn done(specs_dir: &Path, id: &str) -> Result<()> {
    set_progress(specs_dir, id, Progress::Done, None)
}

pub fn block(specs_dir: &Path, id: &str, on: Option<&str>) -> Result<()> {
    set_progress(specs_dir, id, Progress::Blocked, on)
}

pub fn reset(specs_dir: &Path, id: &str) -> Result<()> {
    set_progress(specs_dir, id, Progress::Pending, None)
}

pub fn defer(specs_dir: &Path, id: &str) -> Result<()> {
    set_progress(specs_dir, id, Progress::Deferred, None)
}

pub fn wontdo(specs_dir: &Path, id: &str) -> Result<()> {
    set_progress(specs_dir, id, Progress::WontDo, None)
}

fn set_progress(
    specs_dir: &Path,
    id: &str,
    new_progress: Progress,
    blocker_to_add: Option<&str>,
) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let doc = registry
        .get_by_id(id)
        .with_context(|| format!("no spec with id '{id}'"))?;

    if !matches!(doc.type_fields, TypeSpecificFields::Task { .. }) {
        bail!("spec '{id}' is not a task (type: {})", doc.universal.entity_type.type_name());
    }

    let path = &doc.source_path;
    let original = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let updated = rewrite_progress(&original, new_progress, blocker_to_add)?;
    std::fs::write(path, &updated)
        .with_context(|| format!("writing {}", path.display()))?;

    println!("{id} → {}", new_progress.as_str());
    if let Some(blocker) = blocker_to_add {
        println!("  blocked_by: {blocker}");
    }
    Ok(())
}

/// Rewrite the `progress:` line in a `.spec.md` frontmatter and optionally
/// append a blocker. Preserves all other content byte-for-byte.
fn rewrite_progress(
    content: &str,
    new_progress: Progress,
    blocker_to_add: Option<&str>,
) -> Result<String> {
    let bom = content.starts_with('\u{feff}');
    let body = if bom { &content[3..] } else { content };

    if !body.starts_with("---") {
        bail!("file does not start with YAML frontmatter delimiter '---'");
    }
    let after_open = &body[3..];
    let after_open = after_open
        .strip_prefix('\n')
        .or_else(|| after_open.strip_prefix("\r\n"))
        .unwrap_or(after_open);
    let close_pos = after_open
        .find("\n---")
        .ok_or_else(|| anyhow::anyhow!("no closing '---' delimiter for frontmatter"))?;

    let yaml = &after_open[..close_pos];
    let after_close = &after_open[close_pos..]; // starts with "\n---"

    // Mutate yaml line-by-line.
    let mut out_yaml = String::with_capacity(yaml.len() + 64);
    let mut saw_progress = false;
    let mut saw_blocked_by = false;

    for line in yaml.split_inclusive('\n') {
        if let Some(rest) = strip_key(line, "progress") {
            saw_progress = true;
            out_yaml.push_str(&format!("progress: {}{}", new_progress.as_str(), preserve_eol(rest)));
        } else if blocker_to_add.is_some() && strip_key(line, "blocked_by").is_some() {
            saw_blocked_by = true;
            // Inline list rewrite when blocked_by is on a single line: keep the
            // existing line and append a follow-up line below to add the
            // entry. This is robust to YAML flow vs block style.
            out_yaml.push_str(line);
        } else {
            out_yaml.push_str(line);
        }
    }

    if !saw_progress {
        // Append progress field.
        if !out_yaml.ends_with('\n') {
            out_yaml.push('\n');
        }
        out_yaml.push_str(&format!("progress: {}\n", new_progress.as_str()));
    }

    if let Some(blocker) = blocker_to_add {
        if saw_blocked_by {
            // Best-effort: insert a YAML list item after the blocked_by line.
            // For empty inline `blocked_by: []` we replace it with a block list.
            out_yaml = inject_blocker(&out_yaml, blocker);
        } else {
            if !out_yaml.ends_with('\n') {
                out_yaml.push('\n');
            }
            out_yaml.push_str(&format!("blocked_by:\n  - {blocker}\n"));
        }
    }

    let mut result = String::with_capacity(content.len() + 64);
    if bom {
        result.push('\u{feff}');
    }
    result.push_str("---\n");
    result.push_str(&out_yaml);
    if !out_yaml.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(after_close.trim_start_matches('\n'));
    Ok(result)
}

fn strip_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with(key) {
        return None;
    }
    let after_key = &trimmed[key.len()..];
    let after = after_key.trim_start();
    if !after.starts_with(':') {
        return None;
    }
    Some(&after[1..])
}

fn preserve_eol(s: &str) -> &str {
    if s.ends_with("\r\n") {
        "\r\n"
    } else if s.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

fn inject_blocker(yaml: &str, blocker: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + 32);
    let mut injected = false;
    for line in yaml.split_inclusive('\n') {
        let blocked_by_value = if injected {
            None
        } else {
            strip_key(line, "blocked_by")
        };
        if let Some(rest) = blocked_by_value {
            let trimmed = rest.trim();
            if trimmed.is_empty() || trimmed == "[]" {
                out.push_str(&format!("blocked_by:\n  - {blocker}\n"));
            } else {
                out.push_str(line);
                if !line.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&format!("  - {blocker}\n"));
            }
            injected = true;
        } else {
            out.push_str(line);
        }
    }
    out
}

fn progress_order(p: Progress) -> u8 {
    match p {
        Progress::InProgress => 0,
        Progress::Blocked => 1,
        Progress::Pending => 2,
        Progress::Done => 3,
        Progress::Deferred => 4,
        Progress::WontDo => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_existing_progress_line() {
        let input = "---\nid: TASK:codon/foo\ntype: task\nstatus: accepted\nversion: 0.1.0\nowners: [carlo]\nprogress: pending\n---\n# Body\n";
        let out = rewrite_progress(input, Progress::Done, None).unwrap();
        assert!(out.contains("progress: done\n"));
        assert!(!out.contains("progress: pending\n"));
        assert!(out.ends_with("# Body\n"));
    }

    #[test]
    fn appends_progress_when_missing() {
        let input = "---\nid: TASK:codon/foo\ntype: task\nstatus: accepted\nversion: 0.1.0\nowners: [carlo]\n---\n# Body\n";
        let out = rewrite_progress(input, Progress::InProgress, None).unwrap();
        assert!(out.contains("progress: in-progress\n"));
    }

    #[test]
    fn block_replaces_empty_blocked_by_inline() {
        let input = "---\nid: TASK:codon/foo\ntype: task\nstatus: accepted\nversion: 0.1.0\nowners: [carlo]\nprogress: pending\nblocked_by: []\n---\n# Body\n";
        let out = rewrite_progress(input, Progress::Blocked, Some("ADR:codon/0001-stack")).unwrap();
        assert!(out.contains("progress: blocked\n"));
        assert!(out.contains("blocked_by:\n  - ADR:codon/0001-stack\n"));
        assert!(!out.contains("blocked_by: []"));
    }

    #[test]
    fn block_appends_when_blocked_by_missing() {
        let input = "---\nid: TASK:codon/foo\ntype: task\nstatus: accepted\nversion: 0.1.0\nowners: [carlo]\nprogress: pending\n---\n# Body\n";
        let out = rewrite_progress(input, Progress::Blocked, Some("ADR:codon/0001-stack")).unwrap();
        assert!(out.contains("blocked_by:\n  - ADR:codon/0001-stack\n"));
    }
}
