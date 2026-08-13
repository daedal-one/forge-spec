use std::path::Path;

use anyhow::{bail, Result};

use crate::cli::RenderTarget;
use crate::intellect;
use crate::model::reference::{SourceTarget, SpecReference};
use crate::model::registry::SpecRegistry;
use crate::render::scope::{compute_scope, DetailLevel};
use crate::render::{agent, human};

pub struct RenderOptions<'a> {
    pub target: &'a RenderTarget,
    pub depth: Option<usize>,
    pub ancestors: &'a str,
    pub descendants: &'a str,
    pub include_source: bool,
    pub include_docs: bool,
}

pub fn run(specs_dir: &Path, id_or_query: &str, options: &RenderOptions<'_>) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let adherence = intellect::fetch_or_unknown(&registry)?;

    // Resolve the query — support exact ID or simple glob
    let focal_ids = resolve_query(&registry, id_or_query)?;

    if focal_ids.is_empty() {
        bail!("no specs match '{id_or_query}'");
    }

    let ancestor_detail = DetailLevel::from_str_val(options.ancestors);
    let descendant_detail = DetailLevel::from_str_val(options.descendants);

    for focal_id in &focal_ids {
        let entries = compute_scope(
            &registry,
            focal_id,
            ancestor_detail,
            descendant_detail,
            options.depth,
        );

        let mut output = match options.target {
            RenderTarget::Human => human::render_human(&registry, &entries, Some(&adherence)),
            RenderTarget::Agent => agent::render_agent(&registry, &entries, Some(&adherence)),
        };

        if options.include_source {
            append_source_references(&registry, &entries, options.target, &mut output)?;
        }
        if options.include_docs {
            append_documentation_references(&registry, &entries, options.target, &mut output);
        }

        print!("{output}");
    }

    Ok(())
}

fn append_documentation_references(
    registry: &SpecRegistry,
    entries: &[crate::render::scope::ScopedEntry],
    target: &RenderTarget,
    output: &mut String,
) {
    let mut seen = std::collections::BTreeSet::new();
    let mut excerpts = Vec::new();
    for entry in entries {
        let Some(document) = registry.get_by_id(&entry.id) else {
            continue;
        };
        for located in &document.references {
            let SpecReference::Documentation(reference) = &located.reference else {
                continue;
            };
            let identity = reference.to_string();
            if !seen.insert(identity.clone()) {
                continue;
            }
            let Some((documentation, heading)) = registry.documentation.resolve(reference) else {
                excerpts.push((
                    identity,
                    String::new(),
                    "unresolved".to_string(),
                    "Documentation reference did not resolve.".to_string(),
                ));
                continue;
            };
            let start = heading.map(|heading| heading.line).unwrap_or(1);
            let end = heading
                .map(|heading| heading.end_line)
                .unwrap_or_else(|| documentation.body.lines().count().max(1));
            let snippet = documentation
                .body
                .lines()
                .skip(start.saturating_sub(1))
                .take(end.saturating_sub(start) + 1)
                .collect::<Vec<_>>()
                .join("\n");
            excerpts.push((
                identity,
                documentation.collection_id.clone(),
                "verified".to_string(),
                snippet,
            ));
        }
    }
    if excerpts.is_empty() {
        return;
    }

    match target {
        RenderTarget::Human => {
            output.push_str("\n## Referenced documentation\n\n");
            for (reference, collection, status, snippet) in excerpts {
                output.push_str(&format!(
                    "### `{reference}` ({status}, collection `{collection}`)\n\n{snippet}\n\n"
                ));
            }
        }
        RenderTarget::Agent => {
            let closing = "</specs>\n";
            if output.ends_with(closing) {
                output.truncate(output.len() - closing.len());
            }
            output.push_str("  <documentation>\n");
            for (reference, collection, status, snippet) in excerpts {
                output.push_str(&format!(
                    "    <document reference=\"{}\" collection=\"{}\" status=\"{}\"><![CDATA[{}]]></document>\n",
                    escape_xml(&reference),
                    escape_xml(&collection),
                    status,
                    snippet.replace("]]>", "]]&gt;")
                ));
            }
            output.push_str("  </documentation>\n</specs>\n");
        }
    }
}

