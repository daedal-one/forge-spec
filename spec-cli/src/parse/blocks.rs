use once_cell::sync::Lazy;
use regex::Regex;

use crate::model::block::{BlockKind, TypedBlock};
use crate::parse::anchors::extract_clause_anchors;

static BLOCK_OPEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^:::\{(\w[\w-]*)\s*(.*)\}\s*$").unwrap());
static BLOCK_CLOSE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^:::\s*$").unwrap());
static ATTR_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r#"id="([^"]+)""#).unwrap());
static ATTR_LEVEL: Lazy<Regex> = Lazy::new(|| Regex::new(r#"level="([^"]+)""#).unwrap());

/// Extract typed fenced divs from a spec body.
///
/// `body_start_line` is the 1-based line number where the body begins
/// in the original file (after frontmatter).
pub fn extract_blocks(body: &str, body_start_line: usize) -> Vec<TypedBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if let Some(caps) = BLOCK_OPEN.captures(lines[i]) {
            let tag = caps.get(1).unwrap().as_str();
            let attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");

            let kind = match BlockKind::from_tag(tag) {
                Some(k) => k,
                None => {
                    i += 1;
                    continue;
                }
            };

            let id = ATTR_ID
                .captures(attrs)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            let level = ATTR_LEVEL
                .captures(attrs)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());

            let start_line = body_start_line + i;
            i += 1;

            // Collect body lines until closing `:::`
            let mut body_lines = Vec::new();
            while i < lines.len() {
                if BLOCK_CLOSE.is_match(lines[i]) {
                    break;
                }
                body_lines.push(lines[i]);
                i += 1;
            }

            let end_line = body_start_line + i;
            let block_body = body_lines.join("\n");
            let clauses = extract_clause_anchors(&block_body, start_line + 1);

            blocks.push(TypedBlock {
                kind,
                id,
                level,
                body: block_body,
                clauses,
                start_line,
                end_line,
            });
        }
        i += 1;
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_requirement_block() {
        let body = r#"# Heading

:::{requirement id="session-management" level="MUST"}
The system MUST manage sessions per:

- {#c-lifetime} bounded maximum lifetime
- {#c-idle} expiration on inactivity
:::

Some trailing text.
"#;
        let blocks = extract_blocks(body, 5);
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!(b.kind, BlockKind::Requirement);
        assert_eq!(b.id, "session-management");
        assert_eq!(b.level.as_deref(), Some("MUST"));
        assert_eq!(b.clauses.len(), 2);
        assert_eq!(b.clauses[0].id, "c-lifetime");
        assert_eq!(b.clauses[1].id, "c-idle");
    }

    #[test]
    fn extract_multiple_blocks() {
        let body = r#":::{requirement id="a" level="MUST"}
text
:::

:::{invariant id="b"}
inv text
:::
"#;
        let blocks = extract_blocks(body, 1);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Requirement);
        assert_eq!(blocks[1].kind, BlockKind::Invariant);
    }

    #[test]
    fn non_goal_block() {
        let body = r#":::{non-goal id="no-sliding"}
Sliding window is out of scope.
:::
"#;
        let blocks = extract_blocks(body, 1);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::NonGoal);
    }
}
