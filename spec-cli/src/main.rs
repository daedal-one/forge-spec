mod cli;
mod commands;
mod graph;
mod history;
mod lint;
mod model;
mod parse;
mod render;

use std::path::{Path, PathBuf};
use std::process;

use anyhow::Result;
use clap::Parser;
use walkdir::WalkDir;

use cli::{Cli, Commands};

const SPECS_DIR_NAMES: [&str; 2] = [".specs", "specs"];

fn find_specs_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }

    // Walk up from cwd looking for .specs/ or specs/.
    let mut current = std::env::current_dir()?;
    loop {
        let candidates: Vec<PathBuf> = SPECS_DIR_NAMES
            .iter()
            .map(|name| current.join(name))
            .filter(|p| p.is_dir())
            .collect();

        match candidates.len() {
            0 => {}
            1 => return Ok(candidates.into_iter().next().unwrap()),
            _ => {
                let (chosen, others) = candidates.split_first().unwrap();
                let collisions: Vec<&PathBuf> = others
                    .iter()
                    .filter(|other| !same_location(chosen, other) && has_spec_files(other))
                    .collect();
                if !collisions.is_empty() && has_spec_files(chosen) {
                    let names: Vec<String> = std::iter::once(chosen.display().to_string())
                        .chain(collisions.iter().map(|p| p.display().to_string()))
                        .collect();
                    eprintln!(
                        "warning: multiple spec directories found with spec files: {}. \
                         Using {}. Use --specs-dir to disambiguate.",
                        names.join(", "),
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

    anyhow::bail!(
        "no .specs/ or specs/ directory found (searched from current directory upward). \
         Use --specs-dir to specify the path."
    )
}

fn same_location(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

fn has_spec_files(dir: &Path) -> bool {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_type().is_file()
                && e.file_name()
                    .to_str()
                    .map(|n| n.ends_with(".spec.md"))
                    .unwrap_or(false)
        })
}

fn main() {
    let cli = Cli::parse();

    let result = run(cli);

    if let Err(e) = result {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::New { entity_type, slug } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::new::run(&specs_dir, &entity_type, &slug)
        }
        Commands::Lint { paths: _ } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            let ok = commands::lint::run(&specs_dir)?;
            if !ok {
                process::exit(1);
            }
            Ok(())
        }
        Commands::Render {
            id_or_query,
            target,
            depth,
            ancestors,
            descendants,
            ..
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::render::run(
                &specs_dir,
                &id_or_query,
                &target,
                depth,
                &ancestors,
                &descendants,
            )
        }
        Commands::Graph {
            refinement,
            categorization,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::graph::run(&specs_dir, refinement, categorization)
        }
        Commands::History { update, id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::history::run(&specs_dir, update, id.as_deref())
        }
        Commands::Children { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::query::children(&specs_dir, &id)
        }
        Commands::Ancestors { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::query::ancestors(&specs_dir, &id)
        }
        Commands::Coverage { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::query::coverage(&specs_dir, &id)
        }
        Commands::Orphans => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::query::orphans(&specs_dir)
        }
        Commands::Migrate => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::migrate::run(&specs_dir)
        }
        Commands::Todo { state, under, all } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::task::todo(&specs_dir, state.as_deref(), under.as_deref(), all)
        }
        Commands::Start { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::task::start(&specs_dir, &id)
        }
        Commands::Done { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::task::done(&specs_dir, &id)
        }
        Commands::Block { id, on } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::task::block(&specs_dir, &id, on.as_deref())
        }
        Commands::Reset { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::task::reset(&specs_dir, &id)
        }
        Commands::Defer { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::task::defer(&specs_dir, &id)
        }
        Commands::Wontdo { id } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::task::wontdo(&specs_dir, &id)
        }
        Commands::Tree {
            namespace,
            r#type,
            no_color,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::tree::run(
                &specs_dir,
                namespace.as_deref(),
                r#type.as_deref(),
                no_color,
            )
        }
        Commands::Explore => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::explore::run(&specs_dir)
        }
        Commands::Completions { shell } => commands::completions::run(shell),
        Commands::Complete { what } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::complete::run(&specs_dir, &what)
        }
    }
}
