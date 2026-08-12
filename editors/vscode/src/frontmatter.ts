import { load } from 'js-yaml'
import type { ChangeOperation } from './protocol'

export interface ParsedSpecText {
  metadata: Record<string, unknown>
  body: string
  bodyStartLine: number
}

export interface MetadataUpdates {
  status: string
  summary: string
  owners: string[]
  progress?: string
  level?: string
}

export function parseSpecText(text: string): ParsedSpecText {
  const lines = text.split(/\r?\n/)
  const first = lines[0]?.replace(/^\uFEFF/, '')
  if (first !== '---') throw new Error('Specification is missing YAML frontmatter')
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

/** Compile viewer controls into the same closed Rust operations used by the CLI. */
export function metadataOperations(
  metadata: Record<string, unknown>,
  updates: MetadataUpdates,
): ChangeOperation[] {
  const spec = stringField(metadata, 'id')
  const entityType = stringField(metadata, 'type')
  const operations: ChangeOperation[] = []

  const currentStatus = stringField(metadata, 'status', 'draft')
  if (updates.status !== currentStatus) {
    const op = {
      draft: 'lifecycle.draft',
      accepted: 'lifecycle.accept',
      deprecated: 'lifecycle.deprecate',
    }[updates.status]
    if (!op) throw new Error(`Lifecycle state '${updates.status}' requires a dedicated command`)
    operations.push({ op, spec })
  }

  const currentSummary = stringField(metadata, 'summary', '').trim()
  if (updates.summary.trim() !== currentSummary) {
    operations.push({ op: 'summary.replace', spec, value: updates.summary.trim() })
  }

  const currentOwners = stringList(metadata.owners)
  for (const owner of updates.owners.filter(owner => !currentOwners.includes(owner))) {
    operations.push({ op: 'owner.add', spec, owner })
  }
  for (const owner of currentOwners.filter(owner => !updates.owners.includes(owner))) {
    operations.push({ op: 'owner.remove', spec, owner })
  }

  if (entityType === 'task' && updates.progress) {
    const current = stringField(metadata, 'progress', 'pending')
    if (updates.progress !== current) {
      operations.push({ op: 'task.progress.set', spec, progress: updates.progress })
    }
  }
  if (entityType === 'requirement' && updates.level) {
    const current = stringField(metadata, 'level', 'MUST')
    if (updates.level !== current) {
      operations.push({ op: 'requirement.level.set', spec, level: updates.level })
    }
  }
  return operations
}

function stringField(
  metadata: Record<string, unknown>,
  key: string,
  fallback?: string,
): string {
  const value = metadata[key]
  if (typeof value === 'string') return value
  if (fallback !== undefined) return fallback
  throw new Error(`Specification metadata is missing '${key}'`)
}

function stringList(value: unknown): string[] {
  if (!Array.isArray(value) || value.some(item => typeof item !== 'string')) {
    throw new Error('Specification owners must be a string list')
  }
  return value as string[]
}
