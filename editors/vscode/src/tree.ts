import * as path from 'node:path'
import * as vscode from 'vscode'
import type {
  ExplorerBlock,
  ExplorerDocument,
  ExplorerDocumentation,
  ExplorerDocumentationCollection,
  ExplorerDocumentationHeading,
  ExplorerSnapshot,
  ExplorerSource,
} from './protocol'
import { specificationDisplayName, specificationReference } from './references'
import { ForgeSpecService } from './service'

export type ExplorerNode =
  | { kind: 'spec'; document: ExplorerDocument; ancestry: string[] }
  | { kind: 'block'; owner: ExplorerDocument; block: ExplorerBlock; ancestry: string[] }
  | { kind: 'code'; owner: ExplorerDocument; ancestry: string[] }
  | { kind: 'source'; source: ExplorerSource }
  | { kind: 'documentation-root' }
  | { kind: 'documentation-collection'; collection: ExplorerDocumentationCollection }
  | { kind: 'documentation-directory'; collection: ExplorerDocumentationCollection; prefix: string }
  | { kind: 'documentation'; document: ExplorerDocumentation }
  | { kind: 'documentation-heading'; document: ExplorerDocumentation; heading: ExplorerDocumentationHeading }
  | { kind: 'spec-documentation-group'; owner: ExplorerDocument; direction: 'outgoing' | 'incoming' }
  | { kind: 'documentation-reference'; reference: string; label: string; description?: string }
  | { kind: 'cycle'; id: string }

export class ForgeSpecTreeProvider implements vscode.TreeDataProvider<ExplorerNode> {
  private readonly changedEmitter = new vscode.EventEmitter<ExplorerNode | undefined>()
  readonly onDidChangeTreeData = this.changedEmitter.event
  private snapshotValue: ExplorerSnapshot = {
    generation: 0,
    stats: { parsed: 0, loadedFromCache: 0, removed: 0 },
    projectId: undefined,
    documents: [],
    documentationCollections: [],
    documentation: [],
  }
  private documentsById = new Map<string, ExplorerDocument>()
  private placements = new Map<string, string[]>()

  constructor(private readonly service: ForgeSpecService) {}

  get snapshot(): ExplorerSnapshot {
    return this.snapshotValue
  }

  documentForUri(uri: vscode.Uri): ExplorerDocument | undefined {
    return this.snapshotValue.documents.find(document => document.uri === uri.toString())
  }

  async refresh(): Promise<void> {
    this.snapshotValue = await this.service.snapshot()
    this.rebuildPlacements()
    this.changedEmitter.fire(undefined)
  }

