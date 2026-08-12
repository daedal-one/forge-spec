use std::io::Read as _;
use std::path::Path;

use anyhow::{Context, Result};

use crate::mutation::{ChangeRequest, MutationEngine, Operation};

pub fn run_operations(specs_dir: &Path, operations: Vec<Operation>) -> Result<()> {
    let outcome = MutationEngine::new(specs_dir).execute(&ChangeRequest::new(operations), false)?;
    print_outcome(&outcome);
    Ok(())
}

pub fn run_batch(specs_dir: &Path, source: &str, dry_run: bool) -> Result<()> {
    let mut input = String::new();
    if source == "-" {
        std::io::stdin()
            .read_to_string(&mut input)
            .context("reading change batch from standard input")?;
    } else {
        input = std::fs::read_to_string(source)
            .with_context(|| format!("reading change batch from {source}"))?;
    }
    let request: ChangeRequest =
        serde_json::from_str(&input).context("parsing strict change batch")?;
    let outcome = MutationEngine::new(specs_dir).execute(&request, dry_run)?;
    println!("{}", serde_json::to_string_pretty(&outcome.plan)?);
    Ok(())
}

fn print_outcome(outcome: &crate::mutation::MutationOutcome) {
    if outcome.plan.files.is_empty() {
        println!("No changes needed.");
    } else if outcome.written {
        println!(
            "Applied {} operation(s) to {} file(s).",
            outcome.plan.operations.len(),
            outcome.plan.files.len()
        );
        for file in &outcome.plan.files {
            println!("  {file}");
        }
    }
    for warning in &outcome.plan.warnings {
        eprintln!("{warning}");
    }
}
