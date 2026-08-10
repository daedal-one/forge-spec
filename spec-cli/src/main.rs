use std::path::{Path, PathBuf};
use std::process;

use anyhow::Result;
use clap::Parser;
use walkdir::WalkDir;

use spec_cli::cli::{Cli, Commands};
use spec_cli::commands;

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

fn find_init_specs_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    find_init_specs_dir_at(explicit, &std::env::current_dir()?)
}

fn find_init_specs_dir_at(explicit: Option<PathBuf>, cwd: &Path) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }

    let dot_specs = cwd.join(".specs");
    let plain_specs = cwd.join("specs");
    match (dot_specs.is_dir(), plain_specs.is_dir()) {
        (true, true) if !same_location(&dot_specs, &plain_specs) => anyhow::bail!(
            "both {} and {} exist; use --specs-dir to choose one",
            dot_specs.display(),
            plain_specs.display()
        ),
        (true, _) => Ok(dot_specs),
        (_, true) => Ok(plain_specs),
        _ => Ok(dot_specs),
    }
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
        Commands::Init => {
            let specs_dir = find_init_specs_dir(cli.specs_dir)?;
            commands::init::run(&specs_dir)
        }
        Commands::New { entity_type, slug } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::new::run(&specs_dir, &entity_type, &slug)
        }
        Commands::Lint {
            paths: _,
            require_symbols,
            allow_custom_lsp,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            let ok = commands::lint::run(&specs_dir, require_symbols, allow_custom_lsp)?;
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
            include_source,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::render::run(
                &specs_dir,
                &id_or_query,
                &target,
                depth,
                &ancestors,
                &descendants,
                include_source,
            )
        }
        Commands::Graph {
            hierarchy,
            refinement,
            categorization,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::graph::run(&specs_dir, hierarchy, refinement, categorization)
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
        Commands::Impact {
            subject,
            base,
            head,
            target,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::impact::run(
                &specs_dir,
                subject.as_deref(),
                base.as_deref(),
                head.as_deref(),
                &target,
            )
        }
        Commands::Orphans => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::query::orphans(&specs_dir)
        }
        Commands::Migrate {
            guide,
            target,
            from,
            to,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::migrate::run(&specs_dir, guide, &target, from.as_deref(), to.as_deref())
        }
        Commands::Symbols {
            path,
            query,
            json,
            allow_custom_lsp,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::source::symbols(&specs_dir, &path, query.as_deref(), json, allow_custom_lsp)
        }
        Commands::Resolve {
            reference,
            json,
            allow_custom_lsp,
        } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            commands::source::resolve(&specs_dir, &reference, json, allow_custom_lsp)
        }
        Commands::Lsp { stdio: _ } => {
            let specs_dir = find_specs_dir(cli.specs_dir)?;
            spec_cli::lsp::run_stdio(&specs_dir)
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

        let error = find_init_specs_dir_at(None, temp.path())
            .unwrap_err()
            .to_string();

        assert!(error.contains("use --specs-dir to choose one"));
    }

    #[test]
    fn init_honors_explicit_directory() {
        let temp = tempfile::tempdir().unwrap();
        let explicit = temp.path().join("custom");

        assert_eq!(
            find_init_specs_dir_at(Some(explicit.clone()), temp.path()).unwrap(),
            explicit
        );
    }
}