  getTreeItem(node: ExplorerNode): vscode.TreeItem {
    switch (node.kind) {
      case 'spec':
        return this.specItem(node)
      case 'block': {
        const hasChildren = (this.placements.get(`${node.owner.id}#${node.block.id}`) ?? []).length > 0
        const item = new vscode.TreeItem(
          `#${node.block.id}`,
          hasChildren
            ? vscode.TreeItemCollapsibleState.Collapsed
            : vscode.TreeItemCollapsibleState.None,
        )
        const tooltip = new vscode.MarkdownString()
        tooltip.appendMarkdown(`**#${node.block.id}**\n\n`)
        tooltip.appendMarkdown(`${humanizeBlockKind(node.block.kind)} in **${node.owner.title}**`)
        if (node.block.text) tooltip.appendMarkdown(`\n\n${node.block.text}`)
        item.tooltip = tooltip
        item.iconPath = iconForBlock(node.block.kind)
        item.command = {
          command: 'forgeSpec.openReference',
          title: `Open ${node.block.kind}`,
          arguments: [specificationReference(node.owner.id, node.block.id)],
        }
        item.contextValue = 'forgeSpec.block'
        return item
      }
      case 'code': {
        const item = new vscode.TreeItem('Code', vscode.TreeItemCollapsibleState.Collapsed)
        item.description = String(node.owner.sources.length)
        item.iconPath = new vscode.ThemeIcon('code')
        item.contextValue = 'forgeSpec.code'
        return item
      }
      case 'source': {
        const label = node.source.label || path.basename(node.source.path)
        const item = new vscode.TreeItem(label, vscode.TreeItemCollapsibleState.None)
        item.description = sourceDescription(node.source)
        item.tooltip = node.source.reference
        item.iconPath = new vscode.ThemeIcon('file-code')
        item.command = {
          command: 'forgeSpec.openReference',
          title: 'Open source reference',
          arguments: [node.source.reference],
        }
        item.contextValue = 'forgeSpec.source'
        return item
      }
      case 'documentation-root': {
        const item = new vscode.TreeItem('Documentation', vscode.TreeItemCollapsibleState.Expanded)
        item.description = String(this.snapshotValue.documentation.length)
        item.iconPath = new vscode.ThemeIcon('book')
        item.contextValue = 'forgeSpec.documentationRoot'
        return item
      }
      case 'documentation-collection': {
        const item = new vscode.TreeItem(node.collection.title, vscode.TreeItemCollapsibleState.Collapsed)
        const count = this.snapshotValue.documentation.filter(
          document => document.collectionId === node.collection.id,
        ).length
        item.description = String(count)
        item.tooltip = `Documentation collection ${node.collection.id} · ${node.collection.root}`
        item.iconPath = new vscode.ThemeIcon('library')
        item.contextValue = 'forgeSpec.documentationCollection'
        return item
      }
      case 'documentation-directory': {
        const item = new vscode.TreeItem(
          path.basename(node.prefix),
          vscode.TreeItemCollapsibleState.Collapsed,
        )
        item.iconPath = new vscode.ThemeIcon('folder')
        item.contextValue = 'forgeSpec.documentationDirectory'
        return item
      }
      case 'documentation': {
        const item = new vscode.TreeItem(
          node.document.title,
          node.document.headings.length > 0
            ? vscode.TreeItemCollapsibleState.Collapsed
            : vscode.TreeItemCollapsibleState.None,
        )
        item.description = path.basename(node.document.path)
        const tooltip = new vscode.MarkdownString()
        tooltip.appendMarkdown(`**${node.document.title}**\n\n`)
        tooltip.appendCodeblock(node.document.path)
        if (node.document.summary) tooltip.appendMarkdown(`\n${node.document.summary}`)
        item.tooltip = tooltip
        item.iconPath = new vscode.ThemeIcon('markdown')
        item.resourceUri = vscode.Uri.parse(node.document.uri)
        item.command = {
          command: 'forgeSpec.openReference',
          title: 'Open documentation',
          arguments: [`spec:doc:${node.document.path}`],
        }
        item.contextValue = 'forgeSpec.documentation'
        return item
      }
      case 'documentation-heading': {
        const item = new vscode.TreeItem(node.heading.title, vscode.TreeItemCollapsibleState.None)
        item.description = `H${node.heading.level}`
        item.iconPath = new vscode.ThemeIcon('symbol-key')
        item.command = {
          command: 'forgeSpec.openReference',
          title: 'Open documentation heading',
          arguments: [node.heading.reference],
        }
        item.contextValue = 'forgeSpec.documentationHeading'
        return item
      }
      case 'spec-documentation-group': {
        const count = node.direction === 'outgoing'
          ? node.owner.documentation.length
          : node.owner.documentedBy.length
        const item = new vscode.TreeItem(
          node.direction === 'outgoing' ? 'Documentation' : 'Referenced by documentation',
          vscode.TreeItemCollapsibleState.Collapsed,
        )
        item.description = String(count)
        item.iconPath = new vscode.ThemeIcon(node.direction === 'outgoing' ? 'book' : 'references')
        return item
      }
      case 'documentation-reference': {
        const item = new vscode.TreeItem(node.label, vscode.TreeItemCollapsibleState.None)
        item.description = node.description
        item.tooltip = node.reference
        item.iconPath = new vscode.ThemeIcon('markdown')
        item.command = {
          command: 'forgeSpec.openReference',
          title: 'Open documentation reference',
          arguments: [node.reference],
        }
        return item
      }
      case 'cycle': {
        const item = new vscode.TreeItem(`${node.id} (cycle)`, vscode.TreeItemCollapsibleState.None)
        item.iconPath = new vscode.ThemeIcon('warning')
        return item
      }
    }
  }

