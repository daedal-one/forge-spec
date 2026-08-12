use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use walkdir::WalkDir;

use crate::cli::*;
use crate::commands;
use crate::mutation::Operation;

const SPECS_DIR_NAMES: [&str; 2] = [".specs", "specs"];

pub fn run(cli: Cli) -> Result<()> {
    let explicit = cli.specs_dir;
    match cli.command {
        Commands::Init => commands::init::run(&find_init_specs_dir(explicit)?),
        Commands::New { entity_type, slug } => {
            commands::new::run(&find_specs_dir(explicit)?, &entity_type, &slug)
        }
        Commands::Lint {
            paths: _,
            require_symbols,
            allow_custom_lsp,
        } => {
            if commands::lint::run(
                &find_specs_dir(explicit)?,
                require_symbols,
                allow_custom_lsp,
            )? {
                Ok(())
            } else {
                bail!("lint failed")
            }
        }
        Commands::Render {
            id_or_query,
            target,
            depth,
            ancestors,
            descendants,
            include_source,
        } => commands::render::run(
            &find_specs_dir(explicit)?,
            &id_or_query,
            &target,
            depth,
            &ancestors,
            &descendants,
            include_source,
        ),
        Commands::Impact {
            subject,
            base,
            head,
            target,
        } => commands::impact::run(
            &find_specs_dir(explicit)?,
            subject.as_deref(),
            base.as_deref(),
            head.as_deref(),
            &target,
        ),
        Commands::Explore => commands::explore::run(&find_specs_dir(explicit)?),
        Commands::Inspect(args) => inspect(find_specs_dir(explicit)?, args.command),
        Commands::Change(args) => change(find_specs_dir(explicit)?, args.command),
        Commands::Rename { id, new_id } => {
            commands::rename::run(&find_specs_dir(explicit)?, &id, &new_id)
        }
        Commands::Lifecycle(args) => lifecycle(find_specs_dir(explicit)?, args.command),
        Commands::Relation(args) => relation(find_specs_dir(explicit)?, args.command),
        Commands::Task(args) => task(find_specs_dir(explicit)?, args.command),
        Commands::History(args) => history(find_specs_dir(explicit)?, args.command),
        Commands::Migrate(args) => migrate(find_specs_dir(explicit)?, args.command),
        Commands::Lsp { stdio: _ } => crate::lsp::run_stdio(&find_specs_dir(explicit)?),
        Commands::Completions { shell } => commands::completions::run(shell),
        Commands::Complete { what, context } => {
            commands::complete::run(&find_specs_dir(explicit)?, &what, &context)
        }
    }
}

fn inspect(specs_dir: PathBuf, command: InspectCommands) -> Result<()> {
    match command {
        InspectCommands::Tree {
            namespace,
            r#type,
            no_color,
        } => commands::tree::run(
            &specs_dir,
            namespace.as_deref(),
            r#type.as_deref(),
            no_color,
        ),
        InspectCommands::Graph { view } => commands::graph::run(&specs_dir, view),
        InspectCommands::Relations { id } => commands::query::relations(&specs_dir, &id),
        InspectCommands::Coverage { id } => commands::query::coverage(&specs_dir, &id),
        InspectCommands::Orphans => commands::query::orphans(&specs_dir),
        InspectCommands::Resolve {
            reference,
            json,
            allow_custom_lsp,
        } => commands::source::resolve(&specs_dir, &reference, json, allow_custom_lsp),
        InspectCommands::Symbols {
            path,
            query,
            json,
            allow_custom_lsp,
        } => commands::source::symbols(&specs_dir, &path, query.as_deref(), json, allow_custom_lsp),
    }
}

