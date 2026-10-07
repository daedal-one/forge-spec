use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::model::config::DEFAULT_INTELLECT_PROVIDER;
use crate::model::id::EntityType;
use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;

pub const INTELLECT_PROTOCOL: &str = "forge-spec-intellect/v2";
pub const PROVIDER_CONTROL_SCHEMA: &str = "forge-intellect-provider-control/v1";
pub const DEFAULT_IDLE_TIMEOUT_SECONDS: u64 = 300;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const START_TIMEOUT: Duration = Duration::from_secs(5);
const START_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct SpecAdherence {
    pub id: String,
    pub intent_digest: String,
    pub attestation_id: Option<String>,
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
            .filter(|document| document.universal.entity_type != EntityType::Task)
            .map(|document| SpecAdherence {
                id: document.id_str(),
                intent_digest: crate::projection::specification_intent_digest(document)
                    .unwrap_or_else(|_| "unavailable".into()),
                attestation_id: None,
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
        authorization: String,
    },
    Adherence {
        schema: &'static str,
        authorization: String,
        workspace: WorkspaceState,
        specifications: Vec<SpecificationRequest>,
    },
    Attest {
        schema: &'static str,
        authorization: String,
        workspace: WorkspaceState,
        specifications: Vec<SpecificationRequest>,
        candidate: String,
    },
    Revoke {
        schema: &'static str,
        authorization: String,
        workspace: WorkspaceState,
        specifications: Vec<SpecificationRequest>,
        reason: String,
    },
    ImportLegacy {
        schema: &'static str,
        authorization: String,
        workspace: WorkspaceState,
        specifications: Vec<SpecificationRequest>,
    },
    Shutdown {
        schema: &'static str,
        authorization: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderControl {
    pub schema: String,
    pub provider: String,
    pub protocol: String,
    pub workspace_root: String,
    pub endpoint: String,
    pub authorization: String,
    pub pid: u32,
    pub started_at_micros: u64,
    pub idle_timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderServiceStatus {
    Running(ProviderControl),
    Stopped,
    Stale { reason: String },
}

#[derive(Debug, Serialize)]
struct SpecificationRequest {
    id: String,
    entity_type: String,
    status: String,
    intent_digest: String,
    legacy_checkpoint: Option<String>,
    path: String,
    source_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthResponse {
    schema: String,
    response: String,
    provider: String,
    version: String,
    ready: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
struct ShutdownResponse {
    schema: String,
    response: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderErrorResponse {
    schema: String,
    response: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ProviderResponse<T> {
    Success(T),
    Error(ProviderErrorResponse),
}

enum ClientOperation<'a> {
    Status,
    Attest {
        ids: &'a BTreeSet<String>,
        candidate: &'a str,
    },
    Revoke {
        ids: &'a BTreeSet<String>,
        reason: &'a str,
    },
    ImportLegacy {
        ids: &'a BTreeSet<String>,
    },
}

pub fn fetch(registry: &SpecRegistry) -> Result<AdherenceSnapshot> {
    exchange(registry, ClientOperation::Status)
}

pub fn attest(
    registry: &SpecRegistry,
    ids: &BTreeSet<String>,
    candidate: &str,
) -> Result<AdherenceSnapshot> {
    exchange(registry, ClientOperation::Attest { ids, candidate })
}

pub fn revoke(
    registry: &SpecRegistry,
    ids: &BTreeSet<String>,
    reason: &str,
) -> Result<AdherenceSnapshot> {
    exchange(registry, ClientOperation::Revoke { ids, reason })
}

pub fn import_legacy(registry: &SpecRegistry, ids: &BTreeSet<String>) -> Result<AdherenceSnapshot> {
    exchange(registry, ClientOperation::ImportLegacy { ids })
}

fn exchange(registry: &SpecRegistry, operation: ClientOperation<'_>) -> Result<AdherenceSnapshot> {
    if registry.config.intellect_provider != DEFAULT_INTELLECT_PROVIDER {
        bail!(
            "unsupported intellect provider '{}'; this release supports only '{DEFAULT_INTELLECT_PROVIDER}'",
            registry.config.intellect_provider
        );
    }

    let root = workspace_root(&registry.specs_dir)?;
    let workspace = workspace_state(&root)?;
    let selected = match &operation {
        ClientOperation::Status => None,
        ClientOperation::Attest { ids, .. }
        | ClientOperation::Revoke { ids, .. }
        | ClientOperation::ImportLegacy { ids } => Some(*ids),
    };
    let mut specifications = registry
        .documents
        .iter()
        .filter(|document| document.universal.entity_type != EntityType::Task)
        .filter(|document| selected.map_or(true, |ids| ids.contains(&document.id_str())))
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
                intent_digest: crate::projection::specification_intent_digest(document)?,
                legacy_checkpoint: document.universal.implemented.clone(),
                path,
                source_paths,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    specifications.sort_by(|left, right| left.id.cmp(&right.id));
    if let Some(ids) = selected {
        let actual = specifications
            .iter()
            .map(|specification| specification.id.clone())
            .collect::<BTreeSet<_>>();
        if &actual != ids {
            let missing = ids.difference(&actual).cloned().collect::<Vec<_>>();
            bail!(
                "unknown or inapplicable specification(s): {}",
                missing.join(", ")
            );
        }
    }

    let control = ensure_service(&root, DEFAULT_IDLE_TIMEOUT_SECONDS)?;
    let mut client = ProviderClient::connect(&control)?;
    let health: HealthResponse = client.exchange(&ProviderRequest::Health {
        schema: INTELLECT_PROTOCOL,
        authorization: control.authorization.clone(),
    })?;
    if health.schema != INTELLECT_PROTOCOL
        || health.response != "health"
        || health.provider != DEFAULT_INTELLECT_PROVIDER
        || !health.ready
    {
        bail!("intellect provider returned an invalid health response");
    }

    let (request, expected_response) = match operation {
        ClientOperation::Status => (
            ProviderRequest::Adherence {
                schema: INTELLECT_PROTOCOL,
                authorization: control.authorization.clone(),
                workspace: workspace.clone(),
                specifications,
            },
            "adherence",
        ),
        ClientOperation::Attest { candidate, .. } => (
            ProviderRequest::Attest {
                schema: INTELLECT_PROTOCOL,
                authorization: control.authorization.clone(),
                workspace: workspace.clone(),
                specifications,
                candidate: candidate.into(),
            },
            "attest",
        ),
        ClientOperation::Revoke { reason, .. } => (
            ProviderRequest::Revoke {
                schema: INTELLECT_PROTOCOL,
                authorization: control.authorization.clone(),
                workspace: workspace.clone(),
                specifications,
                reason: reason.into(),
            },
            "revoke",
        ),
        ClientOperation::ImportLegacy { .. } => (
            ProviderRequest::ImportLegacy {
                schema: INTELLECT_PROTOCOL,
                authorization: control.authorization.clone(),
                workspace: workspace.clone(),
                specifications,
            },
            "import-legacy",
        ),
    };
    let response: AdherenceResponse = client.exchange(&request)?;

    if response.schema != INTELLECT_PROTOCOL
        || response.response != expected_response
        || response.provider != DEFAULT_INTELLECT_PROVIDER
        || response.version != health.version
        || response.workspace != workspace
    {
        bail!("intellect provider returned a response for a different protocol or workspace state");
    }
    let expected = selected.cloned().unwrap_or_else(|| {
        registry
            .documents
            .iter()
            .filter(|document| document.universal.entity_type != EntityType::Task)
            .map(|document| document.id_str())
            .collect::<BTreeSet<_>>()
    });
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
    match fetch(registry) {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            let reason = format!("intellect provider unavailable: {error:#}");
            eprintln!("warning: {reason}");
            Ok(AdherenceSnapshot::unavailable(registry, reason))
        }
    }
}

pub fn start_service(
    registry: &SpecRegistry,
    idle_timeout_seconds: u64,
) -> Result<ProviderControl> {
    validate_provider(registry)?;
    if idle_timeout_seconds == 0 {
        bail!("provider idle timeout must be greater than zero");
    }
    ensure_service(&workspace_root(&registry.specs_dir)?, idle_timeout_seconds)
}

pub fn service_status(registry: &SpecRegistry) -> Result<ProviderServiceStatus> {
    validate_provider(registry)?;
    inspect_service(&workspace_root(&registry.specs_dir)?)
}

pub fn stop_service(registry: &SpecRegistry) -> Result<ProviderServiceStatus> {
    validate_provider(registry)?;
    let root = workspace_root(&registry.specs_dir)?;
    let paths = provider_paths(&root)?;
    match inspect_service(&root)? {
        ProviderServiceStatus::Running(control) => {
            let mut client = ProviderClient::connect(&control)?;
            let response: ShutdownResponse = client.exchange(&ProviderRequest::Shutdown {
                schema: INTELLECT_PROTOCOL,
                authorization: control.authorization.clone(),
            })?;
            if response.schema != INTELLECT_PROTOCOL || response.response != "shutdown" {
                bail!("intellect provider returned an invalid shutdown response");
            }
            let deadline = Instant::now() + START_TIMEOUT;
            while paths.control.exists() && Instant::now() < deadline {
                thread::sleep(START_POLL_INTERVAL);
            }
            if paths.control.exists() {
                bail!("intellect provider did not remove its control file after shutdown");
            }
            Ok(ProviderServiceStatus::Stopped)
        }
        ProviderServiceStatus::Stale { reason } => {
            let _ = std::fs::remove_file(&paths.control);
            Ok(ProviderServiceStatus::Stale { reason })
        }
        ProviderServiceStatus::Stopped => Ok(ProviderServiceStatus::Stopped),
    }
}

fn validate_provider(registry: &SpecRegistry) -> Result<()> {
    if registry.config.intellect_provider != DEFAULT_INTELLECT_PROVIDER {
        bail!(
            "unsupported intellect provider '{}'; this release supports only '{DEFAULT_INTELLECT_PROVIDER}'",
            registry.config.intellect_provider
        );
    }
    Ok(())
}

fn ensure_service(root: &Path, idle_timeout_seconds: u64) -> Result<ProviderControl> {
    if let ProviderServiceStatus::Running(control) = inspect_service(root)? {
        return Ok(control);
    }
    let paths = provider_paths(root)?;
    std::fs::create_dir_all(&paths.directory).with_context(|| {
        format!(
            "creating intellect provider state directory {}",
            paths.directory.display()
        )
    })?;

    for attempt in 0..3 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&paths.lock)
        {
            Ok(mut lock) => {
                writeln!(lock, "{}", std::process::id())?;
                let _guard = StartLock(paths.lock.clone());
                if let ProviderServiceStatus::Running(control) = inspect_service(root)? {
                    return Ok(control);
                }
                if paths.control.exists() {
                    std::fs::remove_file(&paths.control).with_context(|| {
                        format!(
                            "removing stale provider registration {}",
                            paths.control.display()
                        )
                    })?;
                }
                return launch_service(root, &paths, idle_timeout_seconds);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let deadline = Instant::now() + START_TIMEOUT;
                while Instant::now() < deadline {
                    if let ProviderServiceStatus::Running(control) = inspect_service(root)? {
                        return Ok(control);
                    }
                    thread::sleep(START_POLL_INTERVAL);
                }
                if attempt < 2 {
                    let _ = std::fs::remove_file(&paths.lock);
                }
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("acquiring provider start lock {}", paths.lock.display())
                });
            }
        }
    }
    bail!(
        "timed out waiting for intellect provider startup lock {}",
        paths.lock.display()
    )
}

fn launch_service(
    root: &Path,
    paths: &ProviderPaths,
    idle_timeout_seconds: u64,
) -> Result<ProviderControl> {
    let executable = std::env::var_os("FORGE_SPEC_INTELLECT_PROVIDER_BIN")
        .unwrap_or_else(|| DEFAULT_INTELLECT_PROVIDER.into());
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .with_context(|| format!("opening provider log {}", paths.log.display()))?;
    let stderr = log.try_clone()?;
    let mut child = Command::new(&executable)
        .args(["provider", "serve", "--workspace-root"])
        .arg(root)
        .arg("--control-file")
        .arg(&paths.control)
        .arg("--idle-timeout-seconds")
        .arg(idle_timeout_seconds.to_string())
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| {
            format!(
                "starting intellect provider {}",
                executable.to_string_lossy()
            )
        })?;

    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            bail!(
                "intellect provider exited during startup with {status}; log: {}",
                paths.log.display()
            );
        }
        if let ProviderServiceStatus::Running(control) = inspect_service(root)? {
            return Ok(control);
        }
        thread::sleep(START_POLL_INTERVAL);
    }
    let _ = child.kill();
    let _ = child.wait();
    bail!(
        "timed out starting intellect provider; inspect {}",
        paths.log.display()
    )
}

