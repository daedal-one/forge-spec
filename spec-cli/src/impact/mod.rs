//! Read-only change-impact analysis across the refinement graph, source links,
//! task state, and Git trailer evidence.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use git2::{ObjectType, Repository, Tree, TreeWalkMode, TreeWalkResult};
use serde::Serialize;
use walkdir::WalkDir;

use crate::history::trailers;
use crate::model::block::TypedBlock;
use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::id::{EntityType, QualifiedAnchor};
use crate::model::reference::{SourceTarget, SpecReference};
use crate::model::registry::SpecRegistry;

const WORKING_TREE: &str = "working-tree";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpactRequest {
    Subject { subject: String },
    Diff { base: String, head: String },
}

impl ImpactRequest {
    pub fn new(subject: Option<&str>, base: Option<&str>, head: Option<&str>) -> Result<Self> {
        match (subject, base, head) {
            (Some(subject), None, None) => Ok(Self::Subject {
                subject: subject.to_string(),
            }),
            (Some(_), _, _) => {
                bail!("an explicit impact subject cannot be combined with --base or --head")
            }
            (None, Some(base), head) => Ok(Self::Diff {
                base: base.to_string(),
                head: normalize_head(head.unwrap_or(WORKING_TREE)),
            }),
            (None, None, Some(_)) => bail!("--head requires --base"),
            (None, None, None) => {
                bail!("provide a spec ID or anchor, or compare changes with --base <revision>")
            }
        }
    }
}

