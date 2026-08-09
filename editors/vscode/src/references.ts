export interface ReferencePresentation {
  href: string
  kind: 'spec' | 'source'
  label: string
  title: string
}

const SPEC_REFERENCE = /^spec:(?:PROJECT:[A-Za-z0-9][\w-]*|(?:REQ|INV|IFC|ADR|GLO|TOPIC|SCN|TASK):[A-Za-z0-9][\w-]*\/[A-Za-z0-9][\w-]*)(?:#[A-Za-z0-9][\w-]*)?/
const SOURCE_REFERENCE = /^spec:src:[A-Za-z0-9_./%-]+(?::\d+(?:-\d+)?|#symbol=[A-Za-z0-9_.~%/-]+)?/

export function matchForgeReference(source: string): string | undefined {
  const sourceReference = source.match(SOURCE_REFERENCE)?.[0]?.replace(/[.,;!?]+$/, '')
  return sourceReference || source.match(SPEC_REFERENCE)?.[0]
}

export function referencePresentation(href: string): ReferencePresentation | undefined {
  if (href.startsWith('spec:src:')) return sourcePresentation(href)
  if (!SPEC_REFERENCE.test(href) || matchForgeReference(href) !== href) return undefined

  const target = href.slice('spec:'.length)
  return {
    href,
    kind: 'spec',
    label: specificationDisplayName(target),
    title: target,
  }
}

export function specificationDisplayName(id: string, title?: string): string {
  const explicit = title?.trim()
  if (explicit) return explicit

  const [documentId, anchor] = id.split('#', 2)
  const identity = documentId.slice(documentId.indexOf(':') + 1)
  const slug = identity.split('/').at(-1) ?? identity
  const name = humanizeIdentifier(slug)
  return anchor ? `${name} › ${humanizeIdentifier(anchor)}` : name
}

export function specificationAnchor(href: string): string | undefined {
  const presentation = referencePresentation(href)
  if (presentation?.kind !== 'spec') return undefined
  const anchor = href.split('#', 2)[1]
  return anchor ? decodeReferencePart(anchor) : undefined
}

export function specificationReference(id: string, anchor?: string): string {
  const href = id.startsWith('spec:') ? id : `spec:${id}`
  const documentHref = href.split('#', 1)[0]
  return anchor ? `${documentHref}#${anchor}` : documentHref
}

function sourcePresentation(href: string): ReferencePresentation | undefined {
  const target = href.slice('spec:src:'.length)
  if (!target || target.includes(' ')) return undefined
  const symbolMarker = '#symbol='
  const symbolIndex = target.indexOf(symbolMarker)
  if (symbolIndex >= 0) {
    const sourcePath = target.slice(0, symbolIndex)
    const encodedSegments = target.slice(symbolIndex + symbolMarker.length).split('/')
    if (!sourcePath || sourcePath.includes('#') || encodedSegments.some(segment => !segment)) {
      return undefined
    }
    if (/:(\d+)(?:-\d+)?$/.test(sourcePath)) return undefined
    const symbol = encodedSegments.map(decodeReferencePart).join(' / ')
    return {
      href,
      kind: 'source',
      label: `${basename(sourcePath)} › ${symbol}`,
      title: `${sourcePath} — ${symbol}`,
    }
  }

  if (target.includes('#')) return undefined
  const lines = target.match(/:(\d+)(?:-(\d+))?$/)
  const sourcePath = lines ? target.slice(0, lines.index) : target
  if (lines) {
    const start = lines[1]
    const end = lines[2]
    return {
      href,
      kind: 'source',
      label: `${basename(sourcePath)} · ${end ? `lines ${start}–${end}` : `line ${start}`}`,
      title: target,
    }
  }

  return {
    href,
    kind: 'source',
    label: basename(sourcePath),
    title: sourcePath,
  }
}

function humanizeIdentifier(value: string): string {
  const readable = decodeReferencePart(value)
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/[-_]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  return readable ? readable[0].toUpperCase() + readable.slice(1) : value
}

function decodeReferencePart(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

function basename(sourcePath: string): string {
  return sourcePath.split('/').filter(Boolean).at(-1) ?? sourcePath
}