fn inspect_service(root: &Path) -> Result<ProviderServiceStatus> {
    let paths = provider_paths(root)?;
    if !paths.control.exists() {
        return Ok(ProviderServiceStatus::Stopped);
    }
    let control = match read_control(&paths.control, root) {
        Ok(control) => control,
        Err(error) => {
            return Ok(ProviderServiceStatus::Stale {
                reason: format!("invalid provider registration: {error:#}"),
            });
        }
    };
    match provider_health(&control) {
        Ok(_) => Ok(ProviderServiceStatus::Running(control)),
        Err(error) => Ok(ProviderServiceStatus::Stale {
            reason: format!("registered provider is unreachable: {error:#}"),
        }),
    }
}

fn provider_health(control: &ProviderControl) -> Result<HealthResponse> {
    let mut client = ProviderClient::connect(control)?;
    let response: HealthResponse = client.exchange(&ProviderRequest::Health {
        schema: INTELLECT_PROTOCOL,
        authorization: control.authorization.clone(),
    })?;
    if response.schema != INTELLECT_PROTOCOL
        || response.response != "health"
        || response.provider != DEFAULT_INTELLECT_PROVIDER
        || !response.ready
    {
        bail!("intellect provider returned an invalid health response");
    }
    Ok(response)
}

fn read_control(path: &Path, root: &Path) -> Result<ProviderControl> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading provider registration {}", path.display()))?;
    let control: ProviderControl = serde_json::from_slice(&bytes)
        .with_context(|| format!("decoding provider registration {}", path.display()))?;
    if control.schema != PROVIDER_CONTROL_SCHEMA
        || control.provider != DEFAULT_INTELLECT_PROVIDER
        || control.protocol != INTELLECT_PROTOCOL
    {
        bail!("provider registration uses an unsupported schema or provider");
    }
    if Path::new(&control.workspace_root) != root {
        bail!(
            "provider registration belongs to workspace {}",
            control.workspace_root
        );
    }
    let endpoint = control
        .endpoint
        .parse::<SocketAddr>()
        .context("provider registration has an invalid endpoint")?;
    if !endpoint.ip().is_loopback() {
        bail!("provider registration endpoint is not loopback");
    }
    Ok(control)
}

