use std::path::Path;

use anyhow::Result;

use crate::mutation::Operation;

pub fn refine(specs_dir: &Path, id: &str, target: &str, aspects: &[String]) -> Result<()> {
    let mut operations = vec![Operation::RelationRefine {
        spec: id.into(),
        target: target.into(),
    }];
    operations.extend(aspects.iter().map(|aspect| Operation::RelationAspectAdd {
        spec: id.into(),
        aspect: aspect.clone(),
    }));
    super::change::run_operations(specs_dir, operations)
}

pub fn unrefine(specs_dir: &Path, id: &str, target: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::RelationUnrefine {
            spec: id.into(),
            target: target.into(),
        }],
    )
}

pub fn categorize(specs_dir: &Path, id: &str, topic: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::RelationCategorize {
            spec: id.into(),
            topic: topic.into(),
        }],
    )
}

pub fn uncategorize(specs_dir: &Path, id: &str, topic: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::RelationUncategorize {
            spec: id.into(),
            topic: topic.into(),
        }],
    )
}

pub fn relate(specs_dir: &Path, id: &str, target: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::RelatedAdd {
            spec: id.into(),
            target: target.into(),
        }],
    )
}

pub fn unrelate(specs_dir: &Path, id: &str, target: &str) -> Result<()> {
    super::change::run_operations(
        specs_dir,
        vec![Operation::RelatedRemove {
            spec: id.into(),
            target: target.into(),
        }],
    )
}
