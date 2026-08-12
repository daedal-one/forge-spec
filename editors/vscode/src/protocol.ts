export interface IndexStats {
  parsed: number
  loadedFromCache: number
  removed: number
}

export interface ExplorerSnapshot {
  generation: number
  stats: IndexStats
  projectId?: string
  documents: ExplorerDocument[]
}

export interface ExplorerDocument {
  id: string
  title: string
  entityType: string
  status: string
  progress?: string
  level?: string
  summary?: string
  owners: string[]
  uri: string
  refines: string[]
  categorizedUnder: string[]
  blocks: ExplorerBlock[]
  sources: ExplorerSource[]
}

export interface ExplorerBlock {
  id: string
  kind: string
  line: number
  text: string
}

export interface ExplorerSource {
  reference: string
  label: string
  path: string
  line: number
  targetKind: 'file' | 'lines' | 'symbol'
  startLine?: number
  endLine?: number
  symbol?: string
}

export interface ProtocolPosition {
  line: number
  character: number
}

export interface ProtocolLocation {
  uri: string
  range: {
    start: ProtocolPosition
    end: ProtocolPosition
  }
}

export type ResolvedLocation = ProtocolLocation | null

export interface ChangeOperation {
  op: string
  spec: string
  [key: string]: unknown
}

export interface ApplyChangesResult {
  schema: 'forge-spec-workspace-edit/v1'
  plan: {
    schema: string
    dry_run: boolean
    operations: Array<{ index: number; operation: string; spec: string }>
    files: string[]
    warnings: string[]
  }
  edit: {
    documentChanges: WorkspaceDocumentChange[]
  }
}

export type WorkspaceDocumentChange =
  | {
      kind: 'rename'
      oldUri: string
      newUri: string
    }
  | {
      textDocument: { uri: string; version: number | null }
      edits: Array<{ range: ProtocolLocation['range']; newText: string }>
    }
