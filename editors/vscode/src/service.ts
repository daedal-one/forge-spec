import * as path from 'node:path'
import * as vscode from 'vscode'
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node'
import type { ExplorerSnapshot, ResolvedLocation } from './protocol'

export class ForgeSpecService implements vscode.Disposable {
  private client: LanguageClient | undefined
  private reconcileTimer: NodeJS.Timeout | undefined
  private readonly indexChangedEmitter = new vscode.EventEmitter<number>()
  readonly onIndexChanged = this.indexChangedEmitter.event

  constructor(
    private readonly context: vscode.ExtensionContext,
    readonly workspaceFolder: vscode.WorkspaceFolder,
    readonly specsUri: vscode.Uri,
  ) {}

  async start(): Promise<void> {
    const configuration = vscode.workspace.getConfiguration('forgeSpec', this.workspaceFolder.uri)
    const configuredPath = configuration.get<string>('serverPath', 'spec')
    const command = process.env.FORGE_SPEC_SERVER_PATH || configuredPath
    const cacheEnabled = configuration.get<boolean>('cache.enabled', true)
    const cachePath = cacheEnabled ? await this.cachePath() : undefined
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(this.specsUri, '**/*'),
    )
    const gitHeadWatcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(this.workspaceFolder, '.git/HEAD'),
    )
    const scheduleForGit = () => this.scheduleReconcile()
    this.context.subscriptions.push(
      watcher,
      gitHeadWatcher,
      gitHeadWatcher.onDidCreate(scheduleForGit),
      gitHeadWatcher.onDidChange(scheduleForGit),
      gitHeadWatcher.onDidDelete(scheduleForGit),
    )

    const serverOptions: ServerOptions = {
      command,
      args: ['--specs-dir', this.specsUri.fsPath, 'lsp'],
      transport: TransportKind.stdio,
      options: {
        cwd: this.workspaceFolder.uri.fsPath,
        env: {
          ...process.env,
          ...(cachePath ? { FORGE_SPEC_CACHE_PATH: cachePath } : {}),
        },
      },
    }
    const clientOptions: LanguageClientOptions = {
      documentSelector: [{ scheme: 'file', pattern: '**/*.spec.md' }],
      workspaceFolder: this.workspaceFolder,
      synchronize: { fileEvents: watcher },
      outputChannelName: 'Forge Spec',
    }
    this.client = new LanguageClient(
      'forgeSpec',
      'Forge Spec Language Server',
      serverOptions,
      clientOptions,
    )
    this.client.onNotification('forgeSpec/indexChanged', (event: { generation?: number }) => {
      this.indexChangedEmitter.fire(event.generation ?? 0)
    })
    await this.client.start()
  }

  async snapshot(): Promise<ExplorerSnapshot> {
    const client = this.requireClient()
    return client.sendRequest<ExplorerSnapshot>('forgeSpec/explorerSnapshot', {})
  }

  async reconcile(): Promise<ExplorerSnapshot> {
    const snapshot = await this.requireClient().sendRequest<ExplorerSnapshot>(
      'forgeSpec/reconcile',
      {},
    )
    this.indexChangedEmitter.fire(snapshot.generation)
    return snapshot
  }

  scheduleReconcile(delay = 150): void {
    if (this.reconcileTimer) clearTimeout(this.reconcileTimer)
    this.reconcileTimer = setTimeout(() => {
      this.reconcileTimer = undefined
      void this.reconcile().catch(error => {
        const message = error instanceof Error ? error.message : String(error)
        void vscode.window.showWarningMessage(`Forge Spec could not refresh its index: ${message}`)
      })
    }, delay)
  }

  async resolveReference(reference: string): Promise<ResolvedLocation> {
    const client = this.requireClient()
    return client.sendRequest<ResolvedLocation>('forgeSpec/resolveReference', { reference })
  }

  async stop(): Promise<void> {
    if (this.reconcileTimer) {
      clearTimeout(this.reconcileTimer)
      this.reconcileTimer = undefined
    }
    if (this.client) {
      if (this.client.needsStop()) await this.client.stop()
      this.client = undefined
    }
  }

  dispose(): void {
    if (this.reconcileTimer) clearTimeout(this.reconcileTimer)
    this.indexChangedEmitter.dispose()
    void this.stop()
  }

  private requireClient(): LanguageClient {
    const client = this.client
    if (!client) {
      throw new Error('Forge Spec language server is not running')
    }
    return client
  }

  private async cachePath(): Promise<string | undefined> {
    const storage = this.context.storageUri
    if (!storage || storage.scheme !== 'file') {
      return undefined
    }
    await vscode.workspace.fs.createDirectory(storage)
    const workspaceKey = Buffer.from(this.workspaceFolder.uri.toString()).toString('base64url')
    return path.join(storage.fsPath, `${workspaceKey}.sqlite3`)
  }
}

export async function findForgeSpecWorkspace(): Promise<{
  folder: vscode.WorkspaceFolder
  specsUri: vscode.Uri
} | undefined> {
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    const specsUri = vscode.Uri.joinPath(folder.uri, '.specs')
    try {
      const config = vscode.Uri.joinPath(specsUri, '_config.toml')
      const stat = await vscode.workspace.fs.stat(config)
      if (stat.type & vscode.FileType.File) {
        return { folder, specsUri }
      }
    } catch {
      // Continue to the next workspace folder.
    }
  }
  return undefined
}
