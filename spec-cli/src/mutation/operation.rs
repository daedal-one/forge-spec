use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const CHANGE_SCHEMA: &str = "forge-spec-change/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeRequest {
    pub schema: String,
    #[serde(default)]
    pub if_match: BTreeMap<String, String>,
    pub operations: Vec<Operation>,
}

impl ChangeRequest {
    pub fn new(operations: Vec<Operation>) -> Self {
        Self {
            schema: CHANGE_SCHEMA.to_string(),
            if_match: BTreeMap::new(),
            operations,
        }
    }
}

/// The complete public mutation vocabulary.
///
/// This is intentionally a closed internally tagged enum. Serde's
/// `deny_unknown_fields` makes both unknown operation names and extra variant
/// fields protocol errors rather than silently ignored input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Operation {
    #[serde(rename = "summary.replace")]
    SummaryReplace { spec: String, value: String },
    #[serde(rename = "owner.add")]
    OwnerAdd { spec: String, owner: String },
    #[serde(rename = "owner.remove")]
    OwnerRemove { spec: String, owner: String },
    #[serde(rename = "pin.set")]
    PinSet { spec: String, value: String },
    #[serde(rename = "pin.clear")]
    PinClear { spec: String },
    #[serde(rename = "related.add")]
    RelatedAdd { spec: String, target: String },
    #[serde(rename = "related.remove")]
    RelatedRemove { spec: String, target: String },

    #[serde(rename = "requirement.level.set")]
    RequirementLevelSet { spec: String, level: String },
    #[serde(rename = "requirement.kind.set")]
    RequirementKindSet { spec: String, kind: String },
    #[serde(rename = "requirement.kind.clear")]
    RequirementKindClear { spec: String },
    #[serde(rename = "requirement.monotonicity.set")]
    RequirementMonotonicitySet { spec: String, value: bool },

    #[serde(rename = "invariant.enforcement.add")]
    InvariantEnforcementAdd { spec: String, value: String },
    #[serde(rename = "invariant.enforcement.remove")]
    InvariantEnforcementRemove { spec: String, value: String },
    #[serde(rename = "invariant.requirement.add")]
    InvariantRequirementAdd { spec: String, requirement: String },
    #[serde(rename = "invariant.requirement.remove")]
    InvariantRequirementRemove { spec: String, requirement: String },

    #[serde(rename = "interface.stability.set")]
    InterfaceStabilitySet { spec: String, stability: String },
    #[serde(rename = "interface.consumer.add")]
    InterfaceConsumerAdd { spec: String, consumer: String },
    #[serde(rename = "interface.consumer.remove")]
    InterfaceConsumerRemove { spec: String, consumer: String },
    #[serde(rename = "interface.provider.add")]
    InterfaceProviderAdd { spec: String, provider: String },
    #[serde(rename = "interface.provider.remove")]
    InterfaceProviderRemove { spec: String, provider: String },

    #[serde(rename = "adr.decision-date.set")]
    AdrDecisionDateSet { spec: String, value: String },
    #[serde(rename = "adr.decision-maker.add")]
    AdrDecisionMakerAdd { spec: String, owner: String },
    #[serde(rename = "adr.decision-maker.remove")]
    AdrDecisionMakerRemove { spec: String, owner: String },

    #[serde(rename = "content.title.replace")]
    ContentTitleReplace { spec: String, value: String },
    #[serde(rename = "content.section.replace")]
    ContentSectionReplace {
        spec: String,
        heading: Vec<String>,
        markdown: String,
    },
    #[serde(rename = "content.block.add")]
    ContentBlockAdd {
        spec: String,
        heading: Vec<String>,
        kind: String,
        block: String,
        level: Option<String>,
        markdown: String,
    },
    #[serde(rename = "content.block.replace")]
    ContentBlockReplace {
        spec: String,
        block: String,
        markdown: String,
    },
    #[serde(rename = "content.block.remove")]
    ContentBlockRemove { spec: String, block: String },
    #[serde(rename = "content.clause.add")]
    ContentClauseAdd {
        spec: String,
        block: String,
        clause: String,
        markdown: String,
    },
    #[serde(rename = "content.clause.replace")]
    ContentClauseReplace {
        spec: String,
        block: String,
        clause: String,
        markdown: String,
    },
    #[serde(rename = "content.clause.remove")]
    ContentClauseRemove {
        spec: String,
        block: String,
        clause: String,
    },

    #[serde(rename = "relation.refine")]
    RelationRefine { spec: String, target: String },
    #[serde(rename = "relation.unrefine")]
    RelationUnrefine { spec: String, target: String },
    #[serde(rename = "relation.aspect.add")]
    RelationAspectAdd { spec: String, aspect: String },
    #[serde(rename = "relation.aspect.remove")]
    RelationAspectRemove { spec: String, aspect: String },
    #[serde(rename = "relation.categorize")]
    RelationCategorize { spec: String, topic: String },
    #[serde(rename = "relation.uncategorize")]
    RelationUncategorize { spec: String, topic: String },

    #[serde(rename = "lifecycle.draft")]
    LifecycleDraft { spec: String },
    #[serde(rename = "lifecycle.accept")]
    LifecycleAccept { spec: String },
    #[serde(rename = "lifecycle.deprecate")]
    LifecycleDeprecate { spec: String },
    #[serde(rename = "lifecycle.supersede")]
    LifecycleSupersede { spec: String, replacement: String },

    #[serde(rename = "task.progress.set")]
    TaskProgressSet { spec: String, progress: String },
    #[serde(rename = "task.blocker.add")]
    TaskBlockerAdd { spec: String, blocker: String },
    #[serde(rename = "task.blocker.remove")]
    TaskBlockerRemove { spec: String, blocker: String },
    #[serde(rename = "task.assignee.set")]
    TaskAssigneeSet { spec: String, assignee: String },
    #[serde(rename = "task.assignee.clear")]
    TaskAssigneeClear { spec: String },
    #[serde(rename = "task.eta.set")]
    TaskEtaSet { spec: String, eta: String },
    #[serde(rename = "task.eta.clear")]
    TaskEtaClear { spec: String },

    #[serde(rename = "spec.rename")]
    SpecRename { spec: String, new_id: String },

    #[serde(rename = "documentation.collection.add")]
    DocumentationCollectionAdd {
        id: String,
        title: String,
        root: String,
        include: Vec<String>,
        #[serde(default)]
        exclude: Vec<String>,
    },
}

