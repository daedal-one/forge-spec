use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::model::config::DEFAULT_INTELLECT_PROVIDER;
use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;

pub const INTELLECT_PROTOCOL: &str = "forge-spec-intellect/v1";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub root: String,
    pub head: String,
    pub worktree: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdherenceState {
    Unverified,
    Current,
    Stale,
    Partial,
    Violated,
    Unknown,
    Unresolved,
    NotApplicable,
}

impl AdherenceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Partial => "partial",
            Self::Violated => "violated",
            Self::Unknown => "unknown",
            Self::Unresolved => "unresolved",
            Self::NotApplicable => "not-applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecAdherence {
    pub id: String,
    pub checkpoint: Option<String>,
    pub state: AdherenceState,
    pub complete: bool,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdherenceSnapshot {
    pub schema: String,
    pub provider: String,
    pub provider_version: String,
    pub workspace: WorkspaceState,
    pub complete: bool,
    pub specifications: Vec<SpecAdherence>,
}

impl AdherenceSnapshot {
    pub fn get(&self, id: &str) -> Option<&SpecAdherence> {
        self.specifications.iter().find(|state| state.id == id)
    }

    fn unavailable(registry: &SpecRegistry, reason: String) -> Self {
        let root = registry
            .specs_dir
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .unwrap_or_else(|_| {
                registry
                    .specs_dir
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            });
        let workspace = workspace_state(&root).unwrap_or_else(|_| WorkspaceState {
            root: root.to_string_lossy().into_owned(),
            head: "unavailable".into(),
            worktree: "unavailable".into(),
        });
        let mut specifications = registry
            .documents
            .iter()
            .map(|document| SpecAdherence {
                id: document.id_str(),
                checkpoint: document.universal.implemented.clone(),
                state: AdherenceState::Unknown,
                complete: false,
                reasons: vec![reason.clone()],
                evidence: Vec::new(),
            })
            .collect::<Vec<_>>();
        specifications.sort_by(|left, right| left.id.cmp(&right.id));
        Self {
            schema: INTELLECT_PROTOCOL.into(),
            provider: registry.config.intellect_provider.clone(),
            provider_version: "unavailable".into(),
            workspace,
            complete: false,
            specifications,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "request", rename_all = "kebab-case")]
enum ProviderRequest {
    Health {
        schema: &'static str,
    },
    Adherence {
        schema: &'static str,
        workspace: WorkspaceState,
        specifications: Vec<SpecificationRequest>,
    },
    Shutdown {
        schema: &'static str,
    },
}

#[derive(Debug, Serialize)]
struct SpecificationRequest {
    id: String,
    entity_type: String,
    status: String,
    implemented: Option<String>,
    candidate: Option<String>,
    path: String,
    source_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    schema: String,
    response: String,
    provider: String,
    version: String,
    ready: bool,
}

#[derive(Debug, Deserialize)]
struct AdherenceResponse {
    schema: String,
    response: String,
    provider: String,
    version: String,
    workspace: WorkspaceState,
    complete: bool,
    specifications: Vec<SpecAdherence>,
}

#[derive(Debug, Deserialize)]
struct ShutdownResponse {
    schema: String,
    response: String,
}

pub fn fetch(
    registry: &SpecRegistry,
    candidates: &BTreeMap<String, String>,
) -> Result<AdherenceSnapshot> {
    if registry.config.intellect_provider != DEFAULT_INTELLECT_PROVIDER {
        bail!(
            "unsupported intellect provider '{}'; this release supports only '{DEFAULT_INTELLECT_PROVIDER}'",
            registry.config.intellect_provider
        );
    }

    let root = workspace_root(&registry.specs_dir)?;
    let workspace = workspace_state(&root)?;
    let mut specifications = registry
        .documents
        .iter()
        .map(|document| {
            let source_path = if document.source_path.is_absolute() {
                document.source_path.clone()
            } else {
                root.join(&document.source_path)
            };
            let path = source_path
                .strip_prefix(&root)
                .with_context(|| {
                    format!(
                        "specification {} is outside workspace {}",
                        source_path.display(),
                        root.display()
                    )
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let mut source_paths = document
                .references
                .iter()
                .filter_map(|located| match &located.reference {
                    SpecReference::Source(source) => Some(source.path.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            source_paths.sort();
            source_paths.dedup();
            Ok(SpecificationRequest {
                id: document.id_str(),
                entity_type: document.universal.entity_type.prefix().into(),
                status: document.universal.status.as_str().into(),
                implemented: document.universal.implemented.clone(),
                candidate: candidates.get(&document.id_str()).cloned(),
                path,
                source_paths,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    specifications.sort_by(|left, right| left.id.cmp(&right.id));

    let mut client = ProviderClient::start(&root)?;
    let health: HealthResponse = client.exchange(&ProviderRequest::Health {
        schema: INTELLECT_PROTOCOL,
    })?;
    if health.schema != INTELLECT_PROTOCOL
        || health.response != "health"
        || health.provider != DEFAULT_INTELLECT_PROVIDER
        || !health.ready
    {
        bail!("intellect provider returned an invalid health response");
    }

    let response: AdherenceResponse = client.exchange(&ProviderRequest::Adherence {
        schema: INTELLECT_PROTOCOL,
        workspace: workspace.clone(),
        specifications,
    })?;
    client.shutdown()?;

    if response.schema != INTELLECT_PROTOCOL
        || response.response != "adherence"
        || response.provider != DEFAULT_INTELLECT_PROVIDER
        || response.version != health.version
        || response.workspace != workspace
    {
        bail!("intellect provider returned a response for a different protocol or workspace state");
    }
    let expected = registry
        .documents
        .iter()
        .map(|document| document.id_str())
        .collect::<BTreeSet<_>>();
    let actual = response
        .specifications
        .iter()
        .map(|state| state.id.clone())
        .collect::<BTreeSet<_>>();
    if actual.len() != response.specifications.len() || actual != expected {
        bail!("intellect provider returned duplicate, missing, or unexpected specification states");
    }

    Ok(AdherenceSnapshot {
        schema: response.schema,
        provider: response.provider,
        provider_version: response.version,
        workspace: response.workspace,
        complete: response.complete,
        specifications: response.specifications,
    })
}

pub fn fetch_or_unknown(registry: &SpecRegistry) -> Result<AdherenceSnapshot> {
    match fetch(registry, &BTreeMap::new()) {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            let reason = format!("intellect provider unavailable: {error:#}");
            eprintln!("warning: {reason}");
            Ok(AdherenceSnapshot::unavailable(registry, reason))
        }
    }
}

pub fn current_workspace_commit(specs_dir: &Path) -> Result<String> {
    Ok(workspace_state(&workspace_root(specs_dir)?)?.head)
}

fn workspace_root(specs_dir: &Path) -> Result<PathBuf> {
    specs_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| Some(Path::new(".")))
        .context("specification directory has no workspace parent")?
        .canonicalize()
        .context("resolving workspace root")
}

fn workspace_state(root: &Path) -> Result<WorkspaceState> {
    let repository =
        git2::Repository::discover(root).context("opening workspace Git repository")?;
    let workdir = repository
        .workdir()
        .context("intellect provider requires a non-bare Git workspace")?
        .canonicalize()
        .context("resolving Git workspace")?;
    if workdir != root {
        bail!(
            "specification directory must be directly under the Git workspace root {}; found {}",
            workdir.display(),
            root.display()
        );
    }
    let head = repository
        .head()
        .context("resolving workspace HEAD")?
        .peel_to_commit()
        .context("resolving workspace HEAD commit")?
        .id()
        .to_string();
    let manifest = worktree_manifest(root)?;
    let worktree = if manifest.is_empty() {
        "clean".into()
    } else {
        git_hash_object(root, &manifest)?
    };
    Ok(WorkspaceState {
        root: root.to_string_lossy().into_owned(),
        head,
        worktree,
    })
}

fn worktree_manifest(root: &Path) -> Result<Vec<u8>> {
    let diff = git_output(root, &["diff", "--binary", "--full-index", "HEAD", "--"])?;
    let untracked = git_output(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let mut manifest = diff;
    for path in untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path_text = std::str::from_utf8(path).context("untracked path is not UTF-8")?;
        manifest.extend_from_slice(b"\0untracked\0");
        manifest.extend_from_slice(path);
        manifest.push(0);
        manifest.extend_from_slice(
            &std::fs::read(root.join(path_text))
                .with_context(|| format!("reading untracked file {path_text}"))?,
        );
    }
    Ok(manifest)
}

fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn git_hash_object(root: &Path, bytes: &[u8]) -> Result<String> {
    let mut child = Command::new("git")
        .args(["hash-object", "--stdin"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting git hash-object")?;
    child
        .stdin
        .take()
        .context("opening git hash-object stdin")?
        .write_all(bytes)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "git hash-object failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

struct ProviderClient {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<std::result::Result<String, String>>,
    reader: Option<JoinHandle<()>>,
    stopped: bool,
}

impl ProviderClient {
    fn start(root: &Path) -> Result<Self> {
        let executable = std::env::var_os("FORGE_SPEC_INTELLECT_PROVIDER_BIN")
            .unwrap_or_else(|| DEFAULT_INTELLECT_PROVIDER.into());
        let mut child = Command::new(&executable)
            .args(["provider", "--stdio"])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "starting intellect provider {}",
                    executable.to_string_lossy()
                )
            })?;
        let stdin = child.stdin.take().context("opening provider stdin")?;
        let stdout = child.stdout.take().context("opening provider stdout")?;
        let (sender, responses) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let result = line.map_err(|error| error.to_string());
                if sender.send(result).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            responses,
            reader: Some(reader),
            stopped: false,
        })
    }

    fn exchange<T: for<'de> Deserialize<'de>>(&mut self, request: &ProviderRequest) -> Result<T> {
        serde_json::to_writer(&mut self.stdin, request).context("encoding provider request")?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        let line = self
            .responses
            .recv_timeout(RESPONSE_TIMEOUT)
            .context("timed out waiting for intellect provider")?
            .map_err(anyhow::Error::msg)?;
        serde_json::from_str(&line).context("decoding intellect provider response")
    }

    fn shutdown(&mut self) -> Result<()> {
        let response: ShutdownResponse = self.exchange(&ProviderRequest::Shutdown {
            schema: INTELLECT_PROTOCOL,
        })?;
        if response.schema != INTELLECT_PROTOCOL || response.response != "shutdown" {
            bail!("intellect provider returned an invalid shutdown response");
        }
        let status = self
            .child
            .wait()
            .context("waiting for intellect provider")?;
        self.stopped = true;
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if !status.success() {
            bail!("intellect provider exited with {status}");
        }
        Ok(())
    }
}

impl Drop for ProviderClient {
    fn drop(&mut self) {
        if !self.stopped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
