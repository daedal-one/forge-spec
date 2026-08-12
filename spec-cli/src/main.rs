use std::process;

use clap::Parser;
use spec_cli::cli::Cli;

fn main() {
    if let Err(error) = spec_cli::commands::dispatch::run(Cli::parse()) {
        eprintln!("error: {error:#}");
        process::exit(1);
    }
}
