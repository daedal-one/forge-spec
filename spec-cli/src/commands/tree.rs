use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use crate::model::frontmatter::Status;
use crate::model::registry::SpecRegistry;

pub fn run(
    specs_dir: &Path,
    namespace_filter: Option<&str>,
    type_filter: Option<&str>,
    no_color: bool,
) -> Result<()> {
    if no_color {
        colored::control::set_override(false);
    }

    let registry = SpecRegistry::load(specs_dir)?;

    // Group: namespace -> type-prefix -> Vec<&SpecDocument>
    let mut grouped: BTreeMap<String, BTreeMap<&'static str, Vec<usize>>> = BTreeMap::new();
    for (idx, doc) in registry.documents.iter().enumerate() {
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

    if grouped.is_empty() {
        println!("(no specs match the filter)");
        return Ok(());
    }

    println!("{}", ".specs/".bold());
    let namespaces: Vec<&String> = grouped.keys().collect();
    let ns_count = namespaces.len();
    for (ns_i, ns) in namespaces.iter().enumerate() {
        let last_ns = ns_i + 1 == ns_count;
        let ns_branch = if last_ns { "└──" } else { "├──" };
        println!("{} {}/", ns_branch, ns.cyan().bold());
        let ns_prefix = if last_ns { "    " } else { "│   " };

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
            let status_label = status_label(doc.universal.status);
            let summary = doc
                .universal
                .summary
                .as_deref()
                .map(|s| s.trim())
                .unwrap_or("");
            let summary_trimmed = first_line(summary);
            let ty_colored = colorize_type(ty);
            let line = if summary_trimmed.is_empty() {
                format!("{ty_colored:<5} {slug} {status_label}")
            } else {
                format!(
                    "{ty_colored:<5} {slug} {status_label} {}",
                    summary_trimmed.dimmed()
                )
            };
            println!("{ns_prefix}{branch} {line}");
        }
    }

    Ok(())
}

fn first_line(s: &str) -> String {
    let line: String = s.lines().next().unwrap_or("").to_string();
    if line.len() > 80 {
        format!("— {}…", &line[..77])
    } else if line.is_empty() {
        String::new()
    } else {
        format!("— {line}")
    }
}

fn colorize_type(ty: &str) -> colored::ColoredString {
    match ty {
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

fn status_label(status: Status) -> colored::ColoredString {
    let s = format!("[{}]", status.as_str());
    match status {
        Status::Accepted => s.green(),
        Status::Draft => s.yellow(),
        Status::Deprecated => s.bright_black(),
        Status::Superseded => s.bright_black(),
    }
}
