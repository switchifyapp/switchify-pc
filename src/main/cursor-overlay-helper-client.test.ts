import { EventEmitter } from 'node:events';
import { describe, expect, it, vi } from 'vitest';
import {
  NativeWindowsCursorOverlayBackend,
  nativeHelperCursorPosition,
  type CursorOverlayBackend
} from './cursor-overlay-helper-client';

class FakeHelperProcess extends EventEmitter {
  readonly writes: string[] = [];
  killed = false;
  readonly stdin = {
    destroyed: false,
    write: vi.fn((chunk: string) => {
      this.writes.push(chunk);
      return true;
    }),
    end: vi.fn()
  };
  readonly stdout = new EventEmitter();
  readonly stderr = new EventEmitter();

  kill(): boolean {
    this.killed = true;
    this.emit('exit', null, 'SIGTERM');
    return true;
  }
}

class FakeOverlayBackend implements CursorOverlayBackend {
  readonly events: string[] = [];

  show(event: 'move' | 'click'): void {
    this.events.push(`show:${event}`);
  }

  hide(): void {
    this.events.push('hide');
  }

  destroy(): void {
    this.events.push('destroy');
  }
}

describe('NativeWindowsCursorOverlayBackend', () => {
  it('converts Electron cursor DIP coordinates to native screen pixels', () => {
    expect(
      nativeHelperCursorPosition({
        getCursorScreenPoint: () => ({ x: 100, y: 200 }),
        dipToScreenPoint: (point) => ({ x: point.x * 2, y: point.y * 2 })
      })
    ).toEqual({ x: 200, y: 400 });
  });

  it('writes show commands to the helper process', () => {
    const helper = new FakeHelperProcess();
    const fallback = new FakeOverlayBackend();
    const backend = new NativeWindowsCursorOverlayBackend({
      helperPath: __filename,
      fallback,
      getCursorPosition: () => ({ x: 100, y: 200 }),
      idleTimeoutMs: 900,
      windowSize: 72,
      spawnProcess: () => helper as never
    });

    backend.show('move');

    expect(fallback.events).toEqual([]);
    expect(JSON.parse(helper.writes[0])).toEqual({
      type: 'show',
      event: 'move',
      x: 100,
      y: 200,
      size: 72,
      durationMs: 900
    });
  });

  it('writes hide and shutdown commands', async () => {
    const helper = new FakeHelperProcess();
    const fallback = new FakeOverlayBackend();
    const backend = new NativeWindowsCursorOverlayBackend({
      helperPath: __filename,
      fallback,
      getCursorPosition: () => ({ x: 0, y: 0 }),
      idleTimeoutMs: 900,
      windowSize: 72,
      shutdownKillDelayMs: 1,
      spawnProcess: () => helper as never
    });

    backend.show('click');
    backend.hide();
    backend.destroy();
    await new Promise((resolve) => setTimeout(resolve, 5));

    expect(JSON.parse(helper.writes[1])).toEqual({ type: 'hide' });
    expect(JSON.parse(helper.writes[2])).toEqual({ type: 'shutdown' });
    expect(helper.stdin.end).toHaveBeenCalled();
    expect(fallback.events).toEqual(['destroy']);
  });

  it('falls back when the helper is missing', () => {
    const fallback = new FakeOverlayBackend();
    const failures: string[] = [];
    const backend = new NativeWindowsCursorOverlayBackend({
      helperPath: 'C:\\missing\\SwitchifyCursorOverlay.exe',
      fallback,
      getCursorPosition: () => ({ x: 100, y: 200 }),
      idleTimeoutMs: 900,
      windowSize: 72,
      onFailure: (message) => failures.push(message)
    });

    backend.show('move');

    expect(fallback.events).toEqual(['show:move']);
    expect(failures[0]).toContain('Cursor overlay helper was not found');
  });

  it('falls back after helper errors', () => {
    const helper = new FakeHelperProcess();
    const fallback = new FakeOverlayBackend();
    const backend = new NativeWindowsCursorOverlayBackend({
      helperPath: __filename,
      fallback,
      getCursorPosition: () => ({ x: 0, y: 0 }),
      idleTimeoutMs: 900,
      windowSize: 72,
      spawnProcess: () => helper as never
    });

    backend.show('move');
    helper.emit('error', new Error('boom'));
    backend.show('click');

    expect(helper.killed).toBe(true);
    expect(fallback.events).toEqual(['show:click']);
  });

  it('falls back when helper reports an error status', () => {
    const helper = new FakeHelperProcess();
    const fallback = new FakeOverlayBackend();
    const backend = new NativeWindowsCursorOverlayBackend({
      helperPath: __filename,
      fallback,
      getCursorPosition: () => ({ x: 0, y: 0 }),
      idleTimeoutMs: 900,
      windowSize: 72,
      spawnProcess: () => helper as never
    });

    backend.show('move');
    helper.stdout.emit('data', '{"type":"error","message":"bad command"}\n');
    backend.show('click');

    expect(fallback.events).toEqual(['show:click']);
  });
});
