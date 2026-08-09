import { randomBytes } from 'node:crypto'
import * as path from 'node:path'
import MarkdownIt from 'markdown-it'
import * as vscode from 'vscode'
import { computeMetadataEdits, MetadataUpdates, parseSpecText } from './frontmatter'
import type { ProtocolLocation } from './protocol'
import { ForgeSpecService } from './service'

const markdown = new MarkdownIt({ html: false, linkify: true, typographer: true })

export class SpecViewerProvider implements vscode.CustomTextEditorProvider {
  static readonly viewType = 'forgeSpec.viewer'

  constructor(private readonly service: ForgeSpecService) {}

  resolveCustomTextEditor(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
  ): void {
    webviewPanel.webview.options = { enableScripts: true }
    const update = () => {
      webviewPanel.webview.html = renderViewer(
        webviewPanel.webview,
        document.getText(),
        document.version,
      )
    }
    update()

    const changed = vscode.workspace.onDidChangeTextDocument(event => {
      if (event.document.uri.toString() === document.uri.toString()) update()
    })
    webviewPanel.onDidDispose(() => changed.dispose())
    webviewPanel.webview.onDidReceiveMessage(async message => {
      try {
        switch (message.type) {
          case 'openText':
            await vscode.commands.executeCommand('vscode.openWith', document.uri, 'default')
            break
          case 'openReference':
            await this.openLink(document.uri, String(message.href))
            break
          case 'updateMetadata':
            await applyMetadataEdits(
              document,
              message.updates as MetadataUpdates,
              Number(message.expectedVersion),
            )
            break
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error)
        void vscode.window.showErrorMessage(`Forge Spec: ${message}`)
      }
    })
  }

  private async openLink(documentUri: vscode.Uri, href: string): Promise<void> {
    if (href.startsWith('spec:')) {
      const location = await this.service.resolveReference(href)
      if (!location) throw new Error(`Reference does not resolve: ${href}`)
      await revealLocation(location)
      return
    }
    if (/^https?:\/\//.test(href)) {
      await vscode.env.openExternal(vscode.Uri.parse(href))
      return
    }
    if (href.startsWith('#')) return
    const [relative, fragment] = href.split('#', 2)
    const target = vscode.Uri.file(path.resolve(path.dirname(documentUri.fsPath), relative))
    const targetDocument = await vscode.workspace.openTextDocument(target)
    const editor = await vscode.window.showTextDocument(targetDocument)
    if (fragment) {
      const wanted = decodeURIComponent(fragment).toLowerCase()
      const line = targetDocument
        .getText()
        .split(/\r?\n/)
        .findIndex(value => headingSlug(value) === wanted)
      if (line >= 0) editor.revealRange(new vscode.Range(line, 0, line, 0))
    }
  }
}

async function applyMetadataEdits(
  document: vscode.TextDocument,
  updates: MetadataUpdates,
  expectedVersion: number,
): Promise<void> {
  if (document.version !== expectedVersion) {
    throw new Error('The specification changed while its metadata was being edited. Review and retry.')
  }
  const edits = computeMetadataEdits(document.getText(), updates)
  const workspaceEdit = new vscode.WorkspaceEdit()
  for (const edit of edits) {
    workspaceEdit.replace(
      document.uri,
      new vscode.Range(edit.startLine, 0, edit.endLine, 0),
      edit.newText,
    )
  }
  if (!(await vscode.workspace.applyEdit(workspaceEdit))) {
    throw new Error('VS Code rejected the metadata edit')
  }
}

async function revealLocation(location: ProtocolLocation): Promise<void> {
  const uri = vscode.Uri.parse(location.uri)
  const document = await vscode.workspace.openTextDocument(uri)
  const range = new vscode.Range(
    location.range.start.line,
    location.range.start.character,
    location.range.end.line,
    location.range.end.character,
  )
  const editor = await vscode.window.showTextDocument(document, { selection: range })
  editor.revealRange(range, vscode.TextEditorRevealType.InCenterIfOutsideViewport)
}