fn provider_paths(root: &Path) -> Result<ProviderPaths> {
    let repository = git2::Repository::open(root).context("opening workspace Git repository")?;
    let git_dir = repository
        .path()
        .canonicalize()
        .context("resolving worktree Git directory")?;
    let directory = git_dir.join("forge-intellect");
    Ok(ProviderPaths {
        control: directory.join("provider.json"),
        lock: directory.join("provider.start.lock"),
        log: directory.join("provider.log"),
        directory,
    })
}

struct ProviderPaths {
    directory: PathBuf,
    control: PathBuf,
    lock: PathBuf,
    log: PathBuf,
}

struct StartLock(PathBuf);

impl Drop for StartLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
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
        let (marker, content) = untracked_entry(root, path_text)?;
        manifest.extend_from_slice(marker);
        manifest.extend_from_slice(path);
        manifest.push(0);
        manifest.extend_from_slice(&content);
    }
    Ok(manifest)
}

fn untracked_entry(root: &Path, path: &str) -> Result<(&'static [u8], Vec<u8>)> {
    let absolute = root.join(path);
    let metadata = std::fs::symlink_metadata(&absolute)
        .with_context(|| format!("inspecting untracked file {path}"))?;
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(&absolute)
            .with_context(|| format!("reading untracked symbolic link {path}"))?;
        Ok((
            b"\0untracked-symlink\0",
            target.into_os_string().into_encoded_bytes(),
        ))
    } else if metadata.is_file() {
        Ok((
            b"\0untracked\0",
            std::fs::read(&absolute).with_context(|| format!("reading untracked file {path}"))?,
        ))
    } else {
        bail!("unsupported untracked file type {path}")
    }
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
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl ProviderClient {
    fn connect(control: &ProviderControl) -> Result<Self> {
        let endpoint = control
            .endpoint
            .parse::<SocketAddr>()
            .context("provider endpoint is invalid")?;
        let writer = TcpStream::connect_timeout(&endpoint, RESPONSE_TIMEOUT)
            .with_context(|| format!("connecting to intellect provider at {endpoint}"))?;
        writer.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        writer.set_write_timeout(Some(RESPONSE_TIMEOUT))?;
        let reader = BufReader::new(writer.try_clone()?);
        Ok(Self { reader, writer })
    }

    fn exchange<T: for<'de> Deserialize<'de>>(&mut self, request: &ProviderRequest) -> Result<T> {
        serde_json::to_writer(&mut self.writer, request).context("encoding provider request")?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        let mut line = String::new();
        let read = self
            .reader
            .read_line(&mut line)
            .context("reading intellect provider response")?;
        if read == 0 {
            bail!("intellect provider closed the connection without a response");
        }
        decode_provider_response(&line)
    }
}