fn append_source_references(
    registry: &SpecRegistry,
    entries: &[crate::render::scope::ScopedEntry],
    target: &RenderTarget,
    output: &mut String,
) -> Result<()> {
    let service = crate::symbol::SymbolService::new(&registry.specs_dir, false)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut excerpts = Vec::new();
    for entry in entries {
        let Some(document) = registry.get_by_id(&entry.id) else {
            continue;
        };
        for located in &document.references {
            let SpecReference::Source(source) = &located.reference else {
                continue;
            };
            let reference = SpecReference::Source(source.clone()).to_string();
            if !seen.insert(reference.clone()) {
                continue;
            }
            service.resolve_safe_path(&source.path)?;
            let resolved = match &source.target {
                SourceTarget::Symbol { .. } => service
                    .resolve(source)
                    .map(|resolved| (resolved.snippet, "verified".to_string())),
                SourceTarget::File => crate::render::source::resolve_source(
                    &registry.specs_dir,
                    &source.path,
                    None,
                    document.universal.pinned_at.as_deref(),
                )
                .map(|snippet| (snippet, "verified".into()))
                .map_err(|error| crate::symbol::SymbolError::Protocol(error.to_string())),
                SourceTarget::Lines { start, end } => crate::render::source::resolve_source(
                    &registry.specs_dir,
                    &source.path,
                    Some((*start, *end)),
                    document.universal.pinned_at.as_deref(),
                )
                .map(|snippet| (snippet, "verified".into()))
                .map_err(|error| crate::symbol::SymbolError::Protocol(error.to_string())),
            };
            match resolved {
                Ok((snippet, status)) => excerpts.push((reference, status, snippet)),
                Err(error) => excerpts.push((reference, "unverified".into(), error.to_string())),
            }
        }
    }
    if excerpts.is_empty() {
        return Ok(());
    }

    match target {
        RenderTarget::Human => {
            output.push_str("\n## Resolved source references\n\n");
            for (reference, status, snippet) in excerpts {
                output.push_str(&format!(
                    "### `{reference}` ({status})\n\n```text\n{snippet}\n```\n\n"
                ));
            }
        }
        RenderTarget::Agent => {
            let closing = "</specs>\n";
            if output.ends_with(closing) {
                output.truncate(output.len() - closing.len());
            }
            output.push_str("  <sources>\n");
            for (reference, status, snippet) in excerpts {
                output.push_str(&format!(
                    "    <source reference=\"{}\" status=\"{}\"><![CDATA[{}]]></source>\n",
                    escape_xml(&reference),
                    status,
                    snippet.replace("]]>", "]]&gt;")
                ));
            }
            output.push_str("  </sources>\n</specs>\n");
        }
    }
    Ok(())
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn resolve_query(registry: &SpecRegistry, query: &str) -> Result<Vec<String>> {
    if query == "project" {
        return registry
            .project_id()
            .map(|id| vec![id])
            .ok_or_else(|| anyhow::anyhow!("configured project not found"));
    }

    // Exact ID match
    if registry.get_by_id(query).is_some() {
        return Ok(vec![query.to_string()]);
    }

    // Simple glob: REQ:auth/* matches REQ:auth/anything
    if query.contains('*') {
        let prefix = query.trim_end_matches('*');
        let matches: Vec<String> = registry
            .id_index
            .keys()
            .filter(|id| id.starts_with(prefix))
            .cloned()
            .collect();
        if matches.is_empty() {
            bail!("no specs match pattern '{query}'");
        }
        return Ok(matches);
    }

    bail!("spec not found: '{query}'")
}
