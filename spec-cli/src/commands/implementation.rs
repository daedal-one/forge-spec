use std::collections::BTreeSet;
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

pub fn verify(specs_dir: &Path, ids: &[String], all: bool, at: Option<&str>) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    if all && !ids.is_empty() {
        bail!("implementation verify accepts explicit IDs or --all, not both");
    }
    if !all && ids.is_empty() {
        bail!("implementation verify requires at least one specification ID or --all");
    }
    let commit = resolve_commit(specs_dir, at.unwrap_or("HEAD"))?;
    let selected = if all {
        implementation_trailer_ids(specs_dir, &commit)?
    } else {
        ids.iter().cloned().collect::<BTreeSet<_>>()
    };
    if selected.is_empty() {
        bail!("candidate commit has no durable Spec-Ref (implements) trailers");
    }
    validate_durable_selection(&registry, &selected)?;
    let snapshot = intellect::attest(&registry, &selected, &commit)?;
    for id in &selected {
        let state = require_complete_current(&snapshot, id, "record adherence attestation")?;
        let attestation = state
            .attestation_id
            .as_deref()
            .context("provider returned current adherence without an attestation ID")?;
        println!("Verified {id} at {commit} · attestation {attestation}");
    }
    Ok(())
}

pub fn revoke(specs_dir: &Path, id: &str, reason: &str) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let selected = BTreeSet::from([id.to_string()]);
    validate_durable_selection(&registry, &selected)?;
    let snapshot = intellect::revoke(&registry, &selected, reason)?;
    let state = snapshot
        .get(id)
        .with_context(|| format!("provider omitted specification '{id}'"))?;
    if state.attestation_id.is_some() {
        bail!("provider retained a selected attestation after revocation");
    }
    println!("Revoked adherence for {id} · {reason}");
    Ok(())
}

pub fn migrate_attestations(specs_dir: &Path) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let selected = registry
        .documents
        .iter()
        .filter(|document| document.universal.entity_type != EntityType::Task)
        .filter(|document| document.universal.implemented.is_some())
        .map(|document| document.id_str())
        .collect::<BTreeSet<_>>();
    if selected.is_empty() {
        println!("No legacy implementation checkpoints to migrate");
        return Ok(());
    }
    let snapshot = intellect::import_legacy(&registry, &selected)?;
    for id in &selected {
        let state = require_complete_current(&snapshot, id, "migrate legacy checkpoint")?;
        if state.attestation_id.is_none() {
            bail!("provider imported '{id}' without returning an attestation ID");
        }
    }
    let operations = selected
        .iter()
        .map(|id| Operation::LegacyImplementationCheckpointClear { spec: id.clone() })
        .collect();
    crate::commands::change::run_operations(specs_dir, operations)?;
    println!("Migrated {} legacy adherence checkpoint(s)", selected.len());
    Ok(())
}

fn require_complete_current<'a>(
    snapshot: &'a AdherenceSnapshot,
    id: &str,
    action: &str,
) -> Result<&'a SpecAdherence> {
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
            "cannot {action} for '{id}': provider reports {} ({reasons})",
            state.state.as_str()
        );
    }
    Ok(state)
}

fn validate_durable_selection(registry: &SpecRegistry, ids: &BTreeSet<String>) -> Result<()> {
    for id in ids {
        let document = registry
            .get_by_id(id)
            .with_context(|| format!("unknown specification '{id}'"))?;
        if document.universal.entity_type == EntityType::Task {
            bail!("TASK work items cannot be implementation-verified; verify the durable specification they address");
        }
    }
    Ok(())
}

fn implementation_trailer_ids(specs_dir: &Path, commit: &str) -> Result<BTreeSet<String>> {
    let root = specs_dir.parent().unwrap_or_else(|| Path::new("."));
    let events = crate::history::trailers::walk_trailers_from(root, Some(commit))?;
    Ok(events
        .into_iter()
        .filter(|event| event.full_sha == commit && event.kind == "implements")
        .map(|event| {
            event
                .spec_ref
                .split_once('#')
                .map_or(event.spec_ref.clone(), |(id, _)| id.to_string())
        })
        .filter(|id| !id.starts_with("TASK:"))
        .collect())
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
