//! Source-symbol discovery and resolution through downstream language servers.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolInformation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use url::Url;

use crate::model::reference::{SourceReference, SourceTarget};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Verified,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl From<lsp_types::Range> for SourceRange {
    fn from(value: lsp_types::Range) -> Self {
        Self {
            start: SourcePosition {
                line: value.start.line,
                character: value.start.character,
            },
            end: SourcePosition {
                line: value.end.line,
                character: value.end.character,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolCandidate {
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub detail: Option<String>,
    pub path: String,
    pub reference: String,
    pub range: SourceRange,
    pub selection_range: SourceRange,
    pub language: String,
    pub server: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSource {
    pub reference: String,
    pub path: String,
    pub symbol: Option<String>,
    pub language: Option<String>,
    pub server: Option<String>,
    pub locations: Vec<SourceRange>,
    pub snippet: String,
    pub status: VerificationStatus,
    pub message: Option<String>,
}

#[derive(Debug, Error, Clone)]
pub enum SymbolError {
    #[error("unsafe source path: {0}")]
    UnsafePath(String),
    #[error("source file not found: {0}")]
    MissingPath(String),
    #[error("no language-server preset for source file: {0}")]
    UnsupportedLanguage(String),
    #[error("language server '{server}' is unavailable: {message}")]
    ProviderUnavailable { server: String, message: String },
    #[error("language server protocol error: {0}")]
    Protocol(String),
    #[error("symbol not found: {0}")]
    NotFound(String),
    #[error("invalid source line range {start}-{end} for {path}")]
    InvalidRange { path: String, start: u32, end: u32 },
}

#[derive(Debug, Clone, Deserialize)]
struct LspConfigFile {
    #[serde(default)]
    servers: BTreeMap<String, ServerOverride>,
}

#[derive(Debug, Clone, Deserialize)]
struct ServerOverride {
    extensions: Option<Vec<String>>,
    command: Option<String>,
    args: Option<Vec<String>>,
    root_markers: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct ServerConfig {
    language: String,
    extensions: Vec<String>,
    command: String,
    args: Vec<String>,
    root_markers: Vec<String>,
}

impl ServerConfig {
    fn builtin() -> Vec<Self> {
        vec![
            Self {
                language: "rust".into(),
                extensions: vec!["rs".into()],
                command: "rust-analyzer".into(),
                args: vec![],
                root_markers: vec!["Cargo.toml".into()],
            },
            Self {
                language: "typescript".into(),
                extensions: ["ts", "tsx", "js", "jsx"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                command: "typescript-language-server".into(),
                args: vec!["--stdio".into()],
                root_markers: vec!["tsconfig.json".into(), "package.json".into()],
            },
            Self {
                language: "python".into(),
                extensions: vec!["py".into(), "pyi".into()],
                command: "basedpyright-langserver".into(),
                args: vec!["--stdio".into()],
                root_markers: vec!["pyproject.toml".into(), "basedpyrightconfig.json".into()],
            },
            Self {
                language: "sql".into(),
                extensions: vec!["sql".into()],
                command: "sqls".into(),
                args: vec![],
                root_markers: vec![".sqls.yml".into(), "sqls.yml".into()],
            },
        ]
    }
}

/// Stateless facade used by the CLI, language server, and Tolaria IPC layer.
/// A downstream server is short-lived for now; callers can safely share this
/// value, and process pooling can be added without changing the public API.
#[derive(Debug, Clone)]
pub struct SymbolService {
    specs_dir: PathBuf,
    repo_root: PathBuf,
    servers: Vec<ServerConfig>,
    allow_custom_lsp: bool,
}

impl SymbolService {
    pub fn new(specs_dir: &Path, allow_custom_lsp: bool) -> Result<Self, SymbolError> {
        let repo_root = git2::Repository::discover(specs_dir)
            .ok()
            .and_then(|repository| repository.workdir().map(Path::to_path_buf))
            .unwrap_or_else(|| specs_dir.parent().unwrap_or(specs_dir).to_path_buf());
        let mut servers = ServerConfig::builtin();
        let config_path = specs_dir.join("_lsp.toml");
        if config_path.is_file() {
            let text = std::fs::read_to_string(&config_path)
                .map_err(|error| SymbolError::Protocol(error.to_string()))?;
            let config: LspConfigFile = toml::from_str(&text).map_err(|error| {
                SymbolError::Protocol(format!("{}: {error}", config_path.display()))
            })?;
            apply_overrides(&mut servers, config, allow_custom_lsp)?;
        }
        Ok(Self {
            specs_dir: specs_dir.to_path_buf(),
            repo_root,
            servers,
            allow_custom_lsp,
        })
    }

    pub fn specs_dir(&self) -> &Path {
        &self.specs_dir
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn allows_custom_lsp(&self) -> bool {
        self.allow_custom_lsp
    }

    pub fn list_symbols(
        &self,
        relative_path: &str,
        query: Option<&str>,
    ) -> Result<Vec<SymbolCandidate>, SymbolError> {
        let source_path = self.resolve_safe_path(relative_path)?;
        let server = self.server_for_path(&source_path)?;
        let root = project_root(&source_path, &self.repo_root, &server.root_markers);
        let text = std::fs::read_to_string(&source_path)
            .map_err(|_| SymbolError::MissingPath(relative_path.to_string()))?;
        let uri = Url::from_file_path(&source_path)
            .map_err(|_| SymbolError::UnsafePath(relative_path.to_string()))?;
        let root_uri = Url::from_directory_path(&root)
            .map_err(|_| SymbolError::UnsafePath(root.display().to_string()))?;

        let mut client = JsonRpcClient::spawn(&server, &root_uri)?;
        let response = client.document_symbols(&uri, &server.language, &text)?;
        let _ = client.shutdown();

        let mut symbols = flatten_symbols(response, relative_path, &server);
        if let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) {
            let query = query.to_lowercase();
            symbols.retain(|symbol| {
                symbol.name.to_lowercase().contains(&query)
                    || symbol.qualified_name.to_lowercase().contains(&query)
            });
        }
        Ok(symbols)
    }

    pub fn resolve(&self, source: &SourceReference) -> Result<ResolvedSource, SymbolError> {
        let source_path = self.resolve_safe_path(&source.path)?;
        let text = std::fs::read_to_string(&source_path)
            .map_err(|_| SymbolError::MissingPath(source.path.clone()))?;
        match &source.target {
            SourceTarget::File => Ok(ResolvedSource {
                reference: format!("spec:src:{}", source.path),
                path: source.path.clone(),
                symbol: None,
                language: None,
                server: None,
                locations: vec![],
                snippet: text,
                status: VerificationStatus::Verified,
                message: None,
            }),
            SourceTarget::Lines { start, end } => {
                let snippet = extract_lines(&text, *start, *end).ok_or_else(|| {
                    SymbolError::InvalidRange {
                        path: source.path.clone(),
                        start: *start,
                        end: *end,
                    }
                })?;
                Ok(ResolvedSource {
                    reference: format!("spec:src:{}:{start}-{end}", source.path),
                    path: source.path.clone(),
                    symbol: None,
                    language: None,
                    server: None,
                    locations: vec![SourceRange {
                        start: SourcePosition {
                            line: start - 1,
                            character: 0,
                        },
                        end: SourcePosition {
                            line: *end,
                            character: 0,
                        },
                    }],
                    snippet,
                    status: VerificationStatus::Verified,
                    message: None,
                })
            }
            SourceTarget::Symbol { segments } => {
                let symbols = self.list_symbols(&source.path, None)?;
                let qualified = segments.join("/");
                let matching: Vec<&SymbolCandidate> = symbols
                    .iter()
                    .filter(|symbol| symbol.qualified_name == qualified)
                    .collect();
                if matching.is_empty() {
                    return Err(SymbolError::NotFound(qualified));
                }
                let locations: Vec<SourceRange> =
                    matching.iter().map(|symbol| symbol.range.clone()).collect();
                let snippet = matching
                    .iter()
                    .filter_map(|symbol| extract_zero_based_range(&text, &symbol.range))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                Ok(ResolvedSource {
                    reference: crate::model::reference::SpecReference::Source(source.clone())
                        .to_string(),
                    path: source.path.clone(),
                    symbol: Some(qualified),
                    language: Some(matching[0].language.clone()),
                    server: Some(matching[0].server.clone()),
                    locations,
                    snippet,
                    status: VerificationStatus::Verified,
                    message: None,
                })
            }
        }
    }

    pub fn resolve_safe_path(&self, relative_path: &str) -> Result<PathBuf, SymbolError> {
        let path = Path::new(relative_path);
        if relative_path.is_empty()
            || path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(SymbolError::UnsafePath(relative_path.to_string()));
        }
        let joined = self.repo_root.join(path);
        if !joined.is_file() {
            return Err(SymbolError::MissingPath(relative_path.to_string()));
        }
        let canonical_root = self
            .repo_root
            .canonicalize()
            .map_err(|_| SymbolError::UnsafePath(relative_path.to_string()))?;
        let canonical = joined
            .canonicalize()
            .map_err(|_| SymbolError::MissingPath(relative_path.to_string()))?;
        if !canonical.starts_with(canonical_root) {
            return Err(SymbolError::UnsafePath(relative_path.to_string()));
        }
        Ok(canonical)
    }

    fn server_for_path(&self, path: &Path) -> Result<ServerConfig, SymbolError> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        self.servers
            .iter()
            .find(|server| server.extensions.iter().any(|value| value == extension))
            .cloned()
            .ok_or_else(|| SymbolError::UnsupportedLanguage(path.display().to_string()))
    }
}

fn apply_overrides(
    servers: &mut Vec<ServerConfig>,
    config: LspConfigFile,
    allow_custom: bool,
) -> Result<(), SymbolError> {
    for (language, override_config) in config.servers {
        let Some(server) = servers
            .iter_mut()
            .find(|server| server.language == language)
        else {
            if !allow_custom {
                return Err(SymbolError::ProviderUnavailable {
                    server: language,
                    message: "custom language servers require --allow-custom-lsp".into(),
                });
            }
            let command = override_config.command.ok_or_else(|| {
                SymbolError::Protocol("custom language-server entries require a command".into())
            })?;
            servers.push(ServerConfig {
                language,
                extensions: override_config.extensions.unwrap_or_default(),
                command,
                args: override_config.args.unwrap_or_default(),
                root_markers: override_config.root_markers.unwrap_or_default(),
            });
            continue;
        };
        if let Some(command) = override_config.command {
            if !allow_custom && command != server.command {
                return Err(SymbolError::ProviderUnavailable {
                    server: language,
                    message: "custom language-server commands require --allow-custom-lsp".into(),
                });
            }
            server.command = command;
        }
        if let Some(extensions) = override_config.extensions {
            server.extensions = extensions;
        }
        if let Some(args) = override_config.args {
            if !allow_custom && args != server.args {
                return Err(SymbolError::ProviderUnavailable {
                    server: language,
                    message: "custom language-server arguments require --allow-custom-lsp".into(),
                });
            }
            server.args = args;
        }
        if let Some(markers) = override_config.root_markers {
            server.root_markers = markers;
        }
    }
    Ok(())
}

fn project_root(path: &Path, boundary: &Path, markers: &[String]) -> PathBuf {
    let mut current = path.parent().unwrap_or(boundary);
    loop {
        if markers.iter().any(|marker| current.join(marker).exists()) {
            return current.to_path_buf();
        }
        if current == boundary {
            return boundary.to_path_buf();
        }
        match current.parent() {
            Some(parent) if parent.starts_with(boundary) => current = parent,
            _ => return boundary.to_path_buf(),
        }
    }
}

fn flatten_symbols(
    response: Option<DocumentSymbolResponse>,
    path: &str,
    server: &ServerConfig,
) -> Vec<SymbolCandidate> {
    let mut result = Vec::new();
    match response {
        Some(DocumentSymbolResponse::Nested(symbols)) => {
            for symbol in symbols {
                flatten_nested_symbol(symbol, &[], path, server, &mut result);
            }
        }
        Some(DocumentSymbolResponse::Flat(symbols)) => {
            for symbol in symbols {
                result.push(flat_symbol(symbol, path, server));
            }
        }
        None => {}
    }
    result.sort_by(|left, right| {
        left.qualified_name
            .cmp(&right.qualified_name)
            .then(left.range.start.line.cmp(&right.range.start.line))
    });
    result
}

#[allow(deprecated)]
fn flatten_nested_symbol(
    symbol: DocumentSymbol,
    parents: &[String],
    path: &str,
    server: &ServerConfig,
    result: &mut Vec<SymbolCandidate>,
) {
    let mut segments = parents.to_vec();
    segments.push(symbol.name.clone());
    let source = SourceReference::symbol(path, segments.clone());
    result.push(SymbolCandidate {
        name: symbol.name.clone(),
        qualified_name: segments.join("/"),
        kind: format!("{:?}", symbol.kind).to_lowercase(),
        detail: symbol.detail.clone(),
        path: path.to_string(),
        reference: crate::model::reference::SpecReference::Source(source).to_string(),
        range: symbol.range.into(),
        selection_range: symbol.selection_range.into(),
        language: server.language.clone(),
        server: server.command.clone(),
    });
    for child in symbol.children.unwrap_or_default() {
        flatten_nested_symbol(child, &segments, path, server, result);
    }
}

#[allow(deprecated)]
fn flat_symbol(symbol: SymbolInformation, path: &str, server: &ServerConfig) -> SymbolCandidate {
    let mut segments = symbol
        .container_name
        .as_deref()
        .map(|container| vec![container.to_string()])
        .unwrap_or_default();
    segments.push(symbol.name.clone());
    let source = SourceReference::symbol(path, segments.clone());
    SymbolCandidate {
        name: symbol.name,
        qualified_name: segments.join("/"),
        kind: format!("{:?}", symbol.kind).to_lowercase(),
        detail: None,
        path: path.to_string(),
        reference: crate::model::reference::SpecReference::Source(source).to_string(),
        range: symbol.location.range.into(),
        selection_range: symbol.location.range.into(),
        language: server.language.clone(),
        server: server.command.clone(),
    }
}

fn extract_lines(text: &str, start: u32, end: u32) -> Option<String> {
    if start == 0 || end < start {
        return None;
    }
    let lines: Vec<&str> = text.lines().collect();
    if end as usize > lines.len() {
        return None;
    }
    Some(lines[(start - 1) as usize..end as usize].join("\n"))
}

fn extract_zero_based_range(text: &str, range: &SourceRange) -> Option<String> {
    extract_lines(
        text,
        range.start.line + 1,
        range.end.line.max(range.start.line) + 1,
    )
}

struct JsonRpcClient {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Result<Value, String>>,
    next_id: i64,
}

impl JsonRpcClient {
    fn spawn(server: &ServerConfig, root_uri: &Url) -> Result<Self, SymbolError> {
        let mut child = Command::new(&server.command)
            .args(&server.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| SymbolError::ProviderUnavailable {
                server: server.command.clone(),
                message: error.to_string(),
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| SymbolError::Protocol("language server did not expose stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SymbolError::Protocol("language server did not expose stdout".into()))?;
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let message = read_lsp_message(&mut reader).map_err(|error| error.to_string());
                let finished = message.is_err();
                if sender.send(message).is_err() || finished {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            messages,
            next_id: 1,
        };
        client.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "clientInfo": { "name": "forge-spec", "version": env!("CARGO_PKG_VERSION") },
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "documentSymbol": { "hierarchicalDocumentSymbolSupport": true }
                    },
                    "window": { "workDoneProgress": false }
                }
            }),
        )?;
        client.notify("initialized", json!({}))?;
        Ok(client)
    }

    fn document_symbols(
        &mut self,
        uri: &Url,
        language_id: &str,
        text: &str,
    ) -> Result<Option<DocumentSymbolResponse>, SymbolError> {
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        )?;
        let result = self.request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        )?;
        serde_json::from_value(result).map_err(|error| SymbolError::Protocol(error.to_string()))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, SymbolError> {
        let id = self.next_id;
        self.next_id += 1;
        write_lsp_message(
            &mut self.stdin,
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
        )?;
        loop {
            let message = self
                .messages
                .recv_timeout(REQUEST_TIMEOUT)
                .map_err(|error| SymbolError::Protocol(format!("{method}: {error}")))?
                .map_err(SymbolError::Protocol)?;
            if message.get("id").and_then(Value::as_i64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(SymbolError::Protocol(format!("{method}: {error}")));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            if message.get("method").is_some() && message.get("id").is_some() {
                let response_id = message.get("id").cloned().unwrap_or(Value::Null);
                let result = match message.get("method").and_then(Value::as_str) {
                    Some("workspace/configuration") => json!([]),
                    _ => Value::Null,
                };
                write_lsp_message(
                    &mut self.stdin,
                    &json!({ "jsonrpc": "2.0", "id": response_id, "result": result }),
                )?;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), SymbolError> {
        write_lsp_message(
            &mut self.stdin,
            &json!({ "jsonrpc": "2.0", "method": method, "params": params }),
        )
    }

    fn shutdown(&mut self) -> Result<(), SymbolError> {
        let _ = self.request("shutdown", Value::Null);
        let _ = self.notify("exit", Value::Null);
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for JsonRpcClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_lsp_message(reader: &mut impl BufRead) -> std::io::Result<Value> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "language server closed stdout",
            ));
        }
        let header = header.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    let length = content_length.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing Content-Length")
    })?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn write_lsp_message(writer: &mut impl Write, value: &Value) -> Result<(), SymbolError> {
    let body =
        serde_json::to_vec(value).map_err(|error| SymbolError::Protocol(error.to_string()))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())
        .and_then(|_| writer.write_all(&body))
        .and_then(|_| writer.flush())
        .map_err(|error| SymbolError::Protocol(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_valid_inclusive_line_ranges() {
        assert_eq!(
            extract_lines("one\ntwo\nthree\n", 2, 3),
            Some("two\nthree".into())
        );
        assert_eq!(extract_lines("one\n", 0, 1), None);
        assert_eq!(extract_lines("one\n", 2, 2), None);
        assert_eq!(extract_lines("one\n", 2, 1), None);
    }

    #[test]
    fn project_root_uses_nearest_marker_inside_boundary() {
        let temp = tempfile::TempDir::new().unwrap();
        let nested = temp.path().join("app/src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(temp.path().join("app/package.json"), "{}").unwrap();
        let file = nested.join("main.ts");
        std::fs::write(&file, "export const x = 1").unwrap();
        assert_eq!(
            project_root(&file, temp.path(), &["package.json".into()]),
            temp.path().join("app")
        );
    }

    #[test]
    fn lsp_framing_round_trips_json() {
        let value = json!({"jsonrpc": "2.0", "id": 1, "result": []});
        let mut bytes = Vec::new();
        write_lsp_message(&mut bytes, &value).unwrap();
        let mut reader = BufReader::new(bytes.as_slice());
        assert_eq!(read_lsp_message(&mut reader).unwrap(), value);
    }

    #[test]
    fn deterministic_document_symbol_fixture_preserves_hierarchy() {
        let response: Option<DocumentSymbolResponse> = serde_json::from_value(json!([
            {
                "name": "SessionStore",
                "kind": 5,
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 8, "character": 1 }
                },
                "selectionRange": {
                    "start": { "line": 1, "character": 7 },
                    "end": { "line": 1, "character": 19 }
                },
                "children": [{
                    "name": "expire/now",
                    "kind": 6,
                    "range": {
                        "start": { "line": 3, "character": 2 },
                        "end": { "line": 5, "character": 3 }
                    },
                    "selectionRange": {
                        "start": { "line": 3, "character": 5 },
                        "end": { "line": 3, "character": 15 }
                    }
                }]
            }
        ]))
        .unwrap();
        let server = ServerConfig::builtin()
            .into_iter()
            .find(|server| server.language == "rust")
            .unwrap();
        let symbols = flatten_symbols(response, "src/session.rs", &server);
        assert_eq!(symbols[1].qualified_name, "SessionStore/expire/now");
        assert_eq!(
            symbols[1].reference,
            "spec:src:src/session.rs#symbol=SessionStore/expire%2Fnow"
        );
    }

    #[test]
    #[ignore = "requires rust-analyzer installed; run explicitly for provider smoke coverage"]
    fn real_rust_analyzer_smoke() {
        let specs_dir = Path::new("../example/.specs");
        let service = SymbolService::new(specs_dir, false).unwrap();
        let symbols = service
            .list_symbols("spec-cli/src/lib.rs", Some(""))
            .expect("rust-analyzer should return document symbols");
        assert!(!symbols.is_empty());
    }
}
