//! Persistent, incremental workspace index shared by editor integrations.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use walkdir::WalkDir;

use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::reference::{SourceTarget, SpecReference};
use crate::model::registry::SpecRegistry;

const CACHE_SCHEMA_VERSION: &str = "2";

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub parsed: usize,
    pub loaded_from_cache: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerSnapshot {
    pub generation: u64,
    pub stats: IndexStats,
    pub documents: Vec<ExplorerDocument>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerDocument {
    pub id: String,
    pub entity_type: String,
    pub status: String,
    pub progress: Option<String>,
    pub level: Option<String>,
    pub summary: Option<String>,
    pub owners: Vec<String>,
    pub uri: String,
    pub refines: Vec<String>,
    pub categorized_under: Vec<String>,
    pub blocks: Vec<ExplorerBlock>,
    pub sources: Vec<ExplorerSource>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerBlock {
    pub id: String,
    pub kind: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerSource {
    pub reference: String,
    pub label: String,
    pub path: String,
    pub line: usize,
    pub target_kind: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileFingerprint {
    modified_ns: i64,
    size: i64,
}

/// Long-lived saved-file index. Unsaved buffers are overlaid by the LSP and
/// never written to this cache.
pub struct WorkspaceIndex {
    specs_dir: PathBuf,
    repository_root: PathBuf,
    connection: Connection,
    documents: BTreeMap<PathBuf, SpecDocument>,
    registry: SpecRegistry,
    generation: u64,
    stats: IndexStats,
}

impl WorkspaceIndex {
    pub fn open(specs_dir: &Path, cache_path: Option<&Path>) -> Result<Self> {
        let specs_dir = canonical_or_absolute(specs_dir)?;
        let repository_root = specs_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| specs_dir.clone());
        let connection = match cache_path {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("creating {}", parent.display()))?;
                }
                Connection::open(path)
                    .with_context(|| format!("opening workspace cache {}", path.display()))?
            }
            None => Connection::open_in_memory().context("opening in-memory workspace cache")?,
        };

        let empty_registry = SpecRegistry::from_documents(&specs_dir, Vec::new())?;
        let mut index = Self {
            specs_dir,
            repository_root,
            connection,
            documents: BTreeMap::new(),
            registry: empty_registry,
            generation: 0,
            stats: IndexStats::default(),
        };
        index.initialize_cache()?;
        index.reconcile()?;
        Ok(index)
    }

    pub fn specs_dir(&self) -> &Path {
        &self.specs_dir
    }

    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    pub fn registry(&self) -> &SpecRegistry {
        &self.registry
    }

    pub fn registry_with_override(&self, path: &Path, content: &str) -> Result<SpecRegistry> {
        self.registry.with_override(path, content)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn stats(&self) -> IndexStats {
        self.stats
    }

    pub fn snapshot(&self) -> ExplorerSnapshot {
        let mut documents = self
            .registry
            .documents
            .iter()
            .map(explorer_document)
            .collect::<Vec<_>>();
        documents.sort_by(|left, right| left.id.cmp(&right.id));
        ExplorerSnapshot {
            generation: self.generation,
            stats: self.stats,
            documents,
        }
    }

    /// Reconcile directory membership and file metadata. Unchanged documents
    /// are deserialized from SQLite without reading or parsing their source.
    pub fn reconcile(&mut self) -> Result<IndexStats> {
        let paths = discover_spec_files(&self.specs_dir);
        let seen = paths.iter().cloned().collect::<BTreeSet<_>>();
        let mut stats = IndexStats::default();
        let mut documents = BTreeMap::new();

        for path in paths {
            if let Some((document, cached)) = self.load_document(&path)? {
                if cached {
                    stats.loaded_from_cache += 1;
                } else {
                    stats.parsed += 1;
                }
                documents.insert(path, document);
            }
        }

        let cached_paths = self.cached_paths()?;
        for path in cached_paths.difference(&seen) {
            self.delete_cached(path)?;
            stats.removed += 1;
        }

        self.documents = documents;
        self.rebuild_registry()?;
        self.stats = stats;
        self.generation = self.generation.saturating_add(1);
        Ok(stats)
    }

    /// Apply watcher notifications and rebuild only the in-memory graph. A
    /// redirect change rebuilds registry metadata without parsing unchanged
    /// specification files. A config change invalidates and rebuilds the cache.
    pub fn refresh_paths<I>(&mut self, paths: I) -> Result<IndexStats>
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut stats = IndexStats::default();
        let mut config_changed = false;
        for path in paths {
            let path = normalize_event_path(&self.repository_root, &path);
            let file_name = path.file_name().and_then(|value| value.to_str());
            if file_name == Some("_config.toml") {
                config_changed = true;
                continue;
            }
            if file_name == Some("_redirects.toml") {
                continue;
            }
            if !is_spec_file(&path) || !path.starts_with(&self.specs_dir) {
                continue;
            }

            if path.is_file() {
                if let Some((document, cached)) = self.load_document(&path)? {
                    self.documents.insert(path.clone(), document);
                    if cached {
                        stats.loaded_from_cache += 1;
                    } else {
                        stats.parsed += 1;
                    }
                } else {
                    self.documents.remove(&path);
                }
            } else {
                self.documents.remove(&path);
                self.delete_cached(&path)?;
                stats.removed += 1;
            }
        }
        if config_changed {
            self.initialize_cache()?;
            return self.reconcile();
        }
        self.rebuild_registry()?;
        self.stats = stats;
        self.generation = self.generation.saturating_add(1);
        Ok(stats)
    }

    fn initialize_cache(&mut self) -> Result<()> {
        self.connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS cache_meta (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;

        let baseline = crate::model::config::SpecConfig::load(&self.specs_dir)?.baseline;
        let repository_identity = self.repository_root.to_string_lossy().into_owned();
        let expected = [
            ("schema", CACHE_SCHEMA_VERSION),
            ("parser", env!("CARGO_PKG_VERSION")),
            ("baseline", baseline.as_str()),
            ("repository", repository_identity.as_str()),
        ];
        let invalid = expected.iter().any(|(key, value)| {
            self.meta_value(key)
                .map(|current| current.as_deref() != Some(*value))
                .unwrap_or(true)
        });
        if invalid {
            self.connection.execute_batch(
                "DROP TABLE IF EXISTS spec_files;
                 CREATE TABLE spec_files (
                     path TEXT PRIMARY KEY,
                     modified_ns INTEGER NOT NULL,
                     size INTEGER NOT NULL,
                     content TEXT NOT NULL,
                     document_json TEXT NOT NULL
                 );",
            )?;
        } else {
            self.connection.execute_batch(
                "CREATE TABLE IF NOT EXISTS spec_files (
                     path TEXT PRIMARY KEY,
                     modified_ns INTEGER NOT NULL,
                     size INTEGER NOT NULL,
                     content TEXT NOT NULL,
                     document_json TEXT NOT NULL
                 );",
            )?;
        }
        for (key, value) in expected {
            self.connection.execute(
                "INSERT INTO cache_meta(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        Ok(())
    }

    fn meta_value(&self, key: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT value FROM cache_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    fn load_document(&self, path: &Path) -> Result<Option<(SpecDocument, bool)>> {
        let fingerprint = fingerprint(&std::fs::metadata(path)?)?;
        let path_text = path.to_string_lossy();
        let cached = self
            .connection
            .query_row(
                "SELECT document_json FROM spec_files
                 WHERE path = ?1 AND modified_ns = ?2 AND size = ?3",
                params![
                    path_text.as_ref(),
                    fingerprint.modified_ns,
                    fingerprint.size
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(json) = cached {
            if let Ok(document) = serde_json::from_str::<SpecDocument>(&json) {
                return Ok(Some((document, true)));
            }
        }

        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        match crate::parse::parse_content(path, &content) {
            Ok(document) => {
                let json = serde_json::to_string(&document)?;
                self.connection.execute(
                    "INSERT INTO spec_files(path, modified_ns, size, content, document_json)
                     VALUES(?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(path) DO UPDATE SET
                         modified_ns = excluded.modified_ns,
                         size = excluded.size,
                         content = excluded.content,
                         document_json = excluded.document_json",
                    params![
                        path_text.as_ref(),
                        fingerprint.modified_ns,
                        fingerprint.size,
                        content,
                        json
                    ],
                )?;
                Ok(Some((document, false)))
            }
            Err(error) => {
                eprintln!("{}: {error:#}", path.display());
                self.delete_cached(path)?;
                Ok(None)
            }
        }
    }

    fn cached_paths(&self) -> Result<BTreeSet<PathBuf>> {
        let mut statement = self.connection.prepare("SELECT path FROM spec_files")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut paths = BTreeSet::new();
        for row in rows {
            paths.insert(PathBuf::from(row?));
        }
        Ok(paths)
    }

    fn delete_cached(&self, path: &Path) -> Result<()> {
        self.connection.execute(
            "DELETE FROM spec_files WHERE path = ?1",
            params![path.to_string_lossy().as_ref()],
        )?;
        Ok(())
    }

    fn rebuild_registry(&mut self) -> Result<()> {
        self.registry = SpecRegistry::from_documents(
            &self.specs_dir,
            self.documents.values().cloned().collect(),
        )?;
        Ok(())
    }
}

fn explorer_document(document: &SpecDocument) -> ExplorerDocument {
    let (progress, level, refines, categorized_under) = match &document.type_fields {
        TypeSpecificFields::Requirement {
            level,
            refines,
            categorized_under,
            ..
        } => (
            None,
            Some(level.as_str().to_string()),
            refines.clone(),
            categorized_under.clone(),
        ),
        TypeSpecificFields::Task {
            progress,
            refines,
            categorized_under,
            ..
        } => (
            Some(progress.as_str().to_string()),
            None,
            refines.clone(),
            categorized_under.clone(),
        ),
        _ => (None, None, Vec::new(), Vec::new()),
    };
    let blocks = document
        .blocks
        .iter()
        .flat_map(|block| {
            let block_entry = ExplorerBlock {
                id: block.id.clone(),
                kind: block.kind.tag().to_string(),
                line: block.start_line,
                text: first_nonempty_line(&block.body),
            };
            let clauses = block.clauses.iter().map(|clause| ExplorerBlock {
                id: clause.id.clone(),
                kind: "clause".to_string(),
                line: clause.line,
                text: clause.text.clone(),
            });
            std::iter::once(block_entry)
                .chain(clauses)
                .collect::<Vec<_>>()
        })
        .collect();
    let sources = document
        .references
        .iter()
        .filter_map(|located| {
            let SpecReference::Source(source) = &located.reference else {
                return None;
            };
            let (target_kind, start_line, end_line, symbol) = match &source.target {
                SourceTarget::File => ("file", None, None, None),
                SourceTarget::Lines { start, end } => ("lines", Some(*start), Some(*end), None),
                SourceTarget::Symbol { segments } => {
                    ("symbol", None, None, Some(segments.join("/")))
                }
            };
            Some(ExplorerSource {
                reference: located.reference.to_string(),
                label: located.link_text.clone(),
                path: source.path.clone(),
                line: located.line,
                target_kind: target_kind.to_string(),
                start_line,
                end_line,
                symbol,
            })
        })
        .collect();

    ExplorerDocument {
        id: document.id_str(),
        entity_type: document.universal.entity_type.prefix().to_string(),
        status: document.universal.status.as_str().to_string(),
        progress,
        level,
        summary: document.universal.summary.clone(),
        owners: document.universal.owners.clone(),
        uri: url::Url::from_file_path(&document.source_path)
            .map(|url| url.to_string())
            .unwrap_or_else(|_| document.source_path.to_string_lossy().into_owned()),
        refines,
        categorized_under,
        blocks,
        sources,
    }
}

fn first_nonempty_line(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn discover_spec_files(specs_dir: &Path) -> Vec<PathBuf> {
    let mut paths = WalkDir::new(specs_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file() && is_spec_file(entry.path()))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn is_spec_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".spec.md"))
}

fn fingerprint(metadata: &Metadata) -> Result<FileFingerprint> {
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let modified_ns = i64::try_from(modified.as_nanos()).unwrap_or(i64::MAX);
    let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    Ok(FileFingerprint { modified_ns, size })
}

fn canonical_or_absolute(path: &Path) -> Result<PathBuf> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(_) if path.is_absolute() => Ok(path.to_path_buf()),
        Err(_) => Ok(std::env::current_dir()?.join(path)),
    }
}

fn absolute_from(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn normalize_event_path(root: &Path, path: &Path) -> PathBuf {
    let absolute = absolute_from(root, path);
    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        return canonical;
    }
    let Some(parent) = absolute.parent() else {
        return absolute;
    };
    let Some(name) = absolute.file_name() else {
        return absolute;
    };
    std::fs::canonicalize(parent)
        .map(|canonical| canonical.join(name))
        .unwrap_or(absolute)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_spec(path: &Path, id: &str, summary: &str) {
        std::fs::write(
            path,
            format!(
                "---\nid: {id}\ntype: requirement\nstatus: accepted\nlevel: MUST\nsummary: {summary}\nowners: [dev]\nrefines: []\n---\n\n# Demo\n\n:::{{requirement id=\"works\" level=\"MUST\"}}\nIt MUST work.\n:::\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn persists_parsed_documents_and_reuses_matching_fingerprints() {
        let temp = tempfile::tempdir().unwrap();
        let specs = temp.path().join(".specs");
        std::fs::create_dir_all(&specs).unwrap();
        std::fs::write(
            specs.join("_config.toml"),
            "baseline = \"forge-spec-v0.2.0\"\n",
        )
        .unwrap();
        write_spec(&specs.join("demo.spec.md"), "REQ:demo/one", "First");
        let cache = temp.path().join("cache/index.sqlite3");

        let cold = WorkspaceIndex::open(&specs, Some(&cache)).unwrap();
        assert_eq!(cold.stats().parsed, 1);
        assert_eq!(cold.snapshot().documents[0].id, "REQ:demo/one");
        drop(cold);

        let warm = WorkspaceIndex::open(&specs, Some(&cache)).unwrap();
        assert_eq!(warm.stats().loaded_from_cache, 1);
        assert_eq!(warm.stats().parsed, 0);

        let cached_content: String = warm
            .connection
            .query_row("SELECT content FROM spec_files", [], |row| row.get(0))
            .unwrap();
        assert!(cached_content.contains("REQ:demo/one"));
    }

    #[test]
    fn invalidates_all_documents_when_the_tree_baseline_changes() {
        let temp = tempfile::tempdir().unwrap();
        let specs = temp.path().join(".specs");
        std::fs::create_dir_all(&specs).unwrap();
        let config = specs.join("_config.toml");
        std::fs::write(&config, "baseline = \"forge-spec-v0.2.0\"\n").unwrap();
        write_spec(&specs.join("demo.spec.md"), "REQ:demo/one", "First");
        let mut index = WorkspaceIndex::open(&specs, None).unwrap();

        std::fs::write(&config, "baseline = \"forge-spec-v0.2.1\"\n").unwrap();
        let stats = index.refresh_paths([config]).unwrap();

        assert_eq!(stats.parsed, 1);
        assert_eq!(stats.loaded_from_cache, 0);
    }

    #[test]
    fn refreshes_changed_and_deleted_files_only() {
        let temp = tempfile::tempdir().unwrap();
        let specs = temp.path().join(".specs");
        std::fs::create_dir_all(&specs).unwrap();
        std::fs::write(
            specs.join("_config.toml"),
            "baseline = \"forge-spec-v0.2.0\"\n",
        )
        .unwrap();
        let first = specs.join("first.spec.md");
        let second = specs.join("second.spec.md");
        write_spec(&first, "REQ:demo/first", "First");
        write_spec(&second, "REQ:demo/second", "Second");
        let mut index = WorkspaceIndex::open(&specs, None).unwrap();

        write_spec(&first, "REQ:demo/first", "First changed and longer");
        std::fs::remove_file(&second).unwrap();
        let stats = index
            .refresh_paths([first.clone(), second.clone()])
            .unwrap();

        assert_eq!(stats.parsed, 1);
        assert_eq!(stats.removed, 1);
        assert_eq!(index.snapshot().documents.len(), 1);
        assert_eq!(
            index.snapshot().documents[0].summary.as_deref(),
            Some("First changed and longer")
        );
    }
}