  getChildren(node?: ExplorerNode): ExplorerNode[] {
    if (!node) {
      const project = this.snapshotValue.projectId
        ? this.documentsById.get(this.snapshotValue.projectId)
        : undefined
      const roots: ExplorerNode[] = []
      if (project) {
        roots.push({ kind: 'spec', document: project, ancestry: [] })
      } else {
        const placed = new Set<string>()
        for (const children of this.placements.values()) {
          for (const child of children) placed.add(child)
        }
        roots.push(...this.snapshotValue.documents
          .filter(document => !placed.has(document.id))
          .sort(compareDocuments)
          .map(document => ({ kind: 'spec', document, ancestry: [] } as ExplorerNode)))
      }
      if (this.snapshotValue.documentationCollections.length > 0) {
        roots.push({ kind: 'documentation-root' })
      }
      return roots
    }

    if (
      node.kind === 'source' ||
      node.kind === 'cycle' ||
      node.kind === 'documentation-heading' ||
      node.kind === 'documentation-reference'
    ) return []
    if (node.kind === 'documentation-root') {
      return this.snapshotValue.documentationCollections.map(collection => ({
        kind: 'documentation-collection',
        collection,
      }))
    }
    if (node.kind === 'documentation-collection') {
      return this.documentationChildren(node.collection, '')
    }
    if (node.kind === 'documentation-directory') {
      return this.documentationChildren(node.collection, node.prefix)
    }
    if (node.kind === 'documentation') {
      return node.document.headings.map(heading => ({
        kind: 'documentation-heading',
        document: node.document,
        heading,
      }))
    }
    if (node.kind === 'spec-documentation-group') {
      if (node.direction === 'outgoing') {
        return node.owner.documentation.map(reference => ({
          kind: 'documentation-reference',
          reference: reference.reference,
          label: reference.label || reference.reference,
        }))
      }
      return node.owner.documentedBy.map(backlink => ({
        kind: 'documentation-reference',
        reference: `spec:doc:${backlink.source}`,
        label: backlink.label || path.basename(backlink.source),
        description: `line ${backlink.line}`,
      }))
    }

    if (node.kind === 'code') {
      return node.owner.sources.map(source => ({ kind: 'source', source }))
    }
    if (node.kind === 'block') {
      return this.specChildren(`${node.owner.id}#${node.block.id}`, node.ancestry)
    }

    const ancestry = [...node.ancestry, node.document.id]
    const children: ExplorerNode[] = []
    children.push(...this.specChildren(node.document.id, ancestry))
    for (const block of node.document.blocks) {
      children.push({ kind: 'block', owner: node.document, block, ancestry })
    }
    if (node.document.sources.length > 0) {
      children.push({ kind: 'code', owner: node.document, ancestry })
    }
    if (node.document.documentation.length > 0) {
      children.push({ kind: 'spec-documentation-group', owner: node.document, direction: 'outgoing' })
    }
    if (node.document.documentedBy.length > 0) {
      children.push({ kind: 'spec-documentation-group', owner: node.document, direction: 'incoming' })
    }
    return children
  }

  private documentationChildren(
    collection: ExplorerDocumentationCollection,
    prefix: string,
  ): ExplorerNode[] {
    const directories = new Set<string>()
    const documents: ExplorerNode[] = []
    const root = collection.root === '.' ? '' : `${collection.root.replace(/\/$/, '')}/`
    const prefixWithSlash = prefix ? `${prefix}/` : ''
    for (const document of this.snapshotValue.documentation) {
      if (document.collectionId !== collection.id) continue
      const relative = root && document.path.startsWith(root)
        ? document.path.slice(root.length)
        : document.path
      if (!relative.startsWith(prefixWithSlash)) continue
      const remaining = relative.slice(prefixWithSlash.length)
      const slash = remaining.indexOf('/')
      if (slash >= 0) {
        directories.add(prefixWithSlash + remaining.slice(0, slash))
      } else {
        documents.push({ kind: 'documentation', document })
      }
    }
    return [
      ...[...directories].sort().map(directory => ({
        kind: 'documentation-directory',
        collection,
        prefix: directory,
      }) as ExplorerNode),
      ...documents.sort((left, right) => {
        if (left.kind !== 'documentation' || right.kind !== 'documentation') return 0
        return left.document.title.localeCompare(right.document.title)
      }),
    ]
  }

  private specChildren(parent: string, ancestry: string[]): ExplorerNode[] {
    return (this.placements.get(parent) ?? [])
      .map(id => {
        if (ancestry.includes(id)) return { kind: 'cycle', id } as ExplorerNode
        const document = this.documentsById.get(id)
        return document ? ({ kind: 'spec', document, ancestry } as ExplorerNode) : undefined
      })
      .filter((node): node is ExplorerNode => node !== undefined)
  }

