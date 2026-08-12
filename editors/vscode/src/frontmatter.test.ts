import { describe, expect, it } from 'vitest'
import { metadataOperations, parseSpecText } from './frontmatter'

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

  it('compiles viewer controls to closed Rust operations', () => {
    const metadata = parseSpecText(source).metadata
    expect(
      metadataOperations(metadata, {
        status: 'draft',
        progress: 'in-progress',
        summary: 'A newly focused summary.',
        owners: ['carlo', 'maya'],
      }),
    ).toEqual([
      { op: 'lifecycle.draft', spec: 'TASK:demo/example' },
      {
        op: 'summary.replace',
        spec: 'TASK:demo/example',
        value: 'A newly focused summary.',
      },
      { op: 'owner.add', spec: 'TASK:demo/example', owner: 'maya' },
      {
        op: 'task.progress.set',
        spec: 'TASK:demo/example',
        progress: 'in-progress',
      },
    ])
  })

  it('refuses direct supersession from a generic metadata control', () => {
    const metadata = parseSpecText(source).metadata
    expect(() =>
      metadataOperations(metadata, {
        status: 'superseded',
        progress: 'pending',
        summary: 'Existing summary over two source lines.',
        owners: ['carlo'],
      }),
    ).toThrow("Lifecycle state 'superseded' requires a dedicated command")
  })

  it('emits no operation when metadata is unchanged', () => {
    const metadata = parseSpecText(source).metadata
    expect(
      metadataOperations(metadata, {
        status: 'accepted',
        progress: 'pending',
        summary: 'Existing summary over two source lines.',
        owners: ['carlo'],
      }),
    ).toEqual([])
  })
})
