//! Renderer-independent architectural subjects and intended scenario behavior.
use super::{id::EntityType, registry::SpecRegistry};
use crate::lint::diagnostic::Diagnostic;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFacet {
    #[serde(default)]
    pub subjects: Vec<Subject>,
    #[serde(default)]
    pub interactions: Vec<Interaction>,
    #[serde(default)]
    pub about: Vec<String>,
}
impl ModelFacet {
    pub fn normalized(mut self) -> Self {
        self.subjects.sort_by(|a, b| a.id.cmp(&b.id));
        for s in &mut self.subjects {
            s.part_of.sort();
            s.part_of.dedup();
        }
        self.interactions.sort_by(|a, b| a.id.cmp(&b.id));
        for i in &mut self.interactions {
            i.governed_by.sort();
            i.governed_by.dedup();
        }
        self.about.sort();
        self.about.dedup();
        self
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub part_of: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    pub id: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub title: String,
    #[serde(default)]
    pub governed_by: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioFlow {
    pub participants: Vec<String>,
    pub steps: Vec<FlowStep>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FlowStep {
    Interaction {
        id: String,
        from: String,
        to: String,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interaction: Option<String>,
        #[serde(default)]
        refs: Vec<String>,
    },
    Parallel {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        steps: Vec<FlowStep>,
    },
    Alternatives {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        branches: Vec<FlowBranch>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowBranch {
    pub id: String,
    pub title: String,
    pub steps: Vec<FlowStep>,
}
impl FlowStep {
    pub fn id(&self) -> &str {
        match self {
            Self::Interaction { id, .. }
            | Self::Parallel { id, .. }
            | Self::Alternatives { id, .. } => id,
        }
    }
    pub fn anchors(&self, result: &mut Vec<String>) {
        result.push(self.id().into());
        match self {
            Self::Interaction { .. } => {}
            Self::Parallel { steps, .. } => {
                for s in steps {
                    s.anchors(result)
                }
            }
            Self::Alternatives { branches, .. } => {
                for b in branches {
                    result.push(b.id.clone());
                    for s in &b.steps {
                        s.anchors(result)
                    }
                }
            }
        }
    }
}
impl ScenarioFlow {
    pub fn anchors(&self, result: &mut Vec<String>) {
        for s in &self.steps {
            s.anchors(result)
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedModel {
    pub subjects: Vec<ProjectedSubject>,
    pub interactions: Vec<ProjectedInteraction>,
    pub about: Vec<SubjectAbout>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedSubject {
    pub id: String,
    pub owner: String,
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub part_of: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectedInteraction {
    pub id: String,
    pub owner: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub title: String,
    pub governed_by: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SubjectAbout {
    pub source: String,
    pub target: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedScenario {
    pub id: String,
    pub participants: Vec<String>,
    pub steps: Vec<FlowStep>,
}
pub fn project_model(registry: &SpecRegistry) -> ProjectedModel {
    let mut result = ProjectedModel::default();
    for doc in &registry.documents {
        if doc.universal.entity_type == EntityType::Task {
            continue;
        }
        let Some(model) = &doc.universal.model else {
            continue;
        };
        let owner = doc.id_str();
        let model = model.clone().normalized();
        result
            .subjects
            .extend(model.subjects.into_iter().map(|s| ProjectedSubject {
                id: format!("{owner}#{}", s.id),
                owner: owner.clone(),
                kind: s.kind,
                title: s.title,
                description: s.description,
                part_of: s.part_of,
            }));
        result.interactions.extend(
            model
                .interactions
                .into_iter()
                .map(|i| ProjectedInteraction {
                    id: format!("{owner}#{}", i.id),
                    owner: owner.clone(),
                    kind: i.kind,
                    from: i.from,
                    to: i.to,
                    title: i.title,
                    governed_by: i.governed_by,
                }),
        );
        result
            .about
            .extend(model.about.into_iter().map(|target| SubjectAbout {
                source: owner.clone(),
                target,
            }));
    }
    result.subjects.sort();
    result.interactions.sort();
    result.about.sort();
    result
}
pub fn project_scenarios(registry: &SpecRegistry) -> Vec<ProjectedScenario> {
    let mut result = registry
        .documents
        .iter()
        .filter(|d| d.universal.entity_type == EntityType::Scn)
        .filter_map(|d| {
            d.universal.flow.as_ref().map(|f| ProjectedScenario {
                id: d.id_str(),
                participants: f.participants.clone(),
                steps: f.steps.clone(),
            })
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result
}
fn valid_local(id: &str) -> bool {
    !id.is_empty()
        && id.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}
fn durable_ref(registry: &SpecRegistry, reference: &str) -> bool {
    let id = reference.split('#').next().unwrap_or(reference);
    registry
        .get_by_id(id)
        .is_some_and(|d| d.universal.entity_type != EntityType::Task)
        && (!reference.contains('#') || registry.get_by_anchor(reference).is_some())
}
pub fn validate(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let model = project_model(registry);
    let subjects = model
        .subjects
        .iter()
        .map(|s| s.id.as_str())
        .collect::<BTreeSet<_>>();
    let interactions = model
        .interactions
        .iter()
        .map(|i| (i.id.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    let mut diags = Vec::new();
    for doc in &registry.documents {
        let mut errors = Vec::new();
        if doc.universal.entity_type == EntityType::Task && doc.universal.model.is_some() {
            errors.push("TASK cannot own architectural declarations or about relationships".into());
        }
        if doc.universal.flow.is_some() && doc.universal.entity_type != EntityType::Scn {
            errors.push("flow is only valid on SCN".into());
        }
        let mut model_anchors = Vec::new();
        if let Some(m) = &doc.universal.model {
            for s in &m.subjects {
                model_anchors.push(s.id.clone());
                if s.kind.trim().is_empty() || s.title.trim().is_empty() {
                    errors.push(format!(
                        "subject '{}' requires nonempty kind and title",
                        s.id
                    ));
                }
                for p in &s.part_of {
                    if !subjects.contains(p.as_str()) {
                        errors.push(format!("unknown composition subject '{p}'"));
                    }
                }
            }
            for i in &m.interactions {
                model_anchors.push(i.id.clone());
                if i.kind.trim().is_empty() || i.title.trim().is_empty() {
                    errors.push(format!(
                        "interaction '{}' requires nonempty kind and title",
                        i.id
                    ));
                }
                for p in [&i.from, &i.to] {
                    if !subjects.contains(p.as_str()) {
                        errors.push(format!("unknown interaction endpoint '{p}'"));
                    }
                }
                for r in &i.governed_by {
                    if !durable_ref(registry, r) {
                        errors.push(format!("unresolved durable governing reference '{r}'"));
                    }
                }
            }
            for r in &m.about {
                if !subjects.contains(r.as_str()) {
                    errors.push(format!("unknown about subject '{r}'"));
                }
            }
        }
        if let Some(f) = &doc.universal.flow {
            f.anchors(&mut model_anchors);
            let participants = f
                .participants
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if participants.len() != f.participants.len() {
                errors.push("scenario participants must be unique".into());
            }
            for p in &f.participants {
                if !subjects.contains(p.as_str()) {
                    errors.push(format!("unknown scenario participant '{p}'"));
                }
            }
            validate_steps(
                &f.steps,
                &participants,
                &interactions,
                registry,
                &mut errors,
                0,
            );
        }
        let mut seen = BTreeSet::new();
        for id in doc.anchors() {
            if !seen.insert(id.clone()) && model_anchors.contains(&id) {
                errors.push(format!("duplicate model/flow anchor '{id}'"));
            }
        }
        for id in model_anchors {
            if !valid_local(&id) {
                errors.push(format!("invalid model/flow local anchor '{id}'"));
            }
        }
        for message in errors {
            diags.push(Diagnostic::error("R034", message, doc.source_path.clone()));
        }
    }
    let parents = model
        .subjects
        .iter()
        .map(|s| (s.id.as_str(), &s.part_of))
        .collect::<BTreeMap<_, _>>();
    for subject in &model.subjects {
        let mut pending = subject
            .part_of
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        while let Some(next) = pending.pop() {
            if next == subject.id {
                let path = registry
                    .get_by_id(&subject.owner)
                    .unwrap()
                    .source_path
                    .clone();
                diags.push(Diagnostic::error(
                    "R035",
                    format!("composition cycle involving '{}'", subject.id),
                    path,
                ));
                break;
            }
            if seen.insert(next) {
                if let Some(ps) = parents.get(next) {
                    pending.extend(ps.iter().map(String::as_str));
                }
            }
        }
    }
    diags
}
fn validate_steps(
    steps: &[FlowStep],
    participants: &BTreeSet<&str>,
    interactions: &BTreeMap<&str, &ProjectedInteraction>,
    registry: &SpecRegistry,
    errors: &mut Vec<String>,
    depth: usize,
) {
    if depth > 32 {
        errors.push("scenario nesting exceeds 32 levels".into());
        return;
    }
    if steps.is_empty() {
        errors.push("scenario step groups must not be empty".into());
    }
    for step in steps {
        match step {
            FlowStep::Interaction {
                id,
                from,
                to,
                title,
                interaction,
                refs,
            } => {
                if title.trim().is_empty() {
                    errors.push(format!("step '{id}' requires a title"));
                }
                for p in [from, to] {
                    if !participants.contains(p.as_str()) {
                        errors.push(format!(
                            "step '{id}' endpoint '{p}' is not a declared participant"
                        ));
                    }
                }
                if let Some(r) = interaction {
                    match interactions.get(r.as_str()) {
                        Some(i) if i.from == *from && i.to == *to => {}
                        Some(_) => errors.push(format!(
                            "step '{id}' endpoints differ from interaction '{r}'"
                        )),
                        None => errors.push(format!("unresolved interaction '{r}'")),
                    }
                }
                for r in refs {
                    if !durable_ref(registry, r) {
                        errors.push(format!("unresolved step reference '{r}'"));
                    }
                }
            }
            FlowStep::Parallel { steps, .. } => validate_steps(
                steps,
                participants,
                interactions,
                registry,
                errors,
                depth + 1,
            ),
            FlowStep::Alternatives { branches, .. } => {
                if branches.len() < 2 {
                    errors.push("alternatives requires at least two branches".into());
                }
                for b in branches {
                    if b.title.trim().is_empty() {
                        errors.push(format!("branch '{}' requires a title", b.id));
                    }
                    validate_steps(
                        &b.steps,
                        participants,
                        interactions,
                        registry,
                        errors,
                        depth + 1,
                    );
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewsFile {
    pub schema: String,
    #[serde(default)]
    pub views: Vec<View>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub id: String,
    pub title: String,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}
pub fn parse_views(bytes: Option<&[u8]>, registry: &SpecRegistry) -> (Vec<View>, Vec<Diagnostic>) {
    let Some(bytes) = bytes else {
        return (Vec::new(), Vec::new());
    };
    let parsed = std::str::from_utf8(bytes)
        .map_err(|e| e.to_string())
        .and_then(|s| toml::from_str::<ViewsFile>(s).map_err(|e| e.to_string()));
    let mut errors = Vec::new();
    let mut views = Vec::new();
    match parsed {
        Err(e) => errors.push(format!("invalid views: {e}")),
        Ok(file) => {
            if file.schema != "forge-spec-views/v1" {
                errors.push(format!("unsupported views schema '{}'", file.schema));
            }
            let mut ids = BTreeSet::new();
            for v in &file.views {
                if !valid_local(&v.id) || !ids.insert(v.id.clone()) {
                    errors.push(format!("invalid or duplicate view id '{}'", v.id));
                }
                if v.title.trim().is_empty()
                    || !["map", "architecture", "scenarios", "work"].contains(&v.mode.as_str())
                {
                    errors.push(format!("invalid title or mode for view '{}'", v.id));
                }
                if v.depth.is_some_and(|d| d > 32) {
                    errors.push(format!("view '{}' depth exceeds 32", v.id));
                }
                if v.profile
                    .as_deref()
                    .is_some_and(|p| p != "c4" && p != "generic")
                {
                    errors.push(format!("unsupported presentation profile for '{}'", v.id));
                }
                for r in v
                    .focus
                    .iter()
                    .chain(v.include.iter())
                    .chain(v.exclude.iter())
                {
                    if registry.get_by_id(r).is_none() && registry.get_by_anchor(r).is_none() {
                        errors.push(format!("unresolved view selector '{r}'"));
                    }
                }
            }
            views = file.views;
            views.sort_by(|a, b| a.id.cmp(&b.id));
        }
    }
    (
        views,
        errors
            .into_iter()
            .map(|e| Diagnostic::error("R036", e, PathBuf::from(".specs/_views.toml")))
            .collect(),
    )
}
pub fn validate_saved_views(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let path = registry.specs_dir.join("_views.toml");
    match std::fs::read(&path) {
        Ok(b) => parse_views(Some(&b), registry).1,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => vec![Diagnostic::error(
            "R036",
            format!("reading views: {e}"),
            path,
        )],
    }
}

impl ModelFacet {
    pub fn references(&self) -> Vec<String> {
        let mut refs = self.about.clone();
        for s in &self.subjects {
            refs.extend(s.part_of.clone());
        }
        for i in &self.interactions {
            refs.extend([i.from.clone(), i.to.clone()]);
            refs.extend(i.governed_by.clone());
        }
        refs
    }
    pub fn rename_reference(&mut self, old: &str, new: &str) {
        for r in &mut self.about {
            rename_ref(r, old, new);
        }
        for s in &mut self.subjects {
            for r in &mut s.part_of {
                rename_ref(r, old, new);
            }
        }
        for i in &mut self.interactions {
            rename_ref(&mut i.from, old, new);
            rename_ref(&mut i.to, old, new);
            for r in &mut i.governed_by {
                rename_ref(r, old, new);
            }
        }
    }
}
fn rename_ref(r: &mut String, old: &str, new: &str) {
    if r == old || r.strip_prefix(old).is_some_and(|s| s.starts_with('#')) {
        *r = format!("{new}{}", &r[old.len()..]);
    }
}
impl ScenarioFlow {
    pub fn references(&self) -> Vec<String> {
        let mut refs = self.participants.clone();
        step_refs(&self.steps, &mut refs);
        refs
    }
    pub fn rename_reference(&mut self, old: &str, new: &str) {
        for p in &mut self.participants {
            rename_ref(p, old, new);
        }
        rename_steps(&mut self.steps, old, new);
    }
}
fn step_refs(steps: &[FlowStep], result: &mut Vec<String>) {
    for s in steps {
        match s {
            FlowStep::Interaction {
                from,
                to,
                interaction,
                refs,
                ..
            } => {
                result.extend([from.clone(), to.clone()]);
                result.extend(interaction.clone());
                result.extend(refs.clone());
            }
            FlowStep::Parallel { steps, .. } => step_refs(steps, result),
            FlowStep::Alternatives { branches, .. } => {
                for b in branches {
                    step_refs(&b.steps, result)
                }
            }
        }
    }
}
fn rename_steps(steps: &mut [FlowStep], old: &str, new: &str) {
    for s in steps {
        match s {
            FlowStep::Interaction {
                from,
                to,
                interaction,
                refs,
                ..
            } => {
                rename_ref(from, old, new);
                rename_ref(to, old, new);
                if let Some(r) = interaction {
                    rename_ref(r, old, new);
                }
                for r in refs {
                    rename_ref(r, old, new);
                }
            }
            FlowStep::Parallel { steps, .. } => rename_steps(steps, old, new),
            FlowStep::Alternatives { branches, .. } => {
                for b in branches {
                    rename_steps(&mut b.steps, old, new);
                }
            }
        }
    }
}
