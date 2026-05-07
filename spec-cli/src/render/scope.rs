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
    _depth: Option<usize>,
) -> Vec<ScopedEntry> {
    let mut entries = Vec::new();
    let mut included: BTreeSet<String> = BTreeSet::new();

    // The focal spec in full
    if registry.get_by_id(focal_id).is_some() {
        entries.push(ScopedEntry {
            id: focal_id.to_string(),
            detail: DetailLevel::Full,
        });
        included.insert(focal_id.to_string());
    }

    // Direct ancestors
    if ancestor_detail != DetailLevel::None {
        let anc = graph::query::ancestors(registry, focal_id);
        for a in anc {
            if included.insert(a.clone()) {
                entries.push(ScopedEntry {
                    id: a,
                    detail: ancestor_detail,
                });
            }
        }
    }

    // Direct descendants
    if descendant_detail != DetailLevel::None {
        let desc = graph::query::children(registry, focal_id);
        for d in desc {
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