fn normalize_head(head: &str) -> String {
    match head {
        "worktree" | "working-tree" => WORKING_TREE.to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotSide {
    Base,
    Head,
    Current,
}

impl SnapshotSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Head => "head",
            Self::Current => "current",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactInput {
    pub reference: String,
    pub change: String,
    pub snapshots: BTreeSet<SnapshotSide>,
    pub cascade: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactedSpec {
    pub id: String,
    pub entity_type: String,
    pub status: String,
    pub summary: Option<String>,
    pub depth: usize,
    pub reason: String,
    pub path: Vec<String>,
    pub snapshots: BTreeSet<SnapshotSide>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSurface {
    pub reference: String,
    pub path: String,
    pub target_kind: String,
    pub symbol: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub specifications: BTreeSet<String>,
    pub snapshots: BTreeSet<SnapshotSide>,
    pub test_path: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEvidence {
    pub spec_ref: String,
    pub kind: String,
    pub commit: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactTask {
    pub id: String,
    pub progress: String,
    pub summary: Option<String>,
    pub depth: usize,
    pub snapshot: SnapshotSide,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactSummary {
    pub changed_inputs: usize,
    pub affected_specs: usize,
    pub requirements: usize,
    pub tasks: usize,
    pub explicit_source_references: usize,
    pub historical_events: usize,
    pub implementation_files: usize,
    pub test_files: usize,
    pub max_depth: usize,
    pub coverage_gaps: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub mode: String,
    pub project: Option<String>,
    pub base: Option<String>,
    pub head: Option<String>,
    pub summary: ImpactSummary,
    pub inputs: Vec<ImpactInput>,
    pub affected_specs: Vec<ImpactedSpec>,
    pub source_surfaces: Vec<SourceSurface>,
    pub history: Vec<HistoryEvidence>,
    pub tasks: Vec<ImpactTask>,
    pub gaps: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug)]
struct Snapshot {
    side: SnapshotSide,
    registry: SpecRegistry,
    contents: BTreeMap<String, String>,
}

impl Snapshot {
    fn working(specs_dir: &Path, side: SnapshotSide) -> Result<Self> {
        let mut documents = Vec::new();
        let mut contents = BTreeMap::new();

        for entry in WalkDir::new(specs_dir) {
            let entry = entry?;
            if !entry.file_type().is_file() || !is_spec_path(entry.path()) {
                continue;
            }
            let content = std::fs::read_to_string(entry.path())
                .with_context(|| format!("reading {}", entry.path().display()))?;
            let document = crate::parse::parse_content(entry.path(), &content)?;
            contents.insert(document.id_str(), content);
            documents.push(document);
        }

        let registry = SpecRegistry::from_documents(specs_dir, documents)?;
        Ok(Self {
            side,
            registry,
            contents,
        })
    }

    fn from_tree(
        specs_dir: &Path,
        repository: &Repository,
        tree: &Tree<'_>,
        side: SnapshotSide,
    ) -> Result<Self> {
        let workdir = repository
            .workdir()
            .context("impact analysis requires a non-bare Git repository")?;
        let relative_specs = specs_dir
            .canonicalize()
            .unwrap_or_else(|_| specs_dir.to_path_buf())
            .strip_prefix(
                workdir
                    .canonicalize()
                    .unwrap_or_else(|_| workdir.to_path_buf()),
            )
            .context("spec directory is outside the Git worktree")?
            .to_path_buf();
        let mut files = Vec::<(PathBuf, String)>::new();
        let mut walk_error = None;

        tree.walk(TreeWalkMode::PreOrder, |root, entry| {
            if walk_error.is_some() || entry.kind() != Some(ObjectType::Blob) {
                return TreeWalkResult::Ok;
            }
            let Some(name) = entry.name() else {
                return TreeWalkResult::Ok;
            };
            let relative_path = PathBuf::from(root).join(name);
            if !relative_path.starts_with(&relative_specs) || !is_spec_path(&relative_path) {
                return TreeWalkResult::Ok;
            }
            match repository.find_blob(entry.id()).and_then(|blob| {
                std::str::from_utf8(blob.content())
                    .map(str::to_string)
                    .map_err(|error| git2::Error::from_str(&error.to_string()))
            }) {
                Ok(content) => files.push((relative_path, content)),
                Err(error) => walk_error = Some(error),
            }
            TreeWalkResult::Ok
        })?;
        if let Some(error) = walk_error {
            return Err(error.into());
        }

        let mut documents = Vec::new();
        let mut contents = BTreeMap::new();
        for (relative_path, content) in files {
            let source_path = workdir.join(relative_path);
            let document = crate::parse::parse_content(&source_path, &content)?;
            contents.insert(document.id_str(), content);
            documents.push(document);
        }
        let registry = SpecRegistry::from_documents(specs_dir, documents)?;
        Ok(Self {
            side,
            registry,
            contents,
        })
    }

    fn document(&self, id: &str) -> Option<&SpecDocument> {
        self.registry.get_by_id(id)
    }
}

fn is_spec_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".spec.md"))
}

#[derive(Debug, Clone)]
struct PendingImpact {
    depth: usize,
    reason: String,
    path: Vec<String>,
    snapshots: BTreeSet<SnapshotSide>,
}

pub fn analyze(specs_dir: &Path, request: &ImpactRequest) -> Result<ImpactReport> {
    match request {
        ImpactRequest::Subject { subject } => analyze_subject(specs_dir, subject),
        ImpactRequest::Diff { base, head } => analyze_diff(specs_dir, base, head),
    }
}

fn analyze_subject(specs_dir: &Path, subject: &str) -> Result<ImpactReport> {
    let current = Snapshot::working(specs_dir, SnapshotSide::Current)?;
    let parsed: QualifiedAnchor = subject
        .parse()
        .map_err(|error: String| anyhow::anyhow!(error))?;
    let (canonical, _) = current.registry.resolve_redirect(&parsed.to_string());
    if !current.registry.reference_exists(&canonical).0 {
        bail!("spec or anchor not found: '{subject}'");
    }

    let mut sides = BTreeSet::new();
    sides.insert(SnapshotSide::Current);
    let inputs = vec![ImpactInput {
        reference: canonical,
        change: "selected".to_string(),
        snapshots: sides,
        cascade: true,
    }];
    finish_report(
        specs_dir,
        "subject",
        None,
        None,
        inputs,
        &[&current],
        Some("HEAD"),
    )
}

fn analyze_diff(specs_dir: &Path, base: &str, head: &str) -> Result<ImpactReport> {
    let repository = Repository::discover(specs_dir)?;
    let base_tree = revision_tree(&repository, base)?;
    let base_snapshot =
        Snapshot::from_tree(specs_dir, &repository, &base_tree, SnapshotSide::Base)?;

    let head_snapshot = if head == WORKING_TREE {
        Snapshot::working(specs_dir, SnapshotSide::Head)?
    } else {
        let head_tree = revision_tree(&repository, head)?;
        Snapshot::from_tree(specs_dir, &repository, &head_tree, SnapshotSide::Head)?
    };

    let inputs = changed_inputs(&base_snapshot, &head_snapshot)?;
    let history_head = if head == WORKING_TREE { "HEAD" } else { head };
    finish_report(
        specs_dir,
        "git-diff",
        Some(base),
        Some(head),
        inputs,
        &[&base_snapshot, &head_snapshot],
        Some(history_head),
    )
}

fn revision_tree<'repo>(repository: &'repo Repository, revision: &str) -> Result<Tree<'repo>> {
    repository
        .revparse_single(revision)
        .with_context(|| format!("resolving Git revision '{revision}'"))?
        .peel_to_tree()
        .with_context(|| format!("resolving tree for Git revision '{revision}'"))
}

fn changed_inputs(base: &Snapshot, head: &Snapshot) -> Result<Vec<ImpactInput>> {
    let ids = base
        .contents
        .keys()
        .chain(head.contents.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut inputs = Vec::new();

    for id in ids {
        match (base.contents.get(&id), head.contents.get(&id)) {
            (None, Some(_)) => inputs.push(input(&id, "added", &[SnapshotSide::Head], true)),
            (Some(_), None) => inputs.push(input(&id, "removed", &[SnapshotSide::Base], true)),
            (Some(old), Some(new)) if old != new => {
                let old_doc = base
                    .document(&id)
                    .context("base snapshot lost parsed document")?;
                let new_doc = head
                    .document(&id)
                    .context("head snapshot lost parsed document")?;
                inputs.extend(modified_inputs(old_doc, new_doc, old, new)?);
            }
            _ => {}
        }
    }

    inputs.sort_by(|left, right| {
        left.reference
            .cmp(&right.reference)
            .then_with(|| left.change.cmp(&right.change))
    });
    inputs.dedup_by(|left, right| {
        left.reference == right.reference
            && left.change == right.change
            && left.cascade == right.cascade
    });
    Ok(inputs)
}

fn input(reference: &str, change: &str, sides: &[SnapshotSide], cascade: bool) -> ImpactInput {
    ImpactInput {
        reference: reference.to_string(),
        change: change.to_string(),
        snapshots: sides.iter().copied().collect(),
        cascade,
    }
}

fn modified_inputs(
    old: &SpecDocument,
    new: &SpecDocument,
    old_raw: &str,
    new_raw: &str,
) -> Result<Vec<ImpactInput>> {
    let id = new.id_str();
    let both = [SnapshotSide::Base, SnapshotSide::Head];
    let mut inputs = Vec::new();
    let old_frontmatter = serde_json::to_value((&old.universal, &old.type_fields))?;
    let new_frontmatter = serde_json::to_value((&new.universal, &new.type_fields))?;

    let old_refines = refinement_targets(old);
    let new_refines = refinement_targets(new);
    for target in old_refines.difference(&new_refines) {
        inputs.push(input(
            target,
            "refinement-removed",
            &[SnapshotSide::Base],
            true,
        ));
    }
    for target in new_refines.difference(&old_refines) {
        inputs.push(input(
            target,
            "refinement-added",
            &[SnapshotSide::Head],
            true,
        ));
    }

    if old_frontmatter != new_frontmatter || outside_blocks(old) != outside_blocks(new) {
        inputs.push(input(&id, "modified", &both, true));
    }

    let old_blocks = block_map(old);
    let new_blocks = block_map(new);
    let block_keys = old_blocks
        .keys()
        .chain(new_blocks.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in block_keys {
        let old_block = old_blocks.get(&key).copied();
        let new_block = new_blocks.get(&key).copied();
        let (change, sides): (&str, &[SnapshotSide]) = match (old_block, new_block) {
            (None, Some(_)) => ("added", &[SnapshotSide::Head]),
            (Some(_), None) => ("removed", &[SnapshotSide::Base]),
            (Some(left), Some(right)) if !same_block(left, right) => ("modified", &both),
            _ => continue,
        };
        if key.is_empty() {
            inputs.push(input(&id, change, sides, true));
            continue;
        }
        inputs.push(input(&format!("{id}#{key}"), change, sides, true));

        // A block-level semantic change can alter the meaning of every nested
        // clause even when the individual clause text did not change.
        let clause_ids = old_block
            .into_iter()
            .flat_map(|block| block.clauses.iter().map(|clause| clause.id.clone()))
            .chain(
                new_block
                    .into_iter()
                    .flat_map(|block| block.clauses.iter().map(|clause| clause.id.clone())),
            )
            .collect::<BTreeSet<_>>();
        for clause_id in clause_ids {
            inputs.push(input(
                &format!("{id}#{clause_id}"),
                "parent-block-modified",
                sides,
                true,
            ));
        }
    }

    if inputs.is_empty() && old_raw != new_raw {
        inputs.push(input(&id, "format-only", &[SnapshotSide::Head], false));
    }
    Ok(inputs)
}

fn refinement_targets(document: &SpecDocument) -> BTreeSet<String> {
    match &document.type_fields {
        TypeSpecificFields::Requirement { refines, .. }
        | TypeSpecificFields::Task { refines, .. } => refines.iter().cloned().collect(),
        _ => BTreeSet::new(),
    }
}

fn block_map(document: &SpecDocument) -> BTreeMap<String, &TypedBlock> {
    document
        .blocks
        .iter()
        .map(|block| (block.id.clone(), block))
        .collect()
}

fn same_block(left: &TypedBlock, right: &TypedBlock) -> bool {
    left.kind == right.kind
        && left.id == right.id
        && left.level == right.level
        && left.body == right.body
        && left
            .clauses
            .iter()
            .map(|clause| (&clause.id, &clause.text))
            .eq(right
                .clauses
                .iter()
                .map(|clause| (&clause.id, &clause.text)))
}

fn outside_blocks(document: &SpecDocument) -> String {
    let mut block_starts = BTreeMap::new();
    for block in &document.blocks {
        let start = block.start_line.saturating_sub(document.body_start_line);
        let end = block.end_line.saturating_sub(document.body_start_line);
        block_starts.insert(start, (end, block));
    }
    let lines = document.body_raw.lines().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;
    while index < lines.len() {
        if let Some((end, block)) = block_starts.get(&index) {
            output.push_str(&format!("<{}:{}>\n", block.kind, block.id));
            index = end.saturating_add(1);
        } else {
            output.push_str(lines[index]);
            output.push('\n');
            index += 1;
        }
    }
    output
}

fn finish_report(
    specs_dir: &Path,
    mode: &str,
    base: Option<&str>,
    head: Option<&str>,
    inputs: Vec<ImpactInput>,
    snapshots: &[&Snapshot],
    history_head: Option<&str>,
) -> Result<ImpactReport> {
    let mut pending = BTreeMap::<String, PendingImpact>::new();
    for input in &inputs {
        for snapshot in snapshots {
            if !input.snapshots.contains(&snapshot.side) {
                continue;
            }
            cascade_input(snapshot, input, &mut pending);
        }
    }

    let affected_specs = finalize_specs(&pending, snapshots);
    let source_surfaces = collect_source_surfaces(&affected_specs, snapshots);
    let tasks = collect_tasks(&affected_specs, snapshots);
    let mut notes = Vec::new();
    if inputs.is_empty() {
        notes.push("No specification changes were found in the selected Git range.".to_string());
    }
    if inputs.iter().all(|item| !item.cascade) && !inputs.is_empty() {
        notes.push(
            "Only formatting-level changes were detected; no downstream cascade was inferred."
                .to_string(),
        );
    }

    let impacted_ids = affected_specs
        .iter()
        .map(|spec| spec.id.clone())
        .collect::<BTreeSet<_>>();
    let history = match history_head {
        Some(revision) => match collect_history(specs_dir, revision, &impacted_ids) {
            Ok(history) => history,
            Err(error) if mode == "subject" => {
                notes.push(format!("Git history evidence unavailable: {error}"));
                Vec::new()
            }
            Err(error) => return Err(error),
        },
        None => Vec::new(),
    };

    let gaps = coverage_gaps(
        &inputs,
        &affected_specs,
        &source_surfaces,
        &history,
        &tasks,
        snapshots,
    );
    let project = snapshots
        .iter()
        .rev()
        .find_map(|snapshot| snapshot.registry.project_id());
    let summary = summarize(
        &inputs,
        &affected_specs,
        &source_surfaces,
        &history,
        &tasks,
        &gaps,
    );

    Ok(ImpactReport {
        mode: mode.to_string(),
        project,
        base: base.map(str::to_string),
        head: head.map(str::to_string),
        summary,
        inputs,
        affected_specs,
        source_surfaces,
        history,
        tasks,
        gaps,
        notes,
    })
}

fn cascade_input(
    snapshot: &Snapshot,
    input: &ImpactInput,
    pending: &mut BTreeMap<String, PendingImpact>,
) {
    let root_id = document_id(&input.reference).to_string();
    let Some(root) = snapshot.document(&root_id) else {
        return;
    };
    merge_pending(
        pending,
        &root_id,
        0,
        format!("{} {}", input.change, input.reference),
        vec![input.reference.clone()],
        snapshot.side,
    );
    if !input.cascade {
        return;
    }

    if root.universal.entity_type == EntityType::Project {
        for document in &snapshot.registry.documents {
            let id = document.id_str();
            if id == root_id {
                continue;
            }
            merge_pending(
                pending,
                &id,
                1,
                format!("inherits project context from {root_id}"),
                vec![input.reference.clone(), id.clone()],
                snapshot.side,
            );
        }
        return;
    }

    let mut queue = VecDeque::new();
    queue.push_back((
        input.reference.clone(),
        0usize,
        vec![input.reference.clone()],
    ));
    let mut visited = BTreeSet::new();
    visited.insert(root_id);

    while let Some((parent_reference, depth, path)) = queue.pop_front() {
        for (child_id, matched_target) in
            direct_refining_children(&snapshot.registry, &parent_reference)
        {
            let mut child_path = path.clone();
            child_path.push(child_id.clone());
            merge_pending(
                pending,
                &child_id,
                depth + 1,
                format!("refines {matched_target}"),
                child_path.clone(),
                snapshot.side,
            );
            if visited.insert(child_id.clone()) {
                queue.push_back((child_id, depth + 1, child_path));
            }
        }
    }
}

fn merge_pending(
    pending: &mut BTreeMap<String, PendingImpact>,
    id: &str,
    depth: usize,
    reason: String,
    path: Vec<String>,
    side: SnapshotSide,
) {
    let entry = pending
        .entry(id.to_string())
        .or_insert_with(|| PendingImpact {
            depth,
            reason: reason.clone(),
            path: path.clone(),
            snapshots: BTreeSet::new(),
        });
    entry.snapshots.insert(side);
    if depth < entry.depth || (depth == entry.depth && path < entry.path) {
        entry.depth = depth;
        entry.reason = reason;
        entry.path = path;
    }
}

fn direct_refining_children(registry: &SpecRegistry, target: &str) -> Vec<(String, String)> {
    let allowed_targets = anchor_scope(registry, target);
    let target_document = document_id(target);
    let target_has_anchor = target.contains('#');
    let mut children = Vec::new();

    for document in &registry.documents {
        let refines: &[String] = match &document.type_fields {
            TypeSpecificFields::Requirement { refines, .. }
            | TypeSpecificFields::Task { refines, .. } => refines,
            _ => continue,
        };
        for refinement in refines {
            let (resolved, _) = registry.resolve_redirect(refinement);
            let matches = if target_has_anchor {
                allowed_targets.contains(&resolved)
            } else {
                document_id(&resolved) == target_document
            };
            if matches {
                children.push((document.id_str(), resolved));
            }
        }
    }
    children.sort();
    children.dedup();
    children
}

fn anchor_scope(registry: &SpecRegistry, target: &str) -> BTreeSet<String> {
    let mut scope = BTreeSet::new();
    let (resolved, _) = registry.resolve_redirect(target);
    scope.insert(resolved.clone());
    let Some((id, anchor)) = resolved.split_once('#') else {
        return scope;
    };
    let Some(document) = registry.get_by_id(id) else {
        return scope;
    };
    if let Some(block) = document.blocks.iter().find(|block| block.id == anchor) {
        for clause in &block.clauses {
            scope.insert(format!("{id}#{}", clause.id));
        }
    }
    scope
}

fn document_id(reference: &str) -> &str {
    reference
        .split_once('#')
        .map(|(id, _)| id)
        .unwrap_or(reference)
}

fn finalize_specs(
    pending: &BTreeMap<String, PendingImpact>,
    snapshots: &[&Snapshot],
) -> Vec<ImpactedSpec> {
    let mut specs = Vec::new();
    for (id, impact) in pending {
        let document = snapshots
            .iter()
            .rev()
            .find_map(|snapshot| snapshot.document(id));
        let Some(document) = document else {
            continue;
        };
        specs.push(ImpactedSpec {
            id: id.clone(),
            entity_type: document.universal.entity_type.type_name().to_string(),
            status: document.universal.status.as_str().to_string(),
            summary: document
                .universal
                .summary
                .as_deref()
                .map(str::trim)
                .map(str::to_string),
            depth: impact.depth,
            reason: impact.reason.clone(),
            path: impact.path.clone(),
            snapshots: impact.snapshots.clone(),
        });
    }
    specs.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| left.id.cmp(&right.id))
    });
    specs
}

