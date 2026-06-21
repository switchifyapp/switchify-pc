import { EventEmitter } from 'node:events';
import { describe, expect, it, vi } from 'vitest';
import { createTextInputBackend } from './text-input-helper-client';

class FakeTextInputHelperProcess extends EventEmitter {
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

function emitJson(helper: FakeTextInputHelperProcess, message: unknown): void {
  helper.stdout.emit('data', `${JSON.stringify(message)}\n`);
}

async function waitForWrites(helper: FakeTextInputHelperProcess, count: number): Promise<void> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < 1_000) {
    if (helper.writes.length >= count) return;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error(`Expected ${count} helper writes, saw ${helper.writes.length}.`);
}

describe('createTextInputBackend', () => {
  it('waits for ready and sends typeText commands', async () => {
    const helper = new FakeTextInputHelperProcess();
    const backend = createTextInputBackend({
      helperPath: 'C:\\SwitchifyTextInput.exe',
      exists: () => true,
      spawnProcess: () => helper as never,
      createRequestId: () => 'request-1'
    });

    const result = backend.typeText('Secret text');
    emitJson(helper, { type: 'ready' });
    await waitForWrites(helper, 1);
    expect(JSON.parse(helper.writes[0])).toEqual({
      type: 'typeText',
      id: 'request-1',
      text: 'Secret text'
    });
    emitJson(helper, { type: 'result', id: 'request-1', ok: true, sentEvents: 22 });

    await expect(result).resolves.toBeUndefined();
  });

  it('rejects if the helper executable is missing', async () => {
    const backend = createTextInputBackend({
      helperPath: 'C:\\missing\\SwitchifyTextInput.exe',
      exists: () => false
    });

    await expect(backend.typeText('Secret text')).rejects.toThrow('Text input helper was not found');
  });

  it('rejects helper error responses without including typed text', async () => {
    const helper = new FakeTextInputHelperProcess();
    const backend = createTextInputBackend({
      helperPath: 'C:\\SwitchifyTextInput.exe',
      exists: () => true,
      spawnProcess: () => helper as never,
      createRequestId: () => 'request-1'
    });

    const result = backend.typeText('Secret text');
    emitJson(helper, { type: 'ready' });
    await waitForWrites(helper, 1);
    emitJson(helper, {
      type: 'error',
      id: 'request-1',
      ok: false,
      code: 'send_input_failed',
      message: 'Secret text should not appear in the client error.'
    });

    const error = await result.catch((item: unknown) => item);
    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toContain('Text input helper failed (send_input_failed).');
    expect((error as Error).message).not.toContain('Secret text');
  });

  it('rejects when the helper does not become ready before timeout', async () => {
    vi.useFakeTimers();
    try {
      const helper = new FakeTextInputHelperProcess();
      const backend = createTextInputBackend({
        helperPath: 'C:\\SwitchifyTextInput.exe',
        exists: () => true,
        spawnProcess: () => helper as never,
        timeoutMs: 50
      });

      const result = backend.typeText('Secret text');
      const expectation = expect(result).rejects.toThrow('Text input helper timed out.');
      await vi.advanceTimersByTimeAsync(50);

      await expectation;
    } finally {
      vi.useRealTimers();
    }
  });

  it('rejects when the helper exits before responding', async () => {
    const helper = new FakeTextInputHelperProcess();
    const backend = createTextInputBackend({
      helperPath: 'C:\\SwitchifyTextInput.exe',
      exists: () => true,
      spawnProcess: () => helper as never,
      createRequestId: () => 'request-1'
    });

    const result = backend.typeText('Secret text');
    emitJson(helper, { type: 'ready' });
    await waitForWrites(helper, 1);
    helper.emit('exit', 1, null);

    await expect(result).rejects.toThrow('Text input helper exited unexpectedly');
  });

  it('sends shutdown on dispose', async () => {
    const helper = new FakeTextInputHelperProcess();
    const backend = createTextInputBackend({
      helperPath: 'C:\\SwitchifyTextInput.exe',
      exists: () => true,
      spawnProcess: () => helper as never,
      createRequestId: () => 'request-1',
      shutdownKillDelayMs: 1
    });

    const result = backend.typeText('Secret text');
    emitJson(helper, { type: 'ready' });
    await waitForWrites(helper, 1);
    emitJson(helper, { type: 'result', id: 'request-1', ok: true, sentEvents: 22 });
    await result;

    backend.dispose();

    expect(JSON.parse(helper.writes[1])).toEqual({ type: 'shutdown' });
    expect(helper.stdin.end).toHaveBeenCalled();
  });
});
