import { describe, expect, it } from 'vitest';
import type { CommandRequest } from '../../shared/protocol';
import { PROTOCOL_VERSION } from '../../shared/protocol';
import type { DesktopInputAdapter } from './desktop-input-adapter';
import { DesktopInputError } from './desktop-input-adapter';
import { DesktopCommandExecutor } from './command-executor';

type RecordedCall = { method: keyof DesktopInputAdapter; args: unknown[] };

class FakeCursorOverlay {
  readonly events: Array<'move' | 'click'> = [];

  show(event: 'move' | 'click'): void {
    this.events.push(event);
  }
}

class FakeInputAdapter implements DesktopInputAdapter {
  readonly calls: RecordedCall[] = [];
  failNext: Error | null = null;
  activeMouseMoves = 0;
  maxActiveMouseMoves = 0;
  mouseMoveDelayMs = 0;

  getMousePosition(): { x: number; y: number } {
    return { x: 100, y: 200 };
  }

  async moveMouseBy(delta: { dx: number; dy: number }): Promise<void> {
    this.activeMouseMoves += 1;
    this.maxActiveMouseMoves = Math.max(this.maxActiveMouseMoves, this.activeMouseMoves);
    try {
      if (this.mouseMoveDelayMs > 0) {
        await new Promise((resolve) => setTimeout(resolve, this.mouseMoveDelayMs));
      }
      this.record('moveMouseBy', [delta]);
    } finally {
      this.activeMouseMoves -= 1;
    }
  }

  async clickMouse(button: 'left' | 'right' | 'middle'): Promise<void> {
    this.record('clickMouse', [button]);
  }

  async doubleClickMouse(button: 'left' | 'right' | 'middle'): Promise<void> {
    this.record('doubleClickMouse', [button]);
  }

  async scrollMouse(delta: { dx: number; dy: number }): Promise<void> {
    this.record('scrollMouse', [delta]);
  }

  async pressKey(key: string): Promise<void> {
    this.record('pressKey', [key]);
  }

  async pressShortcut(keys: string[]): Promise<void> {
    this.record('pressShortcut', [keys]);
  }

  async typeText(text: string): Promise<void> {
    this.record('typeText', [text]);
  }

  async mediaControl(action: string): Promise<void> {
    this.record('mediaControl', [action]);
  }

  private record(method: keyof DesktopInputAdapter, args: unknown[]): void {
    if (this.failNext) {
      const error = this.failNext;
      this.failNext = null;
      throw error;
    }
    this.calls.push({ method, args });
  }
}

describe('DesktopCommandExecutor', () => {
  it('maps mouse commands to the adapter', async () => {
    const { adapter, executor, overlay } = createExecutor();

    await executor.execute(command('mouse.move', { dx: 10, dy: -4 }));
    await executor.execute(command('mouse.click', { button: 'left' }));
    await executor.execute(command('mouse.doubleClick', { button: 'middle' }));
    await executor.execute(command('mouse.rightClick', {}));
    await executor.execute(command('mouse.scroll', { dx: 0, dy: -3 }));

    expect(adapter.calls).toEqual([
      { method: 'moveMouseBy', args: [{ dx: 10, dy: -4 }] },
      { method: 'clickMouse', args: ['left'] },
      { method: 'doubleClickMouse', args: ['middle'] },
      { method: 'clickMouse', args: ['right'] },
      { method: 'scrollMouse', args: [{ dx: 0, dy: -3 }] }
    ]);
    expect(overlay.events).toEqual(['move', 'click', 'click', 'click']);
  });

  it('maps keyboard, text, media, and ping commands', async () => {
    const { adapter, executor } = createExecutor();

    await executor.execute(command('keyboard.key', { key: 'Enter' }));
    await executor.execute(command('keyboard.shortcut', { keys: ['Ctrl', 'C'] }));
    await executor.execute(command('keyboard.typeText', { text: 'Hello' }));
    await executor.execute(command('media.control', { action: 'playPause' }));
    const pingResult = await executor.execute(command('connection.ping', {}));

    expect(pingResult).toEqual({ ok: true });
    expect(adapter.calls).toEqual([
      { method: 'pressKey', args: ['Enter'] },
      { method: 'pressShortcut', args: [['Ctrl', 'C']] },
      { method: 'typeText', args: ['Hello'] },
      { method: 'mediaControl', args: ['playPause'] }
    ]);
  });

  it('passes empty committed text to the adapter', async () => {
    const { adapter, executor } = createExecutor();

    await expect(executor.execute(command('keyboard.typeText', { text: '' }))).resolves.toEqual({ ok: true });

    expect(adapter.calls).toEqual([{ method: 'typeText', args: [''] }]);
  });

  it('rejects unsafe movement, scroll, shortcut, and text values', async () => {
    const { adapter, executor, overlay } = createExecutor();

    await expect(executor.execute(command('mouse.move', { dx: 501, dy: 0 }))).resolves.toMatchObject({
      ok: false,
      code: 'unsafe_payload'
    });
    await expect(executor.execute(command('mouse.scroll', { dx: 0, dy: 51 }))).resolves.toMatchObject({
      ok: false,
      code: 'unsafe_payload'
    });
    await expect(executor.execute(command('keyboard.shortcut', { keys: [] }))).resolves.toMatchObject({
      ok: false,
      code: 'unsafe_payload'
    });
    await expect(executor.execute(command('keyboard.typeText', { text: 'x'.repeat(2_001) }))).resolves.toMatchObject({
      ok: false,
      code: 'unsafe_payload'
    });
    expect(adapter.calls).toHaveLength(0);
    expect(overlay.events).toHaveLength(0);
  });

  it('converts adapter errors into structured failures', async () => {
    const { adapter, executor, overlay } = createExecutor();
    adapter.failNext = new DesktopInputError('adapter_failure', 'Native input failed.');

    await expect(executor.execute(command('mouse.click', { button: 'left' }))).resolves.toEqual({
      ok: false,
      code: 'adapter_failure',
      message: 'Native input failed.'
    });
    expect(overlay.events).toHaveLength(0);
  });

  it('serializes repeated pointer movement commands', async () => {
    const { adapter, executor } = createExecutor();
    adapter.mouseMoveDelayMs = 10;

    await Promise.all([
      executor.execute(command('mouse.move', { dx: 1, dy: 0 })),
      executor.execute(command('mouse.move', { dx: 2, dy: 0 })),
      executor.execute(command('mouse.move', { dx: 3, dy: 0 }))
    ]);

    expect(adapter.maxActiveMouseMoves).toBe(1);
    expect(adapter.calls).toEqual([
      { method: 'moveMouseBy', args: [{ dx: 1, dy: 0 }] },
      { method: 'moveMouseBy', args: [{ dx: 2, dy: 0 }] },
      { method: 'moveMouseBy', args: [{ dx: 3, dy: 0 }] }
    ]);
  });
});

function createExecutor(): { adapter: FakeInputAdapter; overlay: FakeCursorOverlay; executor: DesktopCommandExecutor } {
  const adapter = new FakeInputAdapter();
  const overlay = new FakeCursorOverlay();
  return { adapter, overlay, executor: new DesktopCommandExecutor(adapter, overlay) };
}

function command<TType extends CommandRequest['type']>(
  type: TType,
  payload: Extract<CommandRequest, { type: TType }>['payload']
): Extract<CommandRequest, { type: TType }> {
  return {
    version: PROTOCOL_VERSION,
    id: 'request-1',
    deviceId: 'android-1',
    timestamp: 1,
    type,
    payload,
    auth: 'proof'
  } as Extract<CommandRequest, { type: TType }>;
}
