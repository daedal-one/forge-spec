import { describe, expect, it } from 'vitest'
import { renderSpecBody } from './markdown'

describe('forge-spec Markdown rendering', () => {
  it('turns bare spec references into readable navigable labels', () => {
    const rendered = renderSpecBody('See spec:REQ:auth/session-expiry#c-lifetime for details.')

    expect(rendered).toContain('href="spec:REQ:auth/session-expiry#c-lifetime"')
    expect(rendered).toContain('>Session expiry › C lifetime</a>')
    expect(rendered).toContain('class="forge-reference forge-reference-spec"')
  })

  it('turns bare source references into readable navigable labels', () => {
    const rendered = renderSpecBody(
      'Implemented by spec:src:src/session.rs#symbol=SessionStore/expire.',
    )

    expect(rendered).toContain('href="spec:src:src/session.rs#symbol=SessionStore/expire"')
    expect(rendered).toContain('>session.rs › SessionStore / expire</a>')
    expect(rendered).toContain('class="forge-reference forge-reference-source"')
  })

  it('turns bare documentation references into readable navigable labels', () => {
    const rendered = renderSpecBody(
      'See spec:doc:docs/architecture.md#heading=System%20design/Request%20flow.',
    )

    expect(rendered).toContain(
      'href="spec:doc:docs/architecture.md#heading=System%20design/Request%20flow"',
    )
    expect(rendered).toContain('>architecture.md › System design / Request flow</a>')
    expect(rendered).toContain('class="forge-reference forge-reference-documentation"')
  })

  it('preserves authored link labels and leaves inline code literal', () => {
    const rendered = renderSpecBody(
      '[the session policy](spec:REQ:auth/session-expiry) and `spec:REQ:auth/raw`',
    )

    expect(rendered).toContain('>the session policy</a>')
    expect(rendered).toContain('<code>spec:REQ:auth/raw</code>')
  })

  it('marks typed blocks as native viewer anchor targets', () => {
    const rendered = renderSpecBody(
      ':::{requirement id="viewer" level="MUST"}\nThe viewer MUST work.\n:::',
    )

    expect(rendered).toContain('<section class="spec-block" id="spec-anchor-viewer">')
  })

  it('marks clause anchors as focused native-viewer targets', () => {
    const rendered = renderSpecBody(
      ':::{requirement id="session" level="MUST"}\n- {#c-lifetime} Keep sessions bounded.\n:::',
    )

    expect(rendered).toContain(
      '<span class="clause-anchor" id="spec-anchor-c-lifetime">#c-lifetime</span>',
    )
    expect(rendered).toContain('Keep sessions bounded.')
  })
})
