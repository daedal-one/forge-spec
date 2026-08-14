use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::model::config::{SpecConfig, CURRENT_SPEC_BASELINE, DEFAULT_INTELLECT_PROVIDER};
use crate::parse::frontmatter::split_frontmatter;
use crate::project::{ensure_project_document, existing_project, write_project_config};

pub const LEGACY_SPEC_BASELINE: &str = "forge-spec-v0.1.0";
pub const V0_2_SPEC_BASELINE: &str = "forge-spec-v0.2.0";
pub const V0_3_SPEC_BASELINE: &str = "forge-spec-v0.3.0";
pub const V0_4_SPEC_BASELINE: &str = "forge-spec-v0.4.0";
pub const V0_5_SPEC_BASELINE: &str = "forge-spec-v0.5.0";

type ApplyMigration = fn(&Path) -> Result<MigrationStepReport>;
type VerifyMigration = fn(&Path) -> Result<()>;

struct MigrationDefinition {
    guide: &'static str,
    apply: ApplyMigration,
    verify: VerifyMigration,
}

const MIGRATIONS: &[MigrationDefinition] = &[
    MigrationDefinition {
        guide: include_str!("../../migrations/forge-spec-v0.1.0-to-v0.2.0.yaml"),
        apply: apply_v0_1_to_v0_2,
        verify: verify_v0_1_to_v0_2,
    },
    MigrationDefinition {
        guide: include_str!("../../migrations/forge-spec-v0.2.0-to-v0.3.0.yaml"),
        apply: apply_v0_2_to_v0_3,
        verify: verify_v0_2_to_v0_3,
    },
    MigrationDefinition {
        guide: include_str!("../../migrations/forge-spec-v0.3.0-to-v0.4.0.yaml"),
        apply: apply_v0_3_to_v0_4,
        verify: verify_v0_3_to_v0_4,
    },
    MigrationDefinition {
        guide: include_str!("../../migrations/forge-spec-v0.4.0-to-v0.5.0.yaml"),
        apply: apply_v0_4_to_v0_5,
        verify: verify_v0_4_to_v0_5,
    },
    MigrationDefinition {
        guide: include_str!("../../migrations/forge-spec-v0.5.0-to-v0.6.0.yaml"),
        apply: apply_v0_5_to_v0_6,
        verify: verify_v0_5_to_v0_6,
    },
];

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MigrationGuide {
    pub from: String,
    pub to: String,
    pub summary: String,
    pub changes: Vec<MigrationChange>,
    pub instructions: Vec<MigrationInstruction>,
    pub validation: Vec<MigrationValidation>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MigrationChange {
    pub id: String,
    pub kind: String,
    pub area: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MigrationInstruction {
    pub id: String,
    pub actor: InstructionActor,
    pub when: Option<String>,
    pub action: String,
    pub verification: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MigrationValidation {
    pub command: String,
    pub when: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstructionActor {
    Cli,
    Agent,
}

impl InstructionActor {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedBaseline {
    pub baseline: String,
    pub declared: bool,
}

#[derive(Debug, Clone)]
pub struct PlannedMigration {
    definition_index: usize,
    pub guide: MigrationGuide,
}

#[derive(Debug, Clone)]
pub struct MigrationPlan {
    pub from: String,
    pub to: String,
    pub steps: Vec<PlannedMigration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationStepReport {
    pub documents_changed: usize,
}

impl MigrationPlan {
    pub fn build(from: &str, to: &str) -> Result<Self> {
        let guides = load_guides()?;
        let supported = supported_baselines_from(&guides);

        if !supported.iter().any(|baseline| baseline == from) {
            bail!(
                "unsupported source baseline '{from}'; supported baselines: {}",
                supported.join(", ")
            );
        }
        if !supported.iter().any(|baseline| baseline == to) {
            bail!(
                "unsupported target baseline '{to}'; supported baselines: {}",
                supported.join(", ")
            );
        }

        let mut current = from.to_string();
        let mut visited = BTreeSet::new();
        let mut steps = Vec::new();

        while current != to {
            if !visited.insert(current.clone()) {
                bail!("migration catalog contains a cycle at '{current}'");
            }

            let matches: Vec<(usize, &MigrationGuide)> = guides
                .iter()
                .enumerate()
                .filter(|(_, guide)| guide.from == current)
                .collect();
            if matches.len() != 1 {
                bail!("no forward migration path from '{from}' to '{to}'");
            }

            let (definition_index, guide) = matches[0];
            steps.push(PlannedMigration {
                definition_index,
                guide: guide.clone(),
            });
            current.clone_from(&guide.to);
        }

        Ok(Self {
            from: from.to_string(),
            to: to.to_string(),
            steps,
        })
    }

    pub fn apply(&self, specs_dir: &Path) -> Result<Vec<MigrationStepReport>> {
        let mut reports = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let definition = &MIGRATIONS[step.definition_index];
            let report = (definition.apply)(specs_dir)
                .with_context(|| format!("migrating {} to {}", step.guide.from, step.guide.to))?;
            (definition.verify)(specs_dir).with_context(|| {
                format!(
                    "verifying migration from {} to {}",
                    step.guide.from, step.guide.to
                )
            })?;
            reports.push(report);
        }
        Ok(reports)
    }

    pub fn render_human(&self) -> String {
        let mut output = String::new();
        output.push_str("# Forge-spec migration guide\n\n");
        output.push_str(&format!("- From: `{}`\n", self.from));
        output.push_str(&format!("- To: `{}`\n", self.to));
        output.push_str(&format!("- Apply: `spec migrate apply --to {}`\n", self.to));

        if self.steps.is_empty() {
            output.push_str("\nNo format migration is required.\n");
            return output;
        }

        for step in &self.steps {
            let guide = &step.guide;
            output.push_str(&format!("\n## {} -> {}\n\n", guide.from, guide.to));
            output.push_str(&guide.summary);
            output.push_str("\n\n### Changelog\n\n");
            for change in &guide.changes {
                output.push_str(&format!(
                    "- **{} / {}** (`{}`): {}\n",
                    change.kind, change.area, change.id, change.description
                ));
            }
            output.push_str("\n### Instructions\n\n");
            for (index, instruction) in guide.instructions.iter().enumerate() {
                output.push_str(&format!(
                    "{}. **{}** (`{}`): {}\n",
                    index + 1,
                    instruction.actor.as_str(),
                    instruction.id,
                    instruction.action
                ));
                if let Some(condition) = &instruction.when {
                    output.push_str(&format!("   Applies when: {condition}\n"));
                }
                output.push_str(&format!("   Verify: {}\n", instruction.verification));
            }
            output.push_str("\n### Validation\n\n");
            for validation in &guide.validation {
                output.push_str(&format!("- `{}`", current_command(&validation.command)));
                if let Some(condition) = &validation.when {
                    output.push_str(&format!(" — {condition}"));
                }
                output.push('\n');
            }
        }

        output
    }

    pub fn render_agent(&self) -> String {
        let mut output = String::new();
        output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        output.push_str(&format!(
            "<forge-spec-migration-guide schema-version=\"1\" from=\"{}\" to=\"{}\" steps=\"{}\">\n",
            escape_xml(&self.from),
            escape_xml(&self.to),
            self.steps.len()
        ));
        output.push_str(&format!(
            "  <apply-command>spec migrate apply --to {}</apply-command>\n",
            escape_xml(&self.to)
        ));

        if self.steps.is_empty() {
            output.push_str("  <status>current</status>\n");
        }

        for step in &self.steps {
            let guide = &step.guide;
            output.push_str(&format!(
                "  <migration from=\"{}\" to=\"{}\">\n",
                escape_xml(&guide.from),
                escape_xml(&guide.to)
            ));
            output.push_str(&format!(
                "    <summary>{}</summary>\n",
                escape_xml(&guide.summary)
            ));
            output.push_str("    <changelog>\n");
            for change in &guide.changes {
                output.push_str(&format!(
                    "      <change id=\"{}\" kind=\"{}\" area=\"{}\">{}</change>\n",
                    escape_xml(&change.id),
                    escape_xml(&change.kind),
                    escape_xml(&change.area),
                    escape_xml(&change.description)
                ));
            }
            output.push_str("    </changelog>\n");
            output.push_str("    <instructions>\n");
            for (index, instruction) in guide.instructions.iter().enumerate() {
                output.push_str(&format!(
                    "      <instruction order=\"{}\" id=\"{}\" actor=\"{}\">\n",
                    index + 1,
                    escape_xml(&instruction.id),
                    instruction.actor.as_str()
                ));
                if let Some(condition) = &instruction.when {
                    output.push_str(&format!("        <when>{}</when>\n", escape_xml(condition)));
                }
                output.push_str(&format!(
                    "        <action>{}</action>\n",
                    escape_xml(&instruction.action)
                ));
                output.push_str(&format!(
                    "        <verification>{}</verification>\n",
                    escape_xml(&instruction.verification)
                ));
                output.push_str("      </instruction>\n");
            }
            output.push_str("    </instructions>\n");
            output.push_str("    <validation>\n");
            for validation in &guide.validation {
                if let Some(condition) = &validation.when {
                    output.push_str(&format!(
                        "      <command when=\"{}\">{}</command>\n",
                        escape_xml(condition),
                        escape_xml(&current_command(&validation.command))
                    ));
                } else {
                    output.push_str(&format!(
                        "      <command>{}</command>\n",
                        escape_xml(&current_command(&validation.command))
                    ));
                }
            }
            output.push_str("    </validation>\n");
            output.push_str("  </migration>\n");
        }

        output.push_str("</forge-spec-migration-guide>\n");
        output
    }
}

pub fn detect_baseline(specs_dir: &Path) -> Result<DetectedBaseline> {
    let config_path = specs_dir.join("_config.toml");
    if config_path.exists() {
        let config = SpecConfig::load(specs_dir)?;
        return Ok(DetectedBaseline {
            baseline: config.baseline,
            declared: true,
        });
    }

    let baseline = if has_legacy_version_fields(specs_dir)? {
        LEGACY_SPEC_BASELINE
    } else if existing_project(specs_dir)?.is_some() {
        CURRENT_SPEC_BASELINE
    } else {
        V0_2_SPEC_BASELINE
    };
    Ok(DetectedBaseline {
        baseline: baseline.to_string(),
        declared: false,
    })
}

pub fn write_baseline(specs_dir: &Path, baseline: &str) -> Result<bool> {
    let path = specs_dir.join("_config.toml");
    let replacement = format!("baseline = \"{baseline}\"");
    if !path.exists() {
        crate::mutation::atomic_write_files(&[(
            path.clone(),
            format!("{replacement}\n").into_bytes(),
        )])
        .with_context(|| format!("writing {}", path.display()))?;
        return Ok(true);
    }

    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut found = false;
    let mut output = String::with_capacity(content.len().max(replacement.len() + 1));
    for line in content.split_inclusive('\n') {
        if is_baseline_assignment(line) {
            found = true;
            let newline = if line.ends_with('\n') { "\n" } else { "" };
            output.push_str(&replacement);
            output.push_str(newline);
        } else {
            output.push_str(line);
        }
    }
    if !found {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&replacement);
        output.push('\n');
    }

    if output == content {
        return Ok(false);
    }
    crate::mutation::atomic_write_files(&[(path.clone(), output.into_bytes())])
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

pub fn supported_baselines() -> Result<Vec<String>> {
    let guides = load_guides()?;
    Ok(supported_baselines_from(&guides))
}

fn load_guides() -> Result<Vec<MigrationGuide>> {
    let guides: Vec<MigrationGuide> = MIGRATIONS
        .iter()
        .map(|definition| {
            let guide: MigrationGuide = serde_yaml::from_str(definition.guide)
                .context("parsing embedded migration guide")?;
            validate_guide(&guide)?;
            Ok(guide)
        })
        .collect::<Result<_>>()?;

    for pair in guides.windows(2) {
        if pair[0].to != pair[1].from {
            bail!(
                "migration catalog is not contiguous between '{}' and '{}'",
                pair[0].to,
                pair[1].from
            );
        }
    }
    if guides
        .last()
        .map(|guide| guide.to.as_str())
        .unwrap_or(CURRENT_SPEC_BASELINE)
        != CURRENT_SPEC_BASELINE
    {
        bail!("migration catalog does not terminate at {CURRENT_SPEC_BASELINE}");
    }

    Ok(guides)
}

fn validate_guide(guide: &MigrationGuide) -> Result<()> {
    if !valid_baseline(&guide.from) || !valid_baseline(&guide.to) {
        bail!(
            "invalid migration baseline pair '{} -> {}'",
            guide.from,
            guide.to
        );
    }
    if guide.from == guide.to {
        bail!("migration guide cannot target its source baseline");
    }
    if guide.summary.trim().is_empty()
        || guide.changes.is_empty()
        || guide.instructions.is_empty()
        || guide.validation.is_empty()
    {
        bail!(
            "migration guide '{} -> {}' is incomplete",
            guide.from,
            guide.to
        );
    }
    let mut ids = BTreeSet::new();
    for id in guide.changes.iter().map(|change| change.id.as_str()).chain(
        guide
            .instructions
            .iter()
            .map(|instruction| instruction.id.as_str()),
    ) {
        if id.trim().is_empty() || !ids.insert(id) {
            bail!(
                "migration guide '{} -> {}' has an empty or duplicate entry id",
                guide.from,
                guide.to
            );
        }
    }
    if guide
        .validation
        .iter()
        .any(|validation| validation.command.trim().is_empty())
    {
        bail!(
            "migration guide '{} -> {}' has an empty validation command",
            guide.from,
            guide.to
        );
    }
    Ok(())
}

fn valid_baseline(value: &str) -> bool {
    let Some(version) = value.strip_prefix("forge-spec-v") else {
        return false;
    };
    let core = version
        .split_once(['-', '+'])
        .map(|(core, _)| core)
        .unwrap_or(version);
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn supported_baselines_from(guides: &[MigrationGuide]) -> Vec<String> {
    let mut baselines = Vec::new();
    for guide in guides {
        if !baselines.contains(&guide.from) {
            baselines.push(guide.from.clone());
        }
        if !baselines.contains(&guide.to) {
            baselines.push(guide.to.clone());
        }
    }
    if baselines.is_empty() {
        baselines.push(CURRENT_SPEC_BASELINE.to_string());
    }
    baselines
}

fn has_legacy_version_fields(specs_dir: &Path) -> Result<bool> {
    for path in spec_paths(specs_dir)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let (frontmatter, _, _) = split_frontmatter(&content)
            .with_context(|| format!("reading legacy frontmatter in {}", path.display()))?;
        if frontmatter.lines().any(is_legacy_version_line) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn apply_v0_1_to_v0_2(specs_dir: &Path) -> Result<MigrationStepReport> {
    let mut report = MigrationStepReport::default();
    let mut writes = Vec::new();
    for path in spec_paths(specs_dir)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let migrated = remove_derived_frontmatter(&content)
            .with_context(|| format!("migrating {}", path.display()))?;
        if migrated != content {
            writes.push((path.clone(), migrated.into_bytes()));
            report.documents_changed += 1;
        }
    }
    crate::mutation::atomic_write_files(&writes)?;
    Ok(report)
}

fn verify_v0_1_to_v0_2(specs_dir: &Path) -> Result<()> {
    if has_legacy_version_fields(specs_dir)? {
        bail!("legacy version fields remain after migration");
    }
    Ok(())
}

fn apply_v0_2_to_v0_3(specs_dir: &Path) -> Result<MigrationStepReport> {
    let project = ensure_project_document(specs_dir, None)?;
    if !specs_dir.join("_config.toml").exists() {
        // Preserve the source baseline if migration is interrupted. The
        // command writes the target baseline only after every verifier passes.
        write_baseline(specs_dir, V0_2_SPEC_BASELINE)?;
    }
    write_project_config(specs_dir, &project.id)?;
    Ok(MigrationStepReport {
        documents_changed: usize::from(project.created),
    })
}

fn verify_v0_2_to_v0_3(specs_dir: &Path) -> Result<()> {
    let config = SpecConfig::load(specs_dir)?;
    let configured = config
        .project
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("project is not configured after migration"))?;
    let Some((document_id, _)) = existing_project(specs_dir)? else {
        bail!("PROJECT document is missing after migration");
    };
    if configured != document_id {
        bail!("configured project '{configured}' does not match migrated document '{document_id}'");
    }
    Ok(())
}

fn apply_v0_3_to_v0_4(_specs_dir: &Path) -> Result<MigrationStepReport> {
    // Documentation enrollment is intentionally explicit. Existing Markdown
    // files remain outside the knowledge graph until a maintainer adds a
    // configured collection.
    Ok(MigrationStepReport::default())
}

fn verify_v0_3_to_v0_4(specs_dir: &Path) -> Result<()> {
    // Composed plans write the final baseline only after every step verifies,
    // so the declared value may still identify an earlier source here.
    SpecConfig::load(specs_dir)?;
    Ok(())
}

fn apply_v0_4_to_v0_5(specs_dir: &Path) -> Result<MigrationStepReport> {
    let path = specs_dir.join("_config.toml");
    if !path.exists() {
        write_baseline(specs_dir, V0_4_SPEC_BASELINE)?;
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    if content.lines().any(is_intellect_provider_assignment) {
        return Ok(MigrationStepReport::default());
    }

    let assignment = format!("intellect_provider = \"{DEFAULT_INTELLECT_PROVIDER}\"\n");
    let insert_at = content
        .find("\n[[")
        .map(|offset| offset + 1)
        .unwrap_or(content.len());
    let mut output = String::with_capacity(content.len() + assignment.len() + 1);
    output.push_str(&content[..insert_at]);
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(&assignment);
    if insert_at < content.len() {
        output.push('\n');
    }
    output.push_str(&content[insert_at..]);
    crate::mutation::atomic_write_files(&[(path, output.into_bytes())])?;
    Ok(MigrationStepReport::default())
}

fn verify_v0_4_to_v0_5(specs_dir: &Path) -> Result<()> {
    let config = SpecConfig::load(specs_dir)?;
    if config.intellect_provider != DEFAULT_INTELLECT_PROVIDER {
        bail!(
            "unsupported intellect provider '{}'; forge-spec-v0.5.0 supports only '{DEFAULT_INTELLECT_PROVIDER}'",
            config.intellect_provider
        );
    }
    Ok(())
}

fn apply_v0_5_to_v0_6(specs_dir: &Path) -> Result<MigrationStepReport> {
    let mut report = MigrationStepReport::default();
    let mut writes = Vec::new();
    for path in spec_paths(specs_dir)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let migrated = migrate_task_frontmatter(&content)
            .with_context(|| format!("migrating work item {}", path.display()))?;
        if migrated != content {
            writes.push((path, migrated.into_bytes()));
            report.documents_changed += 1;
        }
    }
    crate::mutation::atomic_write_files(&writes)?;
    Ok(report)
}

fn verify_v0_5_to_v0_6(specs_dir: &Path) -> Result<()> {
    for path in spec_paths(specs_dir)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let (yaml, _, _) = split_frontmatter(&content)?;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
        let Some(mapping) = value.as_mapping() else {
            continue;
        };
        if yaml_string(mapping, "type").is_some_and(|value| value == "task") {
            for legacy in ["refines", "aspects", "categorized_under", "implemented"] {
                if mapping.contains_key(serde_yaml::Value::String(legacy.into())) {
                    bail!("legacy TASK field '{legacy}' remains in {}", path.display());
                }
            }
        }
    }
    Ok(())
}

fn migrate_task_frontmatter(content: &str) -> Result<String> {
    let (yaml, _, _) = split_frontmatter(content)?;
    let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
    let Some(mapping) = value.as_mapping() else {
        return Ok(content.to_string());
    };
    if yaml_string(mapping, "type").is_none_or(|value| value != "task") {
        return Ok(content.to_string());
    }

    let mut output = content.to_string();
    for (legacy, current) in [
        ("refines", "addresses"),
        ("aspects", "labels"),
        ("categorized_under", "groups"),
        ("implemented", "completion_checkpoint"),
    ] {
        let legacy_key = serde_yaml::Value::String(legacy.into());
        let current_key = serde_yaml::Value::String(current.into());
        if !mapping.contains_key(&legacy_key) {
            continue;
        }
        if mapping.contains_key(&current_key) {
            bail!("TASK declares both legacy '{legacy}' and v0.6 '{current}' fields");
        }
        output = rename_top_level_yaml_key(&output, legacy, current);
    }
    Ok(output)
}

fn yaml_string<'a>(mapping: &'a serde_yaml::Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(serde_yaml::Value::String(key.into()))
        .and_then(serde_yaml::Value::as_str)
}

fn rename_top_level_yaml_key(content: &str, from: &str, to: &str) -> String {
    let prefix = format!("{from}:");
    let mut in_frontmatter = false;
    let mut output = String::with_capacity(content.len() + to.len().saturating_sub(from.len()));
    for line in content.split_inclusive('\n') {
        let logical = line.trim_end_matches(['\r', '\n']);
        if logical == "---" {
            in_frontmatter = !in_frontmatter;
            output.push_str(line);
            continue;
        }
        if in_frontmatter && line.starts_with(&prefix) {
            output.push_str(to);
            output.push_str(&line[from.len()..]);
        } else {
            output.push_str(line);
        }
    }
    output
}

fn remove_derived_frontmatter(content: &str) -> Result<String> {
    let close = content
        .strip_prefix("---")
        .and_then(|rest| rest.find("\n---").map(|offset| offset + 3))
        .ok_or_else(|| anyhow::anyhow!("invalid spec frontmatter"))?;
    let (frontmatter, body) = content.split_at(close);
    let mut output = String::with_capacity(content.len());
    for line in frontmatter.split_inclusive('\n') {
        if !is_legacy_version_line(line) {
            output.push_str(line);
        }
    }
    output.push_str(body);
    Ok(output)
}

fn is_legacy_version_line(line: &str) -> bool {
    line.starts_with("version:")
        || line.starts_with("SpecBaseline:")
        || line.starts_with("spec_baseline:")
}

fn is_baseline_assignment(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("baseline")
        .map(str::trim_start)
        .map(|rest| rest.starts_with('='))
        .unwrap_or(false)
}

fn is_intellect_provider_assignment(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("intellect_provider")
        .map(str::trim_start)
        .map(|rest| rest.starts_with('='))
        .unwrap_or(false)
}

fn spec_paths(specs_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in WalkDir::new(specs_dir) {
        let entry = entry?;
        if entry.file_type().is_file()
            && entry
                .file_name()
                .to_str()
                .map(|name| name.ends_with(".spec.md"))
                .unwrap_or(false)
        {
            paths.push(entry.into_path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn current_command(command: &str) -> String {
    match command {
        "spec graph --hierarchy" => "spec inspect graph hierarchy".to_string(),
        "spec graph --refinement" => "spec inspect graph refinement".to_string(),
        "spec graph --categorization" => "spec inspect graph categorization".to_string(),
        "spec tree" => "spec inspect tree".to_string(),
        "spec coverage" => "spec inspect coverage".to_string(),
        "spec orphans" => "spec inspect orphans".to_string(),
        value => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_spec(dir: &Path, extra: &str) -> PathBuf {
        let path = dir.join("example.spec.md");
        std::fs::write(
            &path,
            format!(
                "---\nid: REQ:test/example\ntype: requirement\nstatus: draft\n{extra}owners: [carlo]\nlevel: MUST\n---\n\n# Example\n"
            ),
        )
        .unwrap();
        path
    }

    #[test]
    fn catalog_reaches_current_baseline_from_every_supported_start() {
        let baselines = supported_baselines().unwrap();
        assert_eq!(baselines.last().unwrap(), CURRENT_SPEC_BASELINE);
        for baseline in &baselines {
            MigrationPlan::build(baseline, CURRENT_SPEC_BASELINE).unwrap();
        }
    }

    #[test]
    fn rejects_downgrades() {
        let error = MigrationPlan::build(CURRENT_SPEC_BASELINE, LEGACY_SPEC_BASELINE)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no forward migration path"));
    }

    #[test]
    fn renders_composed_human_and_agent_guides() {
        let plan = MigrationPlan::build(LEGACY_SPEC_BASELINE, CURRENT_SPEC_BASELINE).unwrap();
        let human = plan.render_human();
        assert!(human.contains("### Changelog"));
        assert!(human.contains("remove-derived-version-fields"));

        let agent = plan.render_agent();
        assert!(agent.contains("<forge-spec-migration-guide"));
        assert!(agent.contains("actor=\"agent\""));
        assert!(agent.contains("<validation>"));
    }

    #[test]
    fn detects_unconfigured_baselines_from_document_shape() {
        let legacy = tempfile::tempdir().unwrap();
        write_spec(legacy.path(), "version: 0.1.0\n");
        let detected = detect_baseline(legacy.path()).unwrap();
        assert_eq!(detected.baseline, LEGACY_SPEC_BASELINE);
        assert!(!detected.declared);

        let v0_2 = tempfile::tempdir().unwrap();
        write_spec(v0_2.path(), "");
        let detected = detect_baseline(v0_2.path()).unwrap();
        assert_eq!(detected.baseline, V0_2_SPEC_BASELINE);
        assert!(!detected.declared);

        let current = tempfile::tempdir().unwrap();
        ensure_project_document(current.path(), Some("PROJECT:current")).unwrap();
        let detected = detect_baseline(current.path()).unwrap();
        assert_eq!(detected.baseline, CURRENT_SPEC_BASELINE);
        assert!(!detected.declared);
    }

    #[test]
    fn migrates_legacy_headers_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        let path = write_spec(
            temp.path(),
            "version: 0.1.0\nSpecBaseline: forge-spec-v0.1.0\n",
        );
        let plan = MigrationPlan::build(LEGACY_SPEC_BASELINE, CURRENT_SPEC_BASELINE).unwrap();
        let first = plan.apply(temp.path()).unwrap();
        let second = plan.apply(temp.path()).unwrap();
        assert_eq!(first[0].documents_changed, 1);
        assert_eq!(second[0].documents_changed, 0);
        assert_eq!(first[1].documents_changed, 1);
        assert_eq!(second[1].documents_changed, 0);
        let content = std::fs::read_to_string(path).unwrap();
        assert!(!content.contains("version:"));
        assert!(!content.contains("SpecBaseline:"));
    }

    #[test]
    fn updates_baseline_without_discarding_other_config() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("_config.toml");
        std::fs::write(
            &path,
            "baseline = \"forge-spec-v0.1.0\"\nowner = \"team\"\n",
        )
        .unwrap();
        assert!(write_baseline(temp.path(), CURRENT_SPEC_BASELINE).unwrap());
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains(&format!("baseline = \"{CURRENT_SPEC_BASELINE}\"")));
        assert!(content.contains("owner = \"team\""));
    }

    #[test]
    fn project_migration_never_overwrites_a_colliding_file() {
        let temp = tempfile::tempdir().unwrap();
        let collision = temp.path().join("_project.spec.md");
        let original = "not a project document\n";
        std::fs::write(&collision, original).unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            format!("baseline = \"{V0_2_SPEC_BASELINE}\"\n"),
        )
        .unwrap();

        let plan = MigrationPlan::build(V0_2_SPEC_BASELINE, CURRENT_SPEC_BASELINE).unwrap();
        let error = plan.apply(temp.path()).unwrap_err();

        assert!(format!("{error:#}").contains("already exists"));
        assert_eq!(std::fs::read_to_string(collision).unwrap(), original);
        assert_eq!(
            SpecConfig::load(temp.path()).unwrap().baseline,
            V0_2_SPEC_BASELINE
        );
    }

    #[test]
    fn v0_5_migration_configures_provider_without_inventing_checkpoints() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            format!("baseline = \"{V0_4_SPEC_BASELINE}\"\nproject = \"PROJECT:demo\"\n"),
        )
        .unwrap();
        let spec = temp.path().join("_project.spec.md");
        std::fs::write(
            &spec,
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        let plan = MigrationPlan::build(V0_4_SPEC_BASELINE, CURRENT_SPEC_BASELINE).unwrap();
        plan.apply(temp.path()).unwrap();
        plan.apply(temp.path()).unwrap();

        let config = std::fs::read_to_string(temp.path().join("_config.toml")).unwrap();
        assert_eq!(config.matches("intellect_provider").count(), 1);
        assert!(config.contains(&format!(
            "intellect_provider = \"{DEFAULT_INTELLECT_PROVIDER}\""
        )));
        assert!(!std::fs::read_to_string(spec)
            .unwrap()
            .contains("implemented:"));
    }

    #[test]
    fn v0_6_migration_separates_task_fields_losslessly_and_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            format!("baseline = \"{V0_5_SPEC_BASELINE}\"\nproject = \"PROJECT:demo\"\n"),
        )
        .unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        let task = temp.path().join("work.spec.md");
        let checkpoint = "0123456789abcdef0123456789abcdef01234567";
        std::fs::write(
            &task,
            format!(
                "---\nid: TASK:demo/work\ntype: task\nstatus: accepted\nsummary: Work.\nowners: [dev]\nimplemented: {checkpoint}\nprogress: done\nrefines: [PROJECT:demo]\naspects: [bootstrap]\ncategorized_under: []\nblocked_by: []\n---\n\n# Work\n\nrefines: remains ordinary body text\n"
            ),
        )
        .unwrap();

        let plan = MigrationPlan::build(V0_5_SPEC_BASELINE, CURRENT_SPEC_BASELINE).unwrap();
        let first = plan.apply(temp.path()).unwrap();
        let second = plan.apply(temp.path()).unwrap();
        assert_eq!(first[0].documents_changed, 1);
        assert_eq!(second[0].documents_changed, 0);

        let content = std::fs::read_to_string(&task).unwrap();
        assert!(content.contains("completion_checkpoint: 0123456789abcdef"));
        assert!(content.contains("addresses: [PROJECT:demo]"));
        assert!(content.contains("labels: [bootstrap]"));
        assert!(content.contains("groups: []"));
        assert!(content.contains("refines: remains ordinary body text"));
        let document = crate::parse::parse_document(&task).unwrap();
        let crate::model::frontmatter::TypeSpecificFields::Task {
            addresses,
            labels,
            groups,
            completion_checkpoint,
            ..
        } = document.type_fields
        else {
            panic!("expected work item")
        };
        assert_eq!(addresses, ["PROJECT:demo"]);
        assert_eq!(labels, ["bootstrap"]);
        assert!(groups.is_empty());
        assert_eq!(completion_checkpoint.as_deref(), Some(checkpoint));
    }
}
