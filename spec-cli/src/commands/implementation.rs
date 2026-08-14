use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::intellect::{self, AdherenceSnapshot, AdherenceState, SpecAdherence};
use crate::model::id::EntityType;
use crate::model::registry::SpecRegistry;
use crate::mutation::Operation;

#[derive(Serialize)]
struct StatusOutput<'a> {
    schema: &'static str,
    provider: &'a str,
    provider_version: &'a str,
    workspace: &'a intellect::WorkspaceState,
    complete: bool,
    specifications: Vec<&'a SpecAdherence>,
}

pub fn provider_start(specs_dir: &Path, idle_timeout_seconds: u64) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let control = intellect::start_service(&registry, idle_timeout_seconds)?;
    println!(
        "Provider {} {} is running for {} (pid {}, endpoint {}, idle timeout {}s)",
        control.provider,
        control.protocol,
        control.workspace_root,
        control.pid,
        control.endpoint,
        control.idle_timeout_seconds
    );
    Ok(())
}

pub fn provider_status(specs_dir: &Path) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    match intellect::service_status(&registry)? {
        intellect::ProviderServiceStatus::Running(control) => println!(
            "Provider {} is running for {} (pid {}, endpoint {}, idle timeout {}s)",
            control.provider,
            control.workspace_root,
            control.pid,
            control.endpoint,
            control.idle_timeout_seconds
        ),
        intellect::ProviderServiceStatus::Stopped => {
            println!("Provider {} is stopped", registry.config.intellect_provider)
        }
        intellect::ProviderServiceStatus::Stale { reason } => println!(
            "Provider {} has stale registration — {}",
            registry.config.intellect_provider, reason
        ),
    }
    Ok(())
}

pub fn provider_stop(specs_dir: &Path) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    match intellect::stop_service(&registry)? {
        intellect::ProviderServiceStatus::Stopped => {
            println!("Provider {} is stopped", registry.config.intellect_provider)
        }
        intellect::ProviderServiceStatus::Stale { reason } => println!(
            "Removed stale {} registration — {}",
            registry.config.intellect_provider, reason
        ),
        intellect::ProviderServiceStatus::Running(_) => {
            unreachable!("stop returns a terminal state")
        }
    }
    Ok(())
}

pub fn status(specs_dir: &Path, id: Option<&str>, json: bool) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    if let Some(id) = id {
        let document = registry
            .get_by_id(id)
            .with_context(|| format!("unknown specification '{id}'"))?;
        if document.universal.entity_type == EntityType::Task {
            bail!("TASK work items are outside implementation adherence; inspect task progress or its completion checkpoint instead");
        }
    }
    let snapshot = intellect::fetch_or_unknown(&registry)?;
    let states = selected_states(&snapshot, id);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&StatusOutput {
                schema: intellect::INTELLECT_PROTOCOL,
                provider: &snapshot.provider,
                provider_version: &snapshot.provider_version,
                workspace: &snapshot.workspace,
                complete: snapshot.complete,
                specifications: states,
            })?
        );
        return Ok(());
    }

    println!(
        "Provider: {} {} · workspace {}+{}",
        snapshot.provider,
        snapshot.provider_version,
        short_oid(&snapshot.workspace.head),
        if snapshot.workspace.worktree == "clean" {
            "clean"
        } else {
            "dirty"
        }
    );
    for state in states {
        let reason = state.reasons.first().map(String::as_str).unwrap_or("");
        if reason.is_empty() {
            println!("{:<44} {}", state.id, state.state.as_str());
        } else {
            println!("{:<44} {} — {}", state.id, state.state.as_str(), reason);
        }
    }
    Ok(())
}

pub fn verify(specs_dir: &Path, id: &str, at: Option<&str>) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let document = registry
        .get_by_id(id)
        .with_context(|| format!("unknown specification '{id}'"))?;
    if document.universal.entity_type == EntityType::Task {
        bail!("TASK work items cannot be implementation-verified; verify the durable specification they address");
    }
    let commit = resolve_commit(specs_dir, at.unwrap_or("HEAD"))?;
    let mut candidates = BTreeMap::new();
    candidates.insert(id.to_string(), commit.clone());
    let snapshot = intellect::fetch(&registry, &candidates)?;
    let state = snapshot
        .get(id)
        .with_context(|| format!("provider omitted specification '{id}'"))?;
    if state.state != AdherenceState::Current || !state.complete {
        let reasons = if state.reasons.is_empty() {
            "provider did not return complete evidence".to_string()
        } else {
            state.reasons.join("; ")
        };
        bail!(
            "cannot record implementation checkpoint for '{id}': provider reports {} ({reasons})",
            state.state.as_str()
        );
    }
    crate::commands::change::run_operations(
        specs_dir,
        vec![Operation::ImplementationCheckpointSet {
            spec: id.into(),
            commit: commit.clone(),
        }],
    )?;
    println!("Verified {id} at {commit}");
    Ok(())
}

pub fn clear(specs_dir: &Path, id: &str) -> Result<()> {
    crate::commands::change::run_operations(
        specs_dir,
        vec![Operation::ImplementationCheckpointClear { spec: id.into() }],
    )
}

fn selected_states<'a>(
    snapshot: &'a AdherenceSnapshot,
    id: Option<&str>,
) -> Vec<&'a SpecAdherence> {
    snapshot
        .specifications
        .iter()
        .filter(|state| match id {
            Some(id) => state.id == id,
            None => true,
        })
        .collect()
}

fn resolve_commit(specs_dir: &Path, revision: &str) -> Result<String> {
    let root = specs_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| Some(Path::new(".")))
        .context("specification directory has no workspace parent")?;
    let repository =
        git2::Repository::discover(root).context("opening workspace Git repository")?;
    let object = repository
        .revparse_single(revision)
        .with_context(|| format!("resolving Git revision '{revision}'"))?;
    let commit = object
        .peel_to_commit()
        .with_context(|| format!("Git revision '{revision}' is not a commit"))?
        .id()
        .to_string();
    Ok(commit)
}

fn short_oid(oid: &str) -> &str {
    oid.get(..oid.len().min(12)).unwrap_or(oid)
}