fn collect_source_surfaces(
    affected: &[ImpactedSpec],
    snapshots: &[&Snapshot],
) -> Vec<SourceSurface> {
    let ids = affected
        .iter()
        .map(|spec| spec.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut surfaces = BTreeMap::<String, SourceSurface>::new();
    for snapshot in snapshots {
        for document in &snapshot.registry.documents {
            let id = document.id_str();
            if !ids.contains(id.as_str()) {
                continue;
            }
            for located in &document.references {
                let SpecReference::Source(source) = &located.reference else {
                    continue;
                };
                let reference = SpecReference::Source(source.clone()).to_string();
                let (target_kind, symbol, start_line, end_line) = match &source.target {
                    SourceTarget::File => ("file", None, None, None),
                    SourceTarget::Lines { start, end } => ("lines", None, Some(*start), Some(*end)),
                    SourceTarget::Symbol { segments } => {
                        ("symbol", Some(segments.join("/")), None, None)
                    }
                };
                let entry = surfaces
                    .entry(reference.clone())
                    .or_insert_with(|| SourceSurface {
                        reference,
                        path: source.path.clone(),
                        target_kind: target_kind.to_string(),
                        symbol,
                        start_line,
                        end_line,
                        specifications: BTreeSet::new(),
                        snapshots: BTreeSet::new(),
                        test_path: is_test_path(&source.path),
                    });
                entry.specifications.insert(id.clone());
                entry.snapshots.insert(snapshot.side);
            }
        }
    }
    surfaces.into_values().collect()
}

fn collect_tasks(affected: &[ImpactedSpec], snapshots: &[&Snapshot]) -> Vec<ImpactTask> {
    let mut tasks = Vec::new();
    for impacted in affected {
        let selected = snapshots.iter().rev().find_map(|snapshot| {
            let document = snapshot.document(&impacted.id)?;
            let TypeSpecificFields::Task { progress, .. } = &document.type_fields else {
                return None;
            };
            Some((*snapshot, document, *progress))
        });
        let Some((snapshot, document, progress)) = selected else {
            continue;
        };
        tasks.push(ImpactTask {
            id: impacted.id.clone(),
            progress: progress.as_str().to_string(),
            summary: document
                .universal
                .summary
                .as_deref()
                .map(str::trim)
                .map(str::to_string),
            depth: impacted.depth,
            snapshot: snapshot.side,
        });
    }
    tasks.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| left.id.cmp(&right.id))
    });
    tasks
}

