use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EntityType {
    #[serde(alias = "requirement")]
    Req,
    #[serde(alias = "invariant")]
    Inv,
    #[serde(alias = "interface")]
    Ifc,
    #[serde(alias = "adr")]
    Adr,
    #[serde(alias = "glossary")]
    Glo,
    #[serde(alias = "topic")]
    Topic,
    #[serde(alias = "scenario")]
    Scn,
    #[serde(alias = "task")]
    Task,
}

impl EntityType {
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Req => "REQ",
            Self::Inv => "INV",
            Self::Ifc => "IFC",
            Self::Adr => "ADR",
            Self::Glo => "GLO",
            Self::Topic => "TOPIC",
            Self::Scn => "SCN",
            Self::Task => "TASK",
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Req => "requirement",
            Self::Inv => "invariant",
            Self::Ifc => "interface",
            Self::Adr => "adr",
            Self::Glo => "glossary",
            Self::Topic => "topic",
            Self::Scn => "scenario",
            Self::Task => "task",
        }
    }

    pub fn from_prefix(s: &str) -> Option<Self> {
        match s {
            "REQ" => Some(Self::Req),
            "INV" => Some(Self::Inv),
            "IFC" => Some(Self::Ifc),
            "ADR" => Some(Self::Adr),
            "GLO" => Some(Self::Glo),
            "TOPIC" => Some(Self::Topic),
            "SCN" => Some(Self::Scn),
            "TASK" => Some(Self::Task),
            _ => None,
        }
    }

    pub fn from_type_name(s: &str) -> Option<Self> {
        match s {
            "requirement" => Some(Self::Req),
            "invariant" => Some(Self::Inv),
            "interface" => Some(Self::Ifc),
            "adr" => Some(Self::Adr),
            "glossary" => Some(Self::Glo),
            "topic" => Some(Self::Topic),
            "scenario" => Some(Self::Scn),
            "task" => Some(Self::Task),
            _ => None,
        }
    }
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.prefix())
    }
}

impl FromStr for EntityType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_prefix(s)
            .or_else(|| Self::from_type_name(s))
            .ok_or_else(|| format!("unknown entity type: {s}"))
    }
}

/// A full spec document ID: `<TYPE>:<namespace>/<slug>`
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpecId {
    pub entity_type: EntityType,
    pub namespace: String,
    pub slug: String,
}

impl SpecId {
    pub fn new(entity_type: EntityType, namespace: impl Into<String>, slug: impl Into<String>) -> Self {
        Self {
            entity_type,
            namespace: namespace.into(),
            slug: slug.into(),
        }
    }

    /// Returns `namespace/slug`
    pub fn path(&self) -> String {
        format!("{}/{}", self.namespace, self.slug)
    }
}

impl fmt::Display for SpecId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}/{}", self.entity_type.prefix(), self.namespace, self.slug)
    }
}

impl FromStr for SpecId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (prefix, rest) = s
            .split_once(':')
            .ok_or_else(|| format!("invalid spec ID (missing ':'): {s}"))?;
        let entity_type = EntityType::from_prefix(prefix)
            .ok_or_else(|| format!("unknown entity type prefix: {prefix}"))?;
        let (namespace, slug) = rest
            .split_once('/')
            .ok_or_else(|| format!("invalid spec ID (missing '/'): {s}"))?;
        if namespace.is_empty() || slug.is_empty() {
            return Err(format!("invalid spec ID (empty namespace or slug): {s}"));
        }
        Ok(Self {
            entity_type,
            namespace: namespace.to_string(),
            slug: slug.to_string(),
        })
    }
}

/// A qualified anchor: `<spec-id>#<anchor>` or just `<spec-id>`
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualifiedAnchor {
    pub spec_id: SpecId,
    pub anchor: Option<String>,
}

impl QualifiedAnchor {
    pub fn new(spec_id: SpecId, anchor: Option<String>) -> Self {
        Self { spec_id, anchor }
    }

    pub fn doc_only(spec_id: SpecId) -> Self {
        Self { spec_id, anchor: None }
    }

    /// Key for the anchor index: `"TYPE:ns/slug#anchor"` or `"TYPE:ns/slug"`
    pub fn key(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for QualifiedAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.spec_id)?;
        if let Some(ref anchor) = self.anchor {
            write!(f, "#{anchor}")?;
        }
        Ok(())
    }
}

impl FromStr for QualifiedAnchor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((id_part, anchor)) = s.split_once('#') {
            let spec_id: SpecId = id_part.parse()?;
            if anchor.is_empty() {
                return Err(format!("empty anchor in: {s}"));
            }
            Ok(Self {
                spec_id,
                anchor: Some(anchor.to_string()),
            })
        } else {
            let spec_id: SpecId = s.parse()?;
            Ok(Self {
                spec_id,
                anchor: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_spec_id() {
        let id: SpecId = "REQ:auth/session-expiry".parse().unwrap();
        assert_eq!(id.entity_type, EntityType::Req);
        assert_eq!(id.namespace, "auth");
        assert_eq!(id.slug, "session-expiry");
        assert_eq!(id.to_string(), "REQ:auth/session-expiry");
    }

    #[test]
    fn parse_all_prefixes() {
        for (s, expected) in [
            ("REQ:a/b", EntityType::Req),
            ("INV:a/b", EntityType::Inv),
            ("IFC:a/b", EntityType::Ifc),
            ("ADR:a/b", EntityType::Adr),
            ("GLO:a/b", EntityType::Glo),
            ("TOPIC:a/b", EntityType::Topic),
            ("SCN:a/b", EntityType::Scn),
        ] {
            let id: SpecId = s.parse().unwrap();
            assert_eq!(id.entity_type, expected);
        }
    }

    #[test]
    fn parse_qualified_anchor() {
        let qa: QualifiedAnchor = "REQ:auth/session-management#c-lifetime".parse().unwrap();
        assert_eq!(qa.spec_id.to_string(), "REQ:auth/session-management");
        assert_eq!(qa.anchor, Some("c-lifetime".to_string()));
    }

    #[test]
    fn parse_qualified_anchor_no_anchor() {
        let qa: QualifiedAnchor = "INV:auth/no-stale-tokens".parse().unwrap();
        assert_eq!(qa.anchor, None);
    }

    #[test]
    fn invalid_spec_id() {
        assert!("BAD:auth/foo".parse::<SpecId>().is_err());
        assert!("REQ:".parse::<SpecId>().is_err());
        assert!("REQ:auth".parse::<SpecId>().is_err());
        assert!("noprefix".parse::<SpecId>().is_err());
    }

    #[test]
    fn roundtrip_display_parse() {
        let original = "TOPIC:infra/deployment";
        let id: SpecId = original.parse().unwrap();
        assert_eq!(id.to_string(), original);
    }
}
