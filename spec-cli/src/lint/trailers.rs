use crate::model::registry::SpecRegistry;

use super::diagnostic::Diagnostic;

/// R013: Commit trailer references resolve.
/// This check requires git access — when git is unavailable, it's skipped silently.
pub fn check_trailer_references(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Try to open the git repo
    let repo = match git2::Repository::discover(&registry.specs_dir) {
        Ok(r) => r,
        Err(_) => return diags, // No git repo, skip
    };

    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(_) => return diags,
    };

    if revwalk.push_head().is_err() {
        return diags;
    }

    // Walk recent commits (limit to last 500 to avoid unbounded walks)
    for oid in revwalk.take(501) {
        let oid = match oid {
            Ok(o) => o,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let message = match commit.message() {
            Some(m) => m,
            None => continue,
        };

        for line in message.lines() {
            if let Some(rest) = line.strip_prefix("Spec-Ref:") {
                let rest = rest.trim();
                // Parse: REQ:auth/foo (implements) or just REQ:auth/foo
                let spec_ref = if let Some(pos) = rest.find('(') {
                    rest[..pos].trim()
                } else {
                    rest
                };

                if !spec_ref.is_empty() {
                    let (exists, _) = registry.reference_exists(spec_ref);
                    if !exists {
                        let sha = &oid.to_string()[..7];
                        diags.push(Diagnostic::error(
                            "R013",
                            format!(
                                "commit {sha} has Spec-Ref to '{spec_ref}' which does not exist"
                            ),
                            registry.specs_dir.clone(),
                        ));
                    }
                }
            }
        }
    }

    diags
}
