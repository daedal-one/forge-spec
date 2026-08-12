use std::path::Path;

use anyhow::{bail, Result};
use serde_json::json;

use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;
use crate::parse::references::parse_spec_url;
use crate::symbol::SymbolService;

pub fn symbols(
    specs_dir: &Path,
    path: &str,
    query: Option<&str>,
    as_json: bool,
    allow_custom_lsp: bool,
) -> Result<()> {
    let service = SymbolService::new(specs_dir, allow_custom_lsp)?;
    let symbols = service.list_symbols(path, query)?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&symbols)?);
    } else {
        for symbol in symbols {
            println!(
                "{}\t{}\t{}:{}",
                symbol.reference,
                symbol.kind,
                symbol.range.start.line + 1,
                symbol.range.start.character + 1
            );
        }
    }
    Ok(())
}

pub fn resolve(
    specs_dir: &Path,
    reference: &str,
    as_json: bool,
    allow_custom_lsp: bool,
) -> Result<()> {
    let parsed = parse_spec_url(reference)
        .ok_or_else(|| anyhow::anyhow!("invalid spec reference: {reference}"))?;
    match parsed {
        SpecReference::Source(source) => {
            let resolved = SymbolService::new(specs_dir, allow_custom_lsp)?.resolve(&source)?;
            if as_json {
                println!("{}", serde_json::to_string_pretty(&resolved)?);
            } else {
                println!("{}", resolved.reference);
                if let Some(symbol) = resolved.symbol {
                    println!("symbol: {symbol}");
                }
                println!("path: {}", resolved.path);
                for location in resolved.locations {
                    println!(
                        "location: {}:{}-{}:{}",
                        location.start.line + 1,
                        location.start.character + 1,
                        location.end.line + 1,
                        location.end.character + 1
                    );
                }
                if !resolved.snippet.is_empty() {
                    println!("\n{}", resolved.snippet);
                }
            }
        }
        SpecReference::Spec(anchor) => {
            let registry = SpecRegistry::load(specs_dir)?;
            let key = anchor.to_string();
            let (canonical, redirected) = registry.resolve_redirect(&key);
            if !registry.reference_exists(&key).0 {
                bail!("spec reference not found: {key}");
            }
            let document_id = canonical.split('#').next().unwrap_or(&canonical);
            let document = registry
                .get_by_id(document_id)
                .ok_or_else(|| anyhow::anyhow!("spec not found: {document_id}"))?;
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "reference": reference,
                        "canonical": canonical,
                        "redirected": redirected,
                        "path": document.source_path,
                    }))?
                );
            } else {
                println!("{canonical}\t{}", document.source_path.display());
            }
        }
        SpecReference::Documentation(documentation) => {
            let registry = SpecRegistry::load(specs_dir)?;
            let Some((document, heading)) = registry.documentation.resolve(&documentation) else {
                bail!("documentation reference not found: {documentation}");
            };
            let start = heading.map(|heading| heading.line).unwrap_or(1);
            let end = heading
                .map(|heading| heading.end_line)
                .unwrap_or_else(|| document.body.lines().count().max(1));
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "reference": documentation.to_string(),
                        "path": document.path,
                        "collection": document.collection_id,
                        "title": document.title,
                        "summary": document.summary,
                        "startLine": start,
                        "endLine": end,
                    }))?
                );
            } else {
                println!("{}", documentation);
                println!("collection: {}", document.collection_id);
                println!("path: {}", document.path);
                println!("location: {start}-{end}");
                let snippet = document
                    .body
                    .lines()
                    .skip(start.saturating_sub(1))
                    .take(end.saturating_sub(start) + 1)
                    .collect::<Vec<_>>()
                    .join("\n");
                if !snippet.is_empty() {
                    println!("\n{snippet}");
                }
            }
        }
    }
    Ok(())
}
