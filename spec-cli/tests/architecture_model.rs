use serde_json::json;
use spec_cli::{
    model::registry::SpecRegistry,
    mutation::{ChangeRequest, MutationEngine},
    projection::{project, specification_intent_digest, Overlay, OverlayEntry},
};
use std::path::Path;

fn fixture() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(".specs");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("_config.toml"),
        "baseline = \"forge-spec-v0.7.0\"\nproject = \"PROJECT:demo\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("project.spec.md"),
        r#"---
id: PROJECT:demo
type: project
status: accepted
summary: Model fixture.
owners: [dev]
model:
  subjects:
    - id: system
      kind: product
      title: System
    - id: client
      kind: actor
      title: Client
    - id: api
      kind: service
      title: API
      part_of: ['PROJECT:demo#system']
  interactions:
    - id: request
      kind: request
      title: Request
      from: 'PROJECT:demo#client'
      to: 'PROJECT:demo#api'
      governed_by: ['REQ:demo/policy#c-contract']
---
# Demo
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("policy.spec.md"),
        r#"---
id: REQ:demo/policy
type: requirement
status: accepted
summary: Request policy.
owners: [dev]
level: MUST
model:
  about: ['PROJECT:demo#api']
---
# Policy
:::{requirement id="contract" level="MUST"}
- {#c-contract} The API MUST answer requests.
:::
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("scenario.spec.md"),
        r#"---
id: SCN:demo/retry
type: scenario
status: accepted
summary: Retry behavior.
owners: [dev]
flow:
  participants: ['PROJECT:demo#client', 'PROJECT:demo#api']
  steps:
    - kind: interaction
      id: first
      from: 'PROJECT:demo#client'
      to: 'PROJECT:demo#api'
      title: Initial request
      interaction: 'PROJECT:demo#request'
      refs: ['REQ:demo/policy#c-contract']
    - kind: alternatives
      id: outcome
      branches:
        - id: success
          title: Successful response
          steps:
            - kind: interaction
              id: answer
              from: 'PROJECT:demo#api'
              to: 'PROJECT:demo#client'
              title: Answer
        - id: retry
          title: Retry once
          steps:
            - kind: parallel
              id: repeat
              steps:
                - kind: interaction
                  id: second
                  from: 'PROJECT:demo#client'
                  to: 'PROJECT:demo#api'
                  title: Repeat request
                  interaction: 'PROJECT:demo#request'
---
# Retry
"#,
    )
    .unwrap();
    tmp
}
fn apply(
    dir: &Path,
    operations: serde_json::Value,
    dry: bool,
) -> anyhow::Result<spec_cli::mutation::MutationOutcome> {
    let request: ChangeRequest =
        serde_json::from_value(json!({"schema":"forge-spec-change/v1","operations":operations}))?;
    MutationEngine::new(dir).execute(&request, dry)
}
#[test]
fn canonical_model_preserves_subjects_clauses_and_repeated_nested_steps() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    let state = project(&dir, &Overlay::new()).unwrap();
    assert!(state.valid, "{:?}", state.diagnostics);
    assert_eq!(state.schema_version, "forge-spec-state-v5");
    assert_eq!(state.model.subjects.len(), 3);
    assert_eq!(
        state.model.interactions[0].governed_by,
        ["REQ:demo/policy#c-contract"]
    );
    assert_eq!(state.scenarios.len(), 1);
    let registry = SpecRegistry::load(&dir).unwrap();
    for anchor in ["first", "second", "repeat", "outcome", "retry"] {
        assert!(registry
            .get_by_anchor(&format!("SCN:demo/retry#{anchor}"))
            .is_some());
    }
    assert_eq!(
        state.canonical_json().unwrap(),
        project(&dir, &Overlay::new())
            .unwrap()
            .canonical_json()
            .unwrap()
    );
}
#[test]
fn rejects_cycles_wrong_endpoints_anchor_collisions_and_task_models() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    for (path, before, after, code) in [
        (
            "project.spec.md",
            "part_of: ['PROJECT:demo#system']",
            "part_of: ['PROJECT:demo#api']",
            "R035",
        ),
        (
            "scenario.spec.md",
            "to: 'PROJECT:demo#api'",
            "to: 'PROJECT:demo#system'",
            "R034",
        ),
        ("project.spec.md", "id: request", "id: api", "R034"),
        (
            "scenario.spec.md",
            "kind: alternatives",
            "kind: unknown",
            "P003",
        ),
    ] {
        let text = std::fs::read_to_string(dir.join(path))
            .unwrap()
            .replace(before, after);
        let overlay = Overlay::from([(
            Path::new(".specs").join(path),
            OverlayEntry::Upsert(text.into_bytes()),
        )]);
        let state = project(&dir, &overlay).unwrap();
        assert!(!state.valid);
        assert!(
            state.diagnostics.iter().any(|d| d.code == code),
            "{code}: {:?}",
            state.diagnostics
        );
    }
    std::fs::write(dir.join("task.spec.md"),"---\nid: TASK:demo/work\ntype: task\nstatus: accepted\nsummary: Work.\nowners: [dev]\nmodel: {subjects: [{id: task-owned, kind: service, title: Nope}]}\n---\n# Work\n").unwrap();
    let state = project(&dir, &Overlay::new()).unwrap();
    assert!(!state.valid);
    assert_eq!(state.model.subjects.len(), 3);
    assert_eq!(state.work_items.len(), 1);
}
#[test]
fn typed_model_edit_is_atomic_and_changes_normative_digest() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    let path = dir.join("policy.spec.md");
    let original = std::fs::read(&path).unwrap();
    let before =
        specification_intent_digest(&spec_cli::parse::parse_document(&path).unwrap()).unwrap();
    let invalid = json!([{"op":"model.replace","spec":"REQ:demo/policy","value":{"about":["PROJECT:demo#missing"]}}]);
    assert!(apply(&dir, invalid, false).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let valid = json!([{"op":"model.replace","spec":"REQ:demo/policy","value":{"about":["PROJECT:demo#client"]}}]);
    apply(&dir, valid.clone(), true).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    apply(&dir, valid, false).unwrap();
    assert_ne!(
        before,
        specification_intent_digest(&spec_cli::parse::parse_document(&path).unwrap()).unwrap()
    );
    assert!(project(&dir, &Overlay::new()).unwrap().valid);
}
#[test]
fn presentation_edits_change_state_but_not_normative_intent() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    let before = project(&dir, &Overlay::new()).unwrap();
    let path = dir.join("policy.spec.md");
    let intent =
        specification_intent_digest(&spec_cli::parse::parse_document(&path).unwrap()).unwrap();
    apply(&dir,json!([{"op":"view.replace","value":{"id":"architecture","title":"Architecture","mode":"architecture","focus":"PROJECT:demo#api","depth":2,"profile":"c4"}}]),false).unwrap();
    let after = project(&dir, &Overlay::new()).unwrap();
    assert!(after.valid, "{:?}", after.diagnostics);
    assert_eq!(after.views.len(), 1);
    assert_eq!(
        intent,
        specification_intent_digest(&spec_cli::parse::parse_document(&path).unwrap()).unwrap()
    );
    let delta = before.diff(&after);
    assert!(delta.views.is_some());
    assert!(delta.changed_specifications.is_empty());
    assert!(delta.model.is_none());
    assert!(apply(&dir,json!([{"op":"view.replace","value":{"id":"invalid","title":"Bad","mode":"map","focus":"REQ:missing/nope"}}]),true).is_err());
}
#[test]
fn rename_updates_architecture_scenario_and_view_refs() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    apply(&dir,json!([{"op":"view.replace","value":{"id":"api","title":"API","mode":"architecture","focus":"PROJECT:demo#api"}}]),false).unwrap();
    apply(
        &dir,
        json!([{"op":"spec.rename","spec":"PROJECT:demo","new_id":"PROJECT:renamed"}]),
        false,
    )
    .unwrap();
    let state = project(&dir, &Overlay::new()).unwrap();
    assert!(state.valid, "{:?}", state.diagnostics);
    assert!(state
        .model
        .subjects
        .iter()
        .all(|s| s.owner == "PROJECT:renamed"));
    assert_eq!(state.views[0].focus.as_deref(), Some("PROJECT:renamed#api"));
    assert_eq!(state.scenarios[0].participants[0], "PROJECT:renamed#client");
}
#[test]
fn machine_export_is_read_only_without_git_and_accepts_legacy_prose() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    let config = dir.join("_config.toml");
    std::fs::write(
        &config,
        "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n",
    )
    .unwrap();
    let before = std::fs::read(&config).unwrap();
    let export = spec_cli::commands::model::export(&dir).unwrap();
    assert!(export.state.valid, "{:?}", export.state.diagnostics);
    assert_eq!(export.schema, "forge-spec-model/v1");
    assert!(export
        .revisions
        .values()
        .all(|r| r.revision.is_none() && r.commit.is_none()));
    assert_eq!(before, std::fs::read(&config).unwrap());
}
#[test]
fn migration_preserves_legacy_document_bytes_and_intent() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    let path = dir.join("policy.spec.md");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(
        dir.join("_config.toml"),
        "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n",
    )
    .unwrap();
    let plan = spec_cli::migration::MigrationPlan::build("forge-spec-v0.6.0", "forge-spec-v0.7.0")
        .unwrap();
    plan.apply(&dir).unwrap();
    spec_cli::migration::write_baseline(&dir, "forge-spec-v0.7.0").unwrap();
    assert_eq!(bytes, std::fs::read(&path).unwrap());
    assert!(project(&dir, &Overlay::new()).unwrap().valid);
}

