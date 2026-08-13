//! Deterministic, read-only projection of a specification tree.
//!
//! This module deliberately owns no temporal storage or workspace mutation.
//! Callers supply an optional byte overlay and receive a canonical semantic
//! state that is safe to serialize, hash, diff, and retain.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::documentation::{
    discover_documentation, matching_collections, parse_documentation, DocumentationIndex,
    DocumentationIssue, DocumentationLinkTarget,
};
use crate::lint::diagnostic::{Diagnostic, Severity};
use crate::model::config::{
    DocumentationCollectionConfig, SpecConfig, CURRENT_SPEC_BASELINE, DEFAULT_INTELLECT_PROVIDER,
};
use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::reference::{SourceTarget, SpecReference};
use crate::model::registry::{Redirect, SpecRegistry};

pub const SPEC_STATE_SCHEMA_VERSION: &str = "forge-spec-state-v3";
pub const SPEC_DELTA_SCHEMA_VERSION: &str = "forge-spec-delta-v3";
pub const SPEC_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Repository-relative changes layered over the saved `.specs/` tree.
pub type Overlay = BTreeMap<PathBuf, OverlayEntry>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayEntry {
    Upsert(Vec<u8>),
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedConfig {
    pub baseline: String,
    pub project: Option<String>,
    pub intellect_provider: String,
    pub documentation: Vec<ProjectedDocumentationCollection>,
    pub declared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedDocumentationCollection {
    pub id: String,
    pub title: String,
    pub root: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedClause {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedBlock {
    pub kind: String,
    pub id: String,
    pub level: Option<String>,
    pub body: String,
    pub clauses: Vec<ProjectedClause>,
}

/// Canonical type-specific fields. Relationship-valued fields also appear as
/// normalized entries in [`SpecState::relationships`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectedAttributes {
    Project,
    Requirement {
        level: String,
        refines: Vec<String>,
        aspects: Vec<String>,
        categorized_under: Vec<String>,
        requirement_kind: Option<String>,
        level_monotonic: bool,
    },
    Invariant {
        enforcement: Vec<String>,
        applies_to: Vec<String>,
    },
    Interface {
        consumed_by: Vec<String>,
        provided_by: Vec<String>,
        stability: String,
    },
    Adr {
        decision_date: String,
        decided_by: Vec<String>,
    },
    Glossary,
    Topic,
    Scenario,
    Task {
        progress: String,
        refines: Vec<String>,
        aspects: Vec<String>,
        assignee: Option<String>,
        eta: Option<String>,
        blocked_by: Vec<String>,
        categorized_under: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedSpecification {
    pub id: String,
    pub entity_type: String,
    pub path: String,
    pub status: String,
    pub summary: Option<String>,
    pub owners: Vec<String>,
    pub pinned_at: Option<String>,
    pub implemented: Option<String>,
    pub related: Vec<String>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
    pub attributes: ProjectedAttributes,
    pub blocks: Vec<ProjectedBlock>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedDocumentationHeading {
    pub title: String,
    pub segments: Vec<String>,
    pub level: u8,
    pub line: usize,
    pub end_line: usize,
    pub fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedDocumentation {
    pub collection_id: String,
    pub path: String,
    pub title: String,
    pub summary: Option<String>,
    pub headings: Vec<ProjectedDocumentationHeading>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationLinkKind {
    Specification,
    Documentation,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedDocumentationLink {
    pub source_kind: String,
    pub source: String,
    pub target_kind: DocumentationLinkKind,
    pub target: String,
    pub label: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    ProjectContainment,
    Categorization,
    Refinement,
    Reference,
    Related,
    Supersedes,
    SupersededBy,
    TaskBlockedBy,
    InvariantEnforcement,
    InvariantAppliesTo,
    InterfaceConsumedBy,
    InterfaceProvidedBy,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedRelationship {
    pub kind: RelationshipKind,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aspects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectedSourceSelector {
    File,
    Lines { start: u32, end: u32 },
    Symbol { segments: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedSourceReference {
    /// Content-derived identity, stable across source-document reordering.
    pub id: String,
    /// The specification that owns the explicit source reference.
    pub source: String,
    pub path: String,
    pub selector: ProjectedSourceSelector,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedRedirect {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectedSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedDiagnostic {
    pub code: String,
    pub severity: ProjectedSeverity,
    pub message: String,
    pub path: String,
    pub line: Option<usize>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecState {
    pub schema_version: String,
    pub valid: bool,
    pub config: ProjectedConfig,
    pub specifications: Vec<ProjectedSpecification>,
    pub documentation: Vec<ProjectedDocumentation>,
    pub documentation_links: Vec<ProjectedDocumentationLink>,
    pub redirects: Vec<ProjectedRedirect>,
    pub relationships: Vec<ProjectedRelationship>,
    pub source_references: Vec<ProjectedSourceReference>,
    pub diagnostics: Vec<ProjectedDiagnostic>,
}

impl SpecState {
    /// Serialize the canonical state without host-dependent whitespace.
    pub fn canonical_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).context("serializing canonical specification state")
    }

    pub fn diff(&self, newer: &Self) -> SpecDelta {
        SpecDelta::between(self, newer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedSpecification {
    pub id: String,
    pub path: String,
    pub before: ProjectedSpecification,
    pub after: ProjectedSpecification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedDocumentation {
    pub path: String,
    pub before: ProjectedDocumentation,
    pub after: ProjectedDocumentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigChange {
    pub before: ProjectedConfig,
    pub after: ProjectedConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecDelta {
    pub schema_version: String,
    pub from_state_schema: String,
    pub to_state_schema: String,
    pub validity_changed: bool,
    pub config: Option<ConfigChange>,
    pub added_specifications: Vec<ProjectedSpecification>,
    pub removed_specifications: Vec<ProjectedSpecification>,
    pub changed_specifications: Vec<ChangedSpecification>,
    pub added_documentation: Vec<ProjectedDocumentation>,
    pub removed_documentation: Vec<ProjectedDocumentation>,
    pub changed_documentation: Vec<ChangedDocumentation>,
    pub added_redirects: Vec<ProjectedRedirect>,
    pub removed_redirects: Vec<ProjectedRedirect>,
    pub added_relationships: Vec<ProjectedRelationship>,
    pub removed_relationships: Vec<ProjectedRelationship>,
    pub added_source_references: Vec<ProjectedSourceReference>,
    pub removed_source_references: Vec<ProjectedSourceReference>,
    pub added_documentation_links: Vec<ProjectedDocumentationLink>,
    pub removed_documentation_links: Vec<ProjectedDocumentationLink>,
    pub added_diagnostics: Vec<ProjectedDiagnostic>,
    pub removed_diagnostics: Vec<ProjectedDiagnostic>,
}

impl SpecDelta {
    fn between(before: &SpecState, after: &SpecState) -> Self {
        let before_specs = before
            .specifications
            .iter()
            .map(|spec| ((spec.id.clone(), spec.path.clone()), spec))
            .collect::<BTreeMap<_, _>>();
        let after_specs = after
            .specifications
            .iter()
            .map(|spec| ((spec.id.clone(), spec.path.clone()), spec))
            .collect::<BTreeMap<_, _>>();

        let added_specifications = after_specs
            .iter()
            .filter(|(key, _)| !before_specs.contains_key(*key))
            .map(|(_, value)| (*value).clone())
            .collect();
        let removed_specifications = before_specs
            .iter()
            .filter(|(key, _)| !after_specs.contains_key(*key))
            .map(|(_, value)| (*value).clone())
            .collect();
        let changed_specifications = after_specs
            .iter()
            .filter_map(|((id, path), value)| {
                let previous = before_specs.get(&(id.clone(), path.clone()))?;
                (*previous != *value).then(|| ChangedSpecification {
                    id: id.clone(),
                    path: path.clone(),
                    before: (*previous).clone(),
                    after: (*value).clone(),
                })
            })
            .collect();

        let before_documentation = before
            .documentation
            .iter()
            .map(|document| (document.path.clone(), document))
            .collect::<BTreeMap<_, _>>();
        let after_documentation = after
            .documentation
            .iter()
            .map(|document| (document.path.clone(), document))
            .collect::<BTreeMap<_, _>>();
        let added_documentation = after_documentation
            .iter()
            .filter(|(path, _)| !before_documentation.contains_key(*path))
            .map(|(_, document)| (*document).clone())
            .collect();
        let removed_documentation = before_documentation
            .iter()
            .filter(|(path, _)| !after_documentation.contains_key(*path))
            .map(|(_, document)| (*document).clone())
            .collect();
        let changed_documentation = after_documentation
            .iter()
            .filter_map(|(path, document)| {
                let before = before_documentation.get(path)?;
                (*before != *document).then(|| ChangedDocumentation {
                    path: path.clone(),
                    before: (*before).clone(),
                    after: (*document).clone(),
                })
            })
            .collect();

        Self {
            schema_version: SPEC_DELTA_SCHEMA_VERSION.into(),
            from_state_schema: before.schema_version.clone(),
            to_state_schema: after.schema_version.clone(),
            validity_changed: before.valid != after.valid,
            config: (before.config != after.config).then(|| ConfigChange {
                before: before.config.clone(),
                after: after.config.clone(),
            }),
            added_specifications,
            removed_specifications,
            changed_specifications,
            added_documentation,
            removed_documentation,
            changed_documentation,
            added_redirects: set_difference(&before.redirects, &after.redirects),
            removed_redirects: set_difference(&after.redirects, &before.redirects),
            added_relationships: set_difference(&before.relationships, &after.relationships),
            removed_relationships: set_difference(&after.relationships, &before.relationships),
            added_source_references: set_difference(
                &before.source_references,
                &after.source_references,
            ),
            removed_source_references: set_difference(
                &after.source_references,
                &before.source_references,
            ),
            added_documentation_links: set_difference(
                &before.documentation_links,
                &after.documentation_links,
            ),
            removed_documentation_links: set_difference(
                &after.documentation_links,
                &before.documentation_links,
            ),
            added_diagnostics: set_difference(&before.diagnostics, &after.diagnostics),
            removed_diagnostics: set_difference(&after.diagnostics, &before.diagnostics),
        }
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).context("serializing canonical specification delta")
    }
}

/// Project the saved specification tree, configured documentation, and an
/// in-memory repository-relative overlay.
///
/// Overlay keys may be relative to the supplied `.specs/` directory or may
/// include its repository-relative `.specs/` prefix. Absolute paths and paths
/// containing `..` are rejected before any input is read.
pub fn project(specs_dir: &Path, overlay: &Overlay) -> Result<SpecState> {
    let repository_root = specs_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| specs_dir.to_path_buf());
    let mut normalized_overlay = BTreeMap::new();
    for (path, entry) in overlay {
        let relative = normalize_overlay_path(specs_dir, path)?;
        if normalized_overlay.insert(relative, entry.clone()).is_some() {
            bail!("multiple overlay entries resolve to the same repository input");
        }
    }

    let mut files = load_saved_inputs(specs_dir)?;
    for (path, entry) in &normalized_overlay {
        if !is_specification_input(path) {
            continue;
        }
        match entry {
            OverlayEntry::Upsert(content) => {
                files.insert(path.clone(), content.clone());
            }
            OverlayEntry::Delete => {
                files.remove(path);
            }
        }
    }

    let mut discovery_diagnostics = Vec::new();
    let config = parse_config(
        files.get(Path::new(".specs/_config.toml")),
        &mut discovery_diagnostics,
    );
    let (discovered, issues) = discover_documentation(&repository_root, &config.documentation)?;
    for entry in discovered {
        let bytes = std::fs::read(&entry.source_path)
            .with_context(|| format!("reading documentation {}", entry.source_path.display()))?;
        files.insert(PathBuf::from(entry.repository_path), bytes);
    }
    for (path, entry) in normalized_overlay {
        match entry {
            OverlayEntry::Upsert(content) => {
                files.insert(path, content);
            }
            OverlayEntry::Delete => {
                files.remove(&path);
            }
        }
    }

    project_files(files, issues, discovery_diagnostics)
}

fn project_files(
    files: BTreeMap<PathBuf, Vec<u8>>,
    documentation_issues: Vec<DocumentationIssue>,
    mut raw_diagnostics: Vec<Diagnostic>,
) -> Result<SpecState> {
    let config = parse_config(
        files.get(Path::new(".specs/_config.toml")),
        &mut raw_diagnostics,
    );
    let redirects = parse_redirects(
        files.get(Path::new(".specs/_redirects.toml")),
        &mut raw_diagnostics,
    );
    raw_diagnostics.extend(documentation_issues.into_iter().map(|issue| {
        let mut diagnostic = projection_diagnostic(&issue.code, issue.message, &issue.file);
        if let Some(line) = issue.line {
            diagnostic = diagnostic.at_line(line);
        }
        diagnostic
    }));

    let mut documents = Vec::new();
    let mut documentation_documents = Vec::new();
    for (path, bytes) in &files {
        if !is_spec_repository_path(path) {
            if path == Path::new(".specs/_config.toml")
                || path == Path::new(".specs/_redirects.toml")
            {
                continue;
            }
            let path_text = path_string(path);
            let collections = matching_collections(&path_text, &config.documentation)?;
            if collections.len() != 1 {
                raw_diagnostics.push(projection_diagnostic(
                    "P004",
                    if collections.is_empty() {
                        "unsupported projection input; Markdown is not enrolled by a documentation collection"
                    } else {
                        "documentation input belongs to overlapping collections"
                    },
                    path,
                ));
                continue;
            }
            let Ok(content) = std::str::from_utf8(bytes) else {
                raw_diagnostics.push(projection_diagnostic(
                    "P002",
                    "documentation input is not valid UTF-8",
                    path,
                ));
                continue;
            };
            documentation_documents.push(parse_documentation(
                path,
                &path_text,
                &collections[0],
                content,
            ));
            continue;
        }
        let Ok(content) = std::str::from_utf8(bytes) else {
            raw_diagnostics.push(projection_diagnostic(
                "P002",
                "specification input is not valid UTF-8",
                path,
            ));
            continue;
        };
        match crate::parse::parse_content(path, content) {
            Ok(document) => documents.push(document),
            Err(error) => raw_diagnostics.push(projection_diagnostic(
                "P003",
                format!("invalid specification: {error:#}"),
                path,
            )),
        }
    }

    documents.sort_by(|a, b| {
        a.id_str()
            .cmp(&b.id_str())
            .then_with(|| a.source_path.cmp(&b.source_path))
    });
    let documentation =
        DocumentationIndex::from_documents(Path::new(""), documentation_documents, Vec::new());
    let registry = build_registry(documents, config.clone(), redirects.clone(), documentation);
    raw_diagnostics.extend(projection_lint(&registry));

    let specifications = registry
        .documents
        .iter()
        .map(project_specification)
        .collect::<Vec<_>>();
    let (relationships, source_references, mut source_diagnostics) =
        project_relationships(&registry);
    raw_diagnostics.append(&mut source_diagnostics);
    let documentation = registry
        .documentation
        .documents
        .iter()
        .map(project_documentation)
        .collect();
    let documentation_links = project_documentation_links(&registry);

    let mut diagnostics = raw_diagnostics
        .into_iter()
        .map(project_diagnostic)
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.severity.cmp(&right.severity))
            .then_with(|| left.message.cmp(&right.message))
            .then_with(|| left.detail.cmp(&right.detail))
    });
    diagnostics.dedup();
    let valid = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != ProjectedSeverity::Error);

    Ok(SpecState {
        schema_version: SPEC_STATE_SCHEMA_VERSION.into(),
        valid,
        config: ProjectedConfig {
            baseline: config.baseline,
            project: config.project,
            intellect_provider: config.intellect_provider,
            documentation: project_documentation_collections(&config.documentation),
            declared: config.declared,
        },
        specifications,
        documentation,
        documentation_links,
        redirects: redirects
            .into_iter()
            .map(|redirect| ProjectedRedirect {
                from: redirect.from,
                to: redirect.to,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        relationships,
        source_references,
        diagnostics,
    })
}

fn load_saved_inputs(specs_dir: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    if !specs_dir.is_dir() {
        bail!(
            "specification root is not a directory: {}",
            specs_dir.display()
        );
    }
    for entry in WalkDir::new(specs_dir).follow_links(false) {
        let entry = entry.with_context(|| format!("walking {}", specs_dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(specs_dir)
            .context("resolving specification path relative to root")?
            .to_path_buf();
        if !is_supported_spec_relative_path(&relative) {
            continue;
        }
        let bytes = std::fs::read(entry.path())
            .with_context(|| format!("reading specification input {}", relative.display()))?;
        files.insert(Path::new(".specs").join(relative), bytes);
    }
    Ok(files)
}

fn normalize_overlay_path(_specs_dir: &Path, path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        bail!("overlay paths must be repository-relative");
    }
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => components.push(value.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("overlay path escapes the specification root")
            }
        }
    }
    if components.is_empty() {
        bail!("overlay path must name an input file");
    }

    let normalized: PathBuf = components.into_iter().collect();
    if normalized.starts_with(".specs") {
        return Ok(normalized);
    }
    if is_supported_spec_relative_path(&normalized) {
        return Ok(Path::new(".specs").join(normalized));
    }
    Ok(normalized)
}

fn is_supported_spec_relative_path(path: &Path) -> bool {
    is_spec_file(path) || path == Path::new("_config.toml") || path == Path::new("_redirects.toml")
}

fn is_specification_input(path: &Path) -> bool {
    path.strip_prefix(".specs")
        .is_ok_and(is_supported_spec_relative_path)
}

fn is_spec_repository_path(path: &Path) -> bool {
    path.strip_prefix(".specs").is_ok_and(is_spec_file)
}

fn is_spec_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".spec.md"))
}

fn parse_config(bytes: Option<&Vec<u8>>, diagnostics: &mut Vec<Diagnostic>) -> SpecConfig {
    let Some(bytes) = bytes else {
        return SpecConfig {
            baseline: CURRENT_SPEC_BASELINE.into(),
            project: None,
            intellect_provider: DEFAULT_INTELLECT_PROVIDER.into(),
            documentation: Vec::new(),
            declared: false,
        };
    };
    let Ok(content) = std::str::from_utf8(bytes) else {
        diagnostics.push(projection_diagnostic(
            "P002",
            "configuration input is not valid UTF-8",
            Path::new("_config.toml"),
        ));
        return invalid_config();
    };
    match SpecConfig::from_toml(content) {
        Ok(config) => config,
        Err(error) => {
            diagnostics.push(projection_diagnostic(
                "P003",
                format!("invalid configuration: {error:#}"),
                Path::new("_config.toml"),
            ));
            invalid_config()
        }
    }
}

fn invalid_config() -> SpecConfig {
    SpecConfig {
        baseline: String::new(),
        project: None,
        intellect_provider: DEFAULT_INTELLECT_PROVIDER.into(),
        documentation: Vec::new(),
        declared: true,
    }
}

#[derive(Deserialize)]
struct RawRedirects {
    redirect: Option<Vec<RawRedirect>>,
}

#[derive(Deserialize)]
struct RawRedirect {
    from: String,
    to: String,
}

fn parse_redirects(bytes: Option<&Vec<u8>>, diagnostics: &mut Vec<Diagnostic>) -> Vec<Redirect> {
    let Some(bytes) = bytes else {
        return Vec::new();
    };
    let Ok(content) = std::str::from_utf8(bytes) else {
        diagnostics.push(projection_diagnostic(
            "P002",
            "redirect input is not valid UTF-8",
            Path::new("_redirects.toml"),
        ));
        return Vec::new();
    };
    match toml::from_str::<RawRedirects>(content) {
        Ok(file) => file
            .redirect
            .unwrap_or_default()
            .into_iter()
            .map(|redirect| Redirect {
                from: redirect.from,
                to: redirect.to,
            })
            .collect(),
        Err(error) => {
            diagnostics.push(projection_diagnostic(
                "P003",
                format!("invalid redirects: {error}"),
                Path::new("_redirects.toml"),
            ));
            Vec::new()
        }
    }
}

fn build_registry(
    documents: Vec<SpecDocument>,
    config: SpecConfig,
    redirects: Vec<Redirect>,
    mut documentation: DocumentationIndex,
) -> SpecRegistry {
    let mut id_index = BTreeMap::new();
    let mut anchor_index = BTreeMap::new();
    for (index, document) in documents.iter().enumerate() {
        let id = document.id_str();
        id_index.insert(id.clone(), index);
        for anchor in document.anchors() {
            anchor_index.insert(format!("{id}#{anchor}"), index);
        }
    }
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
    SpecRegistry {
        documents,
        id_index,
        anchor_index,
        redirects,
        specs_dir: PathBuf::new(),
        config,
        documentation,
    }
}

/// Lint only deterministic specification semantics. Source-provider checks and
/// Git trailer history are intentionally outside this projection contract.
fn projection_lint(registry: &SpecRegistry) -> Vec<Diagnostic> {
    use crate::model::frontmatter::Status;

    let mut diagnostics = Vec::new();
    for document in &registry.documents {
        let is_draft = document.universal.status == Status::Draft;
        let mut document_diagnostics = Vec::new();
        document_diagnostics.extend(crate::lint::structural::check_id_pattern(document));
        document_diagnostics.extend(crate::lint::structural::check_type_matches_prefix(document));
        document_diagnostics.extend(crate::lint::structural::check_universal_fields(document));
        document_diagnostics.extend(crate::lint::structural::check_type_specific_fields(
            document,
        ));
        document_diagnostics.extend(crate::lint::structural::check_unique_anchors(document));
        document_diagnostics.extend(crate::lint::content::check_multi_entity(document, 10));
        document_diagnostics.extend(crate::lint::content::check_rfc2119_discipline(document));
        if is_draft {
            for diagnostic in &mut document_diagnostics {
                let number = diagnostic
                    .code
                    .strip_prefix('R')
                    .and_then(|value| value.parse::<u32>().ok());
                if number.is_some_and(|number| (2..=12).contains(&number)) {
                    diagnostic.downgrade();
                }
            }
        }
        diagnostics.extend(document_diagnostics);
    }
    diagnostics.extend(crate::lint::structural::check_spec_config(registry));
    diagnostics.extend(crate::lint::structural::check_project_root(registry));
    diagnostics.extend(crate::lint::structural::check_unique_ids(registry));
    diagnostics.extend(crate::lint::references::check_references(registry));
    diagnostics.extend(crate::lint::references::check_documentation_references(
        registry,
    ));
    diagnostics.extend(crate::lint::references::check_summary_on_referenced(
        registry,
    ));
    diagnostics.extend(crate::lint::refinement::check_refinement(registry));
    diagnostics
}

fn project_specification(document: &SpecDocument) -> ProjectedSpecification {
    let attributes = match &document.type_fields {
        TypeSpecificFields::Project => ProjectedAttributes::Project,
        TypeSpecificFields::Requirement {
            level,
            refines,
            aspects,
            categorized_under,
            kind,
            level_monotonic,
        } => ProjectedAttributes::Requirement {
            level: level.as_str().into(),
            // These arrays are positionally paired by the format.
            refines: refines.clone(),
            aspects: aspects.clone(),
            categorized_under: sorted(categorized_under),
            requirement_kind: kind.clone(),
            level_monotonic: *level_monotonic,
        },
        TypeSpecificFields::Invariant {
            enforcement,
            applies_to,
        } => ProjectedAttributes::Invariant {
            enforcement: sorted(enforcement),
            applies_to: sorted(applies_to),
        },
        TypeSpecificFields::Interface {
            consumed_by,
            provided_by,
            stability,
        } => ProjectedAttributes::Interface {
            consumed_by: sorted(consumed_by),
            provided_by: sorted(provided_by),
            stability: format!("{stability:?}").to_ascii_lowercase(),
        },
        TypeSpecificFields::Adr {
            decision_date,
            decided_by,
        } => ProjectedAttributes::Adr {
            decision_date: decision_date.clone(),
            decided_by: sorted(decided_by),
        },
        TypeSpecificFields::Glossary => ProjectedAttributes::Glossary,
        TypeSpecificFields::Topic => ProjectedAttributes::Topic,
        TypeSpecificFields::Scenario => ProjectedAttributes::Scenario,
        TypeSpecificFields::Task {
            progress,
            refines,
            aspects,
            assignee,
            eta,
            blocked_by,
            categorized_under,
        } => ProjectedAttributes::Task {
            progress: progress.as_str().into(),
            // These arrays are positionally paired by the format.
            refines: refines.clone(),
            aspects: aspects.clone(),
            assignee: assignee.clone(),
            eta: eta.clone(),
            blocked_by: sorted(blocked_by),
            categorized_under: sorted(categorized_under),
        },
    };
    let mut blocks = document
        .blocks
        .iter()
        .map(|block| {
            let mut clauses = block
                .clauses
                .iter()
                .map(|clause| ProjectedClause {
                    id: clause.id.clone(),
                    text: clause.text.clone(),
                })
                .collect::<Vec<_>>();
            clauses.sort();
            ProjectedBlock {
                kind: block.kind.tag().into(),
                id: block.id.clone(),
                level: block.level.clone(),
                body: block.body.clone(),
                clauses,
            }
        })
        .collect::<Vec<_>>();
    blocks.sort();

    ProjectedSpecification {
        id: document.id_str(),
        entity_type: document.universal.entity_type.type_name().into(),
        path: path_string(&document.source_path),
        status: document.universal.status.as_str().into(),
        summary: document.universal.summary.clone(),
        owners: sorted(&document.universal.owners),
        pinned_at: document.universal.pinned_at.clone(),
        implemented: document.universal.implemented.clone(),
        related: sorted(&document.universal.related),
        supersedes: document.universal.supersedes.clone(),
        superseded_by: document.universal.superseded_by.clone(),
        attributes,
        blocks,
        body: document.body_raw.clone(),
    }
}

fn project_documentation_collections(
    collections: &[DocumentationCollectionConfig],
) -> Vec<ProjectedDocumentationCollection> {
    let mut projected = collections
        .iter()
        .map(|collection| ProjectedDocumentationCollection {
            id: collection.id.clone(),
            title: collection.title.clone(),
            root: collection.root.clone(),
            include: collection.include.clone(),
            exclude: collection.exclude.clone(),
        })
        .collect::<Vec<_>>();
    projected.sort();
    projected
}

fn project_documentation(
    document: &crate::documentation::DocumentationDocument,
) -> ProjectedDocumentation {
    ProjectedDocumentation {
        collection_id: document.collection_id.clone(),
        path: document.path.clone(),
        title: document.title.clone(),
        summary: document.summary.clone(),
        headings: document
            .headings
            .iter()
            .map(|heading| ProjectedDocumentationHeading {
                title: heading.title.clone(),
                segments: heading.segments.clone(),
                level: heading.level,
                line: heading.line,
                end_line: heading.end_line,
                fragment: heading.fragment.clone(),
            })
            .collect(),
        body: document.body.clone(),
    }
}

fn project_documentation_links(registry: &SpecRegistry) -> Vec<ProjectedDocumentationLink> {
    let mut links = BTreeSet::new();
    for specification in &registry.documents {
        for located in &specification.references {
            if let SpecReference::Documentation(target) = &located.reference {
                links.insert(ProjectedDocumentationLink {
                    source_kind: "specification".to_string(),
                    source: specification.id_str(),
                    target_kind: DocumentationLinkKind::Documentation,
                    target: target.to_string(),
                    label: located.link_text.clone(),
                    line: located.line,
                });
            }
        }
    }
    for document in &registry.documentation.documents {
        for link in &document.links {
            let Some(target) = registry.documentation.canonical_target(link) else {
                continue;
            };
            let target_kind = match &link.target {
                DocumentationLinkTarget::Forge(SpecReference::Spec(_)) => {
                    DocumentationLinkKind::Specification
                }
                DocumentationLinkTarget::Forge(SpecReference::Source(_)) => {
                    DocumentationLinkKind::Source
                }
                DocumentationLinkTarget::Forge(SpecReference::Documentation(_))
                | DocumentationLinkTarget::Markdown { .. } => DocumentationLinkKind::Documentation,
            };
            links.insert(ProjectedDocumentationLink {
                source_kind: "documentation".to_string(),
                source: document.path.clone(),
                target_kind,
                target,
                label: link.label.clone(),
                line: link.line,
            });
        }
    }
    links.into_iter().collect()
}

fn project_relationships(
    registry: &SpecRegistry,
) -> (
    Vec<ProjectedRelationship>,
    Vec<ProjectedSourceReference>,
    Vec<Diagnostic>,
) {
    let mut relationships = BTreeSet::new();
    let mut source_references = BTreeSet::new();
    let mut diagnostics = Vec::new();
    let project_id = registry.project_id();

    for document in &registry.documents {
        let source = document.id_str();
        let (refines, aspects, categorized_under): (&[String], &[String], &[String]) =
            match &document.type_fields {
                TypeSpecificFields::Requirement {
                    refines,
                    aspects,
                    categorized_under,
                    ..
                }
                | TypeSpecificFields::Task {
                    refines,
                    aspects,
                    categorized_under,
                    ..
                } => (refines, aspects, categorized_under),
                _ => (&[], &[], &[]),
            };
        for (index, target) in refines.iter().enumerate() {
            relationships.insert(relationship(
                RelationshipKind::Refinement,
                &source,
                target,
                aspects.get(index).cloned().into_iter().collect(),
            ));
        }
        for target in categorized_under {
            relationships.insert(relationship(
                RelationshipKind::Categorization,
                &source,
                target,
                Vec::new(),
            ));
        }
        if refines.is_empty() && categorized_under.is_empty() {
            if let Some(project) = &project_id {
                if source != *project {
                    relationships.insert(relationship(
                        RelationshipKind::ProjectContainment,
                        &source,
                        project,
                        Vec::new(),
                    ));
                }
            }
        }

        for target in &document.universal.related {
            relationships.insert(relationship(
                RelationshipKind::Related,
                &source,
                target,
                Vec::new(),
            ));
        }
        if let Some(target) = &document.universal.supersedes {
            relationships.insert(relationship(
                RelationshipKind::Supersedes,
                &source,
                target,
                Vec::new(),
            ));
        }
        if let Some(target) = &document.universal.superseded_by {
            relationships.insert(relationship(
                RelationshipKind::SupersededBy,
                &source,
                target,
                Vec::new(),
            ));
        }

        match &document.type_fields {
            TypeSpecificFields::Task { blocked_by, .. } => {
                for target in blocked_by {
                    relationships.insert(relationship(
                        RelationshipKind::TaskBlockedBy,
                        &source,
                        target,
                        Vec::new(),
                    ));
                }
            }
            TypeSpecificFields::Invariant {
                enforcement,
                applies_to,
            } => {
                for target in enforcement {
                    relationships.insert(relationship(
                        RelationshipKind::InvariantEnforcement,
                        &source,
                        target,
                        Vec::new(),
                    ));
                }
                for target in applies_to {
                    relationships.insert(relationship(
                        RelationshipKind::InvariantAppliesTo,
                        &source,
                        target,
                        Vec::new(),
                    ));
                }
            }
            TypeSpecificFields::Interface {
                consumed_by,
                provided_by,
                ..
            } => {
                for target in consumed_by {
                    relationships.insert(relationship(
                        RelationshipKind::InterfaceConsumedBy,
                        &source,
                        target,
                        Vec::new(),
                    ));
                }
                for target in provided_by {
                    relationships.insert(relationship(
                        RelationshipKind::InterfaceProvidedBy,
                        &source,
                        target,
                        Vec::new(),
                    ));
                }
            }
            _ => {}
        }

        for located in &document.references {
            match &located.reference {
                SpecReference::Spec(target) => {
                    relationships.insert(relationship(
                        RelationshipKind::Reference,
                        &source,
                        &target.to_string(),
                        Vec::new(),
                    ));
                }
                SpecReference::Source(source_reference) => {
                    let path = Path::new(&source_reference.path);
                    if source_reference.path.is_empty()
                        || path.is_absolute()
                        || path
                            .components()
                            .any(|component| matches!(component, Component::ParentDir))
                    {
                        diagnostics.push(
                            projection_diagnostic(
                                "P005",
                                "source reference path must be repository-relative and may not escape the repository",
                                &document.source_path,
                            )
                            .at_line(located.line),
                        );
                        continue;
                    }
                    let normalized_path = path_string(path);
                    let selector = match &source_reference.target {
                        SourceTarget::File => ProjectedSourceSelector::File,
                        SourceTarget::Lines { start, end } => {
                            if *start == 0 || start > end {
                                diagnostics.push(
                                    projection_diagnostic(
                                        "P006",
                                        "source line selector must be an inclusive, one-based range",
                                        &document.source_path,
                                    )
                                    .at_line(located.line),
                                );
                            }
                            ProjectedSourceSelector::Lines {
                                start: *start,
                                end: *end,
                            }
                        }
                        SourceTarget::Symbol { segments } => ProjectedSourceSelector::Symbol {
                            segments: segments.clone(),
                        },
                    };
                    let identity = source_identity(&source, &normalized_path, &selector);
                    source_references.insert(ProjectedSourceReference {
                        id: identity,
                        source: source.clone(),
                        path: normalized_path,
                        selector,
                    });
                }
                SpecReference::Documentation(_) => {}
            }
        }
    }

    (
        relationships.into_iter().collect(),
        source_references.into_iter().collect(),
        diagnostics,
    )
}

fn relationship(
    kind: RelationshipKind,
    source: &str,
    target: &str,
    aspects: Vec<String>,
) -> ProjectedRelationship {
    ProjectedRelationship {
        kind,
        source: source.into(),
        target: target.into(),
        aspects,
    }
}

fn source_identity(source: &str, path: &str, selector: &ProjectedSourceSelector) -> String {
    // JSON preserves segment boundaries (including decoded `/` within a
    // segment), avoiding collisions between distinct hierarchical symbols.
    let selector = serde_json::to_string(selector).expect("source selector is serializable");
    format!("{source}|{path}|{selector}")
}

fn projection_diagnostic(code: &str, message: impl Into<String>, path: &Path) -> Diagnostic {
    Diagnostic::error(code, message, path.to_path_buf())
}

fn project_diagnostic(diagnostic: Diagnostic) -> ProjectedDiagnostic {
    ProjectedDiagnostic {
        code: diagnostic.code,
        severity: match diagnostic.severity {
            Severity::Info => ProjectedSeverity::Info,
            Severity::Warning => ProjectedSeverity::Warning,
            Severity::Error => ProjectedSeverity::Error,
        },
        message: diagnostic.message,
        path: path_string(&diagnostic.file),
        line: diagnostic.line,
        detail: diagnostic.detail,
    }
}

fn path_string(path: &Path) -> String {
    let rendered = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if rendered.is_empty() {
        ".".into()
    } else {
        rendered
    }
}

fn sorted(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Elements present in `newer` but absent from `older`.
fn set_difference<T: Clone + Ord>(older: &[T], newer: &[T]) -> Vec<T> {
    let old = older.iter().collect::<BTreeSet<_>>();
    newer
        .iter()
        .filter(|value| !old.contains(value))
        .cloned()
        .collect()
}