function renderViewer(webview: vscode.Webview, text: string, version: number): string {
  const nonce = randomBytes(16).toString('base64')
  let metadata: Record<string, unknown> = {}
  let body = text
  let error: string | undefined
  try {
    const parsed = parseSpecText(text)
    metadata = parsed.metadata
    body = parsed.body
  } catch (caught) {
    error = caught instanceof Error ? caught.message : String(caught)
  }

  const id = stringValue(metadata.id, 'Invalid specification')
  const entityType = stringValue(metadata.type, 'unknown')
  const status = stringValue(metadata.status, 'draft')
  const summary = stringValue(metadata.summary, '')
  const owners = arrayValue(metadata.owners)
  const progress = stringValue(metadata.progress, '')
  const level = stringValue(metadata.level, '')
  const renderedBody = error
    ? `<div class="error">${escapeHtml(error)}</div><pre>${escapeHtml(text)}</pre>`
    : renderSpecBody(body)

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <style nonce="${nonce}">${viewerStyles}</style>
</head>
<body>
  <header>
    <div class="identity">
      <span class="type">${escapeHtml(entityType)}</span>
      <h1>${escapeHtml(id)}</h1>
      <span class="status status-${escapeHtml(status)}">${escapeHtml(status)}</span>
      ${progress ? `<span class="progress">${escapeHtml(progress)}</span>` : ''}
    </div>
    <p>${escapeHtml(summary)}</p>
    <button id="open-text" type="button">Open Markdown</button>
  </header>
  <main>
    <article>${renderedBody}</article>
    <aside>
      <h2>Metadata</h2>
      <form id="metadata-form" data-version="${version}">
        <label>ID<input value="${escapeAttribute(id)}" disabled></label>
        <label>Type<input value="${escapeAttribute(entityType)}" disabled></label>
        <label>Status
          <select name="status">${options(['draft', 'accepted', 'deprecated', 'superseded'], status)}</select>
        </label>
        ${entityType === 'task' ? `<label>Progress<select name="progress">${options(['pending', 'in-progress', 'done', 'blocked', 'deferred', 'wontdo'], progress)}</select></label>` : ''}
        ${entityType === 'requirement' ? `<label>Level<select name="level">${options(['MUST', 'SHOULD', 'MAY', 'INFO'], level)}</select></label>` : ''}
        <label>Summary<textarea name="summary" rows="5">${escapeHtml(summary)}</textarea></label>
        <label>Owners<input name="owners" value="${escapeAttribute(owners.join(', '))}" placeholder="owner, owner"></label>
        <button class="primary" type="submit">Apply metadata</button>
        <p class="hint">Changes use the document's normal undo, dirty, and save flow.</p>
      </form>
    </aside>
  </main>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    document.getElementById('open-text').addEventListener('click', () => vscode.postMessage({ type: 'openText' }));
    document.addEventListener('click', event => {
      const anchor = event.target.closest('a');
      if (!anchor || anchor.getAttribute('href').startsWith('#')) return;
      event.preventDefault();
      vscode.postMessage({ type: 'openReference', href: anchor.getAttribute('href') });
    });
    document.getElementById('metadata-form').addEventListener('submit', event => {
      event.preventDefault();
      const form = event.currentTarget;
      const data = new FormData(form);
      const updates = {
        status: String(data.get('status') || ''),
        summary: String(data.get('summary') || ''),
        owners: String(data.get('owners') || '').split(',').map(value => value.trim()).filter(Boolean),
      };
      if (data.has('progress')) updates.progress = String(data.get('progress'));
      if (data.has('level')) updates.level = String(data.get('level'));
      vscode.postMessage({ type: 'updateMetadata', expectedVersion: Number(form.dataset.version), updates });
    });
  </script>
</body>
</html>`
}

function renderSpecBody(body: string): string {
  const lines = body.split(/\r?\n/)
  const result: string[] = []
  let plain: string[] = []
  const flush = () => {
    if (plain.length > 0) result.push(markdown.render(plain.join('\n')))
    plain = []
  }
  for (let index = 0; index < lines.length; index += 1) {
    const match = lines[index].match(/^:::\{([a-z-]+)(.*?)\}\s*$/)
    if (!match) {
      plain.push(lines[index])
      continue
    }
    flush()
    const inner: string[] = []
    index += 1
    while (index < lines.length && lines[index].trim() !== ':::') {
      inner.push(lines[index])
      index += 1
    }
    const id = match[2].match(/\bid="([^"]+)"/)?.[1]
    const level = match[2].match(/\blevel="([^"]+)"/)?.[1]
    result.push(`<section class="spec-block"><div class="spec-block-title"><span>${escapeHtml(match[1])}</span>${id ? `<code>#${escapeHtml(id)}</code>` : ''}${level ? `<strong>${escapeHtml(level)}</strong>` : ''}</div>${markdown.render(inner.join('\n'))}</section>`)
  }
  flush()
  return result.join('\n')
}

function options(values: string[], selected: string): string {
  return values
    .map(value => `<option value="${escapeAttribute(value)}"${value === selected ? ' selected' : ''}>${escapeHtml(value)}</option>`)
    .join('')
}

function stringValue(value: unknown, fallback: string): string {
  return typeof value === 'string' ? value : fallback
}

function arrayValue(value: unknown): string[] {
  return Array.isArray(value) ? value.map(String) : []
}

function headingSlug(line: string): string | undefined {
  const heading = line.match(/^ {0,3}#{1,6}\s+(.+?)\s*#*\s*$/)?.[1]
  if (!heading) return undefined
  return heading
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
    .trim()
    .replace(/\s+/g, '-')
}

function escapeAttribute(value: string): string {
  return escapeHtml(value).replace(/`/g, '&#96;')
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

