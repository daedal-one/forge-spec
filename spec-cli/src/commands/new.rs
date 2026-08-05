use std::path::Path;

use crate::model::config::CURRENT_SPEC_BASELINE;
use anyhow::{bail, Result};

pub fn run(specs_dir: &Path, entity_type: &str, slug: &str) -> Result<()> {
    std::fs::create_dir_all(specs_dir)?;
    let config_path = specs_dir.join("_config.toml");
    if !config_path.exists() {
        std::fs::write(
            &config_path,
            format!("baseline = \"{CURRENT_SPEC_BASELINE}\"\n"),
        )?;
    }
    let id = format!("{entity_type}:{slug}");
    let type_name = match entity_type {
        "REQ" => "requirement",
        "INV" => "invariant",
        "IFC" => "interface",
        "ADR" => "adr",
        "GLO" => "glossary",
        "TOPIC" => "topic",
        "SCN" => "scenario",
        "TASK" => "task",
        _ => bail!("unknown entity type: {entity_type}"),
    };

    // Compute file path from slug
    let file_path = specs_dir.join(format!("{slug}.spec.md"));

    // Create parent directories
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if file_path.exists() {
        bail!("file already exists: {}", file_path.display());
    }

    let content = generate_template(entity_type, type_name, &id, slug);
    std::fs::write(&file_path, content)?;

    println!("Created {}", file_path.display());
    Ok(())
}

fn generate_template(entity_type: &str, type_name: &str, id: &str, slug: &str) -> String {
    match entity_type {
        "REQ" => format!(
            r#"---
id: {id}
type: {type_name}
status: draft
level: MUST
summary: >
  TODO: describe what this requirement specifies.
owners: []
refines: []
categorized_under: []
---

# {title}

## Context

TODO: describe the context for this requirement.

:::{{requirement id="{block_id}" level="MUST"}}
TODO: state the requirement using RFC 2119 keywords (MUST/SHOULD/MAY).
:::
"#,
            title = slug_to_title(slug),
            block_id = slug_to_block_id(slug),
        ),
        "INV" => format!(
            r#"---
id: {id}
type: {type_name}
status: draft
summary: >
  TODO: describe the invariant.
owners: []
enforcement: []
applies_to: []
---

# {title}

:::{{invariant id="{block_id}"}}
TODO: state the invariant — the property that must always hold.
:::
"#,
            title = slug_to_title(slug),
            block_id = slug_to_block_id(slug),
        ),
        "IFC" => format!(
            r#"---
id: {id}
type: {type_name}
status: draft
summary: >
  TODO: describe the interface contract.
owners: []
consumed_by: []
provided_by: []
stability: experimental
---

# {title}

:::{{interface id="{block_id}" level="MUST"}}
TODO: describe the API surface contract.
:::
"#,
            title = slug_to_title(slug),
            block_id = slug_to_block_id(slug),
        ),
        "ADR" => format!(
            r#"---
id: {id}
type: {type_name}
status: draft
summary: >
  TODO: one-line decision summary.
owners: []
decision_date: "YYYY-MM-DD"
decided_by: []
---

# {title}

## Context

TODO: describe the context and forces at play.

## Decision

TODO: describe the decision that was made.

## Consequences

TODO: describe the consequences of this decision.
"#,
            title = slug_to_title(slug),
        ),
        "GLO" => format!(
            r#"---
id: {id}
type: {type_name}
status: draft
summary: Glossary terms.
owners: []
---

# {title}

:::{{glossary-entry id="term-name"}}
**Term name** -- TODO: define the term.
:::
"#,
            title = slug_to_title(slug),
        ),
        "TOPIC" => format!(
            r#"---
id: {id}
type: {type_name}
status: draft
summary: >
  TODO: describe what this topic groups.
owners: []
---

# {title}

TODO: describe this topic and link to related specs.
"#,
            title = slug_to_title(slug),
        ),
        "SCN" => format!(
            r#"---
id: {id}
type: {type_name}
status: draft
summary: >
  TODO: describe this scenario.
owners: []
---

# {title}

TODO: walk through the scenario step by step.
"#,
            title = slug_to_title(slug),
        ),
        "TASK" => format!(
            r#"---
id: {id}
type: {type_name}
status: accepted
summary: >
  TODO: one-line description of the work.
owners: []
progress: pending
refines: []
assignee:
eta:
blocked_by: []
---

# {title}

## Plan

TODO: describe the work, links to the parent requirement, and any
specific files / functions to touch. Reference source with
`[ident](spec:src:path/to/file.rs:42-78)`.

## Acceptance

TODO: what observable signal indicates this task is done.
"#,
            title = slug_to_title(slug),
        ),
        _ => format!(
            "---\nid: {id}\ntype: {type_name}\nstatus: draft\nowners: []\n---\n\n# TODO\n"
        ),
    }
}

fn slug_to_title(slug: &str) -> String {
    // Convert "auth/session-expiry" to "Session expiry"
    let name = slug.rsplit('/').next().unwrap_or(slug);
    let mut title = name.replace('-', " ");
    if let Some(first) = title.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    title
}

fn slug_to_block_id(slug: &str) -> String {
    slug.rsplit('/').next().unwrap_or(slug).to_string()
}
