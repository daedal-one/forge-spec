//! Forge-spec language server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use serde::Deserialize;
use serde_json::{json, Value};
use url::Url;

use crate::lint::diagnostic::Severity;
use crate::model::document::SpecDocument;
use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;
use crate::parse::references::parse_spec_url;
use crate::symbol::SymbolService;
use crate::workspace::WorkspaceIndex;

#[derive(Debug, Clone)]
struct OpenDocument {
    path: PathBuf,
    text: String,
    version: i64,
}

pub fn run_stdio(specs_dir: &Path) -> Result<()> {
    let (connection, threads) = Connection::stdio();
    run_connection(connection, specs_dir)?;
    threads
        .join()
        .context("joining language-server I/O threads")?;
    Ok(())
}

fn run_connection(connection: Connection, specs_dir: &Path) -> Result<()> {
    let cache_path = std::env::var_os("FORGE_SPEC_CACHE_PATH").map(PathBuf::from);
    let mut workspace = WorkspaceIndex::open(specs_dir, cache_path.as_deref())?;
    let (initialize_id, _) = connection.initialize_start()?;
    connection.initialize_finish(
        initialize_id,
        json!({
            "capabilities": {
                "textDocumentSync": 1,
                "completionProvider": { "triggerCharacters": [":", "#", "/", "="] },
                "hoverProvider": true,
                "definitionProvider": true,
                "referencesProvider": true,
                "documentSymbolProvider": true
            },
            "serverInfo": {
                "name": "forge-spec",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )?;

    let mut open_documents: HashMap<String, OpenDocument> = HashMap::new();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                let response = handle_request(request, &mut workspace, &open_documents);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                handle_notification(
                    notification,
                    &mut workspace,
                    &mut open_documents,
                    &connection,
                )?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn handle_notification(
    notification: Notification,
    workspace: &mut WorkspaceIndex,
    documents: &mut HashMap<String, OpenDocument>,
    connection: &Connection,
) -> Result<()> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let uri = value_string(&notification.params, "/textDocument/uri")?;
            let text = value_string(&notification.params, "/textDocument/text")?;
            let version = notification
                .params
                .pointer("/textDocument/version")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let path = uri_to_path(&uri)?;
            documents.insert(
                uri.clone(),
                OpenDocument {
                    path,
                    text,
                    version,
                },
            );
            publish_diagnostics(connection, workspace, &uri, documents.get(&uri).unwrap())?;
        }
        "textDocument/didChange" => {
            let uri = value_string(&notification.params, "/textDocument/uri")?;
            if let Some(document) = documents.get_mut(&uri) {
                if let Some(text) = notification
                    .params
                    .pointer("/contentChanges/0/text")
                    .and_then(Value::as_str)
                {
                    document.text = text.to_string();
                }
                document.version = notification
                    .params
                    .pointer("/textDocument/version")
                    .and_then(Value::as_i64)
                    .unwrap_or(document.version);
                publish_diagnostics(connection, workspace, &uri, document)?;
            }
        }
        "textDocument/didSave" => {
            let uri = value_string(&notification.params, "/textDocument/uri")?;
            if let Some(document) = documents.get(&uri) {
                workspace.refresh_paths([document.path.clone()])?;
                publish_diagnostics(connection, workspace, &uri, document)?;
                notify_index_changed(connection, workspace)?;
            }
        }
        "textDocument/didClose" => {
            let uri = value_string(&notification.params, "/textDocument/uri")?;
            documents.remove(&uri);
            send_notification(
                connection,
                "textDocument/publishDiagnostics",
                json!({
                    "uri": uri,
                    "diagnostics": []
                }),
            )?;
        }
        "workspace/didChangeWatchedFiles" => {
            let paths = notification
                .params
                .pointer("/changes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|change| change.get("uri").and_then(Value::as_str))
                .filter_map(|uri| uri_to_path(uri).ok())
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                workspace.refresh_paths(paths)?;
                notify_index_changed(connection, workspace)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    workspace: &WorkspaceIndex,
    uri: &str,
    document: &OpenDocument,
) -> Result<()> {
    let is_documentation = workspace
        .registry()
        .documentation
        .contains_source_path(&document.path);
    if !is_indexed_document(workspace, document) {
        return send_notification(
            connection,
            "textDocument/publishDiagnostics",
            json!({
                "uri": uri,
                "version": document.version,
                "diagnostics": []
            }),
        );
    }
    let registry = if is_documentation {
        workspace.registry_with_documentation_override(&document.path, &document.text)
    } else {
        workspace.registry_with_override(&document.path, &document.text)
    };
    let diagnostics = match registry {
        Ok(registry) => crate::lint::lint_all(&registry)
            .into_iter()
            .filter(|diagnostic| diagnostic.file == document.path)
            .map(|diagnostic| {
                let line = diagnostic.line.unwrap_or(1).saturating_sub(1) as u32;
                let severity = match diagnostic.severity {
                    Severity::Error => 1,
                    Severity::Warning => 2,
                    Severity::Info => 3,
                };
                json!({
                    "range": {
                        "start": { "line": line, "character": 0 },
                        "end": { "line": line, "character": line_utf16_len(&document.text, line) }
                    },
                    "severity": severity,
                    "code": diagnostic.code,
                    "source": "forge-spec",
                    "message": diagnostic.message,
                    "data": { "detail": diagnostic.detail }
                })
            })
            .collect::<Vec<_>>(),
        Err(error) => vec![json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": line_utf16_len(&document.text, 0) }
            },
            "severity": 1,
            "code": "parse",
            "source": "forge-spec",
            "message": format!("{error:#}")
        })],
    };
    send_notification(
        connection,
        "textDocument/publishDiagnostics",
        json!({
            "uri": uri,
            "version": document.version,
            "diagnostics": diagnostics
        }),
    )
}

fn handle_request(
    request: Request,
    workspace: &mut WorkspaceIndex,
    documents: &HashMap<String, OpenDocument>,
) -> Response {
    let id = request.id.clone();
    let result = match request.method.as_str() {
        "textDocument/completion" => completion(workspace, documents, &request.params),
        "textDocument/hover" => hover(workspace, documents, &request.params),
        "textDocument/definition" => definition(workspace, documents, &request.params),
        "textDocument/references" => references(workspace, documents, &request.params),
        "textDocument/documentSymbol" => document_symbols(workspace, documents, &request.params),
        "forgeSpec/explorerSnapshot" => {
            serde_json::to_value(workspace.snapshot()).map_err(Into::into)
        }
        "forgeSpec/reconcile" => workspace
            .reconcile()
            .and_then(|_| serde_json::to_value(workspace.snapshot()).map_err(Into::into)),
        "forgeSpec/resolveReference" => resolve_reference(workspace, &request.params),
        "forgeSpec/applyChanges" => apply_changes(workspace, documents, &request.params),
        _ => {
            return Response::new_err(
                id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported method: {}", request.method),
            )
        }
    };
    match result {
        Ok(value) => Response::new_ok(id, value),
        Err(error) => Response::new_err(id, ErrorCode::InvalidParams as i32, format!("{error:#}")),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyChangesParams {
    text_document: VersionedDocument,
    change: crate::mutation::ChangeRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionedDocument {
    uri: String,
    version: i64,
}

fn apply_changes(
    workspace: &WorkspaceIndex,
    documents: &HashMap<String, OpenDocument>,
    params: &Value,
) -> Result<Value> {
    let params: ApplyChangesParams = serde_json::from_value(params.clone())?;
    let open = documents
        .get(&params.text_document.uri)
        .context("target document is not open")?;
    if open.version != params.text_document.version {
        anyhow::bail!(
            "document version changed: expected {}, current {}",
            params.text_document.version,
            open.version
        );
    }
    let saved = std::fs::read_to_string(&open.path)
        .with_context(|| format!("reading {}", open.path.display()))?;
    if saved != open.text {
        anyhow::bail!("document has unsaved changes; save it before applying a typed mutation");
    }

    let outcome = crate::mutation::MutationEngine::new(workspace.specs_dir())
        .execute(&params.change, true)?;
    let mut document_changes = Vec::new();
    for edit in &outcome.edits {
        let origin = edit.origin.as_deref().unwrap_or(&edit.destination);
        let old_text = std::fs::read_to_string(origin).unwrap_or_default();
        if origin != edit.destination {
            document_changes.push(json!({
                "kind": "rename",
                "oldUri": path_uri(origin),
                "newUri": path_uri(&edit.destination)
            }));
        }
        let uri = path_uri(&edit.destination);
        let version = if same_path(&edit.destination, &open.path) {
            Value::from(open.version)
        } else {
            documents
                .values()
                .find(|document| {
                    same_path(&document.path, origin) && same_path(origin, &edit.destination)
                })
                .map(|document| Value::from(document.version))
                .unwrap_or(Value::Null)
        };
        let (range, new_text) = minimal_text_edit(&old_text, &edit.new_text);
        document_changes.push(json!({
            "textDocument": { "uri": uri, "version": version },
            "edits": [{ "range": range, "newText": new_text }]
        }));
    }
    Ok(json!({
        "schema": "forge-spec-workspace-edit/v1",
        "plan": outcome.plan,
        "edit": { "documentChanges": document_changes }
    }))
}

fn minimal_text_edit(old: &str, new: &str) -> (Value, String) {
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while !old.is_char_boundary(prefix) || !new.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let old_remaining = &old[prefix..];
    let new_remaining = &new[prefix..];
    let mut suffix = old_remaining
        .bytes()
        .rev()
        .zip(new_remaining.bytes().rev())
        .take_while(|(left, right)| left == right)
        .count()
        .min(old_remaining.len())
        .min(new_remaining.len());
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) {
        suffix -= 1;
    }
    let old_end = old.len() - suffix;
    let new_end = new.len() - suffix;
    let (start_line, start_character) = byte_position(old, prefix);
    let (end_line, end_character) = byte_position(old, old_end);
    (
        range(start_line, start_character, end_line, end_character),
        new[prefix..new_end].to_string(),
    )
}

fn byte_position(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let column = before
        .rsplit_once('\n')
        .map(|(_, tail)| tail)
        .unwrap_or(before)
        .encode_utf16()
        .count() as u32;
    (line, column)
}

fn path_uri(path: &Path) -> String {
    Url::from_file_path(path)
        .map(|uri| uri.to_string())
        .unwrap_or_default()
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn completion(
    workspace: &WorkspaceIndex,
    documents: &HashMap<String, OpenDocument>,
    params: &Value,
) -> Result<Value> {
    let (document, line, character) = request_document(documents, params)?;
    if !is_indexed_document(workspace, document) {
        return Ok(json!([]));
    }
    let registry = registry_for(workspace, document)?;
    let prefix = line_prefix(&document.text, line, character).unwrap_or_default();

    if let Some((path, query)) = source_symbol_context(prefix) {
        let service = SymbolService::new(workspace.specs_dir(), false)?;
        let symbols = service.list_symbols(path, Some(query))?;
        return Ok(Value::Array(
            symbols
                .into_iter()
                .map(|symbol| {
                    completion_item(
                        &symbol.qualified_name,
                        3,
                        Some(format!("{} · {}", symbol.kind, symbol.path)),
                        &symbol.reference,
                        prefix,
                        line,
                        character,
                    )
                })
                .collect(),
        ));
    }

    let mut items = Vec::new();
    for spec in &registry.documents {
        let id = spec.id_str();
        items.push(completion_item(
            &id,
            18,
            spec.universal.summary.clone(),
            &format!("spec:{id}"),
            prefix,
            line,
            character,
        ));
        for anchor in spec.anchors() {
            let qualified = format!("{id}#{anchor}");
            items.push(completion_item(
                &qualified,
                6,
                None,
                &format!("spec:{qualified}"),
                prefix,
                line,
                character,
            ));
        }
    }
    for documentation in &registry.documentation.documents {
        let reference =
            crate::documentation::DocumentationReference::file(documentation.path.clone())
                .to_string();
        items.push(completion_item(
            &documentation.title,
            17,
            documentation.summary.clone(),
            &reference,
            prefix,
            line,
            character,
        ));
        for heading in &documentation.headings {
            let reference = crate::documentation::DocumentationReference::heading(
                documentation.path.clone(),
                heading.segments.clone(),
            )
            .to_string();
            items.push(completion_item(
                &heading.segments.join(" / "),
                18,
                Some(format!(
                    "{} · {}",
                    documentation.collection_title, documentation.path
                )),
                &reference,
                prefix,
                line,
                character,
            ));
        }
    }
    Ok(Value::Array(items))
}

fn hover(
    workspace: &WorkspaceIndex,
    documents: &HashMap<String, OpenDocument>,
    params: &Value,
) -> Result<Value> {
    let (document, line, character) = request_document(documents, params)?;
    if !is_indexed_document(workspace, document) {
        return Ok(Value::Null);
    }
    let Some(token) = spec_token_at(&document.text, line, character) else {
        return Ok(Value::Null);
    };
    let Some(reference) = parse_spec_url(token) else {
        return Ok(Value::Null);
    };
    let markdown = match reference {
        SpecReference::Spec(target) => {
            let registry = registry_for(workspace, document)?;
            let qualified = target.to_string();
            let id = target.spec_id.to_string();
            let Some(spec) = registry.get_by_id(&id) else {
                return Ok(Value::Null);
            };
            format!(
                "**{}** · {} · {}\n\n{}",
                qualified,
                spec.universal.entity_type.type_name(),
                spec.universal.status.as_str(),
                spec.universal.summary.as_deref().unwrap_or("No summary")
            )
        }
        SpecReference::Source(source) => {
            let resolved = SymbolService::new(workspace.specs_dir(), false)?.resolve(&source)?;
            format!(
                "**{}**\n\n```\n{}\n```",
                resolved.reference, resolved.snippet
            )
        }
        SpecReference::Documentation(reference) => {
            let registry = registry_for(workspace, document)?;
            let Some((documentation, heading)) = registry.documentation.resolve(&reference) else {
                return Ok(Value::Null);
            };
            let selection = heading
                .map(|heading| heading.segments.join(" / "))
                .unwrap_or_else(|| documentation.title.clone());
            format!(
                "**{}** · documentation · {}\n\n{}",
                selection,
                documentation.collection_title,
                documentation.summary.as_deref().unwrap_or("No summary")
            )
        }
    };
    Ok(json!({ "contents": { "kind": "markdown", "value": markdown } }))
}

fn definition(
    workspace: &WorkspaceIndex,
    documents: &HashMap<String, OpenDocument>,
    params: &Value,
) -> Result<Value> {
    let (document, line, character) = request_document(documents, params)?;
    if !is_indexed_document(workspace, document) {
        return Ok(Value::Null);
    }
    let Some(token) = spec_token_at(&document.text, line, character) else {
        return Ok(Value::Null);
    };
    let Some(reference) = parse_spec_url(token) else {
        return Ok(Value::Null);
    };
    match reference {
        SpecReference::Spec(target) => {
            let registry = registry_for(workspace, document)?;
            let id = target.spec_id.to_string();
            let Some(spec) = registry.get_by_id(&id) else {
                return Ok(Value::Null);
            };
            let target_line = definition_line(spec, target.anchor.as_deref());
            Ok(location(&spec.source_path, target_line, 0, target_line, 0))
        }
        SpecReference::Source(source) => {
            let service = SymbolService::new(workspace.specs_dir(), false)?;
            let path = service.resolve_safe_path(&source.path)?;
            let resolved = service.resolve(&source)?;
            let Some(range) = resolved.locations.first() else {
                return Ok(location(&path, 0, 0, 0, 0));
            };
            Ok(location(
                &path,
                range.start.line,
                range.start.character,
                range.end.line,
                range.end.character,
            ))
        }
        SpecReference::Documentation(reference) => {
            let registry = registry_for(workspace, document)?;
            let Some((documentation, heading)) = registry.documentation.resolve(&reference) else {
                return Ok(Value::Null);
            };
            let start = heading
                .map(|heading| heading.line.saturating_sub(1) as u32)
                .unwrap_or(0);
            let end = heading
                .map(|heading| heading.end_line.saturating_sub(1) as u32)
                .unwrap_or(start);
            Ok(location(&documentation.source_path, start, 0, end, 0))
        }
    }
}

fn references(
    workspace: &WorkspaceIndex,
    documents: &HashMap<String, OpenDocument>,
    params: &Value,
) -> Result<Value> {
    let (document, line, character) = request_document(documents, params)?;
    if !is_indexed_document(workspace, document) {
        return Ok(json!([]));
    }
    let Some(token) = spec_token_at(&document.text, line, character) else {
        return Ok(json!([]));
    };
    let Some(target) = parse_spec_url(token) else {
        return Ok(json!([]));
    };
    let registry = registry_for(workspace, document)?;
    let target = target.to_string();
    let mut found = std::collections::BTreeSet::<(PathBuf, u32)>::new();
    for spec in &registry.documents {
        for reference in &spec.references {
            let candidate = reference.reference.to_string();
            if candidate == target
                || (!target.contains('#') && candidate.starts_with(&format!("{target}#")))
            {
                let line = reference.line.saturating_sub(1) as u32;
                found.insert((spec.source_path.clone(), line));
            }
        }
    }
    for backlink in registry.documentation.backlinks_with_prefix(&target) {
        let source_path = if backlink.source_kind == "documentation" {
            registry
                .documentation
                .get(&backlink.source)
                .map(|document| document.source_path.clone())
        } else {
            registry
                .get_by_id(&backlink.source)
                .map(|document| document.source_path.clone())
        };
        if let Some(path) = source_path {
            found.insert((path, backlink.line.saturating_sub(1) as u32));
        }
    }
    Ok(Value::Array(
        found
            .into_iter()
            .map(|(path, line)| location(&path, line, 0, line, 0))
            .collect(),
    ))
}

fn document_symbols(
    workspace: &WorkspaceIndex,
    documents: &HashMap<String, OpenDocument>,
    params: &Value,
) -> Result<Value> {
    let uri = value_string(params, "/textDocument/uri")?;
    let document = documents.get(&uri).context("document is not open")?;
    if !is_indexed_document(workspace, document) {
        return Ok(json!([]));
    }
    let registry = registry_for(workspace, document)?;
    if let Some(documentation) = registry
        .documentation
        .documents
        .iter()
        .find(|candidate| same_path(&candidate.source_path, &document.path))
    {
        return Ok(Value::Array(
            documentation
                .headings
                .iter()
                .map(|heading| {
                    let start = heading.line.saturating_sub(1) as u32;
                    let end = heading.end_line.saturating_sub(1) as u32;
                    json!({
                        "name": heading.title,
                        "detail": heading.segments.join(" / "),
                        "kind": 3,
                        "range": range(start, 0, end, line_utf16_len(&document.text, end)),
                        "selectionRange": range(start, 0, start, line_utf16_len(&document.text, start))
                    })
                })
                .collect(),
        ));
    }
    let Some(spec) = registry
        .documents
        .iter()
        .find(|spec| spec.source_path == document.path)
    else {
        return Ok(json!([]));
    };
    let mut children = Vec::new();
    for block in &spec.blocks {
        let mut clauses = Vec::new();
        for clause in &block.clauses {
            let line = clause.line.saturating_sub(1) as u32;
            clauses.push(json!({
                "name": clause.id,
                "detail": clause.text,
                "kind": 8,
                "range": range(line, 0, line, line_utf16_len(&document.text, line)),
                "selectionRange": range(line, 0, line, line_utf16_len(&document.text, line))
            }));
        }
        let start = block.start_line.saturating_sub(1) as u32;
        let end = block.end_line.saturating_sub(1) as u32;
        children.push(json!({
            "name": block.id,
            "detail": block.kind.to_string(),
            "kind": 5,
            "range": range(start, 0, end, line_utf16_len(&document.text, end)),
            "selectionRange": range(start, 0, start, line_utf16_len(&document.text, start)),
            "children": clauses
        }));
    }
    let end = document.text.lines().count().saturating_sub(1) as u32;
    Ok(json!([{
        "name": spec.id_str(),
        "detail": spec.universal.entity_type.type_name(),
        "kind": 1,
        "range": range(0, 0, end, line_utf16_len(&document.text, end)),
        "selectionRange": range(1, 0, 1, line_utf16_len(&document.text, 1)),
        "children": children
    }]))
}

fn resolve_reference(workspace: &WorkspaceIndex, params: &Value) -> Result<Value> {
    let reference = value_string(params, "/reference")?;
    let reference = parse_spec_url(&reference).context("invalid spec reference")?;
    match reference {
        SpecReference::Spec(target) => {
            let id = target.spec_id.to_string();
            let Some(spec) = workspace.registry().get_by_id(&id) else {
                return Ok(Value::Null);
            };
            let target_line = definition_line(spec, target.anchor.as_deref());
            Ok(location(&spec.source_path, target_line, 0, target_line, 0))
        }
        SpecReference::Source(source) => {
            let service = SymbolService::new(workspace.specs_dir(), false)?;
            let path = service.resolve_safe_path(&source.path)?;
            let resolved = service.resolve(&source)?;
            let Some(range) = resolved.locations.first() else {
                return Ok(location(&path, 0, 0, 0, 0));
            };
            Ok(location(
                &path,
                range.start.line,
                range.start.character,
                range.end.line,
                range.end.character,
            ))
        }
        SpecReference::Documentation(reference) => {
            let Some((documentation, heading)) =
                workspace.registry().documentation.resolve(&reference)
            else {
                return Ok(Value::Null);
            };
            let start = heading
                .map(|heading| heading.line.saturating_sub(1) as u32)
                .unwrap_or(0);
            let end = heading
                .map(|heading| heading.end_line.saturating_sub(1) as u32)
                .unwrap_or(start);
            Ok(location(&documentation.source_path, start, 0, end, 0))
        }
    }
}

fn registry_for(workspace: &WorkspaceIndex, document: &OpenDocument) -> Result<SpecRegistry> {
    if workspace
        .registry()
        .documentation
        .contains_source_path(&document.path)
    {
        workspace.registry_with_documentation_override(&document.path, &document.text)
    } else if document
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".spec.md"))
    {
        workspace.registry_with_override(&document.path, &document.text)
    } else {
        anyhow::bail!("Markdown document is not enrolled in forge-spec documentation")
    }
}

fn is_indexed_document(workspace: &WorkspaceIndex, document: &OpenDocument) -> bool {
    workspace
        .registry()
        .documentation
        .contains_source_path(&document.path)
        || (path_is_within(&document.path, workspace.specs_dir())
            && document
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".spec.md")))
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    if path.starts_with(root) {
        return true;
    }
    if path
        .canonicalize()
        .is_ok_and(|canonical| canonical.starts_with(root))
    {
        return true;
    }
    let Some((parent, file_name)) = path.parent().zip(path.file_name()) else {
        return false;
    };
    parent
        .canonicalize()
        .is_ok_and(|canonical| canonical.join(file_name).starts_with(root))
}

