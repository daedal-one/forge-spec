use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use super::config::SpecConfig;
use super::document::SpecDocument;
use super::id::EntityType;
use crate::documentation::DocumentationIndex;
use crate::parse;

/// A redirect entry from `_redirects.toml`.
#[derive(Debug, Clone)]
pub struct Redirect {
    pub from: String,
    pub to: String,
}

/// The central registry holding all parsed spec documents and indexes.
#[derive(Debug, Clone)]
pub struct SpecRegistry {
    pub documents: Vec<SpecDocument>,
    /// Map from spec ID string to document index.
    pub id_index: BTreeMap<String, usize>,
    /// Map from `"id#anchor"` to document index.
    pub anchor_index: BTreeMap<String, usize>,
    pub redirects: Vec<Redirect>,
    pub specs_dir: PathBuf,
    pub config: SpecConfig,
    pub documentation: DocumentationIndex,
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
                        .is_some_and(|n| n.ends_with(".spec.md"))
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

        Self::from_documents_with_config(specs_dir, documents, config)
    }

    /// Build a registry from documents already loaded by an incremental index.
    pub fn from_documents(specs_dir: &Path, documents: Vec<SpecDocument>) -> Result<Self> {
        let config = SpecConfig::load(specs_dir)?;
        Self::from_documents_with_config(specs_dir, documents, config)
    }

    pub(crate) fn from_documents_with_config(
        specs_dir: &Path,
        documents: Vec<SpecDocument>,
        config: SpecConfig,
    ) -> Result<Self> {
        let documentation = DocumentationIndex::load(specs_dir, &config)?;
        Self::from_documents_with_config_and_documentation(
            specs_dir,
            documents,
            config,
            documentation,
        )
    }

    pub(crate) fn from_documents_with_config_and_documentation(
        specs_dir: &Path,
        documents: Vec<SpecDocument>,
        config: SpecConfig,
        mut documentation: DocumentationIndex,
    ) -> Result<Self> {
        let redirects_path = specs_dir.join("_redirects.toml");
        let redirects = if redirects_path.exists() {
            parse::redirects::load_redirects(&redirects_path)
                .with_context(|| format!("loading {}", redirects_path.display()))?
        } else {
            Vec::new()
        };

        for document in &documents {
            let source = document.id_str();
            for located in &document.references {
                documentation.add_specification_backlink(
                    &located.reference,
                    &source,
                    &located.link_text,
                    located.line,
                );
            }
        }

        let mut registry = Self {
            documents,
            id_index: BTreeMap::new(),
            anchor_index: BTreeMap::new(),
            redirects,
            specs_dir: specs_dir.to_path_buf(),
            config,
            documentation,
        };
        registry.rebuild_indexes();
        Ok(registry)
    }

    /// Load the registry while replacing one document with unsaved editor
    /// content. Redirects and all indexes are rebuilt consistently.
    pub fn load_with_override(specs_dir: &Path, source_path: &Path, content: &str) -> Result<Self> {
        Self::load(specs_dir)?.with_override(source_path, content)
    }

    /// Clone this registry and overlay one unsaved editor document.
    pub fn with_override(&self, source_path: &Path, content: &str) -> Result<Self> {
        let mut registry = self.clone();
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
        registry.documentation = DocumentationIndex::load(&registry.specs_dir, &registry.config)?;
        for document in &registry.documents {
            let source = document.id_str();
            for located in &document.references {
                registry.documentation.add_specification_backlink(
                    &located.reference,
                    &source,
                    &located.link_text,
                    located.line,
                );
            }
        }
        Ok(registry)
    }

    /// Overlay one enrolled Markdown document for unsaved editor navigation.
    pub fn documentation_with_override(
        &self,
        source_path: &Path,
        content: &str,
    ) -> Result<DocumentationIndex> {
        self.documentation.with_override(source_path, content)
    }

    /// Clone the registry and overlay one enrolled Markdown document.
    pub fn with_documentation_override(&self, source_path: &Path, content: &str) -> Result<Self> {
        let mut registry = self.clone();
        registry.documentation = self.documentation.with_override(source_path, content)?;
        for document in &registry.documents {
            let source = document.id_str();
            for located in &document.references {
                registry.documentation.add_specification_backlink(
                    &located.reference,
                    &source,
                    &located.link_text,
                    located.line,
                );
            }
        }
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

    /// Return the singleton project document selected by `_config.toml`.
    pub fn project(&self) -> Option<&SpecDocument> {
        self.config
            .project
            .as_deref()
            .and_then(|id| self.get_by_id(id))
            .filter(|document| document.universal.entity_type == EntityType::Project)
    }

    /// Return the configured project ID when it resolves to a PROJECT document.
    pub fn project_id(&self) -> Option<String> {
        self.project().map(SpecDocument::id_str)
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
