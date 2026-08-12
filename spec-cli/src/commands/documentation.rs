use std::path::Path;

use anyhow::{bail, Result};
use serde_json::json;

use crate::documentation::DocumentationReference;
use crate::model::registry::SpecRegistry;
use crate::parse::references::parse_spec_url;

pub fn list(specs_dir: &Path, collection: Option<&str>, as_json: bool) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    if let Some(collection) = collection {
        if !registry
            .config
            .documentation
            .iter()
            .any(|configured| configured.id == collection)
        {
            bail!("documentation collection not found: '{collection}'");
        }
    }
    let documents = registry
        .documentation
        .documents
        .iter()
        .filter(|document| collection.map_or(true, |id| document.collection_id == id))
        .map(|document| {
            json!({
                "collection": document.collection_id,
                "path": document.path,
                "reference": DocumentationReference::file(document.path.clone()).to_string(),
                "title": document.title,
                "summary": document.summary,
                "headings": document.headings.iter().map(|heading| json!({
                    "title": heading.title,
                    "level": heading.level,
                    "line": heading.line,
                    "reference": DocumentationReference::heading(
                        document.path.clone(),
                        heading.segments.clone(),
                    ).to_string(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    if as_json {
        println!("{}", serde_json::to_string_pretty(&documents)?);
    } else if documents.is_empty() {
        println!("No configured documentation found.");
    } else {
        for document in documents {
            println!(
                "{}\t{}\t{}",
                document["collection"].as_str().unwrap_or_default(),
                document["reference"].as_str().unwrap_or_default(),
                document["title"].as_str().unwrap_or_default(),
            );
        }
    }
    Ok(())
}

pub fn backlinks(specs_dir: &Path, reference: &str, as_json: bool) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let normalized = parse_spec_url(reference)
        .map(|reference| reference.to_string())
        .or_else(|| {
            registry
                .get_by_id(reference)
                .is_some()
                .then(|| format!("spec:{reference}"))
        })
        .ok_or_else(|| anyhow::anyhow!("invalid or unknown reference: '{reference}'"))?;
    let backlinks = registry.documentation.backlinks_with_prefix(&normalized);
    if as_json {
        println!("{}", serde_json::to_string_pretty(&backlinks)?);
    } else if backlinks.is_empty() {
        println!("No backlinks to {normalized}.");
    } else {
        for backlink in backlinks {
            println!(
                "{}\t{}:{}\t{}",
                backlink.source_kind, backlink.source, backlink.line, backlink.target
            );
        }
    }
    Ok(())
}