  private specItem(node: Extract<ExplorerNode, { kind: 'spec' }>): vscode.TreeItem {
    const hasChildren =
      node.document.blocks.length > 0 ||
      node.document.sources.length > 0 ||
      node.document.documentation.length > 0 ||
      node.document.documentedBy.length > 0 ||
      (this.placements.get(node.document.id) ?? []).length > 0
    const item = new vscode.TreeItem(
      specificationDisplayName(node.document.id, node.document.title),
      hasChildren
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    )
    item.description = [node.document.status, node.document.progress].filter(Boolean).join(' · ')
    const tooltip = new vscode.MarkdownString()
    tooltip.appendMarkdown(`**${node.document.title || specificationDisplayName(node.document.id)}**\n\n`)
    tooltip.appendCodeblock(node.document.id)
    tooltip.appendMarkdown(`\n${node.document.summary ?? 'No summary'}`)
    item.tooltip = tooltip
    item.iconPath = iconForType(node.document.entityType)
    item.command = {
      command: 'forgeSpec.openSpec',
      title: 'Open specification',
      arguments: [node.document],
    }
    item.resourceUri = vscode.Uri.parse(node.document.uri)
    item.contextValue = 'forgeSpec.spec'
    return item
  }

  private rebuildPlacements(): void {
    this.documentsById = new Map(
      this.snapshotValue.documents.map(document => [document.id, document]),
    )
    this.placements = new Map()
    for (const document of this.snapshotValue.documents) {
      for (const parent of [...document.refines, ...document.categorizedUnder]) {
        if (!this.resolvesParent(parent)) continue
        const children = this.placements.get(parent) ?? []
        if (!children.includes(document.id)) children.push(document.id)
        children.sort((left, right) => compareDocuments(
          this.documentsById.get(left)!,
          this.documentsById.get(right)!,
        ))
        this.placements.set(parent, children)
      }
    }

    const projectId = this.snapshotValue.projectId
    if (!projectId || !this.documentsById.has(projectId)) return
    const placed = new Set<string>()
    for (const children of this.placements.values()) {
      for (const child of children) placed.add(child)
    }
    for (const document of this.snapshotValue.documents) {
      if (document.id === projectId || placed.has(document.id)) continue
      const children = this.placements.get(projectId) ?? []
      children.push(document.id)
      children.sort((left, right) => compareDocuments(
        this.documentsById.get(left)!,
        this.documentsById.get(right)!,
      ))
      this.placements.set(projectId, children)
    }
  }

  private resolvesParent(reference: string): boolean {
    const [id, anchor] = reference.split('#', 2)
    const document = this.documentsById.get(id)
    if (!document) return false
    return !anchor || document.blocks.some(block => block.id === anchor)
  }
}

function compareDocuments(left: ExplorerDocument, right: ExplorerDocument): number {
  const order = ['PROJECT', 'TOPIC', 'REQ', 'INV', 'IFC', 'SCN', 'TASK', 'ADR', 'GLO']
  const type = order.indexOf(left.entityType) - order.indexOf(right.entityType)
  return type || left.id.localeCompare(right.id)
}

function iconForType(type: string): vscode.ThemeIcon {
  const icon = {
    PROJECT: 'root-folder',
    TOPIC: 'list-tree',
    REQ: 'law',
    INV: 'shield',
    IFC: 'plug',
    ADR: 'git-commit',
    GLO: 'book',
    SCN: 'play-circle',
    TASK: 'checklist',
  }[type]
  return new vscode.ThemeIcon(icon ?? 'symbol-misc')
}

function iconForBlock(kind: string): vscode.ThemeIcon {
  const icon = {
    requirement: 'symbol-field',
    invariant: 'shield',
    interface: 'plug',
    clause: 'symbol-key',
    assumption: 'question',
    'non-goal': 'circle-slash',
    example: 'beaker',
    'glossary-entry': 'book',
  }[kind]
  return new vscode.ThemeIcon(icon ?? 'symbol-field')
}

function humanizeBlockKind(kind: string): string {
  return kind
    .split('-')
    .map(part => part ? part[0].toUpperCase() + part.slice(1) : part)
    .join(' ')
}

function sourceDescription(source: ExplorerSource): string {
  if (source.targetKind === 'symbol') return source.symbol ?? 'symbol'
  if (source.targetKind === 'lines') {
    return `${source.path}:${source.startLine}${source.endLine !== source.startLine ? `-${source.endLine}` : ''}`
  }
  return source.path
}
