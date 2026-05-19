use std::path::Path;

use anyhow::Result;

use crate::model::registry::SpecRegistry;

pub fn run(specs_dir: &Path, what: &str) -> Result<()> {
    match what {
        "ids" => {
            let registry = SpecRegistry::load(specs_dir)?;
            let mut ids: Vec<&String> = registry.id_index.keys().collect();
            ids.sort();
            for id in ids {
                println!("{id}");
            }
            Ok(())
        }
        other => anyhow::bail!("unknown completion target: {other}"),
    }
}
