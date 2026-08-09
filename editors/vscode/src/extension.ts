import * as vscode from 'vscode'
import type { ExplorerDocument, ProtocolLocation } from './protocol'
import { findForgeSpecWorkspace, ForgeSpecService } from './service'
import { ForgeSpecTreeProvider } from './tree'
import { SpecViewerProvider } from './viewer'

let activeService: ForgeSpecService | undefined

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const workspace = await findForgeSpecWorkspace()
  if (!workspace) {
    void vscode.window.showInformationMessage(
      'Forge Spec: open a workspace containing .specs/_config.toml to use the explorer.',
    )
    return
  }

  const service = new ForgeSpecService(context, workspace.folder, workspace.specsUri)
  activeService = service
  context.subscriptions.push(service)
  try {
    await service.start()
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    void vscode.window.showErrorMessage(
      `Forge Spec could not start the language server: ${message}. Check forgeSpec.serverPath.`,
    )
    return
  }

  const tree = new ForgeSpecTreeProvider(service)
  context.subscriptions.push(
    vscode.window.registerTreeDataProvider('forgeSpec.explorer', tree),
    vscode.window.registerCustomEditorProvider(
      SpecViewerProvider.viewType,
      new SpecViewerProvider(service),
      { supportsMultipleEditorsPerDocument: false },
    ),
    vscode.commands.registerCommand('forgeSpec.refresh', async () => {
      await service.reconcile()
      await tree.refresh()
    }),
    vscode.window.onDidChangeWindowState(event => {
      if (event.focused) service.scheduleReconcile(50)
    }),
    vscode.commands.registerCommand(
      'forgeSpec.openSpec',
      async (document: ExplorerDocument | vscode.Uri) => {
        const uri = document instanceof vscode.Uri ? document : vscode.Uri.parse(document.uri)
        await vscode.commands.executeCommand('vscode.openWith', uri, SpecViewerProvider.viewType)
      },
    ),
    vscode.commands.registerCommand('forgeSpec.openText', async (uri?: vscode.Uri) => {
      const target = uri ?? vscode.window.activeTextEditor?.document.uri
      if (target) await vscode.commands.executeCommand('vscode.openWith', target, 'default')
    }),
    vscode.commands.registerCommand('forgeSpec.openReference', async (reference: string) => {
      const location = await service.resolveReference(reference)
      if (!location) {
        void vscode.window.showWarningMessage(`Forge Spec reference does not resolve: ${reference}`)
        return
      }
      await revealLocation(location)
    }),
    service.onIndexChanged(() => void tree.refresh()),
  )
  await tree.refresh()
}

export async function deactivate(): Promise<void> {
  await activeService?.stop()
  activeService = undefined
}

async function revealLocation(location: ProtocolLocation): Promise<void> {
  const uri = vscode.Uri.parse(location.uri)
  const range = new vscode.Range(
    location.range.start.line,
    location.range.start.character,
    location.range.end.line,
    location.range.end.character,
  )
  const document = await vscode.workspace.openTextDocument(uri)
  const editor = await vscode.window.showTextDocument(document, { selection: range })
  editor.revealRange(range, vscode.TextEditorRevealType.InCenterIfOutsideViewport)
}