fn request_document<'a>(
    documents: &'a HashMap<String, OpenDocument>,
    params: &Value,
) -> Result<(&'a OpenDocument, u32, u32)> {
    let uri = value_string(params, "/textDocument/uri")?;
    let line = params
        .pointer("/position/line")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let character = params
        .pointer("/position/character")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    Ok((
        documents.get(&uri).context("document is not open")?,
        line,
        character,
    ))
}

fn source_symbol_context(prefix: &str) -> Option<(&str, &str)> {
    let marker = "spec:src:";
    let start = prefix.rfind(marker)? + marker.len();
    let value = &prefix[start..];
    let (path, query) = value.split_once("#symbol=")?;
    if path.is_empty() {
        None
    } else {
        Some((path, query))
    }
}

fn completion_item(
    label: &str,
    kind: u32,
    detail: Option<String>,
    new_text: &str,
    prefix: &str,
    line: u32,
    character: u32,
) -> Value {
    let mut item = json!({ "label": label, "kind": kind });
    if let Some(detail) = detail {
        item["detail"] = Value::String(detail);
    }
    if let Some(byte_start) = prefix.rfind("spec:") {
        let start_character = prefix[..byte_start].encode_utf16().count() as u32;
        item["textEdit"] = json!({
            "range": range(line, start_character, line, character),
            "newText": new_text
        });
    } else {
        item["insertText"] = Value::String(new_text.to_string());
    }
    item
}

