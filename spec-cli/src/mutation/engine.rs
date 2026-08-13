use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context, Result};
use git2::{ObjectType, Oid};
use serde::Serialize;
use walkdir::WalkDir;

use super::operation::{ChangeRequest, Operation, CHANGE_SCHEMA};
use crate::editable::EditableDocument;
use crate::lint;
use crate::lint::diagnostic::{Diagnostic, Severity};
use crate::model::block::BlockKind;
use crate::model::config::{SpecConfig, DEFAULT_INTELLECT_PROVIDER};
use crate::model::frontmatter::{Level, Progress, Stability, Status, TypeSpecificFields};
use crate::model::id::{EntityType, QualifiedAnchor, SpecId};
use crate::model::registry::SpecRegistry;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct PlannedOperation {
    pub index: usize,
    pub operation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangePlan {
    pub schema: &'static str,
    pub dry_run: bool,
    pub operations: Vec<PlannedOperation>,
    pub files: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MutationOutcome {
    pub plan: ChangePlan,
    pub written: bool,
    pub edits: Vec<MutationTextEdit>,
}

#[derive(Debug, Clone)]
pub struct MutationTextEdit {
    pub destination: PathBuf,
    pub origin: Option<PathBuf>,
    pub new_text: String,
}

#[derive(Debug, Clone)]
pub struct MutationEngine {
    specs_dir: PathBuf,
}

impl MutationEngine {
    pub fn new(specs_dir: &Path) -> Self {
        Self {
            specs_dir: specs_dir.to_path_buf(),
        }
    }

    pub fn execute(&self, request: &ChangeRequest, dry_run: bool) -> Result<MutationOutcome> {
        if request.schema != CHANGE_SCHEMA {
            bail!(
                "unsupported change schema '{}'; expected '{CHANGE_SCHEMA}'",
                request.schema
            );
        }
        let mut candidate = CandidateWorkspace::load(&self.specs_dir)?;
        candidate.check_fingerprints(&request.if_match)?;
        let original_registry = candidate.registry()?;
        let original_errors = error_signatures(&lint::lint_all(&original_registry));

        for operation in &request.operations {
            candidate.apply(operation)?;
        }

        let registry = candidate.registry()?;
        validate_workspace_contracts(&registry)?;
        let diagnostics = lint::lint_all(&registry);
        let introduced = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .filter(|diagnostic| !original_errors.contains(&diagnostic_signature(diagnostic)))
            .collect::<Vec<_>>();
        if !introduced.is_empty() {
            let messages = introduced
                .iter()
                .map(|diagnostic| diagnostic.to_string())
                .collect::<Vec<_>>()
                .join("\n\n");
            bail!("mutation introduces validation errors:\n{messages}");
        }

        let writes = candidate.changed_writes();
        let mut files = writes
            .iter()
            .map(|write| write.destination.display().to_string())
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        let warnings = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .map(|diagnostic| diagnostic.to_string())
            .collect();
        let plan = ChangePlan {
            schema: CHANGE_SCHEMA,
            dry_run,
            operations: request
                .operations
                .iter()
                .enumerate()
                .map(|(index, operation)| PlannedOperation {
                    index: index + 1,
                    operation: operation.name(),
                    spec: operation.primary_spec().map(str::to_string),
                    config: operation
                        .primary_spec()
                        .is_none()
                        .then_some(".specs/_config.toml"),
                })
                .collect(),
            files,
            warnings,
        };
        let edits = writes
            .iter()
            .map(|write| {
                Ok(MutationTextEdit {
                    destination: write.destination.clone(),
                    origin: write.origin.clone(),
                    new_text: String::from_utf8(write.bytes.clone())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        if dry_run || writes.is_empty() {
            return Ok(MutationOutcome {
                plan,
                written: false,
                edits,
            });
        }

        apply_atomic(&writes, || {
            let written = CandidateWorkspace::load(&self.specs_dir)?;
            let registry = written.registry()?;
            validate_workspace_contracts(&registry)?;
            let errors = lint::lint_all(&registry)
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .filter(|diagnostic| !original_errors.contains(&diagnostic_signature(diagnostic)))
                .collect::<Vec<_>>();
            if !errors.is_empty() {
                bail!("written workspace failed post-commit validation");
            }
            Ok(())
        })?;

        Ok(MutationOutcome {
            plan,
            written: true,
            edits,
        })
    }
}

#[derive(Debug, Clone)]
struct CandidateWorkspace {
    specs_dir: PathBuf,
    documents: BTreeMap<String, EditableDocument>,
    original_documents: BTreeMap<PathBuf, Vec<u8>>,
    extra_writes: BTreeMap<PathBuf, Vec<u8>>,
    renames: BTreeMap<PathBuf, PathBuf>,
}

impl CandidateWorkspace {
    fn load(specs_dir: &Path) -> Result<Self> {
        let mut paths = WalkDir::new(specs_dir)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.ends_with(".spec.md"))
            })
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        paths.sort();
        let mut documents = BTreeMap::new();
        let mut original_documents = BTreeMap::new();
        for path in paths {
            let document = EditableDocument::load(&path)?;
            let id = document.id();
            if documents.insert(id.clone(), document.clone()).is_some() {
                bail!("duplicate specification id '{id}'");
            }
            original_documents.insert(path, document.original.clone());
        }
        Ok(Self {
            specs_dir: specs_dir.to_path_buf(),
            documents,
            original_documents,
            extra_writes: BTreeMap::new(),
            renames: BTreeMap::new(),
        })
    }

    fn registry(&self) -> Result<SpecRegistry> {
        let documents = self
            .documents
            .values()
            .map(|document| document.semantic.clone())
            .collect();
        let config_path = self.specs_dir.join("_config.toml");
        let config = if let Some(bytes) = self.extra_writes.get(&config_path) {
            SpecConfig::from_toml(std::str::from_utf8(bytes)?)?
        } else {
            SpecConfig::load(&self.specs_dir)?
        };
        SpecRegistry::from_documents_with_config(&self.specs_dir, documents, config)
    }

    fn check_fingerprints(&self, expected: &BTreeMap<String, String>) -> Result<()> {
        for (id, fingerprint) in expected {
            let document = self
                .documents
                .get(id)
                .with_context(|| format!("if_match refers to unknown specification '{id}'"))?;
            let actual = content_fingerprint(document.text.as_bytes());
            if &actual != fingerprint {
                bail!(
                    "stale specification '{id}': expected fingerprint '{fingerprint}', current '{actual}'"
                );
            }
        }
        Ok(())
    }

    fn apply(&mut self, operation: &Operation) -> Result<()> {
        use Operation::*;
        match operation {
            SummaryReplace { spec, value } => self
                .doc_mut(spec)?
                .replace_frontmatter_scalar("summary", value),
            OwnerAdd { spec, owner } => self.add_list(spec, "owners", owner),
            OwnerRemove { spec, owner } => self.remove_list(spec, "owners", owner),
            PinSet { spec, value } => self
                .doc_mut(spec)?
                .replace_frontmatter_scalar("pinned_at", value),
            PinClear { spec } => self.doc_mut(spec)?.remove_frontmatter_key("pinned_at"),
            ImplementationCheckpointSet { spec, commit } => {
                validate_git_oid(commit)?;
                self.doc_mut(spec)?
                    .replace_frontmatter_scalar("implemented", commit)
            }
            ImplementationCheckpointClear { spec } => {
                self.doc_mut(spec)?.remove_frontmatter_key("implemented")
            }
            RelatedAdd { spec, target } => {
                self.ensure_exists(target)?;
                self.add_list(spec, "related", target)
            }
            RelatedRemove { spec, target } => self.remove_list(spec, "related", target),

            RequirementLevelSet { spec, level } => {
                self.ensure_type(spec, EntityType::Req)?;
                if Level::from_str_val(level).is_none() {
                    bail!("invalid requirement level '{level}'");
                }
                self.doc_mut(spec)?
                    .replace_frontmatter_scalar("level", level)
            }
            RequirementKindSet { spec, kind } => {
                self.ensure_type(spec, EntityType::Req)?;
                self.doc_mut(spec)?.replace_frontmatter_scalar("kind", kind)
            }
            RequirementKindClear { spec } => {
                self.ensure_type(spec, EntityType::Req)?;
                self.doc_mut(spec)?.remove_frontmatter_key("kind")
            }
            RequirementMonotonicitySet { spec, value } => {
                self.ensure_type(spec, EntityType::Req)?;
                self.doc_mut(spec)?
                    .replace_frontmatter_bool("level_monotonic", *value)
            }

            InvariantEnforcementAdd { spec, value } => {
                self.ensure_type(spec, EntityType::Inv)?;
                self.add_list(spec, "enforcement", value)
            }
            InvariantEnforcementRemove { spec, value } => {
                self.ensure_type(spec, EntityType::Inv)?;
                self.remove_list(spec, "enforcement", value)
            }
            InvariantRequirementAdd { spec, requirement } => {
                self.ensure_type(spec, EntityType::Inv)?;
                self.ensure_type(requirement, EntityType::Req)?;
                self.add_list(spec, "applies_to", requirement)
            }
            InvariantRequirementRemove { spec, requirement } => {
                self.ensure_type(spec, EntityType::Inv)?;
                self.remove_list(spec, "applies_to", requirement)
            }

            InterfaceStabilitySet { spec, stability } => {
                self.ensure_type(spec, EntityType::Ifc)?;
                if Stability::from_str_val(stability).is_none() {
                    bail!("invalid interface stability '{stability}'");
                }
                self.doc_mut(spec)?
                    .replace_frontmatter_scalar("stability", stability)
            }
            InterfaceConsumerAdd { spec, consumer } => {
                self.ensure_type(spec, EntityType::Ifc)?;
                self.add_list(spec, "consumed_by", consumer)
            }
            InterfaceConsumerRemove { spec, consumer } => {
                self.ensure_type(spec, EntityType::Ifc)?;
                self.remove_list(spec, "consumed_by", consumer)
            }
            InterfaceProviderAdd { spec, provider } => {
                self.ensure_type(spec, EntityType::Ifc)?;
                self.add_list(spec, "provided_by", provider)
            }
            InterfaceProviderRemove { spec, provider } => {
                self.ensure_type(spec, EntityType::Ifc)?;
                self.remove_list(spec, "provided_by", provider)
            }

            AdrDecisionDateSet { spec, value } => {
                self.ensure_type(spec, EntityType::Adr)?;
                validate_date(value)?;
                self.doc_mut(spec)?
                    .replace_frontmatter_scalar("decision_date", value)
            }
            AdrDecisionMakerAdd { spec, owner } => {
                self.ensure_type(spec, EntityType::Adr)?;
                self.add_list(spec, "decided_by", owner)
            }
            AdrDecisionMakerRemove { spec, owner } => {
                self.ensure_type(spec, EntityType::Adr)?;
                self.remove_list(spec, "decided_by", owner)
            }

            ContentTitleReplace { spec, value } => self.doc_mut(spec)?.replace_title(value),
            ContentSectionReplace {
                spec,
                heading,
                markdown,
            } => self.doc_mut(spec)?.replace_section(heading, markdown),
            ContentBlockAdd {
                spec,
                heading,
                kind,
                block,
                level,
                markdown,
            } => {
                if BlockKind::from_tag(kind).is_none() {
                    bail!("unknown typed block kind '{kind}'");
                }
                if let Some(level) = level {
                    if Level::from_str_val(level).is_none() {
                        bail!("invalid typed block level '{level}'");
                    }
                }
                self.doc_mut(spec)?
                    .add_block(heading, kind, block, level.as_deref(), markdown)
            }
            ContentBlockReplace {
                spec,
                block,
                markdown,
            } => self.doc_mut(spec)?.replace_block(block, markdown),
            ContentBlockRemove { spec, block } => self.doc_mut(spec)?.remove_block(block),
            ContentClauseAdd {
                spec,
                block,
                clause,
                markdown,
            } => self.doc_mut(spec)?.add_clause(block, clause, markdown),
            ContentClauseReplace {
                spec,
                block,
                clause,
                markdown,
            } => self.doc_mut(spec)?.replace_clause(block, clause, markdown),
            ContentClauseRemove {
                spec,
                block,
                clause,
            } => self.doc_mut(spec)?.remove_clause(block, clause),

            RelationRefine { spec, target } => {
                self.ensure_refinement(spec, target)?;
                self.add_list(spec, "refines", target)
            }
            RelationUnrefine { spec, target } => {
                self.ensure_refining_type(spec)?;
                self.remove_list(spec, "refines", target)
            }
            RelationAspectAdd { spec, aspect } => {
                self.ensure_refining_type(spec)?;
                self.add_list(spec, "aspects", aspect)
            }
            RelationAspectRemove { spec, aspect } => {
                self.ensure_refining_type(spec)?;
                self.remove_list(spec, "aspects", aspect)
            }
            RelationCategorize { spec, topic } => {
                self.ensure_categorizable_type(spec)?;
                self.ensure_type(topic, EntityType::Topic)?;
                self.add_list(spec, "categorized_under", topic)
            }
            RelationUncategorize { spec, topic } => {
                self.ensure_categorizable_type(spec)?;
                self.remove_list(spec, "categorized_under", topic)
            }

            LifecycleDraft { spec } => self.set_lifecycle(spec, Status::Draft),
            LifecycleAccept { spec } => self.set_lifecycle(spec, Status::Accepted),
            LifecycleDeprecate { spec } => self.set_lifecycle(spec, Status::Deprecated),
            LifecycleSupersede { spec, replacement } => self.supersede(spec, replacement),

            TaskProgressSet { spec, progress } => self.set_task_progress(spec, progress),
            TaskBlockerAdd { spec, blocker } => {
                self.ensure_type(spec, EntityType::Task)?;
                self.ensure_type(blocker, EntityType::Task)?;
                self.add_list(spec, "blocked_by", blocker)
            }
            TaskBlockerRemove { spec, blocker } => {
                self.ensure_type(spec, EntityType::Task)?;
                self.remove_list(spec, "blocked_by", blocker)
            }
            TaskAssigneeSet { spec, assignee } => {
                self.ensure_type(spec, EntityType::Task)?;
                self.doc_mut(spec)?
                    .replace_frontmatter_scalar("assignee", assignee)
            }
            TaskAssigneeClear { spec } => {
                self.ensure_type(spec, EntityType::Task)?;
                self.doc_mut(spec)?.remove_frontmatter_key("assignee")
            }
            TaskEtaSet { spec, eta } => {
                self.ensure_type(spec, EntityType::Task)?;
                self.doc_mut(spec)?.replace_frontmatter_scalar("eta", eta)
            }
            TaskEtaClear { spec } => {
                self.ensure_type(spec, EntityType::Task)?;
                self.doc_mut(spec)?.remove_frontmatter_key("eta")
            }

            SpecRename { spec, new_id } => self.rename(spec, new_id),
            DocumentationCollectionAdd {
                id,
                title,
                root,
                include,
                exclude,
            } => self.add_documentation_collection(id, title, root, include, exclude),
            IntellectProviderSet { provider } => self.set_intellect_provider(provider),
        }
    }

    fn doc_mut(&mut self, id: &str) -> Result<&mut EditableDocument> {
        self.documents
            .get_mut(id)
            .with_context(|| format!("no spec with id '{id}'"))
    }

    fn ensure_exists(&self, id: &str) -> Result<()> {
        if self.documents.contains_key(id) {
            Ok(())
        } else {
            bail!("no spec with id '{id}'")
        }
    }

    fn ensure_type(&self, id: &str, expected: EntityType) -> Result<()> {
        let document = self
            .documents
            .get(id)
            .with_context(|| format!("no spec with id '{id}'"))?;
        if document.semantic.universal.entity_type != expected {
            bail!(
                "spec '{id}' is not a {} (type: {})",
                expected.type_name(),
                document.semantic.universal.entity_type.type_name()
            );
        }
        Ok(())
    }

    fn ensure_refining_type(&self, id: &str) -> Result<()> {
        let document = self
            .documents
            .get(id)
            .with_context(|| format!("no spec with id '{id}'"))?;
        if !matches!(
            document.semantic.universal.entity_type,
            EntityType::Req | EntityType::Task
        ) {
            bail!("only REQ and TASK specifications may refine another specification");
        }
        Ok(())
    }

    fn ensure_categorizable_type(&self, id: &str) -> Result<()> {
        let document = self
            .documents
            .get(id)
            .with_context(|| format!("no spec with id '{id}'"))?;
        if !matches!(
            document.semantic.universal.entity_type,
            EntityType::Req | EntityType::Task
        ) {
            bail!("only REQ and TASK specifications may be categorized");
        }
        Ok(())
    }

    fn ensure_refinement(&self, child: &str, target: &str) -> Result<()> {
        self.ensure_refining_type(child)?;
        let qualified: QualifiedAnchor = target
            .parse()
            .map_err(|error: String| anyhow::anyhow!(error))?;
        self.ensure_type(&qualified.spec_id.to_string(), EntityType::Req)?;
        if let Some(anchor) = qualified.anchor {
            let parent = self.documents.get(&qualified.spec_id.to_string()).unwrap();
            if !parent
                .semantic
                .anchors()
                .iter()
                .any(|candidate| candidate == &anchor)
            {
                bail!("refinement target '{target}' does not resolve");
            }
        }
        Ok(())
    }

    fn add_list(&mut self, spec: &str, key: &str, value: &str) -> Result<()> {
        self.doc_mut(spec)?.add_frontmatter_list_item(key, value)
    }

    fn remove_list(&mut self, spec: &str, key: &str, value: &str) -> Result<()> {
        self.doc_mut(spec)?.remove_frontmatter_list_item(key, value)
    }

    fn set_lifecycle(&mut self, spec: &str, status: Status) -> Result<()> {
        let document = self.doc_mut(spec)?;
        if document.semantic.universal.status == Status::Superseded {
            bail!("superseded specifications cannot leave that state through a direct lifecycle command");
        }
        document.replace_frontmatter_scalar("status", status.as_str())
    }

    fn supersede(&mut self, old: &str, replacement: &str) -> Result<()> {
        if old == replacement {
            bail!("a specification cannot supersede itself");
        }
        let old_type = self
            .documents
            .get(old)
            .with_context(|| format!("no spec with id '{old}'"))?
            .semantic
            .universal
            .entity_type;
        self.ensure_type(replacement, old_type)?;
        let old_pointer = self.documents[old]
            .semantic
            .universal
            .superseded_by
            .as_deref();
        if old_pointer.is_some_and(|value| value != replacement) {
            bail!("'{old}' already has a conflicting superseded_by pointer");
        }
        let replacement_pointer = self.documents[replacement]
            .semantic
            .universal
            .supersedes
            .as_deref();
        if replacement_pointer.is_some_and(|value| value != old) {
            bail!("'{replacement}' already has a conflicting supersedes pointer");
        }
        self.doc_mut(old)?
            .replace_frontmatter_scalar("status", "superseded")?;
        self.doc_mut(old)?
            .replace_frontmatter_scalar("superseded_by", replacement)?;
        self.doc_mut(replacement)?
            .replace_frontmatter_scalar("supersedes", old)
    }

    fn set_task_progress(&mut self, spec: &str, progress: &str) -> Result<()> {
        self.ensure_type(spec, EntityType::Task)?;
        let progress = Progress::from_str_val(progress)
            .with_context(|| format!("invalid task progress '{progress}'"))?;
        self.doc_mut(spec)?
            .replace_frontmatter_scalar("progress", progress.as_str())?;
        if progress != Progress::Blocked {
            let blockers = self.doc_mut(spec)?.frontmatter_list("blocked_by")?;
            for blocker in blockers {
                self.doc_mut(spec)?
                    .remove_frontmatter_list_item("blocked_by", &blocker)?;
            }
        }
        Ok(())
    }

    fn rename(&mut self, old: &str, new: &str) -> Result<()> {
        let old_id: SpecId = old
            .parse()
            .map_err(|error: String| anyhow::anyhow!(error))?;
        let new_id: SpecId = new
            .parse()
            .map_err(|error: String| anyhow::anyhow!(error))?;
        if old_id.entity_type != new_id.entity_type {
            bail!(
                "rename cannot change entity type ({} to {})",
                old_id.entity_type,
                new_id.entity_type
            );
        }
        if self.documents.contains_key(new) {
            bail!("rename destination id '{new}' already exists");
        }
        let mut renamed = self
            .documents
            .remove(old)
            .with_context(|| format!("no spec with id '{old}'"))?;
        let old_path = renamed.source_path.clone();
        let original_path = self
            .renames
            .iter()
            .find_map(|(origin, current)| (current == &old_path).then_some(origin.clone()))
            .unwrap_or_else(|| old_path.clone());
        let new_path = if new_id.entity_type == EntityType::Project {
            old_path.clone()
        } else {
            self.specs_dir.join(format!("{}.spec.md", new_id.path()))
        };
        if new_path != old_path
            && (new_path.exists() || self.renames.values().any(|path| path == &new_path))
        {
            bail!(
                "rename destination path '{}' already exists",
                new_path.display()
            );
        }
        renamed.replace_frontmatter_scalar("id", new)?;
        let original = renamed.original.clone();
        renamed = EditableDocument::from_text(&new_path, renamed.text.clone(), original)?;
        self.documents.insert(new.to_string(), renamed);

        let ids = self.documents.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            self.replace_id_in_document(&id, old, new)?;
        }
        if old_id.entity_type == EntityType::Project {
            self.update_config_project(old, new)?;
        }
        self.append_redirect(old, new)?;
        if original_path != new_path {
            self.renames.remove(&original_path);
            self.renames.insert(original_path, new_path);
        }
        Ok(())
    }

    fn replace_id_in_document(&mut self, id: &str, old: &str, new: &str) -> Result<()> {
        let keys = [
            "related",
            "refines",
            "categorized_under",
            "enforcement",
            "applies_to",
            "consumed_by",
            "provided_by",
            "blocked_by",
        ];
        for key in keys {
            let values = self.doc_mut(id)?.frontmatter_list(key).unwrap_or_default();
            if values.is_empty() {
                continue;
            }
            for value in values {
                let replaced = replace_id_prefix(&value, old, new);
                if replaced != value {
                    self.doc_mut(id)?
                        .replace_frontmatter_list_item(key, &value, &replaced)?;
                }
            }
        }
        for key in ["supersedes", "superseded_by"] {
            let current = match key {
                "supersedes" => self.documents[id].semantic.universal.supersedes.clone(),
                _ => self.documents[id].semantic.universal.superseded_by.clone(),
            };
            if let Some(current) = current {
                let replaced = replace_id_prefix(&current, old, new);
                if replaced != current {
                    self.doc_mut(id)?
                        .replace_frontmatter_scalar(key, &replaced)?;
                }
            }
        }
        self.doc_mut(id)?
            .replace_reference_prefix(&format!("spec:{old}"), &format!("spec:{new}"))?;
        Ok(())
    }

    fn update_config_project(&mut self, old: &str, new: &str) -> Result<()> {
        let path = self.specs_dir.join("_config.toml");
        let bytes = self
            .extra_writes
            .get(&path)
            .cloned()
            .unwrap_or(fs::read(&path)?);
        let content = String::from_utf8(bytes)?;
        let replacement = format!("project = {new:?}");
        let updated = replace_toml_assignment(&content, "project", &replacement);
        if !content.contains(&format!("project = {old:?}")) {
            bail!("configured project does not match renamed id '{old}'");
        }
        self.extra_writes.insert(path, updated.into_bytes());
        Ok(())
    }

    fn append_redirect(&mut self, old: &str, new: &str) -> Result<()> {
        let path = self.specs_dir.join("_redirects.toml");
        let bytes = self
            .extra_writes
            .get(&path)
            .cloned()
            .unwrap_or_else(|| fs::read(&path).unwrap_or_default());
        let mut content = String::from_utf8(bytes)?;
        let existing = crate::parse::redirects::load_redirects(&path).unwrap_or_default();
        if existing
            .iter()
            .any(|redirect| redirect.from == old && redirect.to == new)
        {
            return Ok(());
        }
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&format!("\n[[redirect]]\nfrom = {old:?}\nto = {new:?}\n"));
        self.extra_writes.insert(path, content.into_bytes());
        Ok(())
    }

    fn add_documentation_collection(
        &mut self,
        id: &str,
        title: &str,
        root: &str,
        include: &[String],
        exclude: &[String],
    ) -> Result<()> {
        let path = self.specs_dir.join("_config.toml");
        let bytes = self
            .extra_writes
            .get(&path)
            .cloned()
            .unwrap_or(fs::read(&path)?);
        let mut content = String::from_utf8(bytes)?;
        let current = SpecConfig::from_toml(&content)?;
        if current
            .documentation
            .iter()
            .any(|collection| collection.id == id)
        {
            bail!("documentation collection '{id}' already exists");
        }
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str("\n[[documentation]]\n");
        content.push_str(&format!("id = {}\n", serde_json::to_string(id)?));
        content.push_str(&format!("title = {}\n", serde_json::to_string(title)?));
        content.push_str(&format!("root = {}\n", serde_json::to_string(root)?));
        content.push_str(&format!("include = {}\n", serde_json::to_string(include)?));
        if !exclude.is_empty() {
            content.push_str(&format!("exclude = {}\n", serde_json::to_string(exclude)?));
        }
        SpecConfig::from_toml(&content)?;
        self.extra_writes.insert(path, content.into_bytes());
        Ok(())
    }

    fn set_intellect_provider(&mut self, provider: &str) -> Result<()> {
        if provider != DEFAULT_INTELLECT_PROVIDER {
            bail!(
                "unsupported intellect provider '{provider}'; this release supports only '{DEFAULT_INTELLECT_PROVIDER}'"
            );
        }
        let path = self.specs_dir.join("_config.toml");
        let bytes = self
            .extra_writes
            .get(&path)
            .cloned()
            .unwrap_or(fs::read(&path)?);
        let content = String::from_utf8(bytes)?;
        let replacement = format!("intellect_provider = {}", serde_json::to_string(provider)?);
        let updated = replace_root_toml_assignment(&content, "intellect_provider", &replacement);
        SpecConfig::from_toml(&updated)?;
        self.extra_writes.insert(path, updated.into_bytes());
        Ok(())
    }

    fn changed_writes(&self) -> Vec<AtomicWrite> {
        let mut writes = Vec::new();
        for document in self.documents.values() {
            let original_path = self
                .renames
                .iter()
                .find_map(|(old, new)| (new == &document.source_path).then_some(old.clone()))
                .unwrap_or_else(|| document.source_path.clone());
            let original = self.original_documents.get(&original_path);
            if original.map_or(true, |bytes| bytes != document.text.as_bytes())
                || original_path != document.source_path
            {
                writes.push(AtomicWrite {
                    destination: document.source_path.clone(),
                    origin: original.map(|_| original_path),
                    expected: original.cloned(),
                    bytes: document.text.as_bytes().to_vec(),
                });
            }
        }
        for (path, bytes) in &self.extra_writes {
            let original = fs::read(path).ok();
            if original.as_deref() != Some(bytes.as_slice()) {
                writes.push(AtomicWrite {
                    destination: path.clone(),
                    origin: original.as_ref().map(|_| path.clone()),
                    expected: original,
                    bytes: bytes.clone(),
                });
            }
        }
        writes.sort_by(|left, right| left.destination.cmp(&right.destination));
        writes
    }
}

#[derive(Debug, Clone)]
struct AtomicWrite {
    destination: PathBuf,
    origin: Option<PathBuf>,
    expected: Option<Vec<u8>>,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct PreparedWrite {
    write: AtomicWrite,
    temporary: PathBuf,
    backup: Option<PathBuf>,
}

pub fn content_fingerprint(bytes: &[u8]) -> String {
    let oid = Oid::hash_object(ObjectType::Blob, bytes)
        .expect("git blob hashing accepts an in-memory byte slice");
    format!("git-blob:{oid}")
}

/// Atomically replace a group of files without exposing any deletion API.
/// Used by scaffolding, migrations, and derived-data writers so all supported
/// persistence shares the transaction commit layer.
pub fn atomic_write_files(files: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    let mut writes = Vec::with_capacity(files.len());
    for (path, bytes) in files {
        let expected = fs::read(path).ok();
        writes.push(AtomicWrite {
            destination: path.clone(),
            origin: expected.as_ref().map(|_| path.clone()),
            expected,
            bytes: bytes.clone(),
        });
    }
    apply_atomic(&writes, || Ok(()))
}

fn apply_atomic(writes: &[AtomicWrite], validate: impl FnOnce() -> Result<()>) -> Result<()> {
    let mut prepared = Vec::new();
    for write in writes {
        match prepare_write(write) {
            Ok(item) => prepared.push(item),
            Err(error) => {
                for item in prepared {
                    let _ = fs::remove_file(item.temporary);
                }
                return Err(error);
            }
        }
    }

    let mut committed = BTreeSet::new();
    let result = (|| {
        for item in &prepared {
            match (&item.write.origin, &item.write.expected) {
                (Some(origin), Some(expected)) => {
                    let current = fs::read(origin).with_context(|| {
                        format!("checking concurrent changes to {}", origin.display())
                    })?;
                    if &current != expected {
                        bail!(
                            "transaction input changed after validation: {}",
                            origin.display()
                        );
                    }
                }
                (None, None) => {}
                _ => bail!("invalid transaction input state"),
            }
            if item.write.origin.as_ref() != Some(&item.write.destination)
                && item.write.destination.exists()
            {
                bail!(
                    "transaction destination appeared after validation: {}",
                    item.write.destination.display()
                );
            }
            if let (Some(origin), Some(backup)) = (&item.write.origin, &item.backup) {
                fs::rename(origin, backup)
                    .with_context(|| format!("backing up {}", origin.display()))?;
            }
            fs::rename(&item.temporary, &item.write.destination)
                .with_context(|| format!("committing {}", item.write.destination.display()))?;
            committed.insert(item.write.destination.clone());
        }
        validate()?;
        Ok(())
    })();

    if let Err(error) = result {
        for item in prepared.iter().rev() {
            if let (Some(origin), Some(backup)) = (&item.write.origin, &item.backup) {
                if backup.exists() {
                    if item.write.destination.exists() {
                        let _ = fs::remove_file(&item.write.destination);
                    }
                    let _ = fs::rename(backup, origin);
                }
            } else if committed.contains(&item.write.destination) && item.write.destination.exists()
            {
                let _ = fs::remove_file(&item.write.destination);
            }
            if item.temporary.exists() {
                let _ = fs::remove_file(&item.temporary);
            }
        }
        return Err(error);
    }

    for item in prepared {
        if let Some(backup) = item.backup {
            if backup.exists() {
                fs::remove_file(&backup)
                    .with_context(|| format!("removing transaction backup {}", backup.display()))?;
            }
        }
    }
    Ok(())
}

fn prepare_write(write: &AtomicWrite) -> Result<PreparedWrite> {
    if let Some(parent) = write.destination.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let temporary = sibling_path(&write.destination, "tmp");
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .with_context(|| format!("preparing {}", write.destination.display()))?;
        if let Some(origin) = &write.origin {
            file.set_permissions(
                fs::metadata(origin)
                    .with_context(|| format!("reading permissions for {}", origin.display()))?
                    .permissions(),
            )?;
        }
        file.write_all(&write.bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(PreparedWrite {
        write: write.clone(),
        temporary,
        backup: write
            .origin
            .as_ref()
            .map(|origin| sibling_path(origin, "backup")),
    })
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("spec");
    path.with_file_name(format!(
        ".{name}.forge-spec.{}.{counter}.{suffix}",
        std::process::id()
    ))
}

fn validate_workspace_contracts(registry: &SpecRegistry) -> Result<()> {
    let mut supersession = BTreeMap::<String, String>::new();
    for document in &registry.documents {
        let id = document.id_str();
        for related in &document.universal.related {
            ensure_registry_type(registry, related, None)?;
        }
        if let Some(replaced) = &document.universal.supersedes {
            let replaced_document =
                ensure_registry_type(registry, replaced, Some(document.universal.entity_type))?;
            if replaced_document.universal.superseded_by.as_deref() != Some(id.as_str()) {
                bail!("supersession pointers conflict between '{id}' and '{replaced}'");
            }
            if replaced_document.universal.status != Status::Superseded {
                bail!("superseded specification '{replaced}' is not in superseded lifecycle state");
            }
            supersession.insert(replaced.clone(), id.clone());
        }
        if let Some(replacement) = &document.universal.superseded_by {
            let replacement_document =
                ensure_registry_type(registry, replacement, Some(document.universal.entity_type))?;
            if replacement_document.universal.supersedes.as_deref() != Some(id.as_str()) {
                bail!("supersession pointers conflict between '{id}' and '{replacement}'");
            }
            if document.universal.status != Status::Superseded {
                bail!("only superseded specifications may declare superseded_by");
            }
        }
        match &document.type_fields {
            TypeSpecificFields::Requirement {
                refines,
                categorized_under,
                ..
            }
            | TypeSpecificFields::Task {
                refines,
                categorized_under,
                ..
            } => {
                for target in refines {
                    validate_refinement_target(registry, target)?;
                }
                for topic in categorized_under {
                    ensure_registry_type(registry, topic, Some(EntityType::Topic))?;
                }
            }
            _ => {}
        }
        if let TypeSpecificFields::Task { blocked_by, .. } = &document.type_fields {
            for blocker in blocked_by {
                ensure_registry_type(registry, blocker, Some(EntityType::Task))?;
            }
        }
    }
    for start in supersession.keys() {
        let mut visited = BTreeSet::new();
        let mut current = start.as_str();
        while let Some(next) = supersession.get(current) {
            if !visited.insert(current.to_string()) {
                bail!("supersession graph contains a cycle at '{current}'");
            }
            current = next;
        }
    }
    Ok(())
}

fn validate_refinement_target(registry: &SpecRegistry, target: &str) -> Result<()> {
    let qualified: QualifiedAnchor = target
        .parse()
        .map_err(|error: String| anyhow::anyhow!(error))?;
    let document = ensure_registry_type(
        registry,
        &qualified.spec_id.to_string(),
        Some(EntityType::Req),
    )?;
    if let Some(anchor) = qualified.anchor {
        if !document
            .anchors()
            .iter()
            .any(|candidate| candidate == &anchor)
        {
            bail!("refinement target '{target}' does not resolve");
        }
    }
    Ok(())
}

fn ensure_registry_type<'a>(
    registry: &'a SpecRegistry,
    id: &str,
    expected: Option<EntityType>,
) -> Result<&'a crate::model::document::SpecDocument> {
    let document = registry
        .get_by_id(id)
        .with_context(|| format!("relationship target '{id}' does not resolve"))?;
    if let Some(expected) = expected {
        if document.universal.entity_type != expected {
            bail!(
                "relationship target '{id}' must be a {}",
                expected.type_name()
            );
        }
    }
    Ok(document)
}

fn error_signatures(diagnostics: &[Diagnostic]) -> BTreeSet<String> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(diagnostic_signature)
        .collect()
}

