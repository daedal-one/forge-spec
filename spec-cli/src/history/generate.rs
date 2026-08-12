use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use super::trailers::{walk_trailers, TrailerEvent};

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct HistoryFile {
    pub id: String,
    pub events: Vec<HistoryEvent>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct HistoryEvent {
    pub sha: String,
    pub kind: String,
    pub date: String,
    pub author: String,
}

/// Generate or update history files in `.specs/_history/`.
pub fn update_history(specs_dir: &Path) -> Result<Vec<String>> {
    let history_dir = specs_dir.join("_history");
    std::fs::create_dir_all(&history_dir)?;

    let events = walk_trailers(specs_dir)?;

    // Group events by spec ID (strip anchors)
    let mut by_spec: BTreeMap<String, Vec<&TrailerEvent>> = BTreeMap::new();
    for event in &events {
        let spec_id = if let Some(pos) = event.spec_ref.find('#') {
            &event.spec_ref[..pos]
        } else {
            &event.spec_ref
        };
        by_spec.entry(spec_id.to_string()).or_default().push(event);
    }

    let mut written = Vec::new();
    let mut writes = Vec::new();

    for (spec_id, events) in &by_spec {
        let history = HistoryFile {
            id: spec_id.clone(),
            events: events
                .iter()
                .map(|e| HistoryEvent {
                    sha: e.sha.clone(),
                    kind: e.kind.clone(),
                    date: e.date.clone(),
                    author: e.author.clone(),
                })
                .collect(),
        };

        let filename = spec_id.replace(':', "_").replace('/', "_");
        let path = history_dir.join(format!("{filename}.json"));
        let json = serde_json::to_string_pretty(&history)?;

        // Only write if content changed
        let should_write = match std::fs::read_to_string(&path) {
            Ok(existing) => existing != json,
            Err(_) => true,
        };

        if should_write {
            writes.push((path, json.into_bytes()));
            written.push(spec_id.clone());
        }
    }

    crate::mutation::atomic_write_files(&writes)?;

    Ok(written)
}

/// Read history for a single spec ID.
pub fn read_history(specs_dir: &Path, spec_id: &str) -> Result<Option<HistoryFile>> {
    let filename = spec_id.replace(':', "_").replace('/', "_");
    let path = specs_dir.join("_history").join(format!("{filename}.json"));

    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let history: HistoryFile = serde_json::from_str(&content)?;
    Ok(Some(history))
}
