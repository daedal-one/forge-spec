use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use super::config::SpecConfig;
use super::document::SpecDocument;
use crate::parse;

/// A redirect entry from `_redirects.toml`.
#[derive(Debug, Clone)]
pub struct Redirect {
    pub from: String,
    pub to: String,
}

/// The central registry holding all parsed spec documents and indexes.
#[derive(Debug)]
pub struct SpecRegistry {
    pub documents: Vec<SpecDocument>,
    /// Map from spec ID string to document index.
    pub id_index: BTreeMap<String, usize>,
    /// Map from `"id#anchor"` to document index.
    pub anchor_index: BTreeMap<String, usize>,
    pub redirects: Vec<Redirect>,
    pub specs_dir: PathBuf,
    pub config: SpecConfig,
}

impl SpecRegistry {
    /// Load all `.spec.md` files from a `.specs/` directory.
    pub fn load(specs_dir: &Path) -> Result<Self> {
        let mut documents = Vec::new();
        let mut parse_errors: Vec<String> = Vec::new();

        let config = SpecConfig::load(specs_dir)?;

        // Walk all .spec.md files
        for entry in WalkDir::new(specs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map_or(false, |n| n.ends_with(".spec.md"))
            })
        {
            let path = entry.path();
            match parse::parse_document(path) {
                Ok(doc) => documents.push(doc),
                Err(e) => {
                    parse_errors.push(format!("{}: {e:#}", path.display()));
                }
            }
        }

        if !parse_errors.is_empty() {
            // Still build the registry with what we have, but report errors
            eprintln!("Parse errors:");
            for err in &parse_errors {
                eprintln!("  {err}");
            }
        }

        // Build indexes
        let mut id_index = BTreeMap::new();
        let mut anchor_index = BTreeMap::new();

        for (idx, doc) in documents.iter().enumerate() {
            let id_str = doc.id_str();
            id_index.insert(id_str.clone(), idx);

            for anchor in doc.anchors() {
                let key = format!("{id_str}#{anchor}");
                anchor_index.insert(key, idx);
            }
        }

        // Load redirects
        let redirects_path = specs_dir.join("_redirects.toml");
        let redirects = if redirects_path.exists() {
            parse::redirects::load_redirects(&redirects_path)
                .with_context(|| format!("loading {}", redirects_path.display()))?
        } else {
            Vec::new()
        };

        Ok(Self {
            documents,
            id_index,
            anchor_index,
            redirects,
            specs_dir: specs_dir.to_path_buf(),
            config,
        })
    }

    /// Load the registry while replacing one document with unsaved editor
    /// content. Redirects and all indexes are rebuilt consistently.
    pub fn load_with_override(specs_dir: &Path, source_path: &Path, content: &str) -> Result<Self> {
        let mut registry = Self::load(specs_dir)?;
        let document = crate::parse::parse_content(source_path, content)?;
        if let Some(index) = registry
            .documents
            .iter()
            .position(|candidate| candidate.source_path == source_path)
        {
            registry.documents[index] = document;
        } else {
            registry.documents.push(document);
        }
        registry.rebuild_indexes();
        Ok(registry)
    }

    fn rebuild_indexes(&mut self) {
        self.id_index.clear();
        self.anchor_index.clear();
        for (index, document) in self.documents.iter().enumerate() {
            let id = document.id_str();
            self.id_index.insert(id.clone(), index);
            for anchor in document.anchors() {
                self.anchor_index.insert(format!("{id}#{anchor}"), index);
            }
        }
    }

    /// Look up a document by its ID string.
    pub fn get_by_id(&self, id: &str) -> Option<&SpecDocument> {
        self.id_index.get(id).map(|&idx| &self.documents[idx])
    }

    /// Look up a document by a qualified anchor string (`"ID#anchor"`).
    pub fn get_by_anchor(&self, key: &str) -> Option<&SpecDocument> {
        self.anchor_index.get(key).map(|&idx| &self.documents[idx])
    }

    /// Resolve a reference string through redirects. Returns the canonical target.
    pub fn resolve_redirect(&self, reference: &str) -> (String, bool) {
        let mut current = reference.to_string();
        let mut traversed = false;
        let mut visited = std::collections::HashSet::new();
        visited.insert(current.clone());

        loop {
            let mut found = false;
            for redir in &self.redirects {
                if redir.from == current {
                    current = redir.to.clone();
                    traversed = true;
                    found = true;
                    if !visited.insert(current.clone()) {
                        // Cycle detected — stop
                        return (current, traversed);
                    }
                    break;
                }
            }
            if !found {
                break;
            }
        }
        (current, traversed)
    }

    /// Check if a reference (spec ID or qualified anchor) exists.
    /// Returns `(exists, traversed_redirect)`.
    pub fn reference_exists(&self, reference: &str) -> (bool, bool) {
        let (resolved, traversed) = self.resolve_redirect(reference);

        // Check if it's a direct doc ID
        if self.id_index.contains_key(&resolved) {
            return (true, traversed);
        }
        // Check if it's a qualified anchor
        if self.anchor_index.contains_key(&resolved) {
            return (true, traversed);
        }

        (false, traversed)
    }
}