fn definition_line(document: &SpecDocument, anchor: Option<&str>) -> u32 {
    let Some(anchor) = anchor else {
        return document
            .source_path
            .to_str()
            .and_then(|_| std::fs::read_to_string(&document.source_path).ok())
            .and_then(|text| {
                text.lines()
                    .position(|line| line.trim_start().starts_with("id:"))
            })
            .unwrap_or(1) as u32;
    };
    document
        .blocks
        .iter()
        .find_map(|block| {
            if block.id == anchor {
                Some(block.start_line.saturating_sub(1) as u32)
            } else {
                block
                    .clauses
                    .iter()
                    .find(|clause| clause.id == anchor)
                    .map(|clause| clause.line.saturating_sub(1) as u32)
            }
        })
        .unwrap_or(0)
}

fn spec_token_at(text: &str, line: u32, character: u32) -> Option<&str> {
    let line = text.lines().nth(line as usize)?;
    let cursor = utf16_to_byte(line, character);
    for (start, _) in line.match_indices("spec:") {
        let end = line[start..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, ')' | ']' | '}' | '>' | '"' | '\'')
            })
            .map(|offset| start + offset)
            .unwrap_or(line.len());
        if (start..=end).contains(&cursor) {
            return Some(&line[start..end]);
        }
    }
    None
}

