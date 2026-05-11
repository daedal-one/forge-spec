use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::model::id::QualifiedAnchor;
use crate::model::reference::{LocatedReference, SpecReference};

/// Parse a `spec:` URL into a `SpecReference`.
pub fn parse_spec_url(url: &str) -> Option<SpecReference> {
    let rest = url.strip_prefix("spec:")?;

    if let Some(src_path) = rest.strip_prefix("src:") {
        // Source reference: src:path/to/file.ts:42-78
        // Try to split off trailing :NN-NN
        if let Some((path, line_range)) = src_path.rsplit_once(':') {
            if let Some((start_s, end_s)) = line_range.split_once('-') {
                if let (Ok(start), Ok(end)) = (start_s.parse::<u32>(), end_s.parse::<u32>()) {
                    return Some(SpecReference::Source {
                        path: path.to_string(),
                        lines: Some((start, end)),
                    });
                }
            }
            // Single line number
            if let Ok(line) = line_range.parse::<u32>() {
                return Some(SpecReference::Source {
                    path: path.to_string(),
                    lines: Some((line, line)),
                });
            }
        }
        // No line range
        Some(SpecReference::Source {
            path: src_path.to_string(),
            lines: None,
        })
    } else if let Some(kb_path) = rest.strip_prefix("kb:") {
        // Knowledge-base reference: kb:path/to/note.md or kb:path/to/note.md#heading
        if let Some((path, heading)) = kb_path.split_once('#') {
            if path.is_empty() || heading.is_empty() {
                return None;
            }
            Some(SpecReference::KnowledgeBase {
                path: path.to_string(),
                heading: Some(heading.to_string()),
            })
        } else {
            if kb_path.is_empty() {
                return None;
            }
            Some(SpecReference::KnowledgeBase {
                path: kb_path.to_string(),
                heading: None,
            })
        }
    } else {
        // Spec reference: REQ:auth/session-expiry or REQ:auth/session-management#c-lifetime
        let qa: QualifiedAnchor = rest.parse().ok()?;
        Some(SpecReference::Spec(qa))
    }
}

/// Extract all `spec:` references from a Markdown body.
///
/// `body_start_line` is the 1-based line number of the body's first line in the file.
pub fn extract_references(body: &str, body_start_line: usize) -> Vec<LocatedReference> {
    let mut refs = Vec::new();
    let parser = Parser::new_ext(body, Options::all());

    let mut current_line;
    let mut in_link: Option<String> = None;
    let mut link_text = String::new();

    // Track line offsets from the source text
    let line_offsets: Vec<usize> = std::iter::once(0)
        .chain(body.match_indices('\n').map(|(i, _)| i + 1))
        .collect();

    for (event, range) in parser.into_offset_iter() {
        // Compute line from byte offset
        let byte_offset = range.start;
        let line = line_offsets
            .partition_point(|&off| off <= byte_offset)
            .saturating_sub(1);
        current_line = body_start_line + line;

        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let url = dest_url.to_string();
                if url.starts_with("spec:") {
                    in_link = Some(url);
                    link_text.clear();
                }
            }
            Event::Text(text) if in_link.is_some() => {
                link_text.push_str(&text);
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = in_link.take() {
                    if let Some(reference) = parse_spec_url(&url) {
                        refs.push(LocatedReference {
                            reference,
                            link_text: link_text.clone(),
                            line: current_line,
                        });
                    }
                }
                link_text.clear();
            }
            _ => {}
        }
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_spec_ref() {
        let r = parse_spec_url("spec:REQ:auth/session-expiry").unwrap();
        match r {
            SpecReference::Spec(qa) => {
                assert_eq!(qa.spec_id.to_string(), "REQ:auth/session-expiry");
                assert!(qa.anchor.is_none());
            }
            _ => panic!("expected Spec variant"),
        }
    }

    #[test]
    fn parse_spec_ref_with_anchor() {
        let r = parse_spec_url("spec:REQ:auth/session-management#c-lifetime").unwrap();
        match r {
            SpecReference::Spec(qa) => {
                assert_eq!(qa.anchor, Some("c-lifetime".to_string()));
            }
            _ => panic!("expected Spec variant"),
        }
    }

    #[test]
    fn parse_source_ref() {
        let r = parse_spec_url("spec:src:packages/auth/session.ts:42-78").unwrap();
        match r {
            SpecReference::Source { path, lines } => {
                assert_eq!(path, "packages/auth/session.ts");
                assert_eq!(lines, Some((42, 78)));
            }
            _ => panic!("expected Source variant"),
        }
    }

    #[test]
    fn parse_source_ref_no_lines() {
        let r = parse_spec_url("spec:src:packages/auth/session.ts").unwrap();
        match r {
            SpecReference::Source { path, lines } => {
                assert_eq!(path, "packages/auth/session.ts");
                assert!(lines.is_none());
            }
            _ => panic!("expected Source variant"),
        }
    }

    #[test]
    fn parse_kb_ref() {
        let r = parse_spec_url("spec:kb:engineering/auth/session-tokens.md").unwrap();
        match r {
            SpecReference::KnowledgeBase { path, heading } => {
                assert_eq!(path, "engineering/auth/session-tokens.md");
                assert!(heading.is_none());
            }
            _ => panic!("expected KnowledgeBase variant"),
        }
    }

    #[test]
    fn parse_kb_ref_with_heading() {
        let r =
            parse_spec_url("spec:kb:engineering/auth/session-tokens.md#credential-rotation")
                .unwrap();
        match r {
            SpecReference::KnowledgeBase { path, heading } => {
                assert_eq!(path, "engineering/auth/session-tokens.md");
                assert_eq!(heading, Some("credential-rotation".to_string()));
            }
            _ => panic!("expected KnowledgeBase variant"),
        }
    }

    #[test]
    fn parse_kb_ref_empty_path() {
        assert!(parse_spec_url("spec:kb:").is_none());
    }

    #[test]
    fn parse_kb_ref_empty_heading() {
        assert!(parse_spec_url("spec:kb:foo.md#").is_none());
    }

    #[test]
    fn kb_ref_display_roundtrip() {
        let r = SpecReference::KnowledgeBase {
            path: "notes/design.md".to_string(),
            heading: Some("overview".to_string()),
        };
        assert_eq!(r.to_string(), "spec:kb:notes/design.md#overview");

        let r2 = SpecReference::KnowledgeBase {
            path: "notes/design.md".to_string(),
            heading: None,
        };
        assert_eq!(r2.to_string(), "spec:kb:notes/design.md");
    }

    #[test]
    fn extract_refs_from_markdown() {
        let body = r#"See [the policy](spec:REQ:auth/session-expiry) for details.

Also check [session.ts](spec:src:packages/auth/session.ts:42-78).

And read [the design doc](spec:kb:engineering/auth/session-tokens.md#overview).
"#;
        let refs = extract_references(body, 10);
        assert_eq!(refs.len(), 3);
        assert!(matches!(&refs[0].reference, SpecReference::Spec(_)));
        assert!(matches!(&refs[1].reference, SpecReference::Source { .. }));
        assert!(matches!(
            &refs[2].reference,
            SpecReference::KnowledgeBase { .. }
        ));
    }

    #[test]
    fn non_spec_links_ignored() {
        let body = r#"See [google](https://google.com) and [foo](./bar.md).
"#;
        let refs = extract_references(body, 1);
        assert!(refs.is_empty());
    }
}
