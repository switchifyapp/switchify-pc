import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync } from 'node:fs';
import type { BluetoothFrame } from '../../shared/bluetooth-frame';
import type { BluetoothHelperCommand, BluetoothHelperEvent } from './helper-protocol';

export type BluetoothHelperClientOptions = {
  helperPath: string;
  spawnProcess?: (helperPath: string) => ChildProcessWithoutNullStreams;
  onEvent: (event: BluetoothHelperEvent) => void;
  onFailure?: (message: string) => void;
  shutdownKillDelayMs?: number;
};

export class BluetoothHelperClient {
  private process: ChildProcessWithoutNullStreams | null = null;
  private unavailable = false;
  private destroyed = false;
  private stdoutBuffer = '';

  constructor(private readonly options: BluetoothHelperClientOptions) {}

  start(command: Extract<BluetoothHelperCommand, { type: 'start' }>): boolean {
    if (this.destroyed || this.unavailable) return false;
    const helper = this.ensureProcess();
    if (!helper) return false;
    return this.write(command);
  }

  stop(): void {
    this.write({ type: 'stop' });
  }

  send(connectionId: string, frame: BluetoothFrame): void {
    this.write({ type: 'send', connectionId, frame });
  }

  disconnect(connectionId: string): void {
    this.write({ type: 'disconnect', connectionId });
  }

  destroy(): void {
    this.destroyed = true;
    const helper = this.process;
    this.process = null;
    if (!helper) return;

    try {
      helper.stdin.write(`${JSON.stringify({ type: 'shutdown' } satisfies BluetoothHelperCommand)}\n`);
      helper.stdin.end();
    } catch {
      // Ignore shutdown failures; the process is being torn down.
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
      this.fail(`Bluetooth helper was not found: ${this.options.helperPath}`);
      return null;
    }

    try {
      const helper = (this.options.spawnProcess ?? spawnBluetoothHelper)(this.options.helperPath);
      this.process = helper;

      helper.once('error', (error) => {
        this.fail(`Bluetooth helper failed: ${error.message}`);
      });
      helper.once('exit', (code, signal) => {
        if (!this.destroyed) {
          this.fail(`Bluetooth helper exited unexpectedly: ${signal ?? code ?? 'unknown'}`);
        }
      });
      helper.stdout.on('data', (chunk) => this.handleStdout(String(chunk)));
      helper.stderr.on('data', (chunk) => {
        this.options.onFailure?.(`Bluetooth helper stderr: ${String(chunk).trim()}`);
      });

      return helper;
    } catch (error) {
      this.fail(error instanceof Error ? error.message : 'Bluetooth helper could not start.');
      return null;
    }
  }

  private write(command: BluetoothHelperCommand): boolean {
    const helper = this.process;
    if (!helper || helper.stdin.destroyed) {
      this.fail('Bluetooth helper stdin is unavailable.');
      return false;
    }

    try {
      return helper.stdin.write(`${JSON.stringify(command)}\n`);
    } catch (error) {
      this.fail(error instanceof Error ? error.message : 'Bluetooth helper write failed.');
      return false;
    }
  }

  private handleStdout(output: string): void {
    this.stdoutBuffer += output;
    const lines = this.stdoutBuffer.split(/\r?\n/);
    this.stdoutBuffer = lines.pop() ?? '';

    for (const line of lines.map((item) => item.trim()).filter(Boolean)) {
      try {
        this.options.onEvent(JSON.parse(line) as BluetoothHelperEvent);
      } catch {
        this.fail('Bluetooth helper returned malformed status output.');
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

function spawnBluetoothHelper(helperPath: string): ChildProcessWithoutNullStreams {
  return spawn(helperPath, [], {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true
  });
}

