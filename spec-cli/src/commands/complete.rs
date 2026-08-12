use std::path::Path;

use anyhow::Result;

use crate::model::registry::SpecRegistry;

pub fn run(specs_dir: &Path, what: &str, context: &[String]) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    match what {
        "ids" => print_ids(&registry, context.first().map(String::as_str)),
        "task-ids" => print_ids(&registry, Some("TASK")),
        "anchors" => print_anchors(&registry, required_context(context, "anchors")?, false),
        "blocks" => print_blocks(&registry, required_context(context, "blocks")?),
        "clauses" => {
            let id = required_context(context, "clauses")?;
            print_clauses(&registry, id, context.get(1).map(String::as_str))
        }
        "headings" => print_headings(&registry, required_context(context, "headings")?),
        "refinement-targets" => print_refinement_targets(&registry),
        "relation-targets" => match context.first().map(String::as_str) {
            Some("refine") => print_refinement_targets(&registry),
            Some("categorize") => print_ids(&registry, Some("TOPIC")),
            Some("block") => print_ids(&registry, Some("TASK")),
            _ => print_ids(&registry, None),
        },
        "progress" => {
            for state in [
                "pending",
                "in-progress",
                "done",
                "blocked",
                "deferred",
                "wontdo",
            ] {
                println!("{state}");
            }
            Ok(())
        }
        "suggest" => suggest(&registry, context),
        other => anyhow::bail!("unknown completion target: {other}"),
    }
}

fn required_context<'a>(context: &'a [String], name: &str) -> Result<&'a str> {
    context
        .first()
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("{name} requires a spec id"))
}

fn document<'a>(
    registry: &'a SpecRegistry,
    id: &str,
) -> Result<&'a crate::model::document::SpecDocument> {
    registry
        .get_by_id(id)
        .ok_or_else(|| anyhow::anyhow!("unknown spec id '{id}'"))
}

fn print_anchors(registry: &SpecRegistry, id: &str, qualify: bool) -> Result<()> {
    for anchor in document(registry, id)?.anchors() {
        if qualify {
            println!("{id}#{anchor}");
        } else {
            println!("{anchor}");
        }
    }
    Ok(())
}

fn print_blocks(registry: &SpecRegistry, id: &str) -> Result<()> {
    for block in &document(registry, id)?.blocks {
        println!("{}", block.id);
    }
    Ok(())
}

fn print_clauses(registry: &SpecRegistry, id: &str, block_filter: Option<&str>) -> Result<()> {
    for block in &document(registry, id)?.blocks {
        if block_filter.map_or(true, |wanted| wanted == block.id) {
            for clause in &block.clauses {
                println!("{}", clause.id);
            }
        }
    }
    Ok(())
}

fn print_headings(registry: &SpecRegistry, id: &str) -> Result<()> {
    let document = document(registry, id)?;
    let content = std::fs::read(&document.source_path)?;
    let editable = crate::editable::EditableDocument::from_bytes(&document.source_path, content)?;
    for heading in editable.headings {
        println!("{}", heading.title);
    }
    Ok(())
}

fn print_refinement_targets(registry: &SpecRegistry) -> Result<()> {
    let mut ids = registry
        .id_index
        .keys()
        .filter(|id| id.starts_with("REQ:"))
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    for id in ids {
        println!("{id}");
        print_anchors(registry, &id, true)?;
    }
    Ok(())
}