fn collect_history(
    specs_dir: &Path,
    revision: &str,
    impacted_ids: &BTreeSet<String>,
) -> Result<Vec<HistoryEvidence>> {
    if impacted_ids.is_empty() {
        return Ok(Vec::new());
    }
    let repository = Repository::discover(specs_dir)?;
    let events = trailers::walk_trailers_from(specs_dir, Some(revision))?;
    let mut files_by_commit = BTreeMap::<String, Vec<String>>::new();
    let mut evidence = Vec::new();
    for event in events {
        if !impacted_ids.contains(document_id(&event.spec_ref)) {
            continue;
        }
        let files = if let Some(files) = files_by_commit.get(&event.full_sha) {
            files.clone()
        } else {
            let files = changed_files_for_commit(&repository, &event.full_sha)?
                .into_iter()
                .filter(|path| is_implementation_path(path))
                .collect::<Vec<_>>();
            files_by_commit.insert(event.full_sha.clone(), files.clone());
            files
        };
        evidence.push(HistoryEvidence {
            spec_ref: event.spec_ref,
            kind: event.kind,
            commit: event.sha,
            files,
        });
    }
    evidence.sort_by(|left, right| {
        left.spec_ref
            .cmp(&right.spec_ref)
            .then_with(|| left.commit.cmp(&right.commit))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    Ok(evidence)
}

fn changed_files_for_commit(repository: &Repository, oid: &str) -> Result<Vec<String>> {
    let object = repository.revparse_single(oid)?;
    let commit = object.peel_to_commit()?;
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };
    let diff = repository.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
    let mut files = BTreeSet::new();
    for delta in diff.deltas() {
        if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
            files.insert(path.to_string_lossy().into_owned());
        }
    }
    Ok(files.into_iter().collect())
}

