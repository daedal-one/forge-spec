use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use walkdir::WalkDir;

use crate::model::id::{EntityType, SpecId};

pub const PROJECT_FILE_NAME: &str = "_project.spec.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectScaffold {
    pub id: String,
    pub path: PathBuf,
    pub created: bool,
}

/// Find the PROJECT document already present in a tree, if any.
pub fn existing_project(specs_dir: &Path) -> Result<Option<(String, PathBuf)>> {
    let mut projects = Vec::new();
    if !specs_dir.is_dir() {
        return Ok(None);
    }

    for entry in WalkDir::new(specs_dir) {
        let entry = entry?;
        if !entry.file_type().is_file()
            || !entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".spec.md"))
        {
            continue;
        }
        let Ok(document) = crate::parse::parse_document(entry.path()) else {
            continue;
        };
        if document.universal.entity_type == EntityType::Project {
            projects.push((document.id_str(), entry.into_path()));
        }
    }

    if projects.len() > 1 {
        bail!(
            "multiple PROJECT documents found: {}",
            projects
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(projects.pop())
}

/// Ensure a draft PROJECT document exists without inventing project intent.
/// Existing document owners are reused to make migrated scaffolds useful.
pub fn ensure_project_document(
    specs_dir: &Path,
    preferred_id: Option<&str>,
) -> Result<ProjectScaffold> {
    if let Some((id, path)) = existing_project(specs_dir)? {
        if let Some(preferred) = preferred_id {
            if preferred != id {
                bail!(
                    "configured project '{preferred}' does not match existing PROJECT document '{id}'"
                );
            }
        }
        return Ok(ProjectScaffold {
            id,
            path,
            created: false,
        });
    }

    let id = match preferred_id {
        Some(value) => {
            let parsed: SpecId = value
                .parse()
                .map_err(|error: String| anyhow::anyhow!(error))?;
            if parsed.entity_type != EntityType::Project {
                bail!("configured project must use a PROJECT: ID, found '{value}'");
            }
            parsed.to_string()
        }
        None => format!("PROJECT:{}", derive_project_slug(specs_dir)),
    };
    let parsed: SpecId = id.parse().map_err(|error: String| anyhow::anyhow!(error))?;
    let path = specs_dir.join(PROJECT_FILE_NAME);
    if path.exists() {
        bail!(
            "{} already exists but is not a valid PROJECT document",
            path.display()
        );
    }

    std::fs::create_dir_all(specs_dir)
        .with_context(|| format!("creating {}", specs_dir.display()))?;
    let owners = collect_owners(specs_dir)?;
    let content = project_template(&id, &parsed.slug, &owners);
    crate::mutation::atomic_write_files(&[(path.clone(), content.into_bytes())])
        .with_context(|| format!("writing {}", path.display()))?;

    Ok(ProjectScaffold {
        id,
        path,
        created: true,
    })
}

/// Add or replace the configured project while preserving other TOML fields.
pub fn write_project_config(specs_dir: &Path, project_id: &str) -> Result<bool> {
    let path = specs_dir.join("_config.toml");
    let replacement = format!("project = {project_id:?}");
    if !path.exists() {
        crate::mutation::atomic_write_files(&[(
            path.clone(),
            format!("{replacement}\n").into_bytes(),
        )])
        .with_context(|| format!("writing {}", path.display()))?;
        return Ok(true);
    }

    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut found = false;
    let mut output = String::with_capacity(content.len().max(replacement.len() + 1));
    for line in content.split_inclusive('\n') {
        if is_project_assignment(line) {
            found = true;
            let newline = if line.ends_with('\n') { "\n" } else { "" };
            output.push_str(&replacement);
            output.push_str(newline);
        } else {
            output.push_str(line);
        }
    }
    if !found {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&replacement);
        output.push('\n');
    }

    if output == content {
        return Ok(false);
    }
    crate::mutation::atomic_write_files(&[(path.clone(), output.into_bytes())])
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

fn derive_project_slug(specs_dir: &Path) -> String {
    let name = specs_dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let mut slug = String::new();
    let mut previous_dash = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "project".to_string()
    } else {
        slug
    }
}

fn collect_owners(specs_dir: &Path) -> Result<Vec<String>> {
    let mut owners = BTreeSet::new();
    for entry in WalkDir::new(specs_dir) {
        let entry = entry?;
        if !entry.file_type().is_file()
            || !entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".spec.md"))
        {
            continue;
        }
        if let Ok(document) = crate::parse::parse_document(entry.path()) {
            owners.extend(document.universal.owners);
        }
    }
    Ok(owners.into_iter().collect())
}

fn project_template(id: &str, slug: &str, owners: &[String]) -> String {
    let title = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut value = part.to_string();
            if let Some(first) = value.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            value
        })
        .collect::<Vec<_>>()
        .join(" ");
    let owners = serde_json::to_string(owners).unwrap_or_else(|_| "[]".to_string());
    format!(
        "---\nid: {id}\ntype: project\nstatus: draft\nsummary: >\n  TODO: describe the {title} project.\nowners: {owners}\n---\n\n# {title}\n\n## Purpose\n\nTODO: explain why this project exists and who it serves.\n\n## Scope\n\nTODO: define the capabilities and boundaries that belong to this project.\n\n## Non-goals\n\nTODO: name plausible responsibilities that are intentionally outside the project.\n\n## Principles\n\nTODO: record the durable principles that should guide descendant specifications.\n"
    )
}

fn is_project_assignment(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("project")
        .map(str::trim_start)
        .is_some_and(|rest| rest.starts_with('='))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffolds_and_reuses_a_project_document() {
        let temp = tempfile::tempdir().unwrap();
        let specs = temp.path().join("demo-project").join(".specs");
        let first = ensure_project_document(&specs, None).unwrap();
        let second = ensure_project_document(&specs, Some(&first.id)).unwrap();

        assert_eq!(first.id, "PROJECT:demo-project");
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.path, second.path);
    }

    #[test]
    fn project_config_update_preserves_other_fields() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.5.0\"\nowner = \"team\"\n",
        )
        .unwrap();

        assert!(write_project_config(temp.path(), "PROJECT:demo").unwrap());
        assert!(!write_project_config(temp.path(), "PROJECT:demo").unwrap());
        let content = std::fs::read_to_string(temp.path().join("_config.toml")).unwrap();
        assert!(content.contains("project = \"PROJECT:demo\""));
        assert!(content.contains("owner = \"team\""));
    }
}
