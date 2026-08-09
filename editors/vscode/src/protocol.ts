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