fn coverage_gaps(
    inputs: &[ImpactInput],
    affected: &[ImpactedSpec],
    sources: &[SourceSurface],
    history: &[HistoryEvidence],
    tasks: &[ImpactTask],
    snapshots: &[&Snapshot],
) -> Vec<String> {
    if inputs.is_empty() || inputs.iter().all(|item| !item.cascade) {
        return Vec::new();
    }
    let mut gaps = BTreeSet::new();
    let affected_ids = affected
        .iter()
        .map(|spec| spec.id.clone())
        .collect::<BTreeSet<_>>();
    let mut parents = BTreeSet::new();
    for snapshot in snapshots {
        for parent_id in &affected_ids {
            if direct_refining_children(&snapshot.registry, parent_id)
                .iter()
                .any(|(child_id, _)| affected_ids.contains(child_id))
            {
                parents.insert(parent_id.clone());
            }
        }
        if let Some(project_id) = snapshot.registry.project_id() {
            if affected_ids.contains(&project_id) && affected_ids.len() > 1 {
                parents.insert(project_id);
            }
        }
    }
    for leaf in affected.iter().filter(|spec| !parents.contains(&spec.id)) {
        if leaf.entity_type != "requirement" && leaf.entity_type != "task" {
            continue;
        }
        let explicit = sources
            .iter()
            .any(|surface| surface.specifications.contains(&leaf.id));
        let historical = history
            .iter()
            .any(|event| document_id(&event.spec_ref) == leaf.id && event.kind == "implements");
        if !explicit && !historical {
            gaps.insert(format!(
                "{} has no explicit source reference or historical implements evidence",
                leaf.id
            ));
        }
    }
    if tasks.is_empty() {
        gaps.insert("No implementation TASK is attached to the impact closure".to_string());
    }
    let has_test = sources.iter().any(|surface| surface.test_path)
        || history.iter().any(|event| {
            event.kind == "tests" || event.files.iter().any(|path| is_test_path(path))
        });
    if !has_test {
        gaps.insert(
            "No explicit or historical test evidence covers the impact closure".to_string(),
        );
    }
    for input in inputs.iter().filter(|item| item.reference.contains('#')) {
        if affected.len() == 1 {
            gaps.insert(format!(
                "No refining specification or task targets {}",
                input.reference
            ));
        }
    }
    gaps.into_iter().collect()
}

fn summarize(
    inputs: &[ImpactInput],
    affected: &[ImpactedSpec],
    sources: &[SourceSurface],
    history: &[HistoryEvidence],
    tasks: &[ImpactTask],
    gaps: &[String],
) -> ImpactSummary {
    let implementation_files = history
        .iter()
        .filter(|event| event.kind == "implements")
        .flat_map(|event| event.files.iter())
        .filter(|path| !is_test_path(path))
        .collect::<BTreeSet<_>>()
        .len();
    let historical_test_files = history
        .iter()
        .flat_map(|event| event.files.iter())
        .filter(|path| is_test_path(path))
        .cloned()
        .collect::<BTreeSet<_>>();
    let explicit_test_files = sources
        .iter()
        .filter(|surface| surface.test_path)
        .map(|surface| surface.path.clone())
        .collect::<BTreeSet<_>>();
    let test_files = historical_test_files
        .into_iter()
        .chain(explicit_test_files)
        .collect::<BTreeSet<_>>()
        .len();

    ImpactSummary {
        changed_inputs: inputs.len(),
        affected_specs: affected.len(),
        requirements: affected
            .iter()
            .filter(|spec| spec.entity_type == "requirement")
            .count(),
        tasks: tasks.len(),
        explicit_source_references: sources.len(),
        historical_events: history.len(),
        implementation_files,
        test_files,
        max_depth: affected.iter().map(|spec| spec.depth).max().unwrap_or(0),
        coverage_gaps: gaps.len(),
    }
}

fn is_spec_or_history_path(path: &str) -> bool {
    path.ends_with(".spec.md") || path.contains("/_history/")
}

fn is_implementation_path(path: &str) -> bool {
    if is_spec_or_history_path(path) {
        return false;
    }
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "py"
            | "sql"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "sh"
            | "toml"
            | "json"
            | "yaml"
            | "yml"
    )
}

fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/test/")
        || lower.contains("/tests/")
        || lower.contains("/__tests__/")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_test.py")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".spec.tsx")
}

