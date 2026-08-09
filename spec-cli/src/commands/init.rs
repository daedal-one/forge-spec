use std::path::Path;

use anyhow::{bail, Context, Result};
use walkdir::WalkDir;

use crate::model::config::{SpecConfig, CURRENT_SPEC_BASELINE};

pub fn run(specs_dir: &Path) -> Result<()> {
    if specs_dir.exists() && !specs_dir.is_dir() {
        bail!("{} exists and is not a directory", specs_dir.display());
    }

    let config_path = specs_dir.join("_config.toml");
    if config_path.exists() {
        let config = SpecConfig::load(specs_dir)?;
        if config.baseline != CURRENT_SPEC_BASELINE {
            bail!(
                "{} declares baseline '{}'; run `spec migrate --guide --target agent` before changing it",
                config_path.display(),
                config.baseline
            );
        }

        println!(
            "Already initialized {} at {}",
            specs_dir.display(),
            CURRENT_SPEC_BASELINE
        );
        return Ok(());
    }

    if has_spec_files(specs_dir) {
        bail!(
            "{} contains specs without _config.toml; run `spec migrate --guide --target agent`, then `spec migrate`",
            specs_dir.display()
        );
    }

    std::fs::create_dir_all(specs_dir)
        .with_context(|| format!("creating {}", specs_dir.display()))?;
    std::fs::write(
        &config_path,
        format!("baseline = \"{CURRENT_SPEC_BASELINE}\"\n"),
    )
    .with_context(|| format!("writing {}", config_path.display()))?;

    println!("Initialized {}", specs_dir.display());
    println!("Next: spec new REQ <namespace/slug> && spec lint");
    Ok(())
}

fn has_spec_files(specs_dir: &Path) -> bool {
    if !specs_dir.is_dir() {
        return false;
    }

    WalkDir::new(specs_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            entry.file_type().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(".spec.md"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_a_new_tree() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");

        run(&specs_dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(specs_dir.join("_config.toml")).unwrap(),
            format!("baseline = \"{CURRENT_SPEC_BASELINE}\"\n")
        );
    }

    #[test]
    fn current_tree_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");

        run(&specs_dir).unwrap();
        run(&specs_dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(specs_dir.join("_config.toml")).unwrap(),
            format!("baseline = \"{CURRENT_SPEC_BASELINE}\"\n")
        );
    }

    #[test]
    fn refuses_to_stamp_an_unconfigured_existing_tree() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::write(specs_dir.join("legacy.spec.md"), "---\nversion: 1\n---\n").unwrap();

        let error = run(&specs_dir).unwrap_err().to_string();

        assert!(error.contains("contains specs without _config.toml"));
        assert!(!specs_dir.join("_config.toml").exists());
    }

    #[test]
    fn refuses_to_overwrite_a_different_baseline() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::write(
            specs_dir.join("_config.toml"),
            "baseline = \"forge-spec-v0.1.0\"\n",
        )
        .unwrap();

        let error = run(&specs_dir).unwrap_err().to_string();

        assert!(error.contains("run `spec migrate --guide --target agent`"));
        assert_eq!(
            std::fs::read_to_string(specs_dir.join("_config.toml")).unwrap(),
            "baseline = \"forge-spec-v0.1.0\"\n"
        );
    }

    #[test]
    fn refuses_a_file_as_the_tree_path() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");
        std::fs::write(&specs_dir, "not a directory").unwrap();

        let error = run(&specs_dir).unwrap_err().to_string();

        assert!(error.contains("exists and is not a directory"));
    }
}
