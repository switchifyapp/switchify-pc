import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync } from 'node:fs';

export type CursorOverlayEvent = 'move' | 'click';

export type CursorOverlayPoint = {
  x: number;
  y: number;
};

export type CursorOverlayBackend = {
  show(event: CursorOverlayEvent): void;
  hide(): void;
  destroy(): void;
};

export type NativeWindowsCursorOverlayBackendOptions = {
  helperPath: string;
  fallback: CursorOverlayBackend;
  getCursorPosition: () => CursorOverlayPoint;
  windowSize: number;
  idleTimeoutMs: number;
  spawnProcess?: (helperPath: string) => ChildProcessWithoutNullStreams;
  onFailure?: (message: string) => void;
  shutdownKillDelayMs?: number;
};

export type CursorPositionProvider = {
  getCursorScreenPoint: () => CursorOverlayPoint;
  dipToScreenPoint: (point: CursorOverlayPoint) => CursorOverlayPoint;
};

type OverlayHelperCommand =
  | {
      type: 'show';
      event: CursorOverlayEvent;
      x: number;
      y: number;
      size: number;
      durationMs: number;
    }
  | { type: 'hide' }
  | { type: 'shutdown' };

export class NativeWindowsCursorOverlayBackend implements CursorOverlayBackend {
  private process: ChildProcessWithoutNullStreams | null = null;
  private unavailable = false;
  private destroyed = false;

  constructor(private readonly options: NativeWindowsCursorOverlayBackendOptions) {}

  show(event: CursorOverlayEvent): void {
    if (this.destroyed || this.unavailable) {
      this.options.fallback.show(event);
      return;
    }

    const helper = this.ensureProcess();
    if (!helper) {
      this.options.fallback.show(event);
      return;
    }

    const cursor = this.options.getCursorPosition();
    this.writeCommand(
      {
        type: 'show',
        event,
        x: cursor.x,
        y: cursor.y,
        size: this.options.windowSize,
        durationMs: this.options.idleTimeoutMs
      },
      { fallbackEvent: event, fallbackOnBackpressure: false }
    );
  }

  hide(): void {
    if (this.unavailable || this.destroyed) {
      this.options.fallback.hide();
      return;
    }

    if (!this.process) {
      this.options.fallback.hide();
      return;
    }

    this.writeCommand({ type: 'hide' });
  }

  destroy(): void {
    this.destroyed = true;
    this.options.fallback.destroy();

    const helper = this.process;
    this.process = null;
    if (!helper) return;

    try {
      helper.stdin.write(`${JSON.stringify({ type: 'shutdown' } satisfies OverlayHelperCommand)}\n`);
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

  private ensureProcess(): ChildProcessWithoutNullStreams | null {
    if (this.process) return this.process;

    if (!existsSync(this.options.helperPath)) {
      this.fail(`Cursor overlay helper was not found: ${this.options.helperPath}`);
      return null;
    }

    try {
      const helper = (this.options.spawnProcess ?? spawnOverlayHelper)(this.options.helperPath);
      this.process = helper;

      helper.once('error', (error) => {
        this.fail(`Cursor overlay helper failed: ${error.message}`);
      });
      helper.once('exit', (code, signal) => {
        if (!this.destroyed) {
          this.fail(`Cursor overlay helper exited unexpectedly: ${signal ?? code ?? 'unknown'}`);
        }
      });
      helper.stdout.on('data', (chunk) => this.handleStdout(String(chunk)));
      helper.stderr.on('data', (chunk) => {
        this.options.onFailure?.(`Cursor overlay helper stderr: ${String(chunk).trim()}`);
      });

      return helper;
    } catch (error) {
      this.fail(error instanceof Error ? error.message : 'Cursor overlay helper could not start.');
      return null;
    }
  }

  private writeCommand(
    command: OverlayHelperCommand,
    options: { fallbackEvent?: CursorOverlayEvent; fallbackOnBackpressure?: boolean } = {}
  ): void {
    const helper = this.process;
    if (!helper || helper.stdin.destroyed) {
      this.fail('Cursor overlay helper stdin is unavailable.');
      if (options.fallbackEvent) {
        this.options.fallback.show(options.fallbackEvent);
      }
      return;
    }

    try {
      const accepted = helper.stdin.write(`${JSON.stringify(command)}\n`);
      if (!accepted && options.fallbackOnBackpressure) {
        this.options.fallback.show(options.fallbackEvent ?? 'move');
      }
    } catch (error) {
      this.fail(error instanceof Error ? error.message : 'Cursor overlay helper write failed.');
      if (options.fallbackEvent) {
        this.options.fallback.show(options.fallbackEvent);
      }
    }
  }

  private handleStdout(output: string): void {
    for (const line of output.split(/\r?\n/).map((item) => item.trim()).filter(Boolean)) {
      try {
        const message = JSON.parse(line) as { type?: string; message?: string };
        if (message.type === 'error') {
          this.fail(message.message ?? 'Cursor overlay helper reported an error.');
        }
      } catch {
        this.fail('Cursor overlay helper returned malformed status output.');
      }
    }
  }

  private fail(message: string): void {
    if (this.unavailable) return;

    this.unavailable = true;
    this.options.onFailure?.(message);

    const helper = this.process;
    this.process = null;
    if (helper && !helper.killed) {
      helper.kill();
    }
  }
}

function spawnOverlayHelper(helperPath: string): ChildProcessWithoutNullStreams {
  return spawn(helperPath, [], {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true
  });
}

export function nativeHelperCursorPosition(provider: CursorPositionProvider): CursorOverlayPoint {
  return provider.dipToScreenPoint(provider.getCursorScreenPoint());
}
