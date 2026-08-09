import { describe, expect, it } from 'vitest'
import { computeMetadataEdits, parseSpecText } from './frontmatter'

const source = `---
id: TASK:demo/example
type: task
status: accepted
summary: >
  Existing summary over
  two source lines.
owners: [carlo]
progress: pending
related: [REQ:demo/root]
---

# Example

Body.
`

describe('spec frontmatter', () => {
  it('parses YAML metadata separately from the Markdown body', () => {
    const parsed = parseSpecText(source)
    expect(parsed.metadata.id).toBe('TASK:demo/example')
    expect(parsed.metadata.summary).toBe('Existing summary over two source lines.\n')
    expect(parsed.body).toContain('# Example')
  })

  it('updates supported fields without rewriting unrelated frontmatter', () => {
    const updated = apply(
      source,
      computeMetadataEdits(source, {
        status: 'draft',
        progress: 'in-progress',
        summary: 'A newly focused summary.',
        owners: ['carlo', 'maya'],
      }),
    )
    expect(updated).toContain('status: draft')
    expect(updated).toContain('progress: in-progress')
    expect(updated).toContain('summary: "A newly focused summary."')
    expect(updated).toContain('owners: ["carlo", "maya"]')
    expect(updated).toContain('related: [REQ:demo/root]')
    expect(updated).toContain('# Example')
  })

  it('refuses fields outside the supported metadata surface', () => {
    expect(() => computeMetadataEdits(source, { id: 'REQ:other/id' })).toThrow(
      "Metadata field 'id' is not editable",
    )
  })

  it('inserts multiple missing fields as one non-overlapping edit', () => {
    const minimal = `---\nid: REQ:demo/minimal\ntype: requirement\n---\n\n# Minimal\n`
    const edits = computeMetadataEdits(minimal, {
      status: 'draft',
      summary: 'New summary',
      owners: ['carlo'],
      level: 'MUST',
    })
    const updated = apply(minimal, edits)

    expect(edits).toHaveLength(1)
    expect(updated).toContain('status: draft\nsummary: "New summary"\nowners: ["carlo"]\nlevel: MUST\n---')
  })
})

function apply(
  text: string,
  edits: Array<{ startLine: number; endLine: number; newText: string }>,
): string {
  const lines = text.split('\n')
  for (const edit of edits) {
    const replacement = edit.newText.endsWith('\n')
      ? edit.newText.slice(0, -1).split('\n')
      : edit.newText.split('\n')
    lines.splice(edit.startLine, edit.endLine - edit.startLine, ...replacement)
  }
  return lines.join('\n')
}
