import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync } from 'node:fs';
import { validateBluetoothFrame, type BluetoothFrame } from '../../shared/bluetooth-frame';
import type {
  BluetoothDiagnosticEvent,
  BluetoothDisconnectReason,
  BluetoothUnavailableReason
} from '../../shared/bluetooth-status';
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
        const event = parseHelperEvent(JSON.parse(line));
        if (!event) {
          this.fail('Bluetooth helper returned malformed status output.');
          return;
        }
        this.options.onEvent(event);
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

function parseHelperEvent(value: unknown): BluetoothHelperEvent | null {
  if (!isRecord(value) || typeof value.type !== 'string') return null;

  switch (value.type) {
    case 'ready':
      return { type: 'ready' };
    case 'unavailable':
      return isUnavailableReason(value.reason) ? { type: 'unavailable', reason: value.reason } : null;
    case 'connected':
      return typeof value.connectionId === 'string' && typeof value.label === 'string'
        ? { type: 'connected', connectionId: value.connectionId, label: value.label }
        : null;
    case 'message':
      return typeof value.connectionId === 'string' && isBluetoothFrame(value.frame)
        ? { type: 'message', connectionId: value.connectionId, frame: value.frame }
        : null;
    case 'disconnected':
      return typeof value.connectionId === 'string' && isDisconnectReason(value.reason)
        ? { type: 'disconnected', connectionId: value.connectionId, reason: value.reason }
        : null;
    case 'diagnostic':
      return isDiagnosticEvent(value.event) ? { type: 'diagnostic', event: value.event } : null;
    case 'error':
      return typeof value.reason === 'string' ? { type: 'error', reason: value.reason } : null;
    default:
      return null;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isBluetoothFrame(value: unknown): value is BluetoothFrame {
  if (!isRecord(value)) return false;
  const result = validateBluetoothFrame(value as BluetoothFrame);
  return !result.ok && result.reason === 'incomplete';
}

function isUnavailableReason(value: unknown): value is BluetoothUnavailableReason {
  return value === 'unsupported' || value === 'permission_denied' || value === 'adapter_off' || value === 'startup_failed';
}

function isDisconnectReason(value: unknown): value is Exclude<BluetoothDisconnectReason, null> {
  return (
    value === 'notification_unsubscribed' ||
    value === 'client_requested' ||
    value === 'pc_requested' ||
    value === 'helper_stopped' ||
    value === 'helper_error'
  );
}

function isDiagnosticEvent(value: unknown): value is Exclude<BluetoothDiagnosticEvent, null> {
  return (
    value === 'advertising_started' ||
    value === 'subscribed' ||
    value === 'unsubscribed' ||
    value === 'unsubscribe_grace_started' ||
    value === 'unsubscribe_grace_cancelled' ||
    value === 'unsubscribe_grace_timed_out' ||
    value === 'write_received'
  );
}

function spawnBluetoothHelper(helperPath: string): ChildProcessWithoutNullStreams {
  return spawn(helperPath, [], {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true
  });
}

