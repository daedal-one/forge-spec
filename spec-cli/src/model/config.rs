use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

pub const CURRENT_SPEC_BASELINE: &str = "forge-spec-v0.2.0";

#[derive(Debug, Clone)]
pub struct SpecConfig {
    pub baseline: String,
    pub declared: bool,
}

#[derive(Debug, Deserialize)]
struct RawSpecConfig {
    baseline: String,
}

impl SpecConfig {
    pub fn load(specs_dir: &Path) -> Result<Self> {
        let path = specs_dir.join("_config.toml");
        if !path.exists() {
            return Ok(Self {
                baseline: CURRENT_SPEC_BASELINE.into(),
                declared: false,
            });
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let raw: RawSpecConfig =
            toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Self {
            baseline: raw.baseline,
            declared: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_tree_level_baseline() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.2.0\"\n",
        )
        .unwrap();
        let config = SpecConfig::load(temp.path()).unwrap();
        assert!(config.declared);
        assert_eq!(config.baseline, CURRENT_SPEC_BASELINE);
    }
}