#[test]
fn export_disables_repository_fsmonitor_and_leaves_index_untouched() {
    use std::process::Command;
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    let repository = git2::Repository::init(tmp.path()).unwrap();
    let mut index = repository.index().unwrap();
    index
        .add_all([".specs"], git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repository.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@example.invalid").unwrap();
    repository
        .commit(Some("HEAD"), &sig, &sig, "fixture", &tree, &[])
        .unwrap();
    // A config-controlled shell command is deliberately hostile to read-only projection.
    let marker = tmp.path().join("executed");
    assert!(Command::new("git")
        .arg("-C")
        .arg(tmp.path())
        .args(["config", "core.fsmonitor"])
        .arg(format!("touch {}; false", marker.display()))
        .status()
        .unwrap()
        .success());
    let index_path = tmp.path().join(".git/index");
    let before = std::fs::read(&index_path).unwrap();
    let exported = spec_cli::commands::model::export(&dir).unwrap();
    assert!(exported.state.valid);
    assert!(!marker.exists());
    assert_eq!(std::fs::read(index_path).unwrap(), before);
    assert!(exported
        .revisions
        .values()
        .all(|r| r.revision.as_deref() == Some("r1") && r.commit.is_some()));
}

#[test]
fn read_compatibility_does_not_accept_unknown_baselines_or_missing_project() {
    let tmp = fixture();
    let dir = tmp.path().join(".specs");
    let config = dir.join("_config.toml");
    std::fs::write(
        &config,
        "baseline = \"forge-spec-v9.9.0\"\nproject = \"PROJECT:demo\"\n",
    )
    .unwrap();
    assert!(
        !spec_cli::projection::project_compatible(&dir, &Overlay::new())
            .unwrap()
            .valid
    );
    std::fs::write(
        &config,
        "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:missing\"\n",
    )
    .unwrap();
    let state = spec_cli::projection::project_compatible(&dir, &Overlay::new()).unwrap();
    assert!(!state.valid);
    assert!(state.diagnostics.iter().any(|d| d.code == "R025"));
}
