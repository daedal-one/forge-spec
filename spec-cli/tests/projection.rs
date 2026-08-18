use std::collections::BTreeMap;
use std::path::Path;

use spec_cli::model::registry::SpecRegistry;
use spec_cli::projection::{
    project, specification_intent_digest, DocumentationLinkKind, Overlay, OverlayEntry,
    ProjectedSourceSelector, RelationshipKind, SPEC_DELTA_SCHEMA_VERSION,
    SPEC_STATE_SCHEMA_VERSION,
};

const CONFIG: &str = "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n";

const PROJECT: &str = r#"---
id: PROJECT:demo
type: project
status: accepted
summary: Projection test project.
owners: [dev]
---

# Demo
"#;

const PARENT_A: &str = r#"---
id: REQ:demo/parent-a
type: requirement
status: accepted
level: MUST
summary: First parent.
owners: [dev]
refines: []
---

# Parent A

:::{requirement id="policy-a" level="MUST"}
- {#c-a} The system MUST retain the first clause.
:::
"#;

const PARENT_B: &str = r#"---
id: REQ:demo/parent-b
type: requirement
status: accepted
level: MUST
summary: Second parent.
owners: [dev]
refines: []
---

# Parent B

:::{requirement id="policy-b" level="MUST"}
- {#c-b} The system MUST retain the second clause.
:::
"#;

const CHILD: &str = r#"---
id: REQ:demo/child
type: requirement
status: accepted
level: MUST
summary: Child requirement.
owners: [dev]
implemented: 0123456789abcdef0123456789abcdef01234567
refines: [REQ:demo/parent-a#c-a, REQ:demo/parent-b#c-b]
aspects: [first, second]
categorized_under: [TOPIC:demo/security]
---

# Child

See [the exact first clause](spec:REQ:demo/parent-a#c-a),
[the file](spec:src:src/lib.rs),
[the range](spec:src:src/lib.rs:10-12), and
[the symbol](spec:src:src/lib.rs#symbol=Engine/run%2Ffast).
"#;

const TOPIC: &str = r#"---
id: TOPIC:demo/security
type: topic
status: accepted
summary: Security requirements.
owners: [dev]
---

# Security
"#;

const WORK_ITEM: &str = r#"---
id: TASK:demo/implement-child
type: task
status: accepted
summary: Implement the child behavior.
owners: [dev]
progress: done
addresses: [REQ:demo/child, REQ:demo/parent-a#c-a]
labels: [projection, graph]
assignee: dev
eta:
blocked_by: []
groups: [TOPIC:demo/security]
completion_checkpoint: 0123456789abcdef0123456789abcdef01234567
---

# Implement child

[Working notes](spec:src:src/work.rs#symbol=Work/run)
"#;

const OLD: &str = r#"---
id: GLO:demo/old
type: glossary
status: accepted
summary: Removed by the overlay.
owners: [dev]
---

# Old
"#;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn base_tree(root: &Path) {
    write(&root.join("_config.toml"), CONFIG);
    write(&root.join("_project.spec.md"), PROJECT);
    write(&root.join("parents/a.spec.md"), PARENT_A);
    write(&root.join("old.spec.md"), OLD);
}

fn final_tree(root: &Path) {
    write(&root.join("_config.toml"), CONFIG);
    write(&root.join("_project.spec.md"), PROJECT);
    write(&root.join("parents/a.spec.md"), PARENT_A);
    write(&root.join("parents/b.spec.md"), PARENT_B);
    write(&root.join("child.spec.md"), CHILD);
    write(&root.join("topic.spec.md"), TOPIC);
    write(&root.join("work.spec.md"), WORK_ITEM);
    write(
        &root.join("_redirects.toml"),
        "[[redirect]]\nfrom = \"REQ:demo/legacy\"\nto = \"REQ:demo/child\"\n",
    );
}

fn final_overlay() -> Overlay {
    BTreeMap::from([
        (
            ".specs/parents/b.spec.md".into(),
            OverlayEntry::Upsert(PARENT_B.as_bytes().to_vec()),
        ),
        (
            ".specs/child.spec.md".into(),
            OverlayEntry::Upsert(CHILD.as_bytes().to_vec()),
        ),
        (
            ".specs/topic.spec.md".into(),
            OverlayEntry::Upsert(TOPIC.as_bytes().to_vec()),
        ),
        (
            ".specs/work.spec.md".into(),
            OverlayEntry::Upsert(WORK_ITEM.as_bytes().to_vec()),
        ),
        (
            ".specs/_redirects.toml".into(),
            OverlayEntry::Upsert(
                b"[[redirect]]\nfrom = \"REQ:demo/legacy\"\nto = \"REQ:demo/child\"\n".to_vec(),
            ),
        ),
        (".specs/old.spec.md".into(), OverlayEntry::Delete),
    ])
}

#[test]
fn saved_and_multi_file_overlay_bytes_converge_without_writes() {
    let base = tempfile::tempdir().unwrap();
    let expected = tempfile::tempdir().unwrap();
    let specs = base.path().join(".specs");
    let expected_specs = expected.path().join(".specs");
    base_tree(&specs);
    final_tree(&expected_specs);

    let before = std::fs::read(specs.join("old.spec.md")).unwrap();
    let projected = project(&specs, &final_overlay()).unwrap();
    let saved = project(&expected_specs, &Overlay::new()).unwrap();

    assert_eq!(projected.schema_version, SPEC_STATE_SCHEMA_VERSION);
    assert_eq!(projected.config.intellect_provider, "forge-intellect");
    assert!(projected
        .specifications
        .iter()
        .all(|specification| specification.entity_type != "task"));
    assert_eq!(projected.work_items.len(), 1);
    assert_eq!(
        projected.work_items[0].completion_checkpoint.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(
        projected
            .specifications
            .iter()
            .find(|specification| specification.id == "REQ:demo/child")
            .unwrap()
            .implemented
            .as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(
        projected.canonical_json().unwrap(),
        saved.canonical_json().unwrap()
    );
    assert_eq!(std::fs::read(specs.join("old.spec.md")).unwrap(), before);
    assert!(!specs.join("child.spec.md").exists());
    assert!(!projected
        .canonical_json()
        .unwrap()
        .windows(base.path().as_os_str().len())
        .any(|window| window == base.path().as_os_str().as_encoded_bytes()));
}

#[test]
fn canonical_intent_digest_ignores_legacy_checkpoint_but_not_normative_text() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    final_tree(&specs);
    let registry = SpecRegistry::load(&specs).unwrap();
    let child = registry.get_by_id("REQ:demo/child").unwrap();
    let before = specification_intent_digest(child).unwrap();

    let path = specs.join("child.spec.md");
    let content = std::fs::read_to_string(&path).unwrap();
    write(
        &path,
        &content.replace(
            "implemented: 0123456789abcdef0123456789abcdef01234567\n",
            "",
        ),
    );
    let registry = SpecRegistry::load(&specs).unwrap();
    let without_legacy =
        specification_intent_digest(registry.get_by_id("REQ:demo/child").unwrap()).unwrap();
    assert_eq!(without_legacy, before);

    let content = std::fs::read_to_string(&path).unwrap();
    write(&path, &content.replace("status: accepted", "status: draft"));
    let registry = SpecRegistry::load(&specs).unwrap();
    let lifecycle_changed =
        specification_intent_digest(registry.get_by_id("REQ:demo/child").unwrap()).unwrap();
    assert_eq!(lifecycle_changed, before);

    let content = std::fs::read_to_string(&path).unwrap();
    write(&path, &content.replace("# Child", "# Changed child"));
    let registry = SpecRegistry::load(&specs).unwrap();
    let changed =
        specification_intent_digest(registry.get_by_id("REQ:demo/child").unwrap()).unwrap();
    assert_ne!(changed, before);
}

#[test]
fn retains_exact_relationship_kinds_aspect_pairing_and_source_selectors() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    final_tree(&specs);

    let state = project(&specs, &Overlay::new()).unwrap();
    assert!(state.valid, "{:?}", state.diagnostics);

    let refinements = state
        .relationships
        .iter()
        .filter(|edge| edge.source == "REQ:demo/child" && edge.kind == RelationshipKind::Refinement)
        .map(|edge| (edge.target.as_str(), edge.aspects.as_slice()))
        .collect::<Vec<_>>();
    assert_eq!(
        refinements,
        vec![
            ("REQ:demo/parent-a#c-a", ["first".to_string()].as_slice()),
            ("REQ:demo/parent-b#c-b", ["second".to_string()].as_slice()),
        ]
    );
    assert!(state.relationships.iter().any(|edge| {
        edge.kind == RelationshipKind::Reference
            && edge.source == "REQ:demo/child"
            && edge.target == "REQ:demo/parent-a#c-a"
    }));
    assert!(state.relationships.iter().any(|edge| {
        edge.kind == RelationshipKind::TaskAddresses
            && edge.source == "TASK:demo/implement-child"
            && edge.target == "REQ:demo/child"
    }));
    assert!(!state.relationships.iter().any(|edge| {
        edge.source == "TASK:demo/implement-child"
            && matches!(
                edge.kind,
                RelationshipKind::Refinement | RelationshipKind::ProjectContainment
            )
    }));
    assert!(state.relationships.iter().any(|edge| {
        edge.kind == RelationshipKind::Categorization
            && edge.source == "REQ:demo/child"
            && edge.target == "TOPIC:demo/security"
    }));
    assert!(!state.relationships.iter().any(|edge| {
        edge.kind == RelationshipKind::ProjectContainment && edge.source == "REQ:demo/child"
    }));
    assert!(state.relationships.iter().any(|edge| {
        edge.kind == RelationshipKind::ProjectContainment && edge.source == "REQ:demo/parent-a"
    }));

    let selectors = state
        .source_references
        .iter()
        .map(|reference| (&reference.path, &reference.selector))
        .collect::<Vec<_>>();
    assert_eq!(selectors.len(), 3);
    assert!(selectors.iter().any(
        |(path, selector)| *path == "src/lib.rs" && **selector == ProjectedSourceSelector::File
    ));
    assert!(selectors.iter().any(|(path, selector)| {
        *path == "src/lib.rs" && **selector == ProjectedSourceSelector::Lines { start: 10, end: 12 }
    }));
    assert!(selectors.iter().any(|(path, selector)| {
        *path == "src/lib.rs"
            && **selector
                == ProjectedSourceSelector::Symbol {
                    segments: vec!["Engine".into(), "run/fast".into()],
                }
    }));
    assert!(state.source_references.iter().all(|reference| {
        reference.source == "REQ:demo/child" && reference.id.starts_with("REQ:demo/child|")
    }));
    assert!(state
        .source_references
        .iter()
        .all(|reference| reference.source != "TASK:demo/implement-child"));
}

#[test]
fn invalid_inputs_are_retained_as_sorted_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    base_tree(&specs);
    let overlay = BTreeMap::from([
        (
            "_config.toml".into(),
            OverlayEntry::Upsert(b"baseline = [".to_vec()),
        ),
        (
            "_redirects.toml".into(),
            OverlayEntry::Upsert(b"[[redirect]\n".to_vec()),
        ),
        (
            "broken.spec.md".into(),
            OverlayEntry::Upsert(b"not frontmatter".to_vec()),
        ),
        (
            "binary.spec.md".into(),
            OverlayEntry::Upsert(vec![0xff, 0xfe]),
        ),
    ]);

    let state = project(&specs, &overlay).unwrap();
    assert!(!state.valid);
    let paths = state
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.starts_with('P'))
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            ".specs/binary.spec.md",
            ".specs/broken.spec.md",
            "_config.toml",
            "_redirects.toml",
        ]
    );
    assert!(state
        .canonical_json()
        .unwrap()
        .windows(temp.path().as_os_str().len())
        .all(|window| window != temp.path().as_os_str().as_encoded_bytes()));

    assert!(project(
        &specs,
        &BTreeMap::from([("../escape.spec.md".into(), OverlayEntry::Delete)])
    )
    .is_err());
    assert!(project(
        &specs,
        &BTreeMap::from([(specs.join("absolute.spec.md"), OverlayEntry::Delete)])
    )
    .is_err());
}

#[test]
fn semantic_delta_is_complete_and_deterministic() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    base_tree(&specs);
    let before = project(&specs, &Overlay::new()).unwrap();
    let after = project(&specs, &final_overlay()).unwrap();

    let delta = before.diff(&after);
    assert_eq!(delta.schema_version, SPEC_DELTA_SCHEMA_VERSION);
    assert_eq!(delta.added_specifications.len(), 3);
    assert_eq!(delta.added_work_items.len(), 1);
    assert_eq!(delta.removed_specifications.len(), 1);
    assert!(delta
        .added_relationships
        .iter()
        .any(|edge| edge.kind == RelationshipKind::Refinement));
    assert_eq!(delta, before.diff(&after));
    assert_eq!(
        delta.canonical_json().unwrap(),
        before.diff(&after).canonical_json().unwrap()
    );
}

#[test]
fn projects_configured_documentation_links_headings_and_overlay_deltas() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    write(
        &specs.join("_config.toml"),
        "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n\n[[documentation]]\nid = \"guides\"\ntitle = \"Guides\"\nroot = \"docs\"\ninclude = [\"**/*.md\"]\n",
    );
    write(
        &specs.join("_project.spec.md"),
        &format!("{PROJECT}\n[Deployment guide](spec:doc:docs/guide.md#heading=Guide/Deploy)\n"),
    );
    write(
        &temp.path().join("docs/guide.md"),
        "# Guide\n\nSummary.\n\n## Deploy\n\nSee the [runbook](runbook.md#steps) and [project](spec:PROJECT:demo).\n",
    );
    write(
        &temp.path().join("docs/runbook.md"),
        "# Runbook\n\n## Steps\n\nDo it.\n",
    );

    let before = project(&specs, &Overlay::new()).unwrap();
    assert!(before.valid, "{:?}", before.diagnostics);
    assert_eq!(before.schema_version, SPEC_STATE_SCHEMA_VERSION);
    assert_eq!(before.documentation.len(), 2);
    assert_eq!(
        before.documentation[0].headings[1].segments,
        ["Guide".to_string(), "Deploy".to_string()]
    );
    assert!(before.documentation_links.iter().any(|link| {
        link.source_kind == "specification"
            && link.target == "spec:doc:docs/guide.md#heading=Guide/Deploy"
    }));
    assert!(before.documentation_links.iter().any(|link| {
        link.source == "docs/guide.md"
            && link.target_kind == DocumentationLinkKind::Documentation
            && link.target == "spec:doc:docs/runbook.md#heading=Runbook/Steps"
    }));
    assert!(before.documentation_links.iter().any(|link| {
        link.source == "docs/guide.md"
            && link.target_kind == DocumentationLinkKind::Specification
            && link.target == "spec:PROJECT:demo"
    }));

    let overlay = BTreeMap::from([(
        "docs/guide.md".into(),
        OverlayEntry::Upsert(
            b"# Guide\n\nChanged summary.\n\n## Deploy\n\nSee the [project](spec:PROJECT:demo).\n"
                .to_vec(),
        ),
    )]);
    let after = project(&specs, &overlay).unwrap();
    let delta = before.diff(&after);
    assert_eq!(delta.changed_documentation.len(), 1);
    assert_eq!(delta.changed_documentation[0].path, "docs/guide.md");
    assert!(delta
        .removed_documentation_links
        .iter()
        .any(|link| { link.target == "spec:doc:docs/runbook.md#heading=Runbook/Steps" }));
    assert!(delta.added_documentation.is_empty());
    assert!(delta.removed_documentation.is_empty());
}