const viewerStyles = `
:root { color-scheme: light dark; }
* { box-sizing: border-box; }
body { margin: 0; color: var(--vscode-editor-foreground); background: var(--vscode-editor-background); font: var(--vscode-font-size)/1.55 var(--vscode-font-family); }
header { position: sticky; top: 0; z-index: 2; padding: 20px 30px 16px; border-bottom: 1px solid var(--vscode-panel-border); background: color-mix(in srgb, var(--vscode-editor-background) 94%, transparent); backdrop-filter: blur(10px); }
.identity { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
h1 { margin: 0; font-size: 22px; }
header p { margin: 8px 0 0; max-width: 880px; color: var(--vscode-descriptionForeground); }
header button { position: absolute; right: 30px; top: 20px; }
.type, .status, .progress { padding: 2px 7px; border-radius: 999px; font-size: 11px; font-weight: 650; text-transform: uppercase; letter-spacing: .04em; }
.type { color: var(--vscode-badge-foreground); background: var(--vscode-badge-background); }
.status { border: 1px solid var(--vscode-panel-border); }
.status-accepted { color: var(--vscode-testing-iconPassed); }
.status-draft { color: var(--vscode-editorWarning-foreground); }
.progress { color: var(--vscode-gitDecoration-modifiedResourceForeground); border: 1px solid currentColor; }
main { display: grid; grid-template-columns: minmax(0, 1fr) 300px; min-height: calc(100vh - 110px); }
article { width: min(900px, 100%); padding: 30px 42px 80px; }
article h1, article h2, article h3 { line-height: 1.25; margin-top: 1.7em; }
article a { color: var(--vscode-textLink-foreground); }
article code { font-family: var(--vscode-editor-font-family); background: var(--vscode-textCodeBlock-background); padding: 1px 4px; border-radius: 3px; }
article pre { overflow: auto; padding: 14px; background: var(--vscode-textCodeBlock-background); border-radius: 6px; }
article blockquote { border-left: 3px solid var(--vscode-textBlockQuote-border); margin-left: 0; padding-left: 16px; color: var(--vscode-descriptionForeground); }
aside { border-left: 1px solid var(--vscode-panel-border); padding: 24px 20px; background: var(--vscode-sideBar-background); }
aside h2 { margin: 0 0 18px; font-size: 15px; }
label { display: grid; gap: 5px; margin-bottom: 14px; color: var(--vscode-descriptionForeground); font-size: 12px; font-weight: 600; }
input, select, textarea { width: 100%; border: 1px solid var(--vscode-input-border, transparent); color: var(--vscode-input-foreground); background: var(--vscode-input-background); padding: 7px 8px; font: inherit; font-weight: 400; }
input:focus, select:focus, textarea:focus { outline: 1px solid var(--vscode-focusBorder); }
button { border: 1px solid var(--vscode-button-border, transparent); color: var(--vscode-button-foreground); background: var(--vscode-button-background); padding: 6px 11px; cursor: pointer; }
button:hover { background: var(--vscode-button-hoverBackground); }
.primary { width: 100%; }
.hint { color: var(--vscode-descriptionForeground); font-size: 11px; }
.spec-block { margin: 22px 0; padding: 16px 18px 8px; border: 1px solid var(--vscode-panel-border); border-left: 3px solid var(--vscode-focusBorder); border-radius: 5px; background: color-mix(in srgb, var(--vscode-editor-background) 88%, var(--vscode-focusBorder)); }
.spec-block-title { display: flex; gap: 8px; align-items: center; color: var(--vscode-descriptionForeground); font-size: 11px; text-transform: uppercase; letter-spacing: .04em; }
.spec-block-title strong { margin-left: auto; }
.error { padding: 12px; color: var(--vscode-errorForeground); border: 1px solid var(--vscode-inputValidation-errorBorder); background: var(--vscode-inputValidation-errorBackground); }
@media (max-width: 760px) { main { grid-template-columns: 1fr; } aside { border-left: 0; border-top: 1px solid var(--vscode-panel-border); } header button { position: static; margin-top: 14px; } }
`
