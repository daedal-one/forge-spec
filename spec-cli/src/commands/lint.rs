use std::path::Path;

use anyhow::Result;

use crate::lint;
use crate::lint::diagnostic::Severity;
use crate::model::registry::SpecRegistry;

pub fn run(specs_dir: &Path, require_symbols: bool, allow_custom_lsp: bool) -> Result<bool> {
    let registry = SpecRegistry::load(specs_dir)?;

    let diags = lint::lint_all_with_options(&registry, require_symbols, allow_custom_lsp);

    if diags.is_empty() {
        println!(
            "OK: {} spec(s) checked, no issues found.",
            registry.documents.len()
        );
        return Ok(true);
    }

    for d in &diags {
        eprintln!("{d}");
        eprintln!();
    }

    let errors = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diags
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    let infos = diags
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .count();

    eprintln!(
        "Checked {} spec(s): {} error(s), {} warning(s), {} info(s)",
        registry.documents.len(),
        errors,
        warnings,
        infos
    );

    Ok(errors == 0)
}
