use anyhow::{bail, Result};

use crate::model::frontmatter::*;
use crate::model::id::{EntityType, SpecId};

/// Split a `.spec.md` file into YAML frontmatter and body.
/// Returns `(yaml_string, body_string, body_start_line)`.
pub fn split_frontmatter(content: &str) -> Result<(&str, &str, usize)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);

    if !content.starts_with("---") {
        bail!("file does not start with YAML frontmatter delimiter '---'");
    }

    let after_first = &content[3..];
    let after_first = after_first.strip_prefix('\n').unwrap_or(
        after_first.strip_prefix("\r\n").unwrap_or(after_first),
    );

    let end_pos = after_first
        .find("\n---")
        .ok_or_else(|| anyhow::anyhow!("no closing '---' delimiter for frontmatter"))?;

    let yaml = &after_first[..end_pos];
    let after_close = &after_first[end_pos + 4..]; // skip "\n---"
    let body = after_close
        .strip_prefix('\n')
        .or_else(|| after_close.strip_prefix("\r\n"))
        .unwrap_or(after_close);

    // Count lines: first `---` is line 1, yaml starts line 2
    let yaml_lines = yaml.lines().count();
    let body_start_line = 1 + 1 + yaml_lines + 1; // opening --- + yaml lines + closing ---

    Ok((yaml, body, body_start_line))
}

/// Parse raw YAML into validated frontmatter structs.
pub fn parse_frontmatter(
    yaml: &str,
) -> Result<(UniversalFrontmatter, TypeSpecificFields, Vec<String>)> {
    let raw: RawFrontmatter = serde_yaml::from_str(yaml)?;
    let mut warnings = Vec::new();

    // Parse entity type
    let entity_type = EntityType::from_type_name(&raw.entity_type)
        .ok_or_else(|| anyhow::anyhow!("unknown type: {}", raw.entity_type))?;

    // Parse spec ID
    let spec_id: SpecId = raw
        .id
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{e}"))?;

    // Validate type matches ID prefix
    if spec_id.entity_type != entity_type {
        warnings.push(format!(
            "type '{}' does not match ID prefix '{}'",
            raw.entity_type,
            spec_id.entity_type.prefix()
        ));
    }

    // Parse status
    let status = raw
        .status
        .as_deref()
        .and_then(Status::from_str_val)
        .unwrap_or(Status::Draft);

    let version = raw.version.unwrap_or_else(|| "0.1.0".to_string());
    let owners = raw.owners.unwrap_or_default();
    let related = raw.related.unwrap_or_default();

    let universal = UniversalFrontmatter {
        id: spec_id,
        entity_type,
        status,
        version,
        summary: raw.summary,
        owners,
        pinned_at: raw.pinned_at,
        related,
        supersedes: raw.supersedes,
        superseded_by: raw.superseded_by,
    };

    // Parse type-specific fields
    let type_fields = match entity_type {
        EntityType::Req => {
            let level = raw
                .level
                .as_deref()
                .and_then(Level::from_str_val)
                .unwrap_or(Level::Must);
            TypeSpecificFields::Requirement {
                level,
                refines: raw.refines.unwrap_or_default(),
                aspects: raw.aspects.unwrap_or_default(),
                categorized_under: raw.categorized_under.unwrap_or_default(),
                kind: raw.kind,
                level_monotonic: raw.level_monotonic.unwrap_or(true),
            }
        }
        EntityType::Inv => TypeSpecificFields::Invariant {
            enforcement: raw.enforcement.unwrap_or_default(),
            applies_to: raw.applies_to.unwrap_or_default(),
        },
        EntityType::Ifc => {
            let stability = raw
                .stability
                .as_deref()
                .and_then(Stability::from_str_val)
                .unwrap_or(Stability::Experimental);
            TypeSpecificFields::Interface {
                consumed_by: raw.consumed_by.unwrap_or_default(),
                provided_by: raw.provided_by.unwrap_or_default(),
                stability,
            }
        }
        EntityType::Adr => TypeSpecificFields::Adr {
            decision_date: raw.decision_date.unwrap_or_default(),
            decided_by: raw.decided_by.unwrap_or_default(),
        },
        EntityType::Glo => TypeSpecificFields::Glossary,
        EntityType::Topic => TypeSpecificFields::Topic,
        EntityType::Scn => TypeSpecificFields::Scenario,
    };

    Ok((universal, type_fields, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_basic() {
        let input = "---\nid: REQ:a/b\ntype: requirement\n---\n# Body\n";
        let (yaml, body, line) = split_frontmatter(input).unwrap();
        assert!(yaml.contains("id: REQ:a/b"));
        assert!(body.contains("# Body"));
        assert_eq!(line, 5);
    }

    #[test]
    fn parse_req_frontmatter() {
        let yaml = r#"
id: REQ:auth/session-expiry
type: requirement
status: draft
version: 0.1.0
level: MUST
summary: Sessions expire.
owners: [carlo]
refines:
  - REQ:auth/session-management#c-lifetime
aspects: [duration]
"#;
        let (uni, fields, _) = parse_frontmatter(yaml).unwrap();
        assert_eq!(uni.id.to_string(), "REQ:auth/session-expiry");
        assert_eq!(uni.status, Status::Draft);
        if let TypeSpecificFields::Requirement { level, refines, .. } = fields {
            assert_eq!(level, Level::Must);
            assert_eq!(refines.len(), 1);
        } else {
            panic!("expected Requirement variant");
        }
    }
}
