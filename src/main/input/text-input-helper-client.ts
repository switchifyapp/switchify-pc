import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { app } from 'electron';

export type TextInputBackend = {
  typeText(text: string): Promise<void>;
  dispose(): void;
};

export type TextInputBackendOptions = {
  helperPath?: string;
  timeoutMs?: number;
  spawnProcess?: (helperPath: string) => ChildProcessWithoutNullStreams;
  exists?: (helperPath: string) => boolean;
  createRequestId?: () => string;
  shutdownKillDelayMs?: number;
};

type TextInputHelperCommand =
  | {
      type: 'typeText';
      id: string;
      text: string;
    }
  | { type: 'shutdown' };

type PendingRequest = {
  resolve: () => void;
  reject: (error: Error) => void;
  timeout: NodeJS.Timeout;
};

type HelperMessage = {
  type?: string;
  id?: string | null;
  ok?: boolean;
  code?: string;
  message?: string;
};

const DEFAULT_TIMEOUT_MS = 2_000;

export function createTextInputBackend(options: TextInputBackendOptions = {}): TextInputBackend {
  return new NativeTextInputBackend(options);
}

class NativeTextInputBackend implements TextInputBackend {
  private process: ChildProcessWithoutNullStreams | null = null;
  private ready: Promise<void> | null = null;
  private readyResolve: (() => void) | null = null;
  private readyReject: ((error: Error) => void) | null = null;
  private stdoutBuffer = '';
  private disposed = false;
  private readonly pending = new Map<string, PendingRequest>();

  constructor(private readonly options: TextInputBackendOptions) {}

  async typeText(text: string): Promise<void> {
    if (this.disposed) {
      throw new Error('Text input helper is disposed.');
    }

    const helper = this.ensureProcess();
    const timeoutMs = this.options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    await this.waitUntilReady(timeoutMs);

    if (!helper.stdin || helper.stdin.destroyed) {
      throw new Error('Text input helper stdin is unavailable.');
    }

    const id = this.options.createRequestId?.() ?? `text-input-${randomUUID()}`;

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        const error = new Error('Text input helper timed out.');
        this.pending.delete(id);
        this.fail(error);
        reject(error);
      }, timeoutMs);
      timeout.unref?.();
      this.pending.set(id, { resolve, reject, timeout });

      try {
        helper.stdin.write(`${JSON.stringify({ type: 'typeText', id, text } satisfies TextInputHelperCommand)}\n`);
      } catch (error) {
        this.pending.delete(id);
        clearTimeout(timeout);
        reject(error instanceof Error ? error : new Error('Text input helper write failed.'));
      }
    });
  }

  dispose(): void {
    this.disposed = true;
    this.rejectAll(new Error('Text input helper was disposed.'));

    const helper = this.process;
    this.process = null;
    this.ready = null;
    this.readyResolve = null;
    this.readyReject = null;
    if (!helper) return;

    try {
      helper.stdin.write(`${JSON.stringify({ type: 'shutdown' } satisfies TextInputHelperCommand)}\n`);
      helper.stdin.end();
    } catch {
      // Ignore shutdown failures; the process is about to be killed if it remains alive.
    }

    const killDelayMs = this.options.shutdownKillDelayMs ?? 500;
    setTimeout(() => {
      if (!helper.killed) {
        helper.kill();
      }
    }, killDelayMs).unref();
  }

  private ensureProcess(): ChildProcessWithoutNullStreams {
    if (this.process) return this.process;

    const helperPath = this.options.helperPath ?? resolveTextInputHelperPath();
    const exists = this.options.exists ?? existsSync;
    if (!exists(helperPath)) {
      throw new Error(`Text input helper was not found: ${helperPath}`);
    }

    const helper = (this.options.spawnProcess ?? spawnTextInputHelper)(helperPath);
    this.process = helper;
    this.ready = new Promise<void>((resolve, reject) => {
      this.readyResolve = resolve;
      this.readyReject = reject;
    });

    helper.once('error', (error) => {
      this.fail(new Error(`Text input helper failed: ${error.message}`));
    });
    helper.once('exit', (code, signal) => {
      if (!this.disposed) {
        this.fail(new Error(`Text input helper exited unexpectedly: ${signal ?? code ?? 'unknown'}`));
      }
    });
    helper.stdout.on('data', (chunk) => this.handleStdout(String(chunk)));
    helper.stderr.on('data', () => {
      // Stderr is intentionally ignored to avoid leaking target text from unexpected helper output.
    });

    return helper;
  }

  private async waitUntilReady(timeoutMs: number): Promise<void> {
    if (!this.ready) {
      throw new Error('Text input helper is unavailable.');
    }

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        const error = new Error('Text input helper timed out.');
        this.fail(error);
        reject(error);
      }, timeoutMs);
      timeout.unref?.();
      this.ready?.then(
        () => {
          clearTimeout(timeout);
          resolve();
        },
        (error) => {
          clearTimeout(timeout);
          reject(error);
        }
      );
    });
  }

  private handleStdout(output: string): void {
    this.stdoutBuffer += output;

    let newlineIndex = this.stdoutBuffer.search(/\r?\n/);
    while (newlineIndex >= 0) {
      const line = this.stdoutBuffer.slice(0, newlineIndex).trim();
      this.stdoutBuffer = this.stdoutBuffer.slice(newlineIndex + (this.stdoutBuffer[newlineIndex] === '\r' ? 2 : 1));
      if (line) {
        this.handleLine(line);
      }
      newlineIndex = this.stdoutBuffer.search(/\r?\n/);
    }
  }

  private handleLine(line: string): void {
    let message: HelperMessage;
    try {
      message = JSON.parse(line) as HelperMessage;
    } catch {
      this.fail(new Error('Text input helper returned malformed status output.'));
      return;
    }

    if (message.type === 'ready') {
      this.readyResolve?.();
      this.readyResolve = null;
      this.readyReject = null;
      return;
    }

    const id = typeof message.id === 'string' ? message.id : null;
    if (!id) {
      if (message.type === 'error') {
        this.fail(new Error(helperErrorMessage(message)));
      }
      return;
    }

    const pending = this.pending.get(id);
    if (!pending) return;

    this.pending.delete(id);
    clearTimeout(pending.timeout);

    if (message.type === 'result' && message.ok === true) {
      pending.resolve();
      return;
    }

    pending.reject(new Error(helperErrorMessage(message)));
  }

  private fail(error: Error): void {
    this.readyReject?.(error);
    this.readyResolve = null;
    this.readyReject = null;
    this.rejectAll(error);

    const helper = this.process;
    this.process = null;
    if (helper && !helper.killed) {
      helper.kill();
    }
  }

  private rejectAll(error: Error): void {
    for (const [id, pending] of this.pending) {
      this.pending.delete(id);
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
  }
}

function helperErrorMessage(message: HelperMessage): string {
  const code = typeof message.code === 'string' ? message.code : 'helper_error';
  return `Text input helper failed (${code}).`;
}

function spawnTextInputHelper(helperPath: string): ChildProcessWithoutNullStreams {
  return spawn(helperPath, [], {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true
  });
}

function resolveTextInputHelperPath(): string {
  return app.isPackaged
    ? join(process.resourcesPath, 'native', 'SwitchifyTextInput.exe')
    : join(process.cwd(), 'build', 'native', 'text-input-helper', 'win-x64', 'SwitchifyTextInput.exe');
}