pub fn render_human(report: &ImpactReport) -> String {
    let mut output = String::new();
    output.push_str("# Impact analysis\n\n");
    output.push_str(&format!("Mode: {}\n", report.mode));
    if let (Some(base), Some(head)) = (&report.base, &report.head) {
        output.push_str(&format!("Range: {base}..{head}\n"));
    }
    if let Some(project) = &report.project {
        output.push_str(&format!("Project: {project}\n"));
    }
    output.push_str(&format!(
        "Summary: {} changed input(s), {} affected spec(s), {} explicit source reference(s), {} historical event(s), {} implementation file(s), {} test file(s), {} task(s), {} gap(s), max depth {}.\n",
        report.summary.changed_inputs,
        report.summary.affected_specs,
        report.summary.explicit_source_references,
        report.summary.historical_events,
        report.summary.implementation_files,
        report.summary.test_files,
        report.summary.tasks,
        report.summary.coverage_gaps,
        report.summary.max_depth,
    ));

    output.push_str("\n## Changed inputs\n\n");
    if report.inputs.is_empty() {
        output.push_str("- None.\n");
    } else {
        for input in &report.inputs {
            output.push_str(&format!(
                "- `{}` — {} [{}]{}\n",
                input.reference,
                input.change,
                side_list(&input.snapshots),
                if input.cascade {
                    ""
                } else {
                    " (no semantic cascade)"
                },
            ));
        }
    }

    output.push_str("\n## Affected specifications\n\n");
    if report.affected_specs.is_empty() {
        output.push_str("- None.\n");
    } else {
        for spec in &report.affected_specs {
            output.push_str(&format!(
                "- depth {} `{}` ({}, {}) — {}\n  path: {}\n",
                spec.depth,
                spec.id,
                spec.entity_type,
                spec.status,
                spec.reason,
                spec.path.join(" -> "),
            ));
            if let Some(summary) = &spec.summary {
                output.push_str(&format!("  summary: {summary}\n"));
            }
        }
    }

    output.push_str("\n## Implementation surfaces\n\n");
    if report.source_surfaces.is_empty() {
        output.push_str("- No explicit `spec:src:` references.\n");
    } else {
        output.push_str("### Explicit source references\n\n");
        for surface in &report.source_surfaces {
            output.push_str(&format!(
                "- `{}` <- {} [{}]\n",
                surface.reference,
                surface
                    .specifications
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                side_list(&surface.snapshots),
            ));
        }
    }
    if !report.history.is_empty() {
        output.push_str("\n### Historical Git evidence\n\n");
        for event in &report.history {
            output.push_str(&format!(
                "- `{}` {} at `{}` — {}\n",
                event.spec_ref,
                event.kind,
                event.commit,
                if event.files.is_empty() {
                    "no changed paths".to_string()
                } else {
                    event.files.join(", ")
                },
            ));
        }
    }

    output.push_str("\n## Tasks\n\n");
    if report.tasks.is_empty() {
        output.push_str("- None attached to the impact closure.\n");
    } else {
        for task in &report.tasks {
            output.push_str(&format!(
                "- `{}` — {} (depth {}, {})\n",
                task.id,
                task.progress,
                task.depth,
                task.snapshot.as_str(),
            ));
        }
    }

    output.push_str("\n## Coverage gaps\n\n");
    if report.gaps.is_empty() {
        output.push_str("- No evidence gaps detected.\n");
    } else {
        for gap in &report.gaps {
            output.push_str(&format!("- {gap}\n"));
        }
    }
    if !report.notes.is_empty() {
        output.push_str("\n## Notes\n\n");
        for note in &report.notes {
            output.push_str(&format!("- {note}\n"));
        }
    }
    output.push_str("\n## Agent handoff\n\n");
    output.push_str("1. Review every affected path and evidence gap; impact is explicit or historically inferred, not a code-dependency proof.\n");
    output.push_str("2. Render affected specs with `spec render <id> --target agent --include-source` before editing.\n");
    output.push_str("3. Create missing TASK specs deliberately, then use `spec task start <task-id>` when implementation begins.\n");
    output.push_str(
        "4. Validate with `spec lint --require-symbols` and the affected project tests.\n",
    );
    output
}