fn line_prefix(text: &str, line: u32, character: u32) -> Option<&str> {
    let line = text.lines().nth(line as usize)?;
    Some(&line[..utf16_to_byte(line, character)])
}

fn utf16_to_byte(line: &str, target: u32) -> usize {
    let mut units = 0u32;
    for (index, character) in line.char_indices() {
        if units >= target {
            return index;
        }
        units += character.len_utf16() as u32;
    }
    line.len()
}

fn line_utf16_len(text: &str, line: u32) -> u32 {
    text.lines()
        .nth(line as usize)
        .unwrap_or("")
        .encode_utf16()
        .count() as u32
}

fn range(start_line: u32, start_character: u32, end_line: u32, end_character: u32) -> Value {
    json!({
        "start": { "line": start_line, "character": start_character },
        "end": { "line": end_line, "character": end_character }
    })
}

fn location(
    path: &Path,
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
) -> Value {
    let uri = Url::from_file_path(path)
        .map(|uri| uri.to_string())
        .unwrap_or_default();
    json!({ "uri": uri, "range": range(start_line, start_character, end_line, end_character) })
}

fn value_string(value: &Value, pointer: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing string parameter {pointer}"))
}

fn uri_to_path(uri: &str) -> Result<PathBuf> {
    Url::parse(uri)?
        .to_file_path()
        .map_err(|_| anyhow::anyhow!("URI is not a file: {uri}"))
}

