use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use crate::intellect::{self, AdherenceState};
use crate::model::frontmatter::{Progress, Status, TypeSpecificFields};
use crate::model::id::EntityType;
use crate::model::registry::SpecRegistry;

pub fn run(
    specs_dir: &Path,
    namespace_filter: Option<&str>,
    type_filter: Option<&str>,
    no_color: bool,
    include_tasks: bool,
) -> Result<()> {
    if no_color {
        colored::control::set_override(false);
    }

    let registry = SpecRegistry::load(specs_dir)?;
    let adherence = intellect::fetch_or_unknown(&registry)?;

    // Group: namespace -> type-prefix -> Vec<&SpecDocument>
    let mut grouped: BTreeMap<String, BTreeMap<&'static str, Vec<usize>>> = BTreeMap::new();
    for (idx, doc) in registry.documents.iter().enumerate() {
        if doc.universal.entity_type == EntityType::Project {
            continue;
        }
        if doc.universal.entity_type == EntityType::Task {
            continue;
        }
        let ns = &doc.universal.id.namespace;
        let ty = doc.universal.entity_type.prefix();
        if let Some(want_ns) = namespace_filter {
            if ns != want_ns {
                continue;
            }
        }
        if let Some(want_ty) = type_filter {
            if !ty.eq_ignore_ascii_case(want_ty) {
                continue;
            }
        }
        grouped
            .entry(ns.clone())
            .or_default()
            .entry(ty)
            .or_default()
            .push(idx);
    }

    let project = registry.project();
    let has_visible_tasks = include_tasks
        && registry.documents.iter().any(|document| {
            document.universal.entity_type == EntityType::Task
                && namespace_filter.map_or(true, |namespace| {
                    document.universal.id.namespace == namespace
                })
                && type_filter.map_or(true, |entity_type| entity_type.eq_ignore_ascii_case("TASK"))
        });
    if grouped.is_empty() && project.is_none() && !has_visible_tasks {
        println!("(no specs match the filter)");
        return Ok(());
    }

    let tree_prefix = if let Some(project) = project {
        let implementation = adherence.get(&project.id_str()).map(|state| &state.state);
        let state = effective_state_label(project.universal.status, implementation);
        let summary = project
            .universal
            .summary
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        let summary = first_line(summary);
        println!(
            "{} {} {} {}",
            colorize_type("PROJECT"),
            project.universal.id.slug,
            state,
            summary.dimmed()
        );
        if !grouped.is_empty() {
            println!("└── {}", ".specs/".bold());
        }
        "    "
    } else {
        println!("{}", ".specs/".bold());
        ""
    };
    let namespaces: Vec<&String> = grouped.keys().collect();
    let ns_count = namespaces.len();
    for (ns_i, ns) in namespaces.iter().enumerate() {
        let last_ns = ns_i + 1 == ns_count;
        let ns_branch = if last_ns { "└──" } else { "├──" };
        println!("{tree_prefix}{} {}/", ns_branch, ns.cyan().bold());
        let ns_prefix = format!("{tree_prefix}{}", if last_ns { "    " } else { "│   " });

        let types = grouped.get(*ns).unwrap();
        // Flatten all entries for this namespace into a single ordered list
        // (group by type, but render as a flat list under the namespace).
        let mut entries: Vec<(usize, &'static str)> = Vec::new();
        for (ty, idxs) in types {
            for idx in idxs {
                entries.push((*idx, ty));
            }
        }
        entries.sort_by(|a, b| {
            let da = &registry.documents[a.0];
            let db = &registry.documents[b.0];
            a.1.cmp(b.1)
                .then_with(|| da.universal.id.slug.cmp(&db.universal.id.slug))
        });

        let entry_count = entries.len();
        for (i, (idx, ty)) in entries.iter().enumerate() {
            let last = i + 1 == entry_count;
            let branch = if last { "└──" } else { "├──" };
            let doc = &registry.documents[*idx];
            let slug = &doc.universal.id.slug;
            let adherence_state = adherence.get(&doc.id_str()).map(|state| &state.state);
            let summary = doc
                .universal
                .summary
                .as_deref()
                .map(|s| s.trim())
                .unwrap_or("");
            let summary_trimmed = first_line(summary);
            let ty_colored = colorize_type(ty);
            let state = effective_state_label(doc.universal.status, adherence_state);
            let line = if summary_trimmed.is_empty() {
                format!("{ty_colored:<5} {slug} {state}")
            } else {
                format!(
                    "{ty_colored:<5} {slug} {state} {}",
                    summary_trimmed.dimmed()
                )
            };
            println!("{ns_prefix}{branch} {line}");
        }
    }

    if include_tasks {
        render_work_items(&registry, namespace_filter, type_filter);
    }

    Ok(())
}

fn render_work_items(
    registry: &SpecRegistry,
    namespace_filter: Option<&str>,
    type_filter: Option<&str>,
) {
    if type_filter.is_some_and(|entity_type| !entity_type.eq_ignore_ascii_case("TASK")) {
        return;
    }
    let mut tasks = registry
        .documents
        .iter()
        .filter(|document| document.universal.entity_type == EntityType::Task)
        .filter(|document| {
            namespace_filter.map_or(true, |namespace| {
                document.universal.id.namespace == namespace
            })
        })
        .collect::<Vec<_>>();
    tasks.sort_by_key(|document| document.id_str());
    if tasks.is_empty() {
        return;
    }

    println!("{}", "WORK ITEMS".bold());
    for (index, document) in tasks.iter().enumerate() {
        let branch = if index + 1 == tasks.len() {
            "└──"
        } else {
            "├──"
        };
        let TypeSpecificFields::Task { progress, .. } = &document.type_fields else {
            unreachable!("TASK entity has task fields")
        };
        let state = if document.universal.status == Status::Accepted {
            state_label(progress_state(*progress))
        } else {
            effective_state_label(document.universal.status, None)
        };
        let summary = document
            .universal
            .summary
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        println!(
            "{branch} {} {} {state} {}",
            colorize_type("TASK"),
            document.id_str(),
            first_line(summary).dimmed()
        );
    }
}

fn progress_state(progress: Progress) -> DisplayState {
    match progress {
        Progress::Pending => DisplayState::Pending,
        Progress::InProgress => DisplayState::InProgress,
        Progress::Done => DisplayState::Done,
        Progress::Blocked => DisplayState::Blocked,
        Progress::Deferred => DisplayState::Deferred,
        Progress::WontDo => DisplayState::WontDo,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayState {
    Draft,
    Accepted,
    Deprecated,
    Superseded,
    Pending,
    InProgress,
    Done,
    Blocked,
    Deferred,
    WontDo,
    Unverified,
    Current,
    Stale,
    Partial,
    Violated,
    Unknown,
    Unresolved,
}

fn effective_state(status: Status, adherence: Option<&AdherenceState>) -> DisplayState {
    match status {
        Status::Draft => DisplayState::Draft,
        Status::Deprecated => DisplayState::Deprecated,
        Status::Superseded => DisplayState::Superseded,
        Status::Accepted => match adherence {
            Some(AdherenceState::NotApplicable) => DisplayState::Accepted,
            Some(state) => adherence_state(state),
            None => DisplayState::Unknown,
        },
    }
}

fn adherence_state(state: &AdherenceState) -> DisplayState {
    match state {
        AdherenceState::Unverified => DisplayState::Unverified,
        AdherenceState::Current => DisplayState::Current,
        AdherenceState::Stale => DisplayState::Stale,
        AdherenceState::Partial => DisplayState::Partial,
        AdherenceState::Violated => DisplayState::Violated,
        AdherenceState::Unknown => DisplayState::Unknown,
        AdherenceState::Unresolved => DisplayState::Unresolved,
        AdherenceState::NotApplicable => unreachable!("not-applicable is a fallback state"),
    }
}

fn effective_state_label(
    status: Status,
    adherence: Option<&AdherenceState>,
) -> colored::ColoredString {
    state_label(effective_state(status, adherence))
}

fn state_label(state: DisplayState) -> colored::ColoredString {
    let (glyph, name) = state_identity(state);
    let label = format!("{glyph} {name}");
    match state {
        DisplayState::Current | DisplayState::Done => label.green().bold(),
        DisplayState::Accepted => label.green(),
        DisplayState::Draft
        | DisplayState::Pending
        | DisplayState::Stale
        | DisplayState::Partial => label.yellow(),
        DisplayState::InProgress => label.cyan(),
        DisplayState::Blocked | DisplayState::Violated | DisplayState::Unresolved => {
            label.red().bold()
        }
        DisplayState::Deprecated
        | DisplayState::Superseded
        | DisplayState::Deferred
        | DisplayState::WontDo
        | DisplayState::Unverified
        | DisplayState::Unknown => label.bright_black(),
    }
}

fn state_identity(state: DisplayState) -> (&'static str, &'static str) {
    match state {
        DisplayState::Draft => ("◇", "draft"),
        DisplayState::Accepted => ("◆", "accepted"),
        DisplayState::Deprecated => ("−", "deprecated"),
        DisplayState::Superseded => ("↪", "superseded"),
        DisplayState::Pending => ("○", "pending"),
        DisplayState::InProgress => ("◐", "in-progress"),
        DisplayState::Done => ("✓", "done"),
        DisplayState::Blocked => ("⊘", "blocked"),
        DisplayState::Deferred => ("◌", "deferred"),
        DisplayState::WontDo => ("✗", "wontdo"),
        DisplayState::Unverified => ("?", "unverified"),
        DisplayState::Current => ("✓", "current"),
        DisplayState::Stale => ("↻", "stale"),
        DisplayState::Partial => ("◐", "partial"),
        DisplayState::Violated => ("✗", "violated"),
        DisplayState::Unknown => ("?", "unknown"),
        DisplayState::Unresolved => ("!", "unresolved"),
    }
}

fn first_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or("");
    if line.is_empty() {
        return String::new();
    }
    let max_chars = 77;
    if line.chars().count() > max_chars {
        let truncated: String = line.chars().take(max_chars).collect();
        format!("— {truncated}…")
    } else {
        format!("— {line}")
    }
}

fn colorize_type(ty: &str) -> colored::ColoredString {
    match ty {
        "PROJECT" => ty.bright_cyan().bold(),
        "REQ" => ty.green().bold(),
        "INV" => ty.magenta().bold(),
        "IFC" => ty.blue().bold(),
        "ADR" => ty.yellow().bold(),
        "GLO" => ty.white().bold(),
        "TOPIC" => ty.cyan().bold(),
        "SCN" => ty.bright_blue().bold(),
        "TASK" => ty.bright_yellow().bold(),
        _ => ty.normal(),
    }
}

/// (glyph, short-name). Glyphs chosen to render in most terminal fonts.
pub fn progress_glyph(progress: Progress) -> (&'static str, &'static str) {
    match progress {
        Progress::Pending => ("○", "pending"),
        Progress::InProgress => ("◐", "in-progress"),
        Progress::Done => ("✓", "done"),
        Progress::Blocked => ("⊘", "blocked"),
        Progress::Deferred => ("◌", "deferred"),
        Progress::WontDo => ("✗", "wontdo"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_accepted_lifecycle_controls_the_display_state() {
        for (status, expected) in [
            (Status::Draft, DisplayState::Draft),
            (Status::Deprecated, DisplayState::Deprecated),
            (Status::Superseded, DisplayState::Superseded),
        ] {
            assert_eq!(
                effective_state(status, Some(&AdherenceState::Current)),
                expected
            );
        }
    }

    #[test]
    fn task_progress_is_independent_from_adherence() {
        for (progress, expected) in [
            (Progress::Pending, DisplayState::Pending),
            (Progress::InProgress, DisplayState::InProgress),
            (Progress::Done, DisplayState::Done),
            (Progress::Blocked, DisplayState::Blocked),
            (Progress::Deferred, DisplayState::Deferred),
            (Progress::WontDo, DisplayState::WontDo),
        ] {
            assert_eq!(progress_state(progress), expected);
        }
    }

    #[test]
    fn accepted_non_task_uses_adherence_or_lifecycle_fallback() {
        assert_eq!(
            effective_state(Status::Accepted, Some(&AdherenceState::Current)),
            DisplayState::Current
        );
        assert_eq!(
            effective_state(Status::Accepted, Some(&AdherenceState::NotApplicable)),
            DisplayState::Accepted
        );
        assert_eq!(
            effective_state(Status::Accepted, None),
            DisplayState::Unknown
        );
    }

    #[test]
    fn every_state_has_one_compact_bracket_free_identity() {
        for state in [
            DisplayState::Draft,
            DisplayState::Accepted,
            DisplayState::Deprecated,
            DisplayState::Superseded,
            DisplayState::Pending,
            DisplayState::InProgress,
            DisplayState::Done,
            DisplayState::Blocked,
            DisplayState::Deferred,
            DisplayState::WontDo,
            DisplayState::Unverified,
            DisplayState::Current,
            DisplayState::Stale,
            DisplayState::Partial,
            DisplayState::Violated,
            DisplayState::Unknown,
            DisplayState::Unresolved,
        ] {
            let (glyph, name) = state_identity(state);
            assert_eq!(glyph.chars().count(), 1);
            assert!(!name.contains('['));
            assert!(!name.contains(']'));
        }
    }
}