fn change(specs_dir: PathBuf, command: ChangeCommands) -> Result<()> {
    let operation = match command {
        ChangeCommands::Summary(args) => match args.command {
            SummaryCommands::Replace { id, value } => Operation::SummaryReplace { spec: id, value },
        },
        ChangeCommands::Owner(args) => match args.command {
            OwnerCommands::Add { id, owner } => Operation::OwnerAdd { spec: id, owner },
            OwnerCommands::Remove { id, owner } => Operation::OwnerRemove { spec: id, owner },
        },
        ChangeCommands::Pin(args) => match args.command {
            PinCommands::Set { id, value } => Operation::PinSet { spec: id, value },
            PinCommands::Clear { id } => Operation::PinClear { spec: id },
        },
        ChangeCommands::Requirement(args) => match args.command {
            RequirementCommands::Level { id, level } => {
                Operation::RequirementLevelSet { spec: id, level }
            }
            RequirementCommands::KindSet { id, kind } => {
                Operation::RequirementKindSet { spec: id, kind }
            }
            RequirementCommands::KindClear { id } => Operation::RequirementKindClear { spec: id },
            RequirementCommands::Monotonicity { id, value } => {
                Operation::RequirementMonotonicitySet { spec: id, value }
            }
        },
        ChangeCommands::Invariant(args) => match args.command {
            InvariantCommands::EnforcementAdd { id, value } => {
                Operation::InvariantEnforcementAdd { spec: id, value }
            }
            InvariantCommands::EnforcementRemove { id, value } => {
                Operation::InvariantEnforcementRemove { spec: id, value }
            }
            InvariantCommands::RequirementAdd { id, requirement } => {
                Operation::InvariantRequirementAdd {
                    spec: id,
                    requirement,
                }
            }
            InvariantCommands::RequirementRemove { id, requirement } => {
                Operation::InvariantRequirementRemove {
                    spec: id,
                    requirement,
                }
            }
        },
        ChangeCommands::Interface(args) => match args.command {
            InterfaceCommands::Stability { id, stability } => Operation::InterfaceStabilitySet {
                spec: id,
                stability,
            },
            InterfaceCommands::ConsumerAdd { id, consumer } => {
                Operation::InterfaceConsumerAdd { spec: id, consumer }
            }
            InterfaceCommands::ConsumerRemove { id, consumer } => {
                Operation::InterfaceConsumerRemove { spec: id, consumer }
            }
            InterfaceCommands::ProviderAdd { id, provider } => {
                Operation::InterfaceProviderAdd { spec: id, provider }
            }
            InterfaceCommands::ProviderRemove { id, provider } => {
                Operation::InterfaceProviderRemove { spec: id, provider }
            }
        },
        ChangeCommands::Adr(args) => match args.command {
            AdrCommands::DecisionDate { id, value } => {
                Operation::AdrDecisionDateSet { spec: id, value }
            }
            AdrCommands::DecisionMakerAdd { id, owner } => {
                Operation::AdrDecisionMakerAdd { spec: id, owner }
            }
            AdrCommands::DecisionMakerRemove { id, owner } => {
                Operation::AdrDecisionMakerRemove { spec: id, owner }
            }
        },
        ChangeCommands::Content(args) => match args.command {
            ContentCommands::TitleReplace { id, value } => {
                Operation::ContentTitleReplace { spec: id, value }
            }
            ContentCommands::SectionReplace {
                id,
                heading,
                markdown,
            } => Operation::ContentSectionReplace {
                spec: id,
                heading,
                markdown,
            },
            ContentCommands::BlockAdd {
                id,
                heading,
                kind,
                block,
                level,
                markdown,
            } => Operation::ContentBlockAdd {
                spec: id,
                heading,
                kind,
                block,
                level,
                markdown,
            },
            ContentCommands::BlockReplace {
                id,
                block,
                markdown,
            } => Operation::ContentBlockReplace {
                spec: id,
                block,
                markdown,
            },
            ContentCommands::BlockRemove { id, block } => {
                Operation::ContentBlockRemove { spec: id, block }
            }
            ContentCommands::ClauseAdd {
                id,
                block,
                clause,
                markdown,
            } => Operation::ContentClauseAdd {
                spec: id,
                block,
                clause,
                markdown,
            },
            ContentCommands::ClauseReplace {
                id,
                block,
                clause,
                markdown,
            } => Operation::ContentClauseReplace {
                spec: id,
                block,
                clause,
                markdown,
            },
            ContentCommands::ClauseRemove { id, block, clause } => Operation::ContentClauseRemove {
                spec: id,
                block,
                clause,
            },
        },
        ChangeCommands::Batch { from, dry_run } => {
            return commands::change::run_batch(&specs_dir, &from, dry_run);
        }
    };
    commands::change::run_operations(&specs_dir, vec![operation])
}

fn lifecycle(specs_dir: PathBuf, command: LifecycleCommands) -> Result<()> {
    match command {
        LifecycleCommands::Draft { id } => commands::lifecycle::draft(&specs_dir, &id),
        LifecycleCommands::Accept { id } => commands::lifecycle::accept(&specs_dir, &id),
        LifecycleCommands::Deprecate { id } => commands::lifecycle::deprecate(&specs_dir, &id),
        LifecycleCommands::Supersede { id, replacement } => {
            commands::lifecycle::supersede(&specs_dir, &id, &replacement)
        }
    }
}

fn relation(specs_dir: PathBuf, command: RelationCommands) -> Result<()> {
    match command {
        RelationCommands::Refine { id, target, aspect } => {
            commands::relation::refine(&specs_dir, &id, &target, &aspect)
        }
        RelationCommands::Unrefine { id, target } => {
            commands::relation::unrefine(&specs_dir, &id, &target)
        }
        RelationCommands::Categorize { id, topic } => {
            commands::relation::categorize(&specs_dir, &id, &topic)
        }
        RelationCommands::Uncategorize { id, topic } => {
            commands::relation::uncategorize(&specs_dir, &id, &topic)
        }
        RelationCommands::Relate { id, target } => {
            commands::relation::relate(&specs_dir, &id, &target)
        }
        RelationCommands::Unrelate { id, target } => {
            commands::relation::unrelate(&specs_dir, &id, &target)
        }
    }
}

