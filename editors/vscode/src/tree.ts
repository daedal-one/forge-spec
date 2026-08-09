import * as path from 'node:path'
import * as vscode from 'vscode'
import type {
  ExplorerBlock,
  ExplorerDocument,
  ExplorerSnapshot,
  ExplorerSource,
} from './protocol'
import { ForgeSpecService } from './service'

export type ExplorerNode =
  | { kind: 'spec'; document: ExplorerDocument; ancestry: string[] }
  | { kind: 'block'; owner: ExplorerDocument; block: ExplorerBlock; ancestry: string[] }
  | { kind: 'code'; owner: ExplorerDocument; ancestry: string[] }
  | { kind: 'source'; source: ExplorerSource }
  | { kind: 'cycle'; id: string }

export class ForgeSpecTreeProvider implements vscode.TreeDataProvider<ExplorerNode> {
  private readonly changedEmitter = new vscode.EventEmitter<ExplorerNode | undefined>()
  readonly onDidChangeTreeData = this.changedEmitter.event
  private snapshotValue: ExplorerSnapshot = {
    generation: 0,
    stats: { parsed: 0, loadedFromCache: 0, removed: 0 },
    projectId: undefined,
    documents: [],
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
        item.description = node.block.kind
        item.tooltip = node.block.text || `${node.block.kind} ${node.block.id}`
        item.iconPath = new vscode.ThemeIcon('symbol-field')
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
      if (project) return [{ kind: 'spec', document: project, ancestry: [] }]

      const placed = new Set<string>()
      for (const children of this.placements.values()) {
        for (const child of children) placed.add(child)
      }
      return this.snapshotValue.documents
        .filter(document => !placed.has(document.id))
        .sort(compareDocuments)
        .map(document => ({ kind: 'spec', document, ancestry: [] }))
    }

    if (node.kind === 'source' || node.kind === 'cycle') return []
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
    return children
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
      (this.placements.get(node.document.id) ?? []).length > 0
    const item = new vscode.TreeItem(
      node.document.id,
      hasChildren
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    )
    item.description = [node.document.status, node.document.progress].filter(Boolean).join(' · ')
    item.tooltip = new vscode.MarkdownString(
      `**${node.document.id}**\n\n${node.document.summary ?? 'No summary'}`,
    )
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

function sourceDescription(source: ExplorerSource): string {
  if (source.targetKind === 'symbol') return source.symbol ?? 'symbol'
  if (source.targetKind === 'lines') {
    return `${source.path}:${source.startLine}${source.endLine !== source.startLine ? `-${source.endLine}` : ''}`
  }
  return source.path
}
