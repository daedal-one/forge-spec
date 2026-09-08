//! Safe machine export: no hooks, source providers, or repository code.
use crate::projection::{self, Overlay, SpecState};
use anyhow::Result;
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
#[derive(Debug, Serialize)]
pub struct Revision {
    pub revision: Option<String>,
    pub commit: Option<String>,
    pub path: String,
}
#[derive(Debug, Serialize)]
pub struct ModelExport {
    pub schema: &'static str,
    pub baseline: String,
    pub projector_version: &'static str,
    pub state: SpecState,
    pub digest: String,
    pub revisions: BTreeMap<String, Revision>,
    pub intent_digests: BTreeMap<String, String>,
    pub capabilities: Vec<&'static str>,
}
pub fn export(specs_dir: &Path) -> Result<ModelExport> {
    let state = projection::project_compatible(specs_dir, &Overlay::new())?;
    let root = specs_dir.parent().unwrap_or(specs_dir);
    let repository = git2::Repository::discover(root).ok();
    let commit = repository
        .as_ref()
        .and_then(|r| r.head().ok())
        .and_then(|h| h.target())
        .map(|o| o.to_string());
    let mut revisions = BTreeMap::new();
    for (id, path) in state
        .specifications
        .iter()
        .map(|s| (&s.id, &s.path))
        .chain(state.work_items.iter().map(|s| (&s.id, &s.path)))
    {
        let revision = if repository.is_some() {
            Some(crate::history::revision::for_path(&root.join(path))?.to_string())
        } else {
            None
        };
        revisions.insert(
            id.clone(),
            Revision {
                revision,
                commit: commit.clone(),
                path: path.clone(),
            },
        );
    }
    let intent_digests = state
        .specifications
        .iter()
        .map(|spec| {
            Ok((
                spec.id.clone(),
                projection::projected_specification_intent_digest(spec)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let digest = blake3::hash(&state.canonical_json()?).to_hex().to_string();
    Ok(ModelExport {
        schema: "forge-spec-model/v1",
        baseline: state.config.baseline.clone(),
        projector_version: projection::SPEC_CLI_VERSION,
        state,
        digest,
        revisions,
        intent_digests,
        capabilities: vec![
            "canonical-specifications",
            "work-items",
            "documentation",
            "architecture",
            "scenario-flow",
            "saved-views",
            "read-only",
            "baseline-v0.6-compatible",
        ],
    })
}
pub fn run(specs_dir: &Path) -> Result<()> {
    println!("{}", serde_json::to_string(&export(specs_dir)?)?);
    Ok(())
}
