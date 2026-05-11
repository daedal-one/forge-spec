mod cli;
mod commands;
mod graph;
mod history;
mod lint;
mod model;
mod parse;
mod render;

use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

fn find_specs_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }

    // Walk up from cwd looking for .specs/
    let mut current = std::env::current_dir()?;
    loop {
        let candidate = current.join(".specs");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if !current.pop() {
            break;
        }
    }

    anyhow::bail!(
        "no .specs/ directory found (searched from current directory upward). \
         Use --specs-dir to specify the path."
    )
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
    }
}
