use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ConfigFile {
    knowledge_base: Option<KnowledgeBaseConfig>,
}

#[derive(Debug, Deserialize)]
struct KnowledgeBaseConfig {
    path: String,
}

/// Load the knowledge-base root from `.specs/_config.toml`, resolved relative
/// to the repository root (or to the `.specs/` parent when outside a repo).
pub fn load_kb_root(specs_dir: &Path) -> Result<Option<PathBuf>> {
    let config_path = specs_dir.join("_config.toml");
    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path)?;
    let config: ConfigFile = toml::from_str(&content)?;

    let kb_config = match config.knowledge_base {
        Some(c) => c,
        None => return Ok(None),
    };

    let base = find_repo_root(specs_dir)
        .unwrap_or_else(|| specs_dir.parent().unwrap_or(specs_dir).to_path_buf());

    let resolved = base.join(&kb_config.path);
    Ok(Some(resolved))
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    git2::Repository::discover(start)
        .ok()
        .and_then(|r| r.workdir().map(|w| w.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_with_kb() {
        let content = r#"
[knowledge_base]
path = "../my-vault"
"#;
        let config: ConfigFile = toml::from_str(content).unwrap();
        assert_eq!(
            config.knowledge_base.unwrap().path,
            "../my-vault"
        );
    }

    #[test]
    fn parse_config_without_kb() {
        let content = "";
        let config: ConfigFile = toml::from_str(content).unwrap();
        assert!(config.knowledge_base.is_none());
    }
}
