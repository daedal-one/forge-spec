use std::path::Path;

use anyhow::Result;

use crate::mutation::Operation;

pub fn run(specs_dir: &Path, id: &str, new_id: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::SpecRename {
            spec: id.into(),
            new_id: new_id.into(),
        }],
    )
}
