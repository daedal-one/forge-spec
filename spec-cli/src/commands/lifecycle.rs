use std::path::Path;

use anyhow::Result;

use crate::mutation::Operation;

pub fn draft(specs_dir: &Path, id: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::LifecycleDraft { spec: id.into() }],
    )
}

pub fn accept(specs_dir: &Path, id: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::LifecycleAccept { spec: id.into() }],
    )
}

pub fn deprecate(specs_dir: &Path, id: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::LifecycleDeprecate { spec: id.into() }],
    )
}

pub fn supersede(specs_dir: &Path, id: &str, replacement: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::LifecycleSupersede {
            spec: id.into(),
            replacement: replacement.into(),
        }],
    )
}