pub fn render_agent(report: &ImpactReport) -> String {
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str(&format!(
        "<forge-spec-impact schema-version=\"1\" mode=\"{}\"",
        escape_xml(&report.mode)
    ));
    if let Some(project) = &report.project {
        output.push_str(&format!(" project=\"{}\"", escape_xml(project)));
    }
    if let Some(base) = &report.base {
        output.push_str(&format!(" base=\"{}\"", escape_xml(base)));
    }
    if let Some(head) = &report.head {
        output.push_str(&format!(" head=\"{}\"", escape_xml(head)));
    }
    output.push_str(">\n");
    output.push_str(&format!(
        "  <summary changed-inputs=\"{}\" affected-specs=\"{}\" requirements=\"{}\" tasks=\"{}\" explicit-source-references=\"{}\" historical-events=\"{}\" implementation-files=\"{}\" test-files=\"{}\" max-depth=\"{}\" coverage-gaps=\"{}\" />\n",
        report.summary.changed_inputs,
        report.summary.affected_specs,
        report.summary.requirements,
        report.summary.tasks,
        report.summary.explicit_source_references,
        report.summary.historical_events,
        report.summary.implementation_files,
        report.summary.test_files,
        report.summary.max_depth,
        report.summary.coverage_gaps,
    ));
    output.push_str("  <inputs>\n");
    for input in &report.inputs {
        output.push_str(&format!(
            "    <input reference=\"{}\" change=\"{}\" snapshots=\"{}\" cascade=\"{}\" />\n",
            escape_xml(&input.reference),
            escape_xml(&input.change),
            side_list(&input.snapshots),
            input.cascade,
        ));
    }
    output.push_str("  </inputs>\n  <affected-specs>\n");
    for spec in &report.affected_specs {
        output.push_str(&format!(
            "    <spec id=\"{}\" type=\"{}\" status=\"{}\" depth=\"{}\" snapshots=\"{}\" reason=\"{}\">\n",
            escape_xml(&spec.id),
            escape_xml(&spec.entity_type),
            escape_xml(&spec.status),
            spec.depth,
            side_list(&spec.snapshots),
            escape_xml(&spec.reason),
        ));
        if let Some(summary) = &spec.summary {
            output.push_str(&format!(
                "      <description>{}</description>\n",
                escape_xml(summary)
            ));
        }
        output.push_str("      <path>\n");
        for (depth, reference) in spec.path.iter().enumerate() {
            output.push_str(&format!(
                "        <step depth=\"{}\" reference=\"{}\" />\n",
                depth,
                escape_xml(reference),
            ));
        }
        output.push_str("      </path>\n    </spec>\n");
    }
    output.push_str("  </affected-specs>\n  <implementation-surfaces>\n");
    for surface in &report.source_surfaces {
        output.push_str(&format!(
            "    <source reference=\"{}\" path=\"{}\" target=\"{}\" snapshots=\"{}\" test=\"{}\">\n",
            escape_xml(&surface.reference),
            escape_xml(&surface.path),
            escape_xml(&surface.target_kind),
            side_list(&surface.snapshots),
            surface.test_path,
        ));
        if let Some(symbol) = &surface.symbol {
            output.push_str(&format!("      <symbol>{}</symbol>\n", escape_xml(symbol)));
        }
        for id in &surface.specifications {
            output.push_str(&format!(
                "      <declared-by id=\"{}\" />\n",
                escape_xml(id)
            ));
        }
        output.push_str("    </source>\n");
    }
    output.push_str("  </implementation-surfaces>\n  <history>\n");
    for event in &report.history {
        output.push_str(&format!(
            "    <event spec-ref=\"{}\" kind=\"{}\" commit=\"{}\">\n",
            escape_xml(&event.spec_ref),
            escape_xml(&event.kind),
            escape_xml(&event.commit),
        ));
        for path in &event.files {
            output.push_str(&format!("      <file path=\"{}\" />\n", escape_xml(path)));
        }
        output.push_str("    </event>\n");
    }
    output.push_str("  </history>\n  <tasks>\n");
    for task in &report.tasks {
        output.push_str(&format!(
            "    <task id=\"{}\" progress=\"{}\" depth=\"{}\" snapshot=\"{}\" />\n",
            escape_xml(&task.id),
            escape_xml(&task.progress),
            task.depth,
            task.snapshot.as_str(),
        ));
    }
    output.push_str("  </tasks>\n  <coverage-gaps>\n");
    for gap in &report.gaps {
        output.push_str(&format!("    <gap>{}</gap>\n", escape_xml(gap)));
    }
    output.push_str("  </coverage-gaps>\n  <notes>\n");
    for note in &report.notes {
        output.push_str(&format!("    <note>{}</note>\n", escape_xml(note)));
    }
    output.push_str("  </notes>\n  <agent-handoff>\n");
    output.push_str("    <instruction>Review every affected path and evidence gap; this report is not a code-dependency proof.</instruction>\n");
    output.push_str("    <instruction>Render affected specifications with spec render ID --target agent --include-source before editing.</instruction>\n");
    output.push_str("    <instruction>Create missing TASK specifications deliberately and run spec task start TASK-ID when implementation begins.</instruction>\n");
    output.push_str("    <validation>spec lint --require-symbols</validation>\n");
    output.push_str("  </agent-handoff>\n</forge-spec-impact>\n");
    output
}

