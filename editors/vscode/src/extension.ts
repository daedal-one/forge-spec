import * as vscode from 'vscode'
import type { ExplorerDocument } from './protocol'
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
  const viewer = new SpecViewerProvider(service)
  const refreshViews = async () => {
    await tree.refresh()
    viewer.setSnapshot(tree.snapshot)
  }
  context.subscriptions.push(
    vscode.window.registerTreeDataProvider('forgeSpec.explorer', tree),
    vscode.window.registerCustomEditorProvider(
      SpecViewerProvider.viewType,
      viewer,
      { supportsMultipleEditorsPerDocument: false },
    ),
    vscode.commands.registerCommand('forgeSpec.refresh', async () => {
      await service.reconcile()
      await refreshViews()
    }),
    vscode.window.onDidChangeWindowState(event => {
      if (event.focused) service.scheduleReconcile(50)
    }),
    vscode.commands.registerCommand(
      'forgeSpec.openSpec',
      async (document: ExplorerDocument | vscode.Uri) => {
        const uri = document instanceof vscode.Uri ? document : vscode.Uri.parse(document.uri)
        await viewer.openDocument(uri)
      },
    ),
    vscode.commands.registerCommand('forgeSpec.openText', async (uri?: vscode.Uri) => {
      const target = uri ?? vscode.window.activeTextEditor?.document.uri
      if (target) await vscode.commands.executeCommand('vscode.openWith', target, 'default')
    }),
    vscode.commands.registerCommand('forgeSpec.openReference', async (reference: string) => {
      try {
        await viewer.openReference(reference)
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error)
        void vscode.window.showWarningMessage(`Forge Spec: ${message}`)
      }
    }),
    service.onIndexChanged(() => void refreshViews()),
  )
  await refreshViews()
}

export async function deactivate(): Promise<void> {
  await activeService?.stop()
  activeService = undefined
}
