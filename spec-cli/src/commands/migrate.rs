use std::path::Path;

use anyhow::{bail, Result};

use crate::cli::RenderTarget;
use crate::migration::{detect_baseline, write_baseline, MigrationPlan, V0_2_SPEC_BASELINE};
use crate::model::config::{SpecConfig, CURRENT_SPEC_BASELINE};
use crate::model::registry::SpecRegistry;
use crate::project::{ensure_project_document, write_project_config};

pub fn run(
    specs_dir: &Path,
    guide: bool,
    target: &RenderTarget,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<()> {
    let detected = detect_baseline(specs_dir)?;
    let source = from.unwrap_or(&detected.baseline);
    let destination = to.unwrap_or(CURRENT_SPEC_BASELINE);

    if !guide && detected.declared && source != detected.baseline {
        bail!(
            "--from '{source}' does not match the declared baseline '{}'; use --guide to inspect another route without applying it",
            detected.baseline
        );
    }

    let plan = MigrationPlan::build(source, destination)?;
    if guide {
        match target {
            RenderTarget::Human => print!("{}", plan.render_human()),
            RenderTarget::Agent => print!("{}", plan.render_agent()),
        }
        return Ok(());
    }

    if !plan.steps.is_empty() {
        println!("Migrating {} to {}...", plan.from, plan.to);
    }
    let reports = plan.apply(specs_dir)?;
    for (step, report) in plan.steps.iter().zip(&reports) {
        println!(
            "  {} -> {}: {} document(s) updated",
            step.guide.from, step.guide.to, report.documents_changed
        );
    }

    // A tree with an existing PROJECT document but no config is already v0.3
    // by shape, so its migration plan is empty. Finalize the singleton and its
    // config here as well as in the adjacent v0.2 -> v0.3 transformation.
    let (project_documents_changed, project_config_updated) =
        if destination == CURRENT_SPEC_BASELINE {
            if !specs_dir.join("_config.toml").exists() {
                // Keep an interruption recoverable before the target baseline
                // is written last.
                write_baseline(specs_dir, V0_2_SPEC_BASELINE)?;
            }
            let config = SpecConfig::load(specs_dir)?;
            let project = ensure_project_document(specs_dir, config.project.as_deref())?;
            let config_updated = write_project_config(specs_dir, &project.id)?;
            (usize::from(project.created), config_updated)
        } else {
            (0, false)
        };

    let redirect_rewrites = apply_redirects(specs_dir)?;
    let baseline_updated = write_baseline(specs_dir, destination)?;

    let format_updates: usize = reports
        .iter()
        .map(|report| report.documents_changed)
        .sum::<usize>()
        + project_documents_changed;
    if format_updates == 0 && redirect_rewrites == 0 && !project_config_updated && !baseline_updated
    {
        println!("No format or redirect migrations needed.");
    } else {
        println!(
            "Migration complete at {destination}: {format_updates} document(s) updated, {redirect_rewrites} redirect rewrite(s)."
        );
    }

    Ok(())
}

fn apply_redirects(specs_dir: &Path) -> Result<usize> {
    let registry = SpecRegistry::load(specs_dir)?;
    if registry.redirects.is_empty() {
        return Ok(0);
    }

    println!("Processing {} redirect(s)...", registry.redirects.len());
    let mut total_rewrites = 0;

    for document in &registry.documents {
        let content = std::fs::read_to_string(&document.source_path)?;
        let mut migrated = content.clone();
        let mut document_rewrites = 0;

        for redirect in &registry.redirects {
            let from_reference = format!("spec:{}", redirect.from);
            let to_reference = format!("spec:{}", redirect.to);
            if migrated.contains(&from_reference) {
                migrated = migrated.replace(&from_reference, &to_reference);
                document_rewrites += 1;
            }
            if migrated.contains(&redirect.from) {
                migrated = migrated.replace(&redirect.from, &redirect.to);
                document_rewrites += 1;
            }
        }

        if migrated != content {
            std::fs::write(&document.source_path, migrated)?;
            println!(
                "  {} — {} rewrite(s)",
                document.source_path.display(),
                document_rewrites
            );
            total_rewrites += document_rewrites;
        }
    }

    Ok(total_rewrites)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::LEGACY_SPEC_BASELINE;

    fn write_legacy_spec(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("legacy.spec.md");
        std::fs::write(
            &path,
            "---\nid: REQ:test/legacy\ntype: requirement\nstatus: draft\nversion: 0.1.0\nowners: [carlo]\nlevel: MUST\n---\n\n# Legacy\n",
        )
        .unwrap();
        path
    }

    #[test]
    fn guide_is_read_only() {
        let temp = tempfile::tempdir().unwrap();
        let path = write_legacy_spec(temp.path());
        let before = std::fs::read_to_string(&path).unwrap();

        run(temp.path(), true, &RenderTarget::Agent, None, None).unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), before);
        assert!(!temp.path().join("_config.toml").exists());
    }

    #[test]
    fn applies_detected_chain_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let path = write_legacy_spec(temp.path());

        run(temp.path(), false, &RenderTarget::Human, None, None).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("version:"));
        let config = std::fs::read_to_string(temp.path().join("_config.toml")).unwrap();
        assert!(config.contains(&format!("baseline = \"{CURRENT_SPEC_BASELINE}\"")));
        assert!(config.contains("project = \"PROJECT:"));
        assert!(temp.path().join("_project.spec.md").is_file());

        run(temp.path(), false, &RenderTarget::Human, None, None).unwrap();
    }

    #[test]
    fn rejects_mismatched_apply_override() {
        let temp = tempfile::tempdir().unwrap();
        write_legacy_spec(temp.path());
        std::fs::write(
            temp.path().join("_config.toml"),
            format!("baseline = \"{CURRENT_SPEC_BASELINE}\"\n"),
        )
        .unwrap();

        let error = run(
            temp.path(),
            false,
            &RenderTarget::Human,
            Some(LEGACY_SPEC_BASELINE),
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("does not match the declared baseline"));
    }

    #[test]
    fn configures_an_unconfigured_current_project_tree() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: draft\nsummary: Demo.\nowners: []\n---\n\n# Demo\n",
        )
        .unwrap();

        run(temp.path(), false, &RenderTarget::Human, None, None).unwrap();

        let config = std::fs::read_to_string(temp.path().join("_config.toml")).unwrap();
        assert!(config.contains(&format!("baseline = \"{CURRENT_SPEC_BASELINE}\"")));
        assert!(config.contains("project = \"PROJECT:demo\""));
    }
}
