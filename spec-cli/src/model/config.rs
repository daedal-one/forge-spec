use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const CURRENT_SPEC_BASELINE: &str = "forge-spec-v0.5.0";
pub const DEFAULT_INTELLECT_PROVIDER: &str = "forge-intellect";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentationCollectionConfig {
    pub id: String,
    pub title: String,
    pub root: String,
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SpecConfig {
    pub baseline: String,
    pub project: Option<String>,
    pub intellect_provider: String,
    pub documentation: Vec<DocumentationCollectionConfig>,
    pub declared: bool,
}

#[derive(Debug, Deserialize)]
struct RawSpecConfig {
    baseline: String,
    project: Option<String>,
    #[serde(default = "default_intellect_provider")]
    intellect_provider: String,
    #[serde(default)]
    documentation: Vec<DocumentationCollectionConfig>,
}

fn default_intellect_provider() -> String {
    DEFAULT_INTELLECT_PROVIDER.into()
}

impl SpecConfig {
    pub fn load(specs_dir: &Path) -> Result<Self> {
        let path = specs_dir.join("_config.toml");
        if !path.exists() {
            return Ok(Self {
                baseline: CURRENT_SPEC_BASELINE.into(),
                project: None,
                intellect_provider: DEFAULT_INTELLECT_PROVIDER.into(),
                documentation: Vec::new(),
                declared: false,
            });
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let raw: RawSpecConfig =
            toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Self {
            baseline: raw.baseline,
            project: raw.project,
            intellect_provider: raw.intellect_provider,
            documentation: raw.documentation,
            declared: true,
        })
    }

    pub(crate) fn from_toml(content: &str) -> Result<Self> {
        let raw: RawSpecConfig =
            toml::from_str(content).context("parsing candidate _config.toml")?;
        Ok(Self {
            baseline: raw.baseline,
            project: raw.project,
            intellect_provider: raw.intellect_provider,
            documentation: raw.documentation,
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
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:forge-spec\"\n",
        )
        .unwrap();
        let config = SpecConfig::load(temp.path()).unwrap();
        assert!(config.declared);
        assert_eq!(config.baseline, CURRENT_SPEC_BASELINE);
        assert_eq!(config.project.as_deref(), Some("PROJECT:forge-spec"));
        assert_eq!(config.intellect_provider, DEFAULT_INTELLECT_PROVIDER);
        assert!(config.documentation.is_empty());
    }

    #[test]
    fn loads_explicit_intellect_provider() {
        let config = SpecConfig::from_toml(
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:forge-spec\"\nintellect_provider = \"forge-intellect\"\n",
        )
        .unwrap();
        assert_eq!(config.intellect_provider, "forge-intellect");
    }

    #[test]
    fn loads_documentation_collections() {
        let config = SpecConfig::from_toml(
            r#"baseline = "forge-spec-v0.5.0"
project = "PROJECT:forge-spec"

[[documentation]]
id = "guides"
title = "Guides"
root = "docs"
include = ["**/*.md"]
exclude = ["generated/**"]
"#,
        )
        .unwrap();
        assert_eq!(config.documentation.len(), 1);
        assert_eq!(config.documentation[0].id, "guides");
        assert_eq!(config.documentation[0].exclude, ["generated/**"]);
    }
}
