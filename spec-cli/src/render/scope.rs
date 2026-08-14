use std::collections::BTreeSet;

use crate::graph;
use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    Full,
    Summary,
    IdOnly,
    None,
}

impl DetailLevel {
    pub fn from_str_val(s: &str) -> Self {
        match s {
            "full" => Self::Full,
            "summary" => Self::Summary,
            "id-only" => Self::IdOnly,
            "none" => Self::None,
            _ => Self::Full,
        }
    }
}

/// A scoped entry for rendering.
#[derive(Debug, Clone)]
pub struct ScopedEntry {
    pub id: String,
    pub detail: DetailLevel,
}

/// Compute the render scope for a focal spec.
pub fn compute_scope(
    registry: &SpecRegistry,
    focal_id: &str,
    ancestor_detail: DetailLevel,
    descendant_detail: DetailLevel,
    depth: Option<usize>,
) -> Vec<ScopedEntry> {
    let mut entries = Vec::new();
    let mut included: BTreeSet<String> = BTreeSet::new();

    // Project intent is ambient context, independent of refinement depth and
    // ancestor flags. Include it once before the focal specification.
    if let Some(project_id) = registry.project_id() {
        entries.push(ScopedEntry {
            id: project_id.clone(),
            detail: DetailLevel::Full,
        });
        included.insert(project_id);
    }

    // The focal spec in full
    if registry.get_by_id(focal_id).is_some() && included.insert(focal_id.to_string()) {
        entries.push(ScopedEntry {
            id: focal_id.to_string(),
            detail: DetailLevel::Full,
        });
    }

    let max_depth = depth.unwrap_or(1);

    // Ancestors up to the requested traversal depth.
    if ancestor_detail != DetailLevel::None {
        let ancestors = graph::query::transitive_ancestors(registry, focal_id, max_depth);
        for (a, _) in ancestors {
            if included.insert(a.clone()) {
                entries.push(ScopedEntry {
                    id: a,
                    detail: ancestor_detail,
                });
            }
        }
    }

    // Descendants up to the requested traversal depth. PROJECT uses the
    // synthesized hierarchy because it is ambient context, not a refinement
    // parent.
    if descendant_detail != DetailLevel::None {
        let descendants = if registry.project_id().as_deref() == Some(focal_id) {
            graph::query::hierarchy_descendants(registry, focal_id, max_depth)
        } else {
            graph::query::descendants(registry, focal_id, max_depth)
        };
        for (d, _) in descendants {
            if included.insert(d.clone()) {
                entries.push(ScopedEntry {
                    id: d,
                    detail: descendant_detail,
                });
            }
        }
    }

    // Glossary terms used in any included body
    let glossary_ids = collect_glossary_refs(registry, &included);
    for gid in glossary_ids {
        if included.insert(gid.clone()) {
            entries.push(ScopedEntry {
                id: gid,
                detail: DetailLevel::Full,
            });
        }
    }

    entries
}

fn collect_glossary_refs(registry: &SpecRegistry, included_ids: &BTreeSet<String>) -> Vec<String> {
    let mut glossary_ids = BTreeSet::new();

    for id in included_ids {
        if let Some(doc) = registry.get_by_id(id) {
            for loc_ref in &doc.references {
                if let SpecReference::Spec(qa) = &loc_ref.reference {
                    if qa.spec_id.entity_type == crate::model::id::EntityType::Glo {
                        glossary_ids.insert(qa.spec_id.to_string());
                    }
                }
            }
        }
    }

    glossary_ids.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_context_is_first_even_when_ancestors_are_disabled() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("req.spec.md"),
            "---\nid: REQ:demo/work\ntype: requirement\nstatus: accepted\nsummary: Work.\nowners: [dev]\nlevel: MUST\nrefines: []\n---\n\n# Work\n",
        )
        .unwrap();
        let registry = SpecRegistry::load(temp.path()).unwrap();

        let scope = compute_scope(
            &registry,
            "REQ:demo/work",
            DetailLevel::None,
            DetailLevel::None,
            None,
        );
        assert_eq!(scope.len(), 2);
        assert_eq!(scope[0].id, "PROJECT:demo");
        assert_eq!(scope[0].detail, DetailLevel::Full);
        assert_eq!(scope[1].id, "REQ:demo/work");

        let project_scope = compute_scope(
            &registry,
            "PROJECT:demo",
            DetailLevel::None,
            DetailLevel::None,
            None,
        );
        assert_eq!(project_scope.len(), 1);
        assert_eq!(project_scope[0].id, "PROJECT:demo");
    }

    #[test]
    fn depth_controls_transitive_descendant_scope() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        for (file, id, refines) in [
            ("root.spec.md", "REQ:demo/root", "[]"),
            ("child.spec.md", "REQ:demo/child", "[REQ:demo/root]"),
            ("leaf.spec.md", "REQ:demo/leaf", "[REQ:demo/child]"),
        ] {
            std::fs::write(
                temp.path().join(file),
                format!(
                    "---\nid: {id}\ntype: requirement\nstatus: accepted\nsummary: {id}.\nowners: [dev]\nlevel: MUST\nrefines: {refines}\n---\n\n# {id}\n"
                ),
            )
            .unwrap();
        }
        let registry = SpecRegistry::load(temp.path()).unwrap();

        let direct = compute_scope(
            &registry,
            "REQ:demo/root",
            DetailLevel::None,
            DetailLevel::Summary,
            None,
        );
        assert_eq!(
            direct
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["PROJECT:demo", "REQ:demo/root", "REQ:demo/child"]
        );

        let transitive = compute_scope(
            &registry,
            "REQ:demo/root",
            DetailLevel::None,
            DetailLevel::Summary,
            Some(2),
        );
        assert_eq!(
            transitive
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "PROJECT:demo",
                "REQ:demo/root",
                "REQ:demo/child",
                "REQ:demo/leaf",
            ]
        );
    }
}
