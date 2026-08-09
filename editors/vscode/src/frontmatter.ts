import { load } from 'js-yaml'

export interface ParsedSpecText {
  metadata: Record<string, unknown>
  body: string
  bodyStartLine: number
}

export interface MetadataLineEdit {
  startLine: number
  endLine: number
  newText: string
}

export type MetadataUpdates = Record<string, string | string[]>

const editableFields = new Set(['status', 'progress', 'level', 'summary', 'owners'])

export function parseSpecText(text: string): ParsedSpecText {
  const lines = text.split(/\r?\n/)
  if (lines[0] !== '---') throw new Error('Specification is missing YAML frontmatter')
  const closing = lines.indexOf('---', 1)
  if (closing < 0) throw new Error('Specification frontmatter is not closed')
  const metadata = load(lines.slice(1, closing).join('\n'))
  if (!metadata || typeof metadata !== 'object' || Array.isArray(metadata)) {
    throw new Error('Specification frontmatter must be a YAML mapping')
  }
  return {
    metadata: metadata as Record<string, unknown>,
    body: lines.slice(closing + 1).join('\n'),
    bodyStartLine: closing + 2,
  }
}

export function computeMetadataEdits(text: string, updates: MetadataUpdates): MetadataLineEdit[] {
  const lines = text.split(/\r?\n/)
  if (lines[0] !== '---') throw new Error('Specification is missing YAML frontmatter')
  const closing = lines.indexOf('---', 1)
  if (closing < 0) throw new Error('Specification frontmatter is not closed')

  const edits: MetadataLineEdit[] = []
  const insertions: string[] = []
  for (const [key, value] of Object.entries(updates)) {
    if (!editableFields.has(key)) throw new Error(`Metadata field '${key}' is not editable`)
    const range = fieldRange(lines, key, closing)
    const rendered = renderField(key, value)
    if (range) {
      edits.push({ startLine: range.start, endLine: range.end, newText: `${rendered}\n` })
    } else {
      insertions.push(rendered)
    }
  }
  if (insertions.length > 0) {
    edits.push({ startLine: closing, endLine: closing, newText: `${insertions.join('\n')}\n` })
  }
  return edits.sort((left, right) => right.startLine - left.startLine)
}

function fieldRange(
  lines: string[],
  key: string,
  closing: number,
): { start: number; end: number } | undefined {
  const matcher = new RegExp(`^${escapeRegExp(key)}\\s*:`)
  const start = lines.findIndex((line, index) => index > 0 && index < closing && matcher.test(line))
  if (start < 0) return undefined
  let end = start + 1
  while (end < closing && !/^[A-Za-z_][A-Za-z0-9_-]*\s*:/.test(lines[end])) end += 1
  return { start, end }
}

function renderField(key: string, value: string | string[]): string {
  if (Array.isArray(value)) {
    return `${key}: [${value.map(item => JSON.stringify(item)).join(', ')}]`
  }
  const normalized = value.trim()
  if (key === 'summary' && normalized.includes('\n')) {
    return `${key}: >\n${normalized
      .split('\n')
      .map(line => `  ${line}`)
      .join('\n')}`
  }
  if (key === 'status' || key === 'progress' || key === 'level') {
    return `${key}: ${normalized}`
  }
  return `${key}: ${JSON.stringify(normalized)}`
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
