use std::path::Path;
use std::process::Command;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

#[test]
fn agent_impact_report_cascades_from_clause_to_task_and_source() {
    let temp = tempfile::tempdir().unwrap();
    let specs = temp.path().join(".specs");
    write(
        &specs.join("_config.toml"),
        "baseline = \"forge-spec-v0.4.0\"\nproject = \"PROJECT:demo\"\n",
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
        "---\nid: TASK:demo/implement\ntype: task\nstatus: accepted\nsummary: Implement behavior.\nowners: [dev]\nprogress: pending\nrefines: [REQ:demo/root#c-one]\n---\n\n# Implement\n\n[code](spec:src:src/feature.rs#symbol=Feature/run)\n",
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
    assert!(report.contains("<forge-spec-impact schema-version=\"1\" mode=\"subject\""));
    assert!(report.contains("<task id=\"TASK:demo/implement\" progress=\"pending\""));
    assert!(report.contains("spec:src:src/feature.rs#symbol=Feature/run"));
}
