use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::model::id::QualifiedAnchor;
use crate::model::reference::{
    decode_symbol_segments, LocatedReference, SourceReference, SpecReference,
};

/// Parse a `spec:` URL into a `SpecReference`.
pub fn parse_spec_url(url: &str) -> Option<SpecReference> {
    let rest = url.strip_prefix("spec:")?;

    if let Some(src_path) = rest.strip_prefix("src:") {
        if let Some((path, symbol)) = src_path.split_once("#symbol=") {
            if path.is_empty() || path.contains('#') {
                return None;
            }
            if path
                .rsplit_once(':')
                .map(|(_, suffix)| {
                    suffix.parse::<u32>().is_ok()
                        || suffix
                            .split_once('-')
                            .map(|(start, end)| {
                                start.parse::<u32>().is_ok() && end.parse::<u32>().is_ok()
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false)
            {
                return None;
            }
            return Some(SpecReference::Source(SourceReference::symbol(
                path,
                decode_symbol_segments(symbol)?,
            )));
        }

        if src_path.contains('#') {
            return None;
        }

        // Source reference: src:path/to/file.ts:42-78
        // Try to split off trailing :NN-NN
        if let Some((path, line_range)) = src_path.rsplit_once(':') {
            if let Some((start_s, end_s)) = line_range.split_once('-') {
                if let (Ok(start), Ok(end)) = (start_s.parse::<u32>(), end_s.parse::<u32>()) {
                    return Some(SpecReference::Source(SourceReference::lines(
                        path, start, end,
                    )));
                }
            }
            // Single line number
            if let Ok(line) = line_range.parse::<u32>() {
                return Some(SpecReference::Source(SourceReference::lines(
                    path, line, line,
                )));
            }
        }
        // No line range
        if src_path.is_empty() {
            None
        } else {
            Some(SpecReference::Source(SourceReference::file(src_path)))
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
            SpecReference::Source(source) => {
                assert_eq!(source.path, "packages/auth/session.ts");
                assert_eq!(
                    source.target,
                    crate::model::reference::SourceTarget::Lines { start: 42, end: 78 }
                );
            }
            _ => panic!("expected Source variant"),
        }
    }

    #[test]
    fn parse_source_ref_no_lines() {
        let r = parse_spec_url("spec:src:packages/auth/session.ts").unwrap();
        match r {
            SpecReference::Source(source) => {
                assert_eq!(source.path, "packages/auth/session.ts");
                assert_eq!(source.target, crate::model::reference::SourceTarget::File);
            }
            _ => panic!("expected Source variant"),
        }
    }

    #[test]
    fn extract_refs_from_markdown() {
        let body = r#"See [the policy](spec:REQ:auth/session-expiry) for details.

Also check [session.ts](spec:src:packages/auth/session.ts:42-78).
"#;
        let refs = extract_references(body, 10);
        assert_eq!(refs.len(), 2);
        assert!(matches!(&refs[0].reference, SpecReference::Spec(_)));
        assert!(matches!(&refs[1].reference, SpecReference::Source(_)));
    }

    #[test]
    fn parse_source_symbol_ref() {
        let reference =
            parse_spec_url("spec:src:src/session.rs#symbol=SessionStore/expire%2Fnow").unwrap();
        assert_eq!(
            reference,
            SpecReference::Source(SourceReference::symbol(
                "src/session.rs",
                vec!["SessionStore".into(), "expire/now".into()],
            ))
        );
        assert_eq!(
            reference.to_string(),
            "spec:src:src/session.rs#symbol=SessionStore/expire%2Fnow"
        );
    }

    #[test]
    fn rejects_empty_or_mixed_source_selectors() {
        assert!(parse_spec_url("spec:src:").is_none());
        assert!(parse_spec_url("spec:src:src/lib.rs#symbol=").is_none());
        assert!(parse_spec_url("spec:src:src/lib.rs:10#symbol=main").is_none());
        assert!(parse_spec_url("spec:src:src/lib.rs#other=main").is_none());
    }

    #[test]
    fn non_spec_links_ignored() {
        let body = r#"See [google](https://google.com) and [foo](./bar.md).
"#;
        let refs = extract_references(body, 1);
        assert!(refs.is_empty());
    }
}