fn diagnostic_signature(diagnostic: &Diagnostic) -> String {
    format!(
        "{}|{}|{}",
        diagnostic.code,
        diagnostic.file.display(),
        diagnostic.message
    )
}

fn replace_id_prefix(value: &str, old: &str, new: &str) -> String {
    if value == old {
        new.to_string()
    } else if let Some(suffix) = value
        .strip_prefix(old)
        .filter(|suffix| suffix.starts_with('#'))
    {
        format!("{new}{suffix}")
    } else {
        value.to_string()
    }
}

fn replace_toml_assignment(content: &str, key: &str, replacement: &str) -> String {
    let mut output = String::with_capacity(content.len().max(replacement.len() + 1));
    let mut found = false;
    for line in content.split_inclusive('\n') {
        let matches = line
            .trim_start()
            .strip_prefix(key)
            .map(str::trim_start)
            .is_some_and(|rest| rest.starts_with('='));
        if matches {
            found = true;
            output.push_str(replacement);
            if line.ends_with('\n') {
                output.push('\n');
            }
        } else {
            output.push_str(line);
        }
    }
    if !found {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(replacement);
        output.push('\n');
    }
    output
}

fn replace_root_toml_assignment(content: &str, key: &str, replacement: &str) -> String {
    if content.lines().any(|line| {
        line.trim_start()
            .strip_prefix(key)
            .map(str::trim_start)
            .is_some_and(|rest| rest.starts_with('='))
    }) {
        return replace_toml_assignment(content, key, replacement);
    }
    let insert_at = content
        .find("\n[[")
        .map(|offset| offset + 1)
        .unwrap_or(content.len());
    let mut output = String::with_capacity(content.len() + replacement.len() + 2);
    output.push_str(&content[..insert_at]);
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(replacement);
    output.push('\n');
    if insert_at < content.len() {
        output.push('\n');
    }
    output.push_str(&content[insert_at..]);
    output
}

