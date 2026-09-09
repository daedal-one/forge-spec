use std::path::Path;
use std::process::Command;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

#[test]
fn agent_impact_report_keeps_related_work_outside_spec_and_source_closure() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    write(
        &specs.join("_config.toml"),
        "baseline = \"forge-spec-v0.7.0\"\nproject = \"PROJECT:demo\"\n",
    );
    write(
        &specs.join("_project.spec.md"),
        "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
    );
    write(
        &specs.join("requirement.spec.md"),
        "---\nid: REQ:demo/root\ntype: requirement\nstatus: accepted\nsummary: Root behavior.\nowners: [dev]\nlevel: MUST\nrefines: []\n---\n\n# Root\n\n:::{requirement id=\"behavior\" level=\"MUST\"}\n- {#c-one} first behavior\n:::\n",
    );
    write(
        &specs.join("task.spec.md"),
        "---\nid: TASK:demo/implement\ntype: task\nstatus: accepted\nsummary: Implement behavior.\nowners: [dev]\nprogress: pending\naddresses: [REQ:demo/root#c-one]\nlabels: []\ngroups: []\n---\n\n# Implement\n\n[code](spec:src:src/feature.rs#symbol=Feature/run)\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_spec"))
        .arg("--specs-dir")
        .arg(&specs)
        .args(["impact", "REQ:demo/root#c-one", "--target", "agent"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("<forge-spec-impact schema-version=\"2\" mode=\"subject\""));
    assert!(report.contains("<work-item id=\"TASK:demo/implement\" progress=\"pending\""));
    assert!(report.contains("<address target=\"REQ:demo/root#c-one\""));
    assert!(!report.contains("spec:src:src/feature.rs#symbol=Feature/run"));
}

#[test]
fn machine_queries_preserve_native_refinement_and_work_boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    write(
        &specs.join("_config.toml"),
        "baseline = \"forge-spec-v0.7.0\"\nproject = \"PROJECT:demo\"\n",
    );
    write(
        &specs.join("project.spec.md"),
        "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nowners: [dev]\n---\n# Demo\n",
    );
    write(&specs.join("parent.spec.md"), "---\nid: REQ:demo/parent\ntype: requirement\nstatus: accepted\nlevel: MUST\nowners: [dev]\n---\n# Parent\n:::{requirement id=\"behavior\" level=\"MUST\"}\n- {#c-one} First behavior.\n- {#c-two} Second behavior.\n:::\n");
    write(&specs.join("child.spec.md"), "---\nid: REQ:demo/child\ntype: requirement\nstatus: accepted\nlevel: MUST\nowners: [dev]\nrefines: [REQ:demo/parent#c-one]\n---\n# Child\n");
    write(&specs.join("task.spec.md"), "---\nid: TASK:demo/work\ntype: task\nstatus: accepted\nprogress: pending\nowners: [dev]\naddresses: [REQ:demo/parent#c-two]\n---\n# Work\n[Not durable source evidence](spec:src:task-only.rs)\n");
    let before = std::fs::read(specs.join("parent.spec.md")).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_spec"))
            .arg("--specs-dir")
            .arg(&specs)
            .args(args)
            .output()
            .unwrap()
    };
    let coverage = run(&["inspect", "coverage", "REQ:demo/parent", "--json"]);
    assert!(
        coverage.status.success(),
        "{}",
        String::from_utf8_lossy(&coverage.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&coverage.stdout).unwrap();
    assert_eq!(value["schema_version"], "forge-spec-coverage/v1");
    assert_eq!(value["kind"], "refinement");
    assert_eq!(
        value["clauses"][0]["refined_by"],
        serde_json::json!(["REQ:demo/child"])
    );
    assert_eq!(value["clauses"][1]["refined_by"], serde_json::json!([]));
    assert_eq!(
        coverage.stdout,
        run(&["inspect", "coverage", "REQ:demo/parent", "--json"]).stdout
    );
    let task = run(&["inspect", "coverage", "TASK:demo/work", "--json"]);
    let task: serde_json::Value = serde_json::from_slice(&task.stdout).unwrap();
    assert_eq!(task["applicable"], false);
    assert!(!run(&["inspect", "coverage", "REQ:demo/missing", "--json"])
        .status
        .success());
    let impact = run(&["impact", "REQ:demo/parent#c-two", "--json"]);
    assert!(
        impact.status.success(),
        "{}",
        String::from_utf8_lossy(&impact.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&impact.stdout).unwrap();
    assert_eq!(value["schema_version"], "forge-spec-impact/v1");
    assert_eq!(
        value["report"]["inputs"][0]["reference"],
        "REQ:demo/parent#c-two"
    );
    assert_eq!(value["report"]["related_work"][0]["id"], "TASK:demo/work");
    assert_eq!(value["report"]["source_surfaces"], serde_json::json!([]));
    assert_eq!(
        impact.stdout,
        run(&["impact", "REQ:demo/parent#c-two", "--json"]).stdout
    );
    assert_eq!(before, std::fs::read(specs.join("parent.spec.md")).unwrap());
}
