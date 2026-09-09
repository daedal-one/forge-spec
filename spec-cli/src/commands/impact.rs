use std::path::Path;

use anyhow::Result;

use crate::cli::RenderTarget;
use crate::impact::{self, ImpactRequest};

pub fn run(
    specs_dir: &Path,
    subject: Option<&str>,
    base: Option<&str>,
    head: Option<&str>,
    target: &RenderTarget,
    json: bool,
) -> Result<()> {
    let request = ImpactRequest::new(subject, base, head)?;
    let report = impact::analyze(specs_dir, &request)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "schema_version": "forge-spec-impact/v1",
                "report": report,
            }))?
        );
        return Ok(());
    }
    let output = match target {
        RenderTarget::Human => impact::render_human(&report),
        RenderTarget::Agent => impact::render_agent(&report),
    };
    print!("{output}");
    Ok(())
}