impl Operation {
    pub fn primary_spec(&self) -> Option<&str> {
        match self {
            Self::SummaryReplace { spec, .. }
            | Self::OwnerAdd { spec, .. }
            | Self::OwnerRemove { spec, .. }
            | Self::PinSet { spec, .. }
            | Self::PinClear { spec }
            | Self::RelatedAdd { spec, .. }
            | Self::RelatedRemove { spec, .. }
            | Self::RequirementLevelSet { spec, .. }
            | Self::RequirementKindSet { spec, .. }
            | Self::RequirementKindClear { spec }
            | Self::RequirementMonotonicitySet { spec, .. }
            | Self::InvariantEnforcementAdd { spec, .. }
            | Self::InvariantEnforcementRemove { spec, .. }
            | Self::InvariantRequirementAdd { spec, .. }
            | Self::InvariantRequirementRemove { spec, .. }
            | Self::InterfaceStabilitySet { spec, .. }
            | Self::InterfaceConsumerAdd { spec, .. }
            | Self::InterfaceConsumerRemove { spec, .. }
            | Self::InterfaceProviderAdd { spec, .. }
            | Self::InterfaceProviderRemove { spec, .. }
            | Self::AdrDecisionDateSet { spec, .. }
            | Self::AdrDecisionMakerAdd { spec, .. }
            | Self::AdrDecisionMakerRemove { spec, .. }
            | Self::ContentTitleReplace { spec, .. }
            | Self::ContentSectionReplace { spec, .. }
            | Self::ContentBlockAdd { spec, .. }
            | Self::ContentBlockReplace { spec, .. }
            | Self::ContentBlockRemove { spec, .. }
            | Self::ContentClauseAdd { spec, .. }
            | Self::ContentClauseReplace { spec, .. }
            | Self::ContentClauseRemove { spec, .. }
            | Self::RelationRefine { spec, .. }
            | Self::RelationUnrefine { spec, .. }
            | Self::RelationAspectAdd { spec, .. }
            | Self::RelationAspectRemove { spec, .. }
            | Self::RelationCategorize { spec, .. }
            | Self::RelationUncategorize { spec, .. }
            | Self::LifecycleDraft { spec }
            | Self::LifecycleAccept { spec }
            | Self::LifecycleDeprecate { spec }
            | Self::LifecycleSupersede { spec, .. }
            | Self::TaskProgressSet { spec, .. }
            | Self::TaskBlockerAdd { spec, .. }
            | Self::TaskBlockerRemove { spec, .. }
            | Self::TaskAssigneeSet { spec, .. }
            | Self::TaskAssigneeClear { spec }
            | Self::TaskEtaSet { spec, .. }
            | Self::TaskEtaClear { spec }
            | Self::SpecRename { spec, .. } => Some(spec),
            Self::DocumentationCollectionAdd { .. } => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::SummaryReplace { .. } => "summary.replace",
            Self::OwnerAdd { .. } => "owner.add",
            Self::OwnerRemove { .. } => "owner.remove",
            Self::PinSet { .. } => "pin.set",
            Self::PinClear { .. } => "pin.clear",
            Self::RelatedAdd { .. } => "related.add",
            Self::RelatedRemove { .. } => "related.remove",
            Self::RequirementLevelSet { .. } => "requirement.level.set",
            Self::RequirementKindSet { .. } => "requirement.kind.set",
            Self::RequirementKindClear { .. } => "requirement.kind.clear",
            Self::RequirementMonotonicitySet { .. } => "requirement.monotonicity.set",
            Self::InvariantEnforcementAdd { .. } => "invariant.enforcement.add",
            Self::InvariantEnforcementRemove { .. } => "invariant.enforcement.remove",
            Self::InvariantRequirementAdd { .. } => "invariant.requirement.add",
            Self::InvariantRequirementRemove { .. } => "invariant.requirement.remove",
            Self::InterfaceStabilitySet { .. } => "interface.stability.set",
            Self::InterfaceConsumerAdd { .. } => "interface.consumer.add",
            Self::InterfaceConsumerRemove { .. } => "interface.consumer.remove",
            Self::InterfaceProviderAdd { .. } => "interface.provider.add",
            Self::InterfaceProviderRemove { .. } => "interface.provider.remove",
            Self::AdrDecisionDateSet { .. } => "adr.decision-date.set",
            Self::AdrDecisionMakerAdd { .. } => "adr.decision-maker.add",
            Self::AdrDecisionMakerRemove { .. } => "adr.decision-maker.remove",
            Self::ContentTitleReplace { .. } => "content.title.replace",
            Self::ContentSectionReplace { .. } => "content.section.replace",
            Self::ContentBlockAdd { .. } => "content.block.add",
            Self::ContentBlockReplace { .. } => "content.block.replace",
            Self::ContentBlockRemove { .. } => "content.block.remove",
            Self::ContentClauseAdd { .. } => "content.clause.add",
            Self::ContentClauseReplace { .. } => "content.clause.replace",
            Self::ContentClauseRemove { .. } => "content.clause.remove",
            Self::RelationRefine { .. } => "relation.refine",
            Self::RelationUnrefine { .. } => "relation.unrefine",
            Self::RelationAspectAdd { .. } => "relation.aspect.add",
            Self::RelationAspectRemove { .. } => "relation.aspect.remove",
            Self::RelationCategorize { .. } => "relation.categorize",
            Self::RelationUncategorize { .. } => "relation.uncategorize",
            Self::LifecycleDraft { .. } => "lifecycle.draft",
            Self::LifecycleAccept { .. } => "lifecycle.accept",
            Self::LifecycleDeprecate { .. } => "lifecycle.deprecate",
            Self::LifecycleSupersede { .. } => "lifecycle.supersede",
            Self::TaskProgressSet { .. } => "task.progress.set",
            Self::TaskBlockerAdd { .. } => "task.blocker.add",
            Self::TaskBlockerRemove { .. } => "task.blocker.remove",
            Self::TaskAssigneeSet { .. } => "task.assignee.set",
            Self::TaskAssigneeClear { .. } => "task.assignee.clear",
            Self::TaskEtaSet { .. } => "task.eta.set",
            Self::TaskEtaClear { .. } => "task.eta.clear",
            Self::SpecRename { .. } => "spec.rename",
            Self::DocumentationCollectionAdd { .. } => "documentation.collection.add",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_operations_and_fields() {
        let unknown =
            r#"{"schema":"forge-spec-change/v1","operations":[{"op":"raw.set","spec":"REQ:a/b"}]}"#;
        assert!(serde_json::from_str::<ChangeRequest>(unknown).is_err());
        let extra = r#"{"schema":"forge-spec-change/v1","operations":[{"op":"summary.replace","spec":"REQ:a/b","value":"x","path":"summary"}]}"#;
        assert!(serde_json::from_str::<ChangeRequest>(extra).is_err());
    }
}
