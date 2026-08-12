use std::path::Path;

use anyhow::Result;

use crate::cli::TaskState;
use crate::model::frontmatter::{Progress, TypeSpecificFields};
use crate::model::registry::SpecRegistry;
use crate::mutation::Operation;

#[derive(Debug, Clone, Copy)]
enum StateFilter {
    Open,
    All,
    Specific(Progress),
}

pub fn list(
    specs_dir: &Path,
    state: Option<TaskState>,
    under: Option<&str>,
    all: bool,
) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let filter = if all || matches!(state, Some(TaskState::All)) {
        StateFilter::All
    } else if let Some(state) = state {
        StateFilter::Specific(
            Progress::from_str_val(state.as_str()).expect("TaskState maps to Progress"),
        )
    } else {
        StateFilter::Open
    };
    let mut rows = Vec::<(Progress, String, String)>::new();
    for document in &registry.documents {
        let TypeSpecificFields::Task {
            progress, refines, ..
        } = &document.type_fields
        else {
            continue;
        };
        if under.is_some_and(|under| {
            !refines.iter().any(|target| {
                target == under
                    || target
                        .split('#')
                        .next()
                        .is_some_and(|parent| parent == under)
            })
        }) {
            continue;
        }
        let keep = match filter {
            StateFilter::Open => progress.is_open(),
            StateFilter::All => true,
            StateFilter::Specific(target) => *progress == target,
        };
        if keep {
            rows.push((
                *progress,
                document.id_str(),
                document
                    .universal
                    .summary
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("(no summary)")
                    .to_string(),
            ));
        }
    }
    rows.sort_by(|left, right| {
        progress_order(left.0)
            .cmp(&progress_order(right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    if rows.is_empty() {
        println!("(no tasks match)");
        return Ok(());
    }
    let width = rows.iter().map(|(_, id, _)| id.len()).max().unwrap_or(0);
    for (progress, id, summary) in rows {
        println!(
            "  {:<13}  {:<width$}  {summary}",
            progress.as_str(),
            id,
            width = width
        );
    }
    Ok(())
}

pub fn start(specs_dir: &Path, id: &str) -> Result<()> {
    progress(specs_dir, id, Progress::InProgress)
}

pub fn done(specs_dir: &Path, id: &str) -> Result<()> {
    progress(specs_dir, id, Progress::Done)
}

pub fn reset(specs_dir: &Path, id: &str) -> Result<()> {
    progress(specs_dir, id, Progress::Pending)
}

pub fn defer(specs_dir: &Path, id: &str) -> Result<()> {
    progress(specs_dir, id, Progress::Deferred)
}

pub fn wontdo(specs_dir: &Path, id: &str) -> Result<()> {
    progress(specs_dir, id, Progress::WontDo)
}

pub fn block(specs_dir: &Path, id: &str, blockers: &[String]) -> Result<()> {
    let mut operations = vec![Operation::TaskProgressSet {
        spec: id.into(),
        progress: Progress::Blocked.as_str().into(),
    }];
    operations.extend(blockers.iter().map(|blocker| Operation::TaskBlockerAdd {
        spec: id.into(),
        blocker: blocker.clone(),
    }));
    super::change::run_operations(specs_dir, operations)
}

pub fn assign(specs_dir: &Path, id: &str, assignee: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::TaskAssigneeSet {
            spec: id.into(),
            assignee: assignee.into(),
        }],
    )
}

pub fn unassign(specs_dir: &Path, id: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::TaskAssigneeClear { spec: id.into() }],
    )
}

pub fn schedule(specs_dir: &Path, id: &str, eta: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::TaskEtaSet {
            spec: id.into(),
            eta: eta.into(),
        }],
    )
}

pub fn unschedule(specs_dir: &Path, id: &str) -> Result<()> {
    super::change::run_operations(specs_dir, vec![Operation::TaskEtaClear { spec: id.into() }])
}

fn progress(specs_dir: &Path, id: &str, progress: Progress) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::TaskProgressSet {
            spec: id.into(),
            progress: progress.as_str().into(),
        }],
    )
}

fn progress_order(progress: Progress) -> u8 {
    match progress {
        Progress::InProgress => 0,
        Progress::Blocked => 1,
        Progress::Pending => 2,
        Progress::Done => 3,
        Progress::Deferred => 4,
        Progress::WontDo => 5,
    }
}
