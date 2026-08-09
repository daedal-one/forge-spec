import { describe, expect, it } from 'vitest'
import {
  matchForgeReference,
  referencePresentation,
  specificationAnchor,
  specificationDisplayName,
  specificationReference,
} from './references'

describe('reference presentation', () => {
  it('uses document titles and concise identifier fallbacks', () => {
    expect(specificationDisplayName('REQ:explorer/root-to-code-tree')).toBe('Root to code tree')
    expect(specificationDisplayName('PROJECT:forge-spec', 'forge-spec')).toBe('forge-spec')
    expect(specificationDisplayName('REQ:auth/session-expiry#c-lifetime')).toBe(
      'Session expiry › C lifetime',
    )
  })

  it('describes source locations without exposing the reference protocol', () => {
    expect(referencePresentation('spec:src:src/session.rs:42-78')?.label).toBe(
      'session.rs · lines 42–78',
    )
    expect(
      referencePresentation('spec:src:src/session.rs#symbol=SessionStore/expire%2Fnow')?.label,
    ).toBe('session.rs › SessionStore / expire/now')
    expect(referencePresentation('spec:src:packages/@scope/foo+bar.ts')?.label).toBe(
      'foo+bar.ts',
    )
  })

  it('matches one complete reference without trailing prose punctuation', () => {
    expect(matchForgeReference('spec:REQ:auth/session-expiry, then continue')).toBe(
      'spec:REQ:auth/session-expiry',
    )
  })

  it('keeps specification anchors for native-viewer navigation only', () => {
    expect(specificationAnchor('spec:REQ:explorer/workbench#viewer')).toBe('viewer')
    expect(specificationAnchor('spec:REQ:explorer/workbench')).toBeUndefined()
    expect(specificationAnchor('spec:src:src/session.rs#symbol=Session/expire')).toBeUndefined()
  })

  it('builds exact native-viewer references for tree block rows', () => {
    expect(specificationReference('REQ:explorer/root-to-code-tree', 'roots')).toBe(
      'spec:REQ:explorer/root-to-code-tree#roots',
    )
    expect(specificationReference('spec:REQ:explorer/root-to-code-tree#old', 'dag')).toBe(
      'spec:REQ:explorer/root-to-code-tree#dag',
    )
  })
})