fn side_list(sides: &BTreeSet<SnapshotSide>) -> String {
    sides
        .iter()
        .map(|side| side.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn project() -> &'static str {
        "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n"
    }

    fn requirement(id: &str, refines: &str, body: &str) -> String {
        format!(
            "---\nid: {id}\ntype: requirement\nstatus: accepted\nsummary: {id}.\nowners: [dev]\nlevel: MUST\nrefines: {refines}\n---\n\n# {id}\n\n{body}\n"
        )
    }

    fn task(id: &str, refines: &str, source: &str) -> String {
        format!(
            "---\nid: {id}\ntype: task\nstatus: accepted\nsummary: Implement it.\nowners: [dev]\nprogress: pending\nrefines: [{refines}]\n---\n\n# Task\n\n[implementation](spec:src:{source})\n"
        )
    }

    fn fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let specs = temp.path().join(".specs");
        write(
            &specs.join("_config.toml"),
            "baseline = \"forge-spec-v0.3.0\"\nproject = \"PROJECT:demo\"\n",
        );
        write(&specs.join("_project.spec.md"), project());
        write(
            &specs.join("root.spec.md"),
            &requirement(
                "REQ:demo/root",
                "[]",
                ":::{requirement id=\"behavior\" level=\"MUST\"}\n- {#c-one} first behavior\n:::",
            ),
        );
        write(
            &specs.join("child.spec.md"),
            &requirement(
                "REQ:demo/child",
                "[REQ:demo/root#c-one]",
                ":::{requirement id=\"detail\" level=\"MUST\"}\nDetailed behavior.\n:::",
            ),
        );
        write(
            &specs.join("task.spec.md"),
            &task(
                "TASK:demo/implement",
                "REQ:demo/child#detail",
                "src/feature.rs#symbol=Feature/run",
            ),
        );
        temp
    }

    fn commit_all(repository: &Repository, message: &str) -> git2::Oid {
        let mut index = repository.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repository.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repository
            .head()
            .ok()
            .and_then(|head| head.target())
            .and_then(|oid| repository.find_commit(oid).ok());
        let parents = parent.iter().collect::<Vec<_>>();
        repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .unwrap()
    }

    #[test]
    fn validates_request_modes() {
        assert!(ImpactRequest::new(None, None, None).is_err());
        assert!(ImpactRequest::new(Some("REQ:demo/root"), Some("HEAD"), None).is_err());
        assert_eq!(
            ImpactRequest::new(None, Some("HEAD"), None).unwrap(),
            ImpactRequest::Diff {
                base: "HEAD".into(),
                head: WORKING_TREE.into(),
            }
        );
    }

    #[test]
    fn changed_refinement_records_old_and_new_parent_impact_roots() {
        let old_content = requirement(
            "REQ:demo/child",
            "[REQ:demo/old#behavior]",
            "Child behavior.",
        );
        let new_content = requirement(
            "REQ:demo/child",
            "[REQ:demo/new#behavior]",
            "Child behavior.",
        );
        let old = crate::parse::parse_content(Path::new("old.spec.md"), &old_content).unwrap();
        let new = crate::parse::parse_content(Path::new("new.spec.md"), &new_content).unwrap();

        let inputs = modified_inputs(&old, &new, &old_content, &new_content).unwrap();

        assert!(inputs.iter().any(|input| {
            input.reference == "REQ:demo/old#behavior"
                && input.change == "refinement-removed"
                && input.snapshots == BTreeSet::from([SnapshotSide::Base])
        }));
        assert!(inputs.iter().any(|input| {
            input.reference == "REQ:demo/new#behavior"
                && input.change == "refinement-added"
                && input.snapshots == BTreeSet::from([SnapshotSide::Head])
        }));
    }

    #[test]
    fn parsed_format_only_change_does_not_cascade() {
        let old_content = requirement("REQ:demo/root", "[]", "Behavior.");
        let new_content = old_content.replace("owners: [dev]", "owners: [ dev ]");
        let old = crate::parse::parse_content(Path::new("old.spec.md"), &old_content).unwrap();
        let new = crate::parse::parse_content(Path::new("new.spec.md"), &new_content).unwrap();

        let inputs = modified_inputs(&old, &new, &old_content, &new_content).unwrap();

        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].change, "format-only");
        assert!(!inputs[0].cascade);
    }

    #[test]
    fn project_selection_includes_all_ambient_context_consumers() {
        let temp = fixture();
        let report = analyze_subject(&temp.path().join(".specs"), "PROJECT:demo").unwrap();

        assert_eq!(report.summary.affected_specs, 4);
        assert_eq!(report.affected_specs[0].id, "PROJECT:demo");
        assert!(report
            .affected_specs
            .iter()
            .skip(1)
            .all(|spec| spec.depth == 1));
    }

    #[test]
    fn selected_clause_cascades_transitively_to_code_and_tasks() {
        let temp = fixture();
        let report = analyze_subject(&temp.path().join(".specs"), "REQ:demo/root#c-one").unwrap();

        assert_eq!(
            report
                .affected_specs
                .iter()
                .map(|spec| (&spec.id, spec.depth))
                .collect::<Vec<_>>(),
            vec![
                (&"REQ:demo/root".to_string(), 0),
                (&"REQ:demo/child".to_string(), 1),
                (&"TASK:demo/implement".to_string(), 2),
            ]
        );
        assert_eq!(report.tasks.len(), 1);
        assert_eq!(report.source_surfaces.len(), 1);
        assert_eq!(
            report.source_surfaces[0].symbol.as_deref(),
            Some("Feature/run")
        );
    }

    #[test]
    fn unrelated_clause_does_not_cascade() {
        let temp = fixture();
        let report =
            analyze_subject(&temp.path().join(".specs"), "REQ:demo/root#behavior").unwrap();

        // Selecting a typed block includes its nested clauses, hence the child
        // refining c-one is intentionally affected.
        assert_eq!(report.affected_specs.len(), 3);

        let report = analyze_subject(&temp.path().join(".specs"), "REQ:demo/child#detail").unwrap();
        assert_eq!(report.affected_specs.len(), 2);
        assert_eq!(report.affected_specs[1].id, "TASK:demo/implement");
    }

    #[test]
    fn agent_report_is_structured_and_deterministic() {
        let temp = fixture();
        let report = analyze_subject(&temp.path().join(".specs"), "REQ:demo/root#c-one").unwrap();
        let xml = render_agent(&report);

        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<forge-spec-impact schema-version=\"1\" mode=\"subject\""));
        assert!(xml.contains("<source reference=\"spec:src:src/feature.rs#symbol=Feature/run\""));
        assert!(xml.contains("<task id=\"TASK:demo/implement\" progress=\"pending\""));
    }

    #[test]
    fn git_diff_detects_changed_clause_and_uses_the_working_tree_graph() {
        let temp = fixture();
        let repository = Repository::init(temp.path()).unwrap();
        commit_all(&repository, "Base");

        let root_path = temp.path().join(".specs/root.spec.md");
        let changed = std::fs::read_to_string(&root_path)
            .unwrap()
            .replace("first behavior", "changed behavior");
        std::fs::write(&root_path, changed).unwrap();

        let report = analyze_diff(&temp.path().join(".specs"), "HEAD", WORKING_TREE).unwrap();

        assert!(report.inputs.iter().any(|input| {
            input.reference == "REQ:demo/root#c-one" && input.change == "parent-block-modified"
        }));
        assert_eq!(report.summary.affected_specs, 3);
        assert_eq!(report.affected_specs[2].id, "TASK:demo/implement");
        assert_eq!(report.base.as_deref(), Some("HEAD"));
        assert_eq!(report.head.as_deref(), Some(WORKING_TREE));
    }

    #[test]
    fn git_diff_keeps_deleted_spec_relationships_from_the_base_graph() {
        let temp = fixture();
        let repository = Repository::init(temp.path()).unwrap();
        commit_all(&repository, "Base");
        std::fs::remove_file(temp.path().join(".specs/child.spec.md")).unwrap();

        let report = analyze_diff(&temp.path().join(".specs"), "HEAD", WORKING_TREE).unwrap();

        assert!(report.inputs.iter().any(|input| {
            input.reference == "REQ:demo/child"
                && input.change == "removed"
                && input.snapshots == BTreeSet::from([SnapshotSide::Base])
        }));
        assert_eq!(report.summary.affected_specs, 2);
        assert_eq!(report.affected_specs[0].id, "REQ:demo/child");
        assert_eq!(report.affected_specs[1].id, "TASK:demo/implement");
        assert_eq!(
            report.affected_specs[1].snapshots,
            BTreeSet::from([SnapshotSide::Base])
        );
    }
}
