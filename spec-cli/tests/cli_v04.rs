use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_spec")
}

fn workspace() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("_config.toml"),
        "baseline = \"forge-spec-v0.3.0\"\nproject = \"PROJECT:demo\"\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("_project.spec.md"),
        "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [carlo]\n---\n\n# Demo\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("example.spec.md"),
        "---\nid: REQ:demo/example\ntype: requirement\nstatus: accepted\nsummary: Original.\nowners: [carlo]\nlevel: MUST\nrefines: []\n---\n\n# Example\n\n## Policy\n\n:::{requirement id=\"example\" level=\"MUST\"}\n- {#c-rule} A rule.\n:::\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("work.spec.md"),
        "---\nid: TASK:demo/work\ntype: task\nstatus: accepted\nsummary: Implement the rule.\nowners: [carlo]\nprogress: pending\nrefines: [REQ:demo/example#c-rule]\nblocked_by: []\n---\n\n# Work\n",
    )
    .unwrap();
    temp
}

#[test]
fn exposes_only_the_v04_top_level_hierarchy() {
    let output = Command::new(binary()).arg("--help").output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for command in [
        "init",
        "new",
        "lint",
        "render",
        "impact",
        "explore",
        "inspect",
        "change",
        "rename",
        "lifecycle",
        "relation",
        "task",
        "history",
        "migrate",
        "lsp",
        "completions",
    ] {
        assert!(help.contains(command), "missing {command} from help");
    }
    for removed in [
        "graph",
        "children",
        "ancestors",
        "coverage",
        "orphans",
        "symbols",
        "resolve",
        "todo",
        "start",
        "done",
        "block",
        "reset",
        "defer",
        "wontdo",
        "tree",
        "edit",
        "patch",
        "set",
        "delete",
        "remove-document",
    ] {
        let output = Command::new(binary()).arg(removed).output().unwrap();
        assert!(
            !output.status.success(),
            "removed command {removed} still succeeds"
        );
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("unrecognized subcommand"));
    }
}

#[test]
fn every_bare_namespace_prints_contextual_help() {
    for path in [
        vec!["inspect"],
        vec!["change"],
        vec!["change", "summary"],
        vec!["change", "owner"],
        vec!["change", "pin"],
        vec!["change", "requirement"],
        vec!["change", "invariant"],
        vec!["change", "interface"],
        vec!["change", "adr"],
        vec!["change", "content"],
        vec!["lifecycle"],
        vec!["relation"],
        vec!["task"],
        vec!["history"],
        vec!["migrate"],
    ] {
        let output = Command::new(binary()).args(&path).output().unwrap();
        assert!(
            !output.status.success(),
            "bare namespace {path:?} succeeded"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Usage:"),
            "bare namespace {path:?} did not print help"
        );
    }
}

#[test]
fn batch_dry_run_is_deterministic_and_read_only() {
    let temp = workspace();
    let document = temp.path().join("example.spec.md");
    let before = std::fs::read(&document).unwrap();
    let request = temp.path().join("change.json");
    std::fs::write(
        &request,
        r#"{
  "schema": "forge-spec-change/v1",
  "if_match": {},
  "operations": [
    { "op": "summary.replace", "spec": "REQ:demo/example", "value": "Changed." }
  ]
}"#,
    )
    .unwrap();
    let output = Command::new(binary())
        .args([
            "--specs-dir",
            temp.path().to_str().unwrap(),
            "change",
            "batch",
            "--from",
        ])
        .arg(&request)
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan["schema"], "forge-spec-change/v1");
    assert_eq!(plan["dry_run"], true);
    assert_eq!(std::fs::read(document).unwrap(), before);
}

#[test]
fn bare_migrate_never_writes() {
    let temp = workspace();
    let before = std::fs::read(temp.path().join("_config.toml")).unwrap();
    let output = Command::new(binary())
        .args(["--specs-dir", temp.path().to_str().unwrap(), "migrate"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read(temp.path().join("_config.toml")).unwrap(),
        before
    );
}

#[test]
fn dynamic_completion_reaches_typed_document_selectors() {
    let temp = workspace();
    let complete = |context: &[&str]| {
        let output = Command::new(binary())
            .args([
                "--specs-dir",
                temp.path().to_str().unwrap(),
                "__complete",
                "suggest",
            ])
            .args(context)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    assert!(complete(&["task", "start"]).contains("TASK:demo/work"));
    assert!(complete(&["change", "requirement", "level"]).contains("REQ:demo/example"));
    assert!(
        complete(&["change", "content", "block-replace", "REQ:demo/example"]).contains("example")
    );
    assert!(complete(&[
        "change",
        "content",
        "clause-replace",
        "REQ:demo/example",
        "example"
    ])
    .contains("c-rule"));
    assert!(complete(&[
        "change",
        "content",
        "section-replace",
        "REQ:demo/example",
        "--heading"
    ])
    .contains("Policy"));
    let refinement = complete(&["relation", "refine", "REQ:demo/example"]);
    assert!(refinement.contains("REQ:demo/example#c-rule"));
}
