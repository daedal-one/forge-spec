use anyhow::Result;
use clap::{Command, CommandFactory as _};

use crate::cli::{Cli, Shell};

pub fn run(shell: Shell) -> Result<()> {
    let rows = command_rows(&Cli::command());
    let script = match shell {
        Shell::Bash => bash_script(&rows),
        Shell::Zsh => zsh_script(&rows),
        Shell::Fish => fish_script(&rows),
    };
    print!("{script}");
    Ok(())
}

fn command_rows(command: &Command) -> Vec<(String, Vec<(String, String)>)> {
    fn visit(
        command: &Command,
        path: &mut Vec<String>,
        rows: &mut Vec<(String, Vec<(String, String)>)>,
    ) {
        let children = command
            .get_subcommands()
            .filter(|child| child.get_name() != "help" && !child.is_hide_set())
            .map(|child| {
                (
                    child.get_name().to_string(),
                    child
                        .get_about()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        if !children.is_empty() {
            rows.push((path.join(" "), children));
        }
        for child in command
            .get_subcommands()
            .filter(|child| child.get_name() != "help" && !child.is_hide_set())
        {
            path.push(child.get_name().to_string());
            visit(child, path, rows);
            path.pop();
        }
    }
    let mut rows = Vec::new();
    visit(command, &mut Vec::new(), &mut rows);
    rows
}

fn bash_script(rows: &[(String, Vec<(String, String)>)]) -> String {
    let mut cases = String::new();
    for (path, children) in rows {
        let words = children
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        cases.push_str(&format!("        {path:?}) choices={words:?} ;;\n"));
    }
    format!(
        r#"# bash completion for spec 0.4; command levels are derived from Clap
_spec() {{
    local cur path choices value
    COMPREPLY=()
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    path="${{COMP_WORDS[*]:1:$((COMP_CWORD-1))}}"
    case "$path" in
{cases}    esac
    if [[ -n "$choices" ]]; then
        COMPREPLY=( $(compgen -W "$choices" -- "$cur") )
        return
    fi
    while IFS= read -r value; do
        [[ "$value" == "$cur"* ]] && COMPREPLY+=("$value")
    done < <(spec __complete suggest "${{COMP_WORDS[@]:1:COMP_CWORD-1}}" 2>/dev/null)
}}
complete -F _spec spec
"#,
        cases = cases
    )
}

fn zsh_script(rows: &[(String, Vec<(String, String)>)]) -> String {
    let mut cases = String::new();
    for (path, children) in rows {
        let words = children
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        cases.push_str(&format!("    {path:?}) choices=({words}) ;;\n"));
    }
    format!(
        r#"#compdef spec
# zsh completion for spec 0.4; command levels are derived from Clap
_spec() {{
  local path
  local -a choices values
  path="${{(j: :)words[2,CURRENT-1]}}"
  case "$path" in
{cases}  esac
  if (( ${{#choices}} )); then
    _describe 'spec command' choices
    return
  fi
  values=("${{(@f)$(spec __complete suggest ${{words[2,CURRENT-1]}} 2>/dev/null)}}")
  (( ${{#values}} )) && _describe 'value' values
}}
compdef _spec spec
"#,
        cases = cases
    )
}

fn fish_script(rows: &[(String, Vec<(String, String)>)]) -> String {
    let mut output = String::from("# fish completion for spec 0.4; command levels are derived from Clap\ncomplete -c spec -e\n");
    for (path, children) in rows {
        let parent = path.split_whitespace().last();
        for (name, description) in children {
            let predicate = match parent {
                None => "__fish_use_subcommand".to_string(),
                Some(parent) => format!("__fish_seen_subcommand_from {parent}"),
            };
            output.push_str(&format!(
                "complete -c spec -n {predicate:?} -a {name:?} -d {description:?}\n"
            ));
        }
    }
    output.push_str("complete -c spec -f -a '(spec __complete suggest (commandline -opc)[2..-1] 2>/dev/null)'\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tree_contains_every_namespace_level() {
        let rows = command_rows(&Cli::command());
        assert!(rows.iter().any(|(path, _)| path == "change content"));
        assert!(rows.iter().any(|(path, _)| path == "task"));
        assert!(!rows
            .iter()
            .any(|(_, children)| children.iter().any(|(name, _)| name == "todo")));
    }
}