fn validate_date(value: &str) -> Result<()> {
    let valid = value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 4 | 7) || ch.is_ascii_digit());
    if !valid {
        bail!("decision date must use YYYY-MM-DD");
    }
    Ok(())
}

fn validate_git_oid(value: &str) -> Result<()> {
    let valid_length = value.len() == 40 || value.len() == 64;
    if !valid_length || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        bail!("implementation checkpoint must be a full 40- or 64-character Git object ID");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn workspace() -> TempDir {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [carlo]\n---\n\n# Demo\n",
        )
        .unwrap();
        fs::create_dir_all(temp.path().join("auth")).unwrap();
        fs::write(
            temp.path().join("auth/session.spec.md"),
            "---\nid: REQ:auth/session\ntype: requirement\nstatus: accepted\nsummary: Original.\nowners: [carlo]\nlevel: MUST\nrefines: []\n---\n\n# Session\n\n:::{requirement id=\"session\" level=\"MUST\"}\n- {#c-lifetime} Old.\n:::\n",
        )
        .unwrap();
        temp
    }

    #[test]
    fn stale_fingerprint_rejects_without_writing() {
        let temp = workspace();
        let path = temp.path().join("auth/session.spec.md");
        let before = fs::read(&path).unwrap();
        let mut request = ChangeRequest::new(vec![Operation::SummaryReplace {
            spec: "REQ:auth/session".into(),
            value: "Changed.".into(),
        }]);
        request
            .if_match
            .insert("REQ:auth/session".into(), "stale".into());
        assert!(MutationEngine::new(temp.path())
            .execute(&request, false)
            .is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn dry_run_is_read_only_and_reports_file() {
        let temp = workspace();
        let path = temp.path().join("auth/session.spec.md");
        let before = fs::read(&path).unwrap();
        let request = ChangeRequest::new(vec![Operation::SummaryReplace {
            spec: "REQ:auth/session".into(),
            value: "Changed.".into(),
        }]);
        let outcome = MutationEngine::new(temp.path())
            .execute(&request, true)
            .unwrap();
        assert!(!outcome.written);
        assert_eq!(outcome.plan.files, vec![path.display().to_string()]);
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn task_operation_rejects_requirement_without_writing() {
        let temp = workspace();
        let path = temp.path().join("auth/session.spec.md");
        let before = fs::read(&path).unwrap();
        let request = ChangeRequest::new(vec![Operation::TaskProgressSet {
            spec: "REQ:auth/session".into(),
            progress: "done".into(),
        }]);
        assert!(MutationEngine::new(temp.path())
            .execute(&request, false)
            .is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn categorization_rejects_non_requirement_and_non_task_targets() {
        let temp = workspace();
        fs::write(
            temp.path().join("topic.spec.md"),
            "---\nid: TOPIC:demo/general\ntype: topic\nstatus: accepted\nsummary: General.\nowners: [carlo]\n---\n\n# General\n",
        )
        .unwrap();
        let project = temp.path().join("_project.spec.md");
        let before = fs::read(&project).unwrap();
        let request = ChangeRequest::new(vec![Operation::RelationCategorize {
            spec: "PROJECT:demo".into(),
            topic: "TOPIC:demo/general".into(),
        }]);

        assert!(MutationEngine::new(temp.path())
            .execute(&request, false)
            .is_err());
        assert_eq!(fs::read(project).unwrap(), before);
    }

    #[test]
    fn multi_document_failure_is_atomic() {
        let temp = workspace();
        let project = temp.path().join("_project.spec.md");
        let requirement = temp.path().join("auth/session.spec.md");
        let before_project = fs::read(&project).unwrap();
        let before_requirement = fs::read(&requirement).unwrap();
        let request = ChangeRequest::new(vec![
            Operation::SummaryReplace {
                spec: "PROJECT:demo".into(),
                value: "Changed project.".into(),
            },
            Operation::RelationCategorize {
                spec: "REQ:auth/session".into(),
                topic: "REQ:auth/session".into(),
            },
        ]);
        assert!(MutationEngine::new(temp.path())
            .execute(&request, false)
            .is_err());
        assert_eq!(fs::read(project).unwrap(), before_project);
        assert_eq!(fs::read(requirement).unwrap(), before_requirement);
    }

    #[test]
    fn concurrent_change_is_preserved_when_transaction_aborts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("document.spec.md");
        fs::write(&path, b"concurrent").unwrap();
        let write = AtomicWrite {
            destination: path.clone(),
            origin: Some(path.clone()),
            expected: Some(b"original".to_vec()),
            bytes: b"replacement".to_vec(),
        };

        let error = apply_atomic(&[write], || Ok(())).unwrap_err();

        assert!(error.to_string().contains("changed after validation"));
        assert_eq!(fs::read(&path).unwrap(), b"concurrent");
        let leftovers = fs::read_dir(temp.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("forge-spec"))
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn post_commit_validation_failure_rolls_back_every_file() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first.spec.md");
        let second = temp.path().join("second.spec.md");
        fs::write(&first, b"first-original").unwrap();
        fs::write(&second, b"second-original").unwrap();
        let writes = [
            AtomicWrite {
                destination: first.clone(),
                origin: Some(first.clone()),
                expected: Some(b"first-original".to_vec()),
                bytes: b"first-changed".to_vec(),
            },
            AtomicWrite {
                destination: second.clone(),
                origin: Some(second.clone()),
                expected: Some(b"second-original".to_vec()),
                bytes: b"second-changed".to_vec(),
            },
        ];

        assert!(apply_atomic(&writes, || anyhow::bail!("validation failed")).is_err());

        assert_eq!(fs::read(first).unwrap(), b"first-original");
        assert_eq!(fs::read(second).unwrap(), b"second-original");
    }

    #[test]
    fn rename_moves_the_file_updates_incoming_references_and_adds_redirect() {
        let temp = workspace();
        fs::create_dir_all(temp.path().join("work")).unwrap();
        fs::write(
            temp.path().join("work/session.spec.md"),
            "---\nid: TASK:work/session\ntype: task\nstatus: accepted\nsummary: Implement session.\nowners: [carlo]\nprogress: pending\nrefines: [REQ:auth/session#c-lifetime]\nassignee:\neta:\nblocked_by: []\n---\n# Work\n\nSee [session](spec:REQ:auth/session#c-lifetime).\n",
        )
        .unwrap();
        let request = ChangeRequest::new(vec![Operation::SpecRename {
            spec: "REQ:auth/session".into(),
            new_id: "REQ:auth/session-policy".into(),
        }]);
        MutationEngine::new(temp.path())
            .execute(&request, false)
            .unwrap();
        assert!(!temp.path().join("auth/session.spec.md").exists());
        assert!(temp.path().join("auth/session-policy.spec.md").exists());
        let task = fs::read_to_string(temp.path().join("work/session.spec.md")).unwrap();
        assert!(task.contains("REQ:auth/session-policy#c-lifetime"));
        let redirects = fs::read_to_string(temp.path().join("_redirects.toml")).unwrap();
        assert!(redirects.contains("from = \"REQ:auth/session\""));
        assert!(redirects.contains("to = \"REQ:auth/session-policy\""));
    }

    #[test]
    fn chained_renames_keep_the_original_path_as_the_transaction_origin() {
        let temp = workspace();
        let request = ChangeRequest::new(vec![
            Operation::SpecRename {
                spec: "REQ:auth/session".into(),
                new_id: "REQ:auth/session-policy".into(),
            },
            Operation::SpecRename {
                spec: "REQ:auth/session-policy".into(),
                new_id: "REQ:auth/final-session-policy".into(),
            },
        ]);

        MutationEngine::new(temp.path())
            .execute(&request, false)
            .unwrap();

        assert!(!temp.path().join("auth/session.spec.md").exists());
        assert!(!temp.path().join("auth/session-policy.spec.md").exists());
        assert!(temp
            .path()
            .join("auth/final-session-policy.spec.md")
            .exists());
    }

    #[test]
    fn documentation_collection_add_preserves_config_and_indexes_only_matches() {
        let temp = tempfile::tempdir().unwrap();
        let specs = temp.path().join(".specs");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(temp.path().join("docs/generated")).unwrap();
        fs::write(
            specs.join("_config.toml"),
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        fs::write(
            specs.join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [carlo]\n---\n\n# Demo\n",
        )
        .unwrap();
        fs::write(temp.path().join("docs/guide.md"), "# Guide\n").unwrap();
        fs::write(
            temp.path().join("docs/generated/output.md"),
            "# Generated\n",
        )
        .unwrap();
        let request = ChangeRequest::new(vec![Operation::DocumentationCollectionAdd {
            id: "guides".into(),
            title: "Guides".into(),
            root: "docs".into(),
            include: vec!["**/*.md".into()],
            exclude: vec!["generated/**".into()],
        }]);

        let outcome = MutationEngine::new(&specs)
            .execute(&request, false)
            .unwrap();
        assert!(outcome.written);
        assert_eq!(outcome.plan.operations[0].spec, None);
        assert_eq!(
            outcome.plan.operations[0].config,
            Some(".specs/_config.toml")
        );
        let config = SpecConfig::load(&specs).unwrap();
        assert_eq!(config.project.as_deref(), Some("PROJECT:demo"));
        assert_eq!(config.documentation.len(), 1);
        let registry = SpecRegistry::load(&specs).unwrap();
        assert_eq!(
            registry
                .documentation
                .documents
                .iter()
                .map(|document| document.path.as_str())
                .collect::<Vec<_>>(),
            ["docs/guide.md"]
        );
    }

    #[test]
    fn implementation_checkpoint_set_and_clear_are_typed_and_validated() {
        let temp = workspace();
        let checkpoint = "0123456789abcdef0123456789abcdef01234567";
        let set = ChangeRequest::new(vec![Operation::ImplementationCheckpointSet {
            spec: "REQ:auth/session".into(),
            commit: checkpoint.into(),
        }]);
        MutationEngine::new(temp.path())
            .execute(&set, false)
            .unwrap();
        let registry = SpecRegistry::load(temp.path()).unwrap();
        assert_eq!(
            registry
                .get_by_id("REQ:auth/session")
                .unwrap()
                .universal
                .implemented
                .as_deref(),
            Some(checkpoint)
        );

        let clear = ChangeRequest::new(vec![Operation::ImplementationCheckpointClear {
            spec: "REQ:auth/session".into(),
        }]);
        MutationEngine::new(temp.path())
            .execute(&clear, false)
            .unwrap();
        assert!(SpecRegistry::load(temp.path())
            .unwrap()
            .get_by_id("REQ:auth/session")
            .unwrap()
            .universal
            .implemented
            .is_none());

        let invalid = ChangeRequest::new(vec![Operation::ImplementationCheckpointSet {
            spec: "REQ:auth/session".into(),
            commit: "short".into(),
        }]);
        assert!(MutationEngine::new(temp.path())
            .execute(&invalid, false)
            .is_err());
    }

    #[test]
    fn intellect_provider_mutation_stays_at_the_toml_root() {
        let temp = workspace();
        fs::write(temp.path().join("guide.md"), "# Guide\n").unwrap();
        let documentation_root = temp.path().file_name().unwrap().to_string_lossy();
        fs::write(
            temp.path().join("_config.toml"),
            format!(
                "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:demo\"\n\n[[documentation]]\nid = \"guides\"\ntitle = \"Guides\"\nroot = {documentation_root:?}\ninclude = [\"guide.md\"]\n"
            ),
        )
        .unwrap();
        let request = ChangeRequest::new(vec![Operation::IntellectProviderSet {
            provider: "forge-intellect".into(),
        }]);
        MutationEngine::new(temp.path())
            .execute(&request, false)
            .unwrap();
        let content = fs::read_to_string(temp.path().join("_config.toml")).unwrap();
        assert!(
            content.find("intellect_provider").unwrap()
                < content.find("[[documentation]]").unwrap()
        );
        assert_eq!(
            SpecConfig::load(temp.path()).unwrap().intellect_provider,
            DEFAULT_INTELLECT_PROVIDER
        );

        let unsupported = ChangeRequest::new(vec![Operation::IntellectProviderSet {
            provider: "arbitrary-command".into(),
        }]);
        assert!(MutationEngine::new(temp.path())
            .execute(&unsupported, false)
            .is_err());
    }
}
