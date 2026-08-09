import type { ExplorerDocument } from './protocol'
import { specificationDisplayName, specificationReference } from './references'

type RefinementDocument = Pick<
  ExplorerDocument,
  'id' | 'title' | 'status' | 'progress' | 'refines'
>

export interface RefinementLink {
  reference: string
  label: string
  title: string
  status: string
  progress?: string
}

export function incomingRefinements(
  documents: readonly RefinementDocument[],
  target: string,
): RefinementLink[] {
  return documents
    .filter(document => document.refines.includes(target))
    .map(document => ({
      reference: specificationReference(document.id),
      label: specificationDisplayName(document.id, document.title),
      title: document.id,
      status: document.status,
      progress: document.progress,
    }))
    .sort((left, right) => left.label.localeCompare(right.label) || left.title.localeCompare(right.title))
}
