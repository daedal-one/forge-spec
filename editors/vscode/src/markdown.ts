import MarkdownIt from 'markdown-it'
import { matchForgeReference, referencePresentation } from './references'

interface MutableMarkdownToken {
  type: string
  content: string
  attrs: Array<[string, string]> | null
  markup: string
}

type MarkdownTokenConstructor<Token extends MutableMarkdownToken> = new (
  type: string,
  tag: string,
  nesting: -1 | 0 | 1,
) => Token

const markdown = new MarkdownIt({ html: false, linkify: true, typographer: true })

markdown.core.ruler.after('inline', 'forge_references', state => {
  for (const token of state.tokens) {
    if (token.type !== 'inline' || !token.children) continue
    const children = []
    let linkDepth = 0
    for (const child of token.children) {
      if (child.type === 'link_open') linkDepth += 1
      if (child.type === 'text' && linkDepth === 0) {
        children.push(...referenceTokens(child.content, state.Token))
      } else {
        children.push(child)
      }
      if (child.type === 'link_close') linkDepth -= 1
    }
    token.children = children
  }
})

markdown.core.ruler.after('forge_references', 'forge_clause_anchors', state => {
  for (const token of state.tokens) {
    if (token.type !== 'inline' || !token.children) continue
    token.children = token.children.flatMap(child =>
      child.type === 'text' ? clauseAnchorTokens(child.content, state.Token) : [child],
    )
  }
})

const defaultLinkOpen = markdown.renderer.rules.link_open
markdown.renderer.rules.link_open = (tokens, index, options, environment, renderer) => {
  const href = tokens[index].attrGet('href')
  const presentation = href ? referencePresentation(href) : undefined
  if (presentation) {
    tokens[index].attrJoin('class', `forge-reference forge-reference-${presentation.kind}`)
    tokens[index].attrSet('title', presentation.title)
    tokens[index].attrSet('data-forge-reference', presentation.href)
  }
  return defaultLinkOpen
    ? defaultLinkOpen(tokens, index, options, environment, renderer)
    : renderer.renderToken(tokens, index, options)
}

export function renderSpecBody(body: string): string {
  const lines = body.split(/\r?\n/)
  const result: string[] = []
  let plain: string[] = []
  const flush = () => {
    if (plain.length > 0) result.push(markdown.render(plain.join('\n')))
    plain = []
  }
  for (let index = 0; index < lines.length; index += 1) {
    const match = lines[index].match(/^:::\{([a-z-]+)(.*?)\}\s*$/)
    if (!match) {
      plain.push(lines[index])
      continue
    }
    flush()
    const inner: string[] = []
    index += 1
    while (index < lines.length && lines[index].trim() !== ':::') {
      inner.push(lines[index])
      index += 1
    }
    const id = match[2].match(/\bid="([^"]+)"/)?.[1]
    const level = match[2].match(/\blevel="([^"]+)"/)?.[1]
    result.push(`<section class="spec-block"${id ? ` id="spec-anchor-${escapeHtml(id)}"` : ''}><div class="spec-block-title"><span>${escapeHtml(match[1])}</span>${id ? `<code>#${escapeHtml(id)}</code>` : ''}${level ? `<strong>${escapeHtml(level)}</strong>` : ''}</div>${markdown.render(inner.join('\n'))}</section>`)
  }
  flush()
  return result.join('\n')
}

function referenceTokens<Token extends MutableMarkdownToken>(
  content: string,
  Token: MarkdownTokenConstructor<Token>,
): Token[] {
  const tokens: Token[] = []
  let consumed = 0
  let search = 0
  while (search < content.length) {
    const start = content.indexOf('spec:', search)
    if (start < 0) break
    if (start > 0 && /[\p{L}\p{N}_]/u.test(content[start - 1])) {
      search = start + 'spec:'.length
      continue
    }
    const href = matchForgeReference(content.slice(start))
    const presentation = href ? referencePresentation(href) : undefined
    if (!href || !presentation) {
      search = start + 'spec:'.length
      continue
    }

    if (start > consumed) {
      const text = new Token('text', '', 0)
      text.content = content.slice(consumed, start)
      tokens.push(text)
    }
    const opening = new Token('link_open', 'a', 1)
    opening.attrs = [['href', href]]
    opening.markup = 'forge-reference'
    tokens.push(opening)

    const label = new Token('text', '', 0)
    label.content = presentation.label
    tokens.push(label)

    const closing = new Token('link_close', 'a', -1)
    closing.markup = 'forge-reference'
    tokens.push(closing)
    consumed = start + href.length
    search = consumed
  }

  if (consumed < content.length) {
    const text = new Token('text', '', 0)
    text.content = content.slice(consumed)
    tokens.push(text)
  }
  return tokens
}

function clauseAnchorTokens<Token extends MutableMarkdownToken>(
  content: string,
  Token: MarkdownTokenConstructor<Token>,
): Token[] {
  const tokens: Token[] = []
  const pattern = /\{#([\p{L}\p{N}_-]+)\}/gu
  let consumed = 0
  for (const match of content.matchAll(pattern)) {
    const start = match.index
    if (start > consumed) {
      const text = new Token('text', '', 0)
      text.content = content.slice(consumed, start)
      tokens.push(text)
    }
    const anchor = new Token('html_inline', '', 0)
    anchor.content = `<span class="clause-anchor" id="spec-anchor-${escapeHtml(match[1])}">#${escapeHtml(match[1])}</span>`
    tokens.push(anchor)
    consumed = start + match[0].length
  }
  if (consumed < content.length) {
    const text = new Token('text', '', 0)
    text.content = content.slice(consumed)
    tokens.push(text)
  }
  return tokens
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}