fn send_notification(connection: &Connection, method: &str, params: Value) -> Result<()> {
    connection
        .sender
        .send(Message::Notification(Notification::new(
            method.into(),
            params,
        )))?;
    Ok(())
}

fn notify_index_changed(connection: &Connection, workspace: &WorkspaceIndex) -> Result<()> {
    send_notification(
        connection,
        "forgeSpec/indexChanged",
        json!({ "generation": workspace.generation() }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn finds_reference_at_utf16_position() {
        let text = "See [policy](spec:REQ:auth/session-expiry).";
        assert_eq!(
            spec_token_at(text, 0, 20),
            Some("spec:REQ:auth/session-expiry")
        );
    }

    #[test]
    fn recognizes_symbol_completion_context() {
        assert_eq!(
            source_symbol_context("[x](spec:src:src/lib.rs#symbol=Ser"),
            Some(("src/lib.rs", "Ser"))
        );
    }

    #[test]
    fn apply_changes_returns_a_versioned_edit_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::write(
            specs_dir.join("_config.toml"),
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            specs_dir.join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [carlo]\n---\n# Demo\n",
        )
        .unwrap();
        let path = specs_dir.join("example.spec.md");
        let saved = "---\nid: REQ:demo/example\ntype: requirement\nstatus: accepted\nsummary: Example.\nowners: [carlo]\nlevel: MUST\nrefines: []\n---\n# Example\n";
        std::fs::write(&path, saved).unwrap();
        let uri = Url::from_file_path(&path).unwrap().to_string();
        let workspace = WorkspaceIndex::open(&specs_dir, None).unwrap();
        let documents = HashMap::from([(
            uri.clone(),
            OpenDocument {
                path: path.clone(),
                text: saved.into(),
                version: 7,
            },
        )]);
        let result = apply_changes(
            &workspace,
            &documents,
            &json!({
                "textDocument": { "uri": uri, "version": 7 },
                "change": {
                    "schema": "forge-spec-change/v1",
                    "if_match": {},
                    "operations": [{
                        "op": "owner.add",
                        "spec": "REQ:demo/example",
                        "owner": "maya"
                    }]
                }
            }),
        )
        .unwrap();
        assert_eq!(result["schema"], "forge-spec-workspace-edit/v1");
        assert_eq!(
            result["edit"]["documentChanges"][0]["textDocument"]["version"], 7,
            "{result}"
        );
        assert!(result["edit"]["documentChanges"][0]["edits"][0]["newText"]
            .as_str()
            .unwrap()
            .contains("maya"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), saved);
    }

    #[test]
    fn memory_protocol_publishes_unsaved_diagnostics_and_document_symbols() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::write(
            specs_dir.join("_config.toml"),
            "baseline = \"forge-spec-v0.2.0\"\n",
        )
        .unwrap();
        let path = specs_dir.join("demo.spec.md");
        let saved = "---\nid: REQ:demo/example\ntype: requirement\nstatus: accepted\nowners: [dev]\nlevel: MUST\n---\n# Example\n\n:::{requirement id=\"works\" level=\"MUST\"}\nIt MUST work.\n:::\n";
        std::fs::write(&path, saved).unwrap();
        let unsaved = saved.replace("owners: [dev]", "owners: []");
        let uri = Url::from_file_path(&path).unwrap().to_string();

        let (server, client) = Connection::memory();
        let server_specs = specs_dir.clone();
        let handle = thread::spawn(move || run_connection(server, &server_specs));

        client
            .sender
            .send(Message::Request(Request::new(
                1.into(),
                "initialize".into(),
                json!({ "capabilities": {} }),
            )))
            .unwrap();
        assert!(matches!(
            client.receiver.recv().unwrap(),
            Message::Response(_)
        ));
        client
            .sender
            .send(Message::Notification(Notification::new(
                "initialized".into(),
                json!({}),
            )))
            .unwrap();
        client
            .sender
            .send(Message::Notification(Notification::new(
                "textDocument/didOpen".into(),
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": "markdown",
                        "version": 2,
                        "text": unsaved
                    }
                }),
            )))
            .unwrap();
        let Message::Notification(diagnostics) = client.receiver.recv().unwrap() else {
            panic!("expected diagnostics notification")
        };
        assert_eq!(diagnostics.method, "textDocument/publishDiagnostics");
        assert!(diagnostics.params["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "R003"));

        client
            .sender
            .send(Message::Request(Request::new(
                2.into(),
                "textDocument/documentSymbol".into(),
                json!({ "textDocument": { "uri": uri } }),
            )))
            .unwrap();
        let Message::Response(symbols) = client.receiver.recv().unwrap() else {
            panic!("expected document-symbol response")
        };
        assert_eq!(symbols.result.unwrap()[0]["name"], "REQ:demo/example");

        client
            .sender
            .send(Message::Request(Request::new(
                3.into(),
                "shutdown".into(),
                json!(null),
            )))
            .unwrap();
        client
            .sender
            .send(Message::Notification(Notification::new(
                "exit".into(),
                json!(null),
            )))
            .unwrap();
        assert!(matches!(
            client.receiver.recv().unwrap(),
            Message::Response(_)
        ));
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn enrolled_markdown_uses_unsaved_diagnostics_symbols_hover_and_definition() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");
        let docs_dir = temp.path().join("docs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            specs_dir.join("_config.toml"),
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:demo\"\n\n[[documentation]]\nid = \"guides\"\ntitle = \"Guides\"\nroot = \"docs\"\ninclude = [\"**/*.md\"]\n",
        )
        .unwrap();
        std::fs::write(
            specs_dir.join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [carlo]\n---\n\n# Demo\n",
        )
        .unwrap();
        let guide_path = docs_dir.join("guide.md");
        let saved = "# Guide\n\n## Deploy\n\nSee the [steps](spec:doc:docs/runbook.md#heading=Runbook/Steps).\n";
        let unsaved = "# Guide\n\n## Deploy\n\nSee the [steps](spec:doc:docs/runbook.md#heading=Runbook/Steps) and [missing](missing.md).\n";
        std::fs::write(&guide_path, saved).unwrap();
        let runbook_path = docs_dir.join("runbook.md");
        std::fs::write(&runbook_path, "# Runbook\n\n## Steps\n\nDo it.\n").unwrap();

        let workspace = WorkspaceIndex::open(&specs_dir, None).unwrap();
        let uri = Url::from_file_path(&guide_path).unwrap().to_string();
        let document = OpenDocument {
            path: guide_path,
            text: unsaved.to_string(),
            version: 2,
        };
        let documents = HashMap::from([(uri.clone(), document.clone())]);
        let (server, client) = Connection::memory();
        publish_diagnostics(&server, &workspace, &uri, &document).unwrap();
        let Message::Notification(diagnostics) = client.receiver.recv().unwrap() else {
            panic!("expected diagnostics notification")
        };
        assert!(diagnostics.params["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "R029"));

        let symbols = document_symbols(
            &workspace,
            &documents,
            &json!({ "textDocument": { "uri": uri } }),
        )
        .unwrap();
        assert_eq!(symbols[0]["name"], "Guide");
        assert_eq!(symbols[1]["detail"], "Guide / Deploy");

        let token_character = unsaved.lines().nth(4).unwrap().find("spec:doc").unwrap() as u32 + 8;
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": 4, "character": token_character }
        });
        let hovered = hover(&workspace, &documents, &params).unwrap();
        assert!(hovered["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("Runbook / Steps"));
        let defined = definition(&workspace, &documents, &params).unwrap();
        assert_eq!(
            Url::parse(defined["uri"].as_str().unwrap())
                .unwrap()
                .to_file_path()
                .unwrap(),
            runbook_path.canonicalize().unwrap()
        );
        assert_eq!(defined["range"]["start"]["line"], 2);
    }

    #[test]
    fn unenrolled_markdown_is_inert() {
        let temp = tempfile::tempdir().unwrap();
        let specs_dir = temp.path().join(".specs");
        let docs_dir = temp.path().join("docs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            specs_dir.join("_config.toml"),
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:demo\"\n\n[[documentation]]\nid = \"guides\"\ntitle = \"Guides\"\nroot = \"docs\"\ninclude = [\"**/*.md\"]\n",
        )
        .unwrap();
        std::fs::write(
            specs_dir.join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [carlo]\n---\n\n# Demo\n",
        )
        .unwrap();

        let notes_path = temp.path().join("notes.md");
        let text = "# Notes\n\nSee spec:PROJECT:demo.\n";
        std::fs::write(&notes_path, text).unwrap();
        let workspace = WorkspaceIndex::open(&specs_dir, None).unwrap();
        let uri = Url::from_file_path(&notes_path).unwrap().to_string();
        let document = OpenDocument {
            path: notes_path,
            text: text.to_string(),
            version: 1,
        };
        let documents = HashMap::from([(uri.clone(), document.clone())]);
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 10 }
        });

        let (server, client) = Connection::memory();
        publish_diagnostics(&server, &workspace, &uri, &document).unwrap();
        let Message::Notification(diagnostics) = client.receiver.recv().unwrap() else {
            panic!("expected diagnostics notification")
        };
        assert!(diagnostics.params["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            completion(&workspace, &documents, &params).unwrap(),
            json!([])
        );
        assert_eq!(hover(&workspace, &documents, &params).unwrap(), Value::Null);
        assert_eq!(
            definition(&workspace, &documents, &params).unwrap(),
            Value::Null
        );
        assert_eq!(
            references(&workspace, &documents, &params).unwrap(),
            json!([])
        );
        assert_eq!(
            document_symbols(
                &workspace,
                &documents,
                &json!({ "textDocument": { "uri": uri } }),
            )
            .unwrap(),
            json!([])
        );
    }
}