fn decode_provider_response<T: for<'de> Deserialize<'de>>(line: &str) -> Result<T> {
    let response: ProviderResponse<T> =
        serde_json::from_str(line).context("decoding intellect provider response")?;
    match response {
        ProviderResponse::Success(response) => Ok(response),
        ProviderResponse::Error(error) => {
            if error.schema != INTELLECT_PROTOCOL || error.response != "error" {
                bail!("intellect provider returned an invalid error response");
            }
            bail!("intellect provider rejected request: {}", error.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_workspace() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "--quiet"],
            vec![
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "initial",
            ],
        ] {
            git_output(temp.path(), &args).unwrap();
        }
        temp
    }

    #[test]
    fn untracked_regular_files_preserve_manifest_encoding() {
        let temp = git_workspace();
        assert!(worktree_manifest(temp.path()).unwrap().is_empty());
        std::fs::write(temp.path().join("entry"), "first").unwrap();
        assert_eq!(
            worktree_manifest(temp.path()).unwrap(),
            b"\0untracked\0entry\0first"
        );
        std::fs::write(temp.path().join("entry"), "second").unwrap();
        assert_eq!(
            worktree_manifest(temp.path()).unwrap(),
            b"\0untracked\0entry\0second"
        );
    }

    #[cfg(unix)]
    #[test]
    fn untracked_symlinks_hash_targets_without_following_them() {
        use std::os::unix::fs::symlink;

        let temp = git_workspace();
        let external = tempfile::tempdir().unwrap();
        let target = external.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("content"), "first").unwrap();
        let link = temp.path().join("entry");
        symlink(&target, &link).unwrap();
        let mut expected = b"\0untracked-symlink\0entry\0".to_vec();
        expected.extend_from_slice(target.as_os_str().as_encoded_bytes());
        assert_eq!(worktree_manifest(temp.path()).unwrap(), expected);
        std::fs::write(target.join("content"), "second").unwrap();
        assert_eq!(worktree_manifest(temp.path()).unwrap(), expected);

        std::fs::remove_file(&link).unwrap();
        symlink("missing", &link).unwrap();
        let dangling = worktree_manifest(temp.path()).unwrap();
        assert_eq!(dangling, b"\0untracked-symlink\0entry\0missing");
        assert_ne!(dangling, expected);

        std::fs::remove_file(&link).unwrap();
        let file = external.path().join("file");
        std::fs::write(&file, "first").unwrap();
        symlink(&file, &link).unwrap();
        let before = worktree_manifest(temp.path()).unwrap();
        std::fs::write(&file, "second").unwrap();
        assert_eq!(worktree_manifest(temp.path()).unwrap(), before);
        std::fs::remove_file(&link).unwrap();
        std::fs::write(&link, file.as_os_str().as_encoded_bytes()).unwrap();
        assert_ne!(worktree_manifest(temp.path()).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn untracked_special_files_are_rejected_without_reading() {
        let temp = git_workspace();
        let status = Command::new("mkfifo")
            .arg(temp.path().join("pipe"))
            .status()
            .unwrap();
        assert!(status.success());
        let error = untracked_entry(temp.path(), "pipe").unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported untracked file type pipe"));
    }

    #[test]
    fn provider_error_envelope_surfaces_the_rejection_message() {
        let line = format!(
            r#"{{"schema":"{INTELLECT_PROTOCOL}","response":"error","message":"candidate is not attestable"}}"#
        );
        let error = decode_provider_response::<AdherenceResponse>(&line).unwrap_err();
        assert_eq!(
            error.to_string(),
            "intellect provider rejected request: candidate is not attestable"
        );
    }

    #[test]
    fn provider_registration_must_use_loopback() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = root.join("provider.json");
        let control = ProviderControl {
            schema: PROVIDER_CONTROL_SCHEMA.into(),
            provider: DEFAULT_INTELLECT_PROVIDER.into(),
            protocol: INTELLECT_PROTOCOL.into(),
            workspace_root: root.to_string_lossy().into_owned(),
            endpoint: "192.0.2.1:4000".into(),
            authorization: "secret".into(),
            pid: 1,
            started_at_micros: 1,
            idle_timeout_seconds: DEFAULT_IDLE_TIMEOUT_SECONDS,
        };
        std::fs::write(&path, serde_json::to_vec(&control).unwrap()).unwrap();

        let error = read_control(&path, &root).unwrap_err();
        assert!(error.to_string().contains("not loopback"));
    }
}