fn task(specs_dir: PathBuf, command: TaskCommands) -> Result<()> {
    match command {
        TaskCommands::List { state, under, all } => {
            commands::task::list(&specs_dir, state, under.as_deref(), all)
        }
        TaskCommands::Start { id } => commands::task::start(&specs_dir, &id),
        TaskCommands::Done { id } => commands::task::done(&specs_dir, &id),
        TaskCommands::Block { id, on } => commands::task::block(&specs_dir, &id, &on),
        TaskCommands::Reset { id } => commands::task::reset(&specs_dir, &id),
        TaskCommands::Defer { id } => commands::task::defer(&specs_dir, &id),
        TaskCommands::Wontdo { id } => commands::task::wontdo(&specs_dir, &id),
        TaskCommands::Assign { id, assignee } => commands::task::assign(&specs_dir, &id, &assignee),
        TaskCommands::Unassign { id } => commands::task::unassign(&specs_dir, &id),
        TaskCommands::Schedule { id, eta } => commands::task::schedule(&specs_dir, &id, &eta),
        TaskCommands::Unschedule { id } => commands::task::unschedule(&specs_dir, &id),
    }
}

fn history(specs_dir: PathBuf, command: HistoryCommands) -> Result<()> {
    match command {
        HistoryCommands::Show { id } => commands::history::run(&specs_dir, false, id.as_deref()),
        HistoryCommands::Rebuild => commands::history::run(&specs_dir, true, None),
    }
}

fn migrate(specs_dir: PathBuf, command: MigrateCommands) -> Result<()> {
    match command {
        MigrateCommands::Plan { target, from, to } => {
            commands::migrate::run(&specs_dir, true, &target, from.as_deref(), to.as_deref())
        }
        MigrateCommands::Apply { from, to } => commands::migrate::run(
            &specs_dir,
            false,
            &RenderTarget::Human,
            from.as_deref(),
            to.as_deref(),
        ),
    }
}

fn find_specs_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(directory) = explicit {
        return Ok(directory);
    }
    let mut current = std::env::current_dir()?;
    loop {
        let candidates = SPECS_DIR_NAMES
            .iter()
            .map(|name| current.join(name))
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        match candidates.len() {
            0 => {}
            1 => return Ok(candidates[0].clone()),
            _ => {
                let chosen = &candidates[0];
                let collisions = candidates[1..]
                    .iter()
                    .filter(|other| !same_location(chosen, other) && has_spec_files(other))
                    .collect::<Vec<_>>();
                if !collisions.is_empty() && has_spec_files(chosen) {
                    eprintln!(
                        "warning: multiple spec directories contain specifications; using {}. Use --specs-dir to disambiguate.",
                        chosen.display()
                    );
                }
                return Ok(chosen.clone());
            }
        }
        if !current.pop() {
            break;
        }
    }
    bail!(
        "no .specs/ or specs/ directory found (searched upward from the current directory); use --specs-dir"
    )
}

fn find_init_specs_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    find_init_specs_dir_at(explicit, &std::env::current_dir()?)
}

fn find_init_specs_dir_at(explicit: Option<PathBuf>, current: &Path) -> Result<PathBuf> {
    if let Some(directory) = explicit {
        return Ok(directory);
    }
    let dot_specs = current.join(".specs");
    let plain_specs = current.join("specs");
    match (dot_specs.is_dir(), plain_specs.is_dir()) {
        (true, true) if !same_location(&dot_specs, &plain_specs) => {
            bail!(
                "both {} and {} exist; use --specs-dir to choose one",
                dot_specs.display(),
                plain_specs.display()
            )
        }
        (true, _) => Ok(dot_specs),
        (_, true) => Ok(plain_specs),
        _ => Ok(dot_specs),
    }
}

fn same_location(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn has_spec_files(directory: &Path) -> bool {
    WalkDir::new(directory)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            entry.file_type().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(".spec.md"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_defaults_to_dot_specs() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            find_init_specs_dir_at(None, temp.path()).unwrap(),
            temp.path().join(".specs")
        );
    }

    #[test]
    fn init_reuses_plain_specs_directory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("specs")).unwrap();
        assert_eq!(
            find_init_specs_dir_at(None, temp.path()).unwrap(),
            temp.path().join("specs")
        );
    }

    #[test]
    fn init_requires_a_choice_when_both_directories_exist() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join(".specs")).unwrap();
        std::fs::create_dir(temp.path().join("specs")).unwrap();
        assert!(find_init_specs_dir_at(None, temp.path())
            .unwrap_err()
            .to_string()
            .contains("choose one"));
    }
}
