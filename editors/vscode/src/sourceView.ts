import * as vscode from 'vscode'

/** Read-only virtual source documents for inspecting the authoritative Markdown. */
export class ForgeSpecSourceView implements vscode.TextDocumentContentProvider {
  static readonly scheme = 'forge-spec-source'

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const encoded = uri.path.slice(1).replace(/\.md$/, '')
    const source = vscode.Uri.parse(Buffer.from(encoded, 'base64url').toString('utf8'))
    return Buffer.from(await vscode.workspace.fs.readFile(source)).toString('utf8')
  }

  async open(source: vscode.Uri): Promise<void> {
    const encoded = Buffer.from(source.toString()).toString('base64url')
    const virtual = vscode.Uri.from({
      scheme: ForgeSpecSourceView.scheme,
      path: `/${encoded}.md`,
      fragment: source.fsPath,
    })
    const document = await vscode.workspace.openTextDocument(virtual)
    await vscode.window.showTextDocument(document, { preview: true })
  }
}
