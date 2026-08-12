import { randomBytes } from 'node:crypto'
import * as path from 'node:path'
import * as vscode from 'vscode'
import { metadataOperations, MetadataUpdates, parseSpecText } from './frontmatter'
import { renderSpecBody } from './markdown'
import type { ExplorerSnapshot, ProtocolLocation } from './protocol'
import { referencePresentation, specificationAnchor } from './references'
import { incomingRefinements, type RefinementLink } from './relationships'
import { ForgeSpecService } from './service'

export class SpecViewerProvider implements vscode.CustomTextEditorProvider {
  static readonly viewType = 'forgeSpec.viewer'
  private readonly panels = new Map<string, Set<vscode.WebviewPanel>>()
  private readonly panelAnchors = new Map<vscode.WebviewPanel, string | undefined>()
  private readonly pendingAnchors = new Map<string, string>()
  private snapshot: ExplorerSnapshot = {
    generation: 0,
    stats: { parsed: 0, loadedFromCache: 0, removed: 0 },
    documents: [],
    documentationCollections: [],
    documentation: [],
  }

  constructor(private readonly service: ForgeSpecService) {}

  resolveCustomTextEditor(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
  ): void {
    const documentKey = document.uri.toString()
    const documentPanels = this.panels.get(documentKey) ?? new Set<vscode.WebviewPanel>()
    documentPanels.add(webviewPanel)
    this.panels.set(documentKey, documentPanels)

    webviewPanel.webview.options = { enableScripts: true }
    const initialAnchor = this.pendingAnchors.get(documentKey)
    this.pendingAnchors.delete(documentKey)
    this.panelAnchors.set(webviewPanel, initialAnchor)
    webviewPanel.title = viewerTitle(document.uri, initialAnchor)
    const update = () => {
      const anchor = this.panelAnchors.get(webviewPanel)
      webviewPanel.webview.html = renderViewer(
        webviewPanel.webview,
        document.getText(),
        document.version,
        anchor,
        this.refinementsFor(document.uri, anchor),
      )
    }
    update()

    const changed = vscode.workspace.onDidChangeTextDocument(event => {
      if (event.document.uri.toString() === document.uri.toString()) update()
    })
    webviewPanel.onDidDispose(() => {
      changed.dispose()
      this.panelAnchors.delete(webviewPanel)
      documentPanels.delete(webviewPanel)
      if (documentPanels.size === 0) this.panels.delete(documentKey)
    })
    webviewPanel.webview.onDidReceiveMessage(async message => {
      try {
        switch (message.type) {
          case 'openSource':
            await vscode.commands.executeCommand('forgeSpec.inspectSource', document.uri)
            break
          case 'openReference':
            await this.openLink(document.uri, String(message.href))
            break
          case 'updateMetadata':
            await applyMetadataChanges(
              this.service,
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

  setSnapshot(snapshot: ExplorerSnapshot): void {
    this.snapshot = snapshot
    for (const [documentKey, panels] of this.panels) {
      const uri = vscode.Uri.parse(documentKey)
      for (const panel of panels) {
        const anchor = this.panelAnchors.get(panel)
        if (!anchor) continue
        void panel.webview.postMessage({
          type: 'updateRefinements',
          anchor,
          refinements: this.refinementsFor(uri, anchor),
        })
      }
    }
  }

  async openReference(reference: string): Promise<void> {
    const location = await this.service.resolveReference(reference)
    if (!location) throw new Error(`Reference does not resolve: ${reference}`)

    const presentation = referencePresentation(reference)
    if (presentation?.kind === 'spec') {
      await this.openSpecLocation(location, specificationAnchor(reference))
    } else {
      await revealLocation(location)
    }
  }

  async openDocument(uri: vscode.Uri, anchor?: string): Promise<void> {
    const documentKey = uri.toString()
    const existing = this.panels.get(documentKey)?.values().next().value
    if (existing) {
      this.panelAnchors.set(existing, anchor)
      existing.title = viewerTitle(uri, anchor)
      existing.reveal()
      await existing.webview.postMessage({
        type: 'revealAnchor',
        anchor: anchor ?? '',
        refinements: this.refinementsFor(uri, anchor),
      })
      return
    }

    if (anchor) this.pendingAnchors.set(documentKey, anchor)
    try {
      await vscode.commands.executeCommand('vscode.openWith', uri, SpecViewerProvider.viewType)
    } catch (error) {
      if (anchor) this.pendingAnchors.delete(documentKey)
      throw error
    }
  }

  private async openLink(documentUri: vscode.Uri, href: string): Promise<void> {
    if (href.startsWith('spec:')) {
      await this.openReference(href)
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

  private async openSpecLocation(
    location: ProtocolLocation,
    anchor: string | undefined,
  ): Promise<void> {
    await this.openDocument(vscode.Uri.parse(location.uri), anchor)
  }

  private refinementsFor(uri: vscode.Uri, anchor: string | undefined): RefinementLink[] {
    if (!anchor) return []
    const owner = this.snapshot.documents.find(document => document.uri === uri.toString())
    if (!owner) return []
    return incomingRefinements(this.snapshot.documents, `${owner.id}#${anchor}`)
  }
}

async function applyMetadataChanges(
  service: ForgeSpecService,
  document: vscode.TextDocument,
  updates: MetadataUpdates,
  expectedVersion: number,
): Promise<void> {
  if (document.version !== expectedVersion) {
    throw new Error('The specification changed while its metadata was being edited. Review and retry.')
  }
  const metadata = parseSpecText(document.getText()).metadata
  const operations = metadataOperations(metadata, updates)
  if (operations.length === 0) return
  if (!(await service.applyChanges(document, operations))) {
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

function renderViewer(
  webview: vscode.Webview,
  text: string,
  version: number,
  revealAnchor?: string,
  refinements: readonly RefinementLink[] = [],
): string {
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
  const relationships = renderRelationships(metadata)
  const renderedBody = error
    ? `<div class="error">${escapeHtml(error)}</div><pre>${escapeHtml(text)}</pre>`
    : renderSpecBody(body)
  const statusControl = status === 'superseded'
    ? '<input name="status" value="superseded" readonly>'
    : `<select name="status">${options(['draft', 'accepted', 'deprecated'], status)}</select>`

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <style nonce="${nonce}">${viewerStyles}</style>
</head>
<body${revealAnchor ? ` data-reveal-anchor="${escapeAttribute(revealAnchor)}"` : ''}>
  <header>
    <div class="identity">
      <span class="type">${escapeHtml(entityType)}</span>
      <h1>${escapeHtml(id)}</h1>
      <span class="status status-${escapeHtml(status)}">${escapeHtml(status)}</span>
      ${progress ? `<span class="progress">${escapeHtml(progress)}</span>` : ''}
    </div>
    <p>${escapeHtml(summary)}</p>
    <button id="inspect-source" type="button">Inspect source</button>
  </header>
  <main>
    <article>${renderedBody}</article>
    <aside>
      ${renderRefinementContext(revealAnchor, refinements)}
      <h2>Specification metadata</h2>
      <form id="metadata-form" data-version="${version}">
        <label>ID<input value="${escapeAttribute(id)}" disabled></label>
        <label>Type<input value="${escapeAttribute(entityType)}" disabled></label>
        <label>Status${statusControl}</label>
        ${entityType === 'task' ? `<label>Progress<select name="progress">${options(['pending', 'in-progress', 'done', 'blocked', 'deferred', 'wontdo'], progress)}</select></label>` : ''}
        ${entityType === 'requirement' ? `<label>Level<select name="level">${options(['MUST', 'SHOULD', 'MAY', 'INFO'], level)}</select></label>` : ''}
        <label>Summary<textarea name="summary" rows="5">${escapeHtml(summary)}</textarea></label>
        <label>Owners<input name="owners" value="${escapeAttribute(owners.join(', '))}" placeholder="owner, owner"></label>
        ${relationships}
        <button class="primary" type="submit">Apply metadata</button>
        <p class="hint">Rust validates typed changes before VS Code adds them to the normal undo, dirty, and save flow.</p>
      </form>
    </aside>
  </main>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    let currentAnchor = document.body.dataset.revealAnchor || '';
    const refinementContext = document.getElementById('refinement-context');
    const refinementAnchor = document.getElementById('refinement-anchor');
    const refinementList = document.getElementById('refinement-list');
    const refinementEmpty = document.getElementById('refinement-empty');
    const renderRefinementContext = (anchor, refinements) => {
      currentAnchor = anchor;
      refinementContext.hidden = !anchor;
      refinementAnchor.textContent = anchor ? '#' + anchor : '';
      refinementList.replaceChildren();
      for (const refinement of refinements) {
        const row = document.createElement('div');
        row.className = 'refinement-item';
        const link = document.createElement('a');
        link.className = 'forge-reference forge-reference-spec';
        link.href = String(refinement.reference || '');
        link.title = String(refinement.title || '');
        link.textContent = String(refinement.label || refinement.title || 'Specification');
        const state = document.createElement('span');
        state.className = 'refinement-state';
        state.textContent = [refinement.status, refinement.progress].filter(Boolean).join(' · ');
        row.append(link, state);
        refinementList.append(row);
      }
      refinementEmpty.hidden = refinements.length > 0;
    };
    const clearAnchorFocus = () => {
      document.querySelectorAll('.spec-anchor-focused').forEach(element => {
        element.classList.remove('spec-anchor-focused');
        element.removeAttribute('aria-current');
      });
      document.querySelectorAll('.spec-anchor-context').forEach(element => element.classList.remove('spec-anchor-context'));
    };
    const revealAnchor = anchor => {
      clearAnchorFocus();
      if (!anchor) {
        document.querySelector('article')?.scrollTo({ top: 0, behavior: 'instant' });
        return;
      }
      requestAnimationFrame(() => {
        const target = document.getElementById('spec-anchor-' + anchor);
        if (!target) return;
        target.classList.add('spec-anchor-focused');
        target.setAttribute('aria-current', 'true');
        target.closest('.spec-block')?.classList.add('spec-anchor-context');
        target.scrollIntoView({ block: 'start' });
      });
    };
    revealAnchor(document.body.dataset.revealAnchor);
    window.addEventListener('message', event => {
      if (event.data?.type === 'revealAnchor') {
        const anchor = String(event.data.anchor || '');
        renderRefinementContext(anchor, Array.isArray(event.data.refinements) ? event.data.refinements : []);
        revealAnchor(anchor);
      }
      if (event.data?.type === 'updateRefinements' && String(event.data.anchor || '') === currentAnchor) {
        renderRefinementContext(currentAnchor, Array.isArray(event.data.refinements) ? event.data.refinements : []);
      }
    });
    document.getElementById('inspect-source').addEventListener('click', () => vscode.postMessage({ type: 'openSource' }));
    document.addEventListener('click', event => {
      const anchor = event.target instanceof Element ? event.target.closest('a') : null;
      const href = anchor?.getAttribute('href');
      if (!href || href.startsWith('#')) return;
      event.preventDefault();
      vscode.postMessage({ type: 'openReference', href });
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

function renderRefinementContext(
  anchor: string | undefined,
  refinements: readonly RefinementLink[],
): string {
  return `<section id="refinement-context" class="refinement-context"${anchor ? '' : ' hidden'}>
    <h2>Requirement <code id="refinement-anchor">${anchor ? `#${escapeHtml(anchor)}` : ''}</code></h2>
    <h3>Refined by</h3>
    <div id="refinement-list" class="refinement-list">${refinements.map(renderRefinement).join('')}</div>
    <p id="refinement-empty" class="hint"${refinements.length > 0 ? ' hidden' : ''}>No specifications refine this requirement yet.</p>
  </section>`
}

function renderRefinement(refinement: RefinementLink): string {
  const state = [refinement.status, refinement.progress].filter(Boolean).join(' · ')
  return `<div class="refinement-item"><a class="forge-reference forge-reference-spec" href="${escapeAttribute(refinement.reference)}" title="${escapeAttribute(refinement.title)}">${escapeHtml(refinement.label)}</a><span class="refinement-state">${escapeHtml(state)}</span></div>`
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

function renderRelationships(metadata: Record<string, unknown>): string {
  const groups: Array<[string, string[]]> = [
    ['Refines', arrayValue(metadata.refines)],
    ['Categorized under', arrayValue(metadata.categorized_under)],
    ['Related', arrayValue(metadata.related)],
    ['Blocked by', arrayValue(metadata.blocked_by)],
    ['Supersedes', arrayValue(metadata.supersedes)],
  ]
  const populated = groups.filter(([, references]) => references.length > 0)
  if (populated.length === 0) return ''

  return `<section class="relationships"><h3>Relationships</h3>${populated.map(([label, references]) => `<div class="relationship"><span>${escapeHtml(label)}</span><div>${references.map(renderRelationship).join('')}</div></div>`).join('')}</section>`
}

function renderRelationship(reference: string): string {
  const href = reference.startsWith('spec:') ? reference : `spec:${reference}`
  const presentation = referencePresentation(href)
  const label = presentation?.label ?? reference
  return `<a class="forge-reference forge-reference-${presentation?.kind ?? 'spec'}" href="${escapeAttribute(href)}" title="${escapeAttribute(presentation?.title ?? reference)}">${escapeHtml(label)}</a>`
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

function viewerTitle(uri: vscode.Uri, anchor?: string): string {
  const filename = path.basename(uri.fsPath)
  return anchor ? `#${anchor} · ${filename}` : filename
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
body { display: flex; flex-direction: column; height: 100vh; margin: 0; overflow: hidden; color: var(--vscode-editor-foreground); background: var(--vscode-editor-background); font: var(--vscode-font-size)/1.55 var(--vscode-font-family); }
header { position: relative; z-index: 2; flex: 0 0 auto; padding: 20px 30px 16px; border-bottom: 1px solid var(--vscode-panel-border); background: color-mix(in srgb, var(--vscode-editor-background) 94%, transparent); backdrop-filter: blur(10px); }
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
main { display: grid; grid-template-columns: minmax(0, 1fr) 300px; flex: 1 1 auto; min-height: 0; }
article { width: min(900px, 100%); min-height: 0; overflow-y: auto; padding: 30px 42px 80px; }
article h1, article h2, article h3 { line-height: 1.25; margin-top: 1.7em; }
article a { color: var(--vscode-textLink-foreground); }
article a.forge-reference, .relationships a.forge-reference { display: inline-flex; align-items: center; gap: 4px; max-width: 100%; margin: 0 2px; padding: 0 5px; border: 1px solid color-mix(in srgb, var(--vscode-textLink-foreground) 35%, transparent); border-radius: 4px; background: color-mix(in srgb, var(--vscode-textLink-foreground) 9%, transparent); text-decoration: none; vertical-align: baseline; }
article a.forge-reference:hover, .relationships a.forge-reference:hover { border-color: var(--vscode-textLink-activeForeground); color: var(--vscode-textLink-activeForeground); }
article a.forge-reference::before, .relationships a.forge-reference::before { color: var(--vscode-descriptionForeground); font-size: .85em; }
article a.forge-reference-spec::before, .relationships a.forge-reference-spec::before { content: '◇'; }
article a.forge-reference-source::before, .relationships a.forge-reference-source::before { content: '⌘'; }
article a.forge-reference-documentation::before, .relationships a.forge-reference-documentation::before { content: '§'; }
article code { font-family: var(--vscode-editor-font-family); background: var(--vscode-textCodeBlock-background); padding: 1px 4px; border-radius: 3px; }
article pre { overflow: auto; padding: 14px; background: var(--vscode-textCodeBlock-background); border-radius: 6px; }
article blockquote { border-left: 3px solid var(--vscode-textBlockQuote-border); margin-left: 0; padding-left: 16px; color: var(--vscode-descriptionForeground); }
aside { min-height: 0; overflow-y: auto; border-left: 1px solid var(--vscode-panel-border); padding: 24px 20px; background: var(--vscode-sideBar-background); }
aside h2 { margin: 0 0 18px; font-size: 15px; }
aside h2 code { color: var(--vscode-textLink-foreground); font-family: var(--vscode-editor-font-family); font-size: .9em; }
label { display: grid; gap: 5px; margin-bottom: 14px; color: var(--vscode-descriptionForeground); font-size: 12px; font-weight: 600; }
input, select, textarea { width: 100%; border: 1px solid var(--vscode-input-border, transparent); color: var(--vscode-input-foreground); background: var(--vscode-input-background); padding: 7px 8px; font: inherit; font-weight: 400; }
input:focus, select:focus, textarea:focus { outline: 1px solid var(--vscode-focusBorder); }
button { border: 1px solid var(--vscode-button-border, transparent); color: var(--vscode-button-foreground); background: var(--vscode-button-background); padding: 6px 11px; cursor: pointer; }
button:hover { background: var(--vscode-button-hoverBackground); }
.primary { width: 100%; }
.hint { color: var(--vscode-descriptionForeground); font-size: 11px; }
.relationships { margin: 20px 0; padding-top: 16px; border-top: 1px solid var(--vscode-panel-border); }
.relationships h3 { margin: 0 0 12px; color: var(--vscode-foreground); font-size: 12px; }
.relationship { display: grid; gap: 5px; margin-bottom: 10px; }
.relationship > span { color: var(--vscode-descriptionForeground); font-size: 11px; font-weight: 600; }
.relationship > div { display: flex; flex-wrap: wrap; gap: 5px; }
.refinement-context { margin: 0 0 22px; padding: 0 0 20px; border-bottom: 1px solid var(--vscode-panel-border); }
.refinement-context h2 { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; margin-bottom: 14px; }
.refinement-context h3 { margin: 0 0 9px; color: var(--vscode-descriptionForeground); font-size: 11px; }
.refinement-list { display: grid; gap: 7px; }
.refinement-item { display: grid; gap: 2px; justify-items: start; }
.refinement-state { color: var(--vscode-descriptionForeground); font-size: 10px; text-transform: uppercase; letter-spacing: .03em; }
[hidden] { display: none !important; }
.spec-block { margin: 22px 0; padding: 16px 18px 8px; border: 1px solid var(--vscode-panel-border); border-left: 3px solid var(--vscode-focusBorder); border-radius: 5px; background: color-mix(in srgb, var(--vscode-editor-background) 88%, var(--vscode-focusBorder)); }
.spec-block[id] { scroll-margin-top: 120px; }
.spec-block.spec-anchor-focused, .spec-block.spec-anchor-context { border-color: var(--vscode-focusBorder); box-shadow: 0 0 0 1px var(--vscode-focusBorder), 0 10px 30px color-mix(in srgb, var(--vscode-focusBorder) 14%, transparent); }
.spec-block-title { display: flex; gap: 8px; align-items: center; color: var(--vscode-descriptionForeground); font-size: 11px; text-transform: uppercase; letter-spacing: .04em; }
.spec-block-title strong { margin-left: auto; }
.clause-anchor { display: inline-flex; scroll-margin-top: 120px; margin-right: 6px; padding: 0 5px; border-radius: 4px; color: var(--vscode-symbolIcon-fieldForeground, var(--vscode-textLink-foreground)); background: color-mix(in srgb, currentColor 10%, transparent); font-family: var(--vscode-editor-font-family); font-size: .9em; }
.clause-anchor.spec-anchor-focused { outline: 1px solid var(--vscode-focusBorder); background: color-mix(in srgb, var(--vscode-focusBorder) 22%, transparent); }
.error { padding: 12px; color: var(--vscode-errorForeground); border: 1px solid var(--vscode-inputValidation-errorBorder); background: var(--vscode-inputValidation-errorBackground); }
@media (max-width: 760px) { body { display: block; height: auto; min-height: 100vh; overflow: auto; } header { position: sticky; top: 0; } main { display: grid; grid-template-columns: 1fr; min-height: calc(100vh - 110px); } article, aside { overflow: visible; } aside { border-left: 0; border-top: 1px solid var(--vscode-panel-border); } header button { position: static; margin-top: 14px; } }
`