fn suggest(registry: &SpecRegistry, words: &[String]) -> Result<()> {
    let top = word(words, 0);
    let namespace = word(words, 1);
    let action = word(words, 2);
    let last = words.last().map(String::as_str);

    if top == Some("new") && words.len() == 1 {
        return print_values(&["REQ", "INV", "IFC", "ADR", "GLO", "TOPIC", "SCN", "TASK"]);
    }
    if top == Some("completions") && words.len() == 1 {
        return print_values(&["bash", "zsh", "fish"]);
    }
    if top == Some("inspect") && namespace == Some("graph") && words.len() == 2 {
        return print_values(&["hierarchy", "refinement", "categorization"]);
    }
    if top == Some("task") {
        if namespace == Some("list") && last == Some("--state") {
            return print_values(&[
                "pending",
                "in-progress",
                "done",
                "blocked",
                "deferred",
                "wontdo",
                "all",
            ]);
        }
        if words.len() == 2
            && matches!(
                namespace,
                Some(
                    "start"
                        | "done"
                        | "block"
                        | "reset"
                        | "defer"
                        | "wontdo"
                        | "assign"
                        | "unassign"
                        | "schedule"
                        | "unschedule"
                )
            )
            || namespace == Some("block") && last == Some("--on")
        {
            return print_ids(registry, Some("TASK"));
        }
    }
    if matches!(top, Some("render" | "impact")) && words.len() == 1
        || top == Some("inspect")
            && words.len() == 2
            && matches!(namespace, Some("relations" | "coverage"))
        || top == Some("lifecycle") && words.len() == 2
        || top == Some("rename") && words.len() == 1
    {
        return print_ids(registry, None);
    }
    if top == Some("lifecycle") && namespace == Some("supersede") && words.len() == 3 {
        return print_same_type(registry, &words[2]);
    }
    if top == Some("relation") {
        if words.len() == 2 {
            return match namespace {
                Some("refine" | "unrefine" | "categorize" | "uncategorize") => {
                    print_ids_multi(registry, &["REQ", "TASK"])
                }
                _ => print_ids(registry, None),
            };
        }
        if words.len() == 3 {
            return match namespace {
                Some("refine") => print_refinement_targets(registry),
                Some("categorize" | "uncategorize") => print_ids(registry, Some("TOPIC")),
                Some("relate" | "unrelate") => print_ids(registry, None),
                Some("unrefine") => print_refinement_targets(registry),
                _ => Ok(()),
            };
        }
    }
    if top == Some("change") {
        if words.len() == 3 {
            return match namespace {
                Some("requirement") => print_ids(registry, Some("REQ")),
                Some("invariant") => print_ids(registry, Some("INV")),
                Some("interface") => print_ids(registry, Some("IFC")),
                Some("adr") => print_ids(registry, Some("ADR")),
                Some("summary" | "owner" | "pin" | "content") => print_ids(registry, None),
                _ => Ok(()),
            };
        }
        if namespace == Some("requirement") && action == Some("level") && words.len() == 4 {
            return print_values(&["MUST", "SHOULD", "MAY", "INFO"]);
        }
        if namespace == Some("interface") && action == Some("stability") && words.len() == 4 {
            return print_values(&["experimental", "stable", "frozen"]);
        }
        if namespace == Some("invariant")
            && matches!(action, Some("requirement-add" | "requirement-remove"))
            && words.len() == 4
        {
            return print_ids(registry, Some("REQ"));
        }
        if namespace == Some("content") {
            let id = words.get(3).map(String::as_str);
            if last == Some("--heading") {
                return id.map_or(Ok(()), |id| print_headings(registry, id));
            }
            if matches!(action, Some("block-replace" | "block-remove")) && words.len() == 4
                || matches!(
                    action,
                    Some("clause-add" | "clause-replace" | "clause-remove")
                ) && words.len() == 4
            {
                return id.map_or(Ok(()), |id| print_blocks(registry, id));
            }
            if matches!(action, Some("clause-replace" | "clause-remove")) && words.len() == 5 {
                return print_clauses(registry, &words[3], Some(&words[4]));
            }
        }
    }
    Ok(())
}

fn word(words: &[String], index: usize) -> Option<&str> {
    words.get(index).map(String::as_str)
}

fn print_values(values: &[&str]) -> Result<()> {
    for value in values {
        println!("{value}");
    }
    Ok(())
}

fn print_ids_multi(registry: &SpecRegistry, entity_types: &[&str]) -> Result<()> {
    let mut ids = registry.id_index.keys().collect::<Vec<_>>();
    ids.sort();
    for id in ids {
        if id
            .split_once(':')
            .is_some_and(|(prefix, _)| entity_types.contains(&prefix))
        {
            println!("{id}");
        }
    }
    Ok(())
}

fn print_same_type(registry: &SpecRegistry, id: &str) -> Result<()> {
    let entity_type = id
        .split_once(':')
        .map(|(prefix, _)| prefix)
        .ok_or_else(|| anyhow::anyhow!("invalid spec id '{id}'"))?;
    print_ids(registry, Some(entity_type))
}

fn print_ids(registry: &SpecRegistry, entity_type: Option<&str>) -> Result<()> {
    let mut ids = registry.id_index.keys().collect::<Vec<_>>();
    ids.sort();
    for id in ids {
        if entity_type.map_or(true, |wanted| {
            id.split_once(':')
                .is_some_and(|(prefix, _)| prefix == wanted)
        }) {
            println!("{id}");
        }
    }
    Ok(())
}
