use regex::Regex;
use std::sync::LazyLock;

use crate::model::block::ClauseAnchor;

static CLAUSE_ANCHOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{#([\w-]+)\}\s*(.*)").unwrap());

/// Extract clause anchors (e.g. `{#c-lifetime} bounded lifetime`) from block body text.
///
/// `start_line` is the 1-based line number of the first line of `body` in the file.
pub fn extract_clause_anchors(body: &str, start_line: usize) -> Vec<ClauseAnchor> {
    let mut anchors = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if let Some(caps) = CLAUSE_ANCHOR.captures(line) {
            let id = caps.get(1).unwrap().as_str().to_string();
            let text = caps.get(2).unwrap().as_str().trim().to_string();
            anchors.push(ClauseAnchor {
                id,
                text,
                line: start_line + i,
            });
        }
    }
    anchors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_anchors() {
        let body = "- {#c-lifetime} bounded maximum lifetime\n- {#c-idle} expiration on inactivity";
        let anchors = extract_clause_anchors(body, 10);
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].id, "c-lifetime");
        assert_eq!(anchors[0].text, "bounded maximum lifetime");
        assert_eq!(anchors[0].line, 10);
        assert_eq!(anchors[1].id, "c-idle");
        assert_eq!(anchors[1].line, 11);
    }

    #[test]
    fn no_anchors() {
        let body = "Just normal text\nNothing special";
        let anchors = extract_clause_anchors(body, 1);
        assert!(anchors.is_empty());
    }
}
