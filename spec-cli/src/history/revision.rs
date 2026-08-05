use std::fmt;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileRevision {
    pub committed: u64,
    pub dirty: bool,
}

impl fmt::Display for FileRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "r{}", self.committed)?;
        if self.dirty {
            formatter.write_str("+dirty")?;
        }
        Ok(())
    }
}

/// Derive a file revision from Git. The committed component is the number of
/// commits that touched the file, following renames; working-tree changes are
/// represented separately so editing never mutates spec frontmatter.
pub fn for_path(path: &Path) -> Result<FileRevision> {
    let repository = match git2::Repository::discover(path) {
        Ok(repository) => repository,
        Err(_) => {
            return Ok(FileRevision {
                committed: 0,
                dirty: true,
            })
        }
    };
    let root = repository
        .workdir()
        .context("bare repositories cannot provide file revisions")?
        .canonicalize()
        .context("canonicalizing repository root")?;
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("canonicalizing {}", path.display()))?;
    let relative = canonical_path
        .strip_prefix(&root)
        .with_context(|| format!("{} is outside {}", path.display(), root.display()))?;

    let history = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["log", "--follow", "--format=%H", "--"])
        .arg(relative)
        .output()
        .context("running git log for spec revision")?;
    if !history.status.success() {
        anyhow::bail!(
            "git log failed while deriving revision for {}",
            path.display()
        );
    }
    let committed = String::from_utf8_lossy(&history.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count() as u64;

    let status = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["status", "--porcelain", "--untracked-files=all", "--"])
        .arg(relative)
        .output()
        .context("running git status for spec revision")?;
    if !status.status.success() {
        anyhow::bail!(
            "git status failed while deriving revision for {}",
            path.display()
        );
    }

    Ok(FileRevision {
        committed,
        dirty: !status.stdout.is_empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit_file(repository: &git2::Repository, relative: &Path, message: &str) {
        let mut index = repository.index().unwrap();
        index.add_path(relative).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repository.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Test", "test@example.com").unwrap();
        let parents = repository
            .head()
            .ok()
            .and_then(|head| head.target())
            .and_then(|id| repository.find_commit(id).ok())
            .into_iter()
            .collect::<Vec<_>>();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .unwrap();
    }

    #[test]
    fn display_separates_committed_and_working_tree_state() {
        assert_eq!(
            FileRevision {
                committed: 7,
                dirty: false
            }
            .to_string(),
            "r7"
        );
        assert_eq!(
            FileRevision {
                committed: 7,
                dirty: true
            }
            .to_string(),
            "r7+dirty"
        );
    }

    #[test]
    fn derives_incremental_revision_and_dirty_state_from_git() {
        let temp = tempfile::tempdir().unwrap();
        let repository = git2::Repository::init(temp.path()).unwrap();
        let relative = Path::new(".specs/demo.spec.md");
        let path = temp.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "first").unwrap();
        commit_file(&repository, relative, "first");
        assert_eq!(
            for_path(&path).unwrap(),
            FileRevision {
                committed: 1,
                dirty: false
            }
        );

        std::fs::write(&path, "second").unwrap();
        assert_eq!(
            for_path(&path).unwrap(),
            FileRevision {
                committed: 1,
                dirty: true
            }
        );
        commit_file(&repository, relative, "second");
        assert_eq!(
            for_path(&path).unwrap(),
            FileRevision {
                committed: 2,
                dirty: false
            }
        );
    }
}
