import { describe, expect, it } from 'vitest'
import { incomingRefinements } from './relationships'

describe('incoming refinement presentation', () => {
  it('returns only specifications refining the exact requirement anchor', () => {
    const refinements = incomingRefinements([
      {
        id: 'REQ:explorer/root-to-code-tree',
        title: 'Root-to-code tree',
        status: 'accepted',
        refines: ['REQ:explorer/workbench#tree', 'REQ:explorer/workbench#navigation'],
      },
      {
        id: 'REQ:explorer/spec-viewer',
        title: 'Specification viewer',
        status: 'accepted',
        refines: ['REQ:explorer/workbench#viewer'],
      },
      {
        id: 'TASK:explorer/follow-up',
        title: 'Follow-up work',
        status: 'accepted',
        progress: 'in-progress',
        refines: ['REQ:explorer/workbench#tree'],
      },
    ], 'REQ:explorer/workbench#tree')

    expect(refinements).toEqual([
      {
        reference: 'spec:TASK:explorer/follow-up',
        label: 'Follow-up work',
        title: 'TASK:explorer/follow-up',
        status: 'accepted',
        progress: 'in-progress',
      },
      {
        reference: 'spec:REQ:explorer/root-to-code-tree',
        label: 'Root-to-code tree',
        title: 'REQ:explorer/root-to-code-tree',
        status: 'accepted',
        progress: undefined,
      },
    ])
  })
})
