use std::path::Path;

use anyhow::Result;

/// Resolve a `src:` reference to file content.
/// Tries `pinned_at` SHA first (via git2), falls back to working-tree read.
pub fn resolve_source(
    specs_dir: &Path,
    src_path: &str,
    lines: Option<(u32, u32)>,
    pinned_at: Option<&str>,
) -> Result<String> {
    // Find the repo root
    let repo_root = find_repo_root(specs_dir);

    // Try git2 if pinned
    if let Some(sha) = pinned_at {
        if let Some(ref root) = repo_root {
            if let Ok(content) = read_from_git(root, src_path, sha) {
                return Ok(extract_lines(&content, lines));
            }
        }
    }

    // Fall back to filesystem read
    let full_path = if let Some(ref root) = repo_root {
        root.join(src_path)
    } else {
        specs_dir.parent().unwrap_or(specs_dir).join(src_path)
    };

    if full_path.exists() {
        let content = std::fs::read_to_string(&full_path)?;
        Ok(extract_lines(&content, lines))
    } else {
        Ok(format!("<!-- source not found: {src_path} -->"))
    }
}

fn find_repo_root(start: &Path) -> Option<std::path::PathBuf> {
    git2::Repository::discover(start)
        .ok()
        .and_then(|r| r.workdir().map(|w| w.to_path_buf()))
}

fn read_from_git(repo_root: &Path, file_path: &str, sha: &str) -> Result<String> {
    let repo = git2::Repository::open(repo_root)?;
    let oid = git2::Oid::from_str(sha)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let entry = tree.get_path(Path::new(file_path))?;
    let blob = repo.find_blob(entry.id())?;
    let content = std::str::from_utf8(blob.content())?;
    Ok(content.to_string())
}

fn extract_lines(content: &str, lines: Option<(u32, u32)>) -> String {
    match lines {
        Some((start, end)) => {
            let start = start.saturating_sub(1) as usize;
            let end = end as usize;
            content
                .lines()
                .skip(start)
                .take(end - start)
                .collect::<Vec<_>>()
                .join("\n")
        }
        None => content.to_string(),
    }
}
