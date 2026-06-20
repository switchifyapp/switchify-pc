import { describe, expect, it } from 'vitest';
import type { CommandRequest, WindowControlAction } from '../../shared/protocol';
import { PROTOCOL_VERSION } from '../../shared/protocol';
import type { DesktopInputAdapter } from './desktop-input-adapter';
import { DesktopInputError } from './desktop-input-adapter';
import { DesktopCommandExecutor } from './command-executor';

type RecordedCall = { method: keyof DesktopInputAdapter; args: unknown[] };

class FakeCursorOverlay {
  readonly events: Array<'move' | 'click' | 'drag'> = [];
  readonly dragActiveChanges: boolean[] = [];
  activeCount = 0;
  hideCount = 0;

  show(event: 'move' | 'click' | 'drag'): void {
    this.events.push(event);
  }

  setDragActive(active: boolean): void {
    this.dragActiveChanges.push(active);
  }

  markControlActive(): void {
    this.activeCount += 1;
  }

  hide(): void {
    this.hideCount += 1;
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

  async setMouseButtonDown(button: 'left' | 'right' | 'middle', down: boolean): Promise<void> {
    this.record('setMouseButtonDown', [button, down]);
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

  async controlWindow(action: WindowControlAction): Promise<void> {
    this.record('controlWindow', [action]);
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
    expect(overlay.activeCount).toBe(5);
    expect(overlay.hideCount).toBe(0);
  });

  it('maps keyboard, text, media, and ping commands', async () => {
    const { adapter, executor, overlay } = createExecutor();

    await executor.execute(command('keyboard.key', { key: 'Enter' }));
    await executor.execute(command('keyboard.key', { key: 'F12' }));
    await executor.execute(command('keyboard.shortcut', { keys: ['Ctrl', 'C'] }));
    await executor.execute(command('keyboard.shortcut', { keys: ['Ctrl', 'F5'] }));
    await executor.execute(command('keyboard.typeText', { text: 'Hello' }));
    await executor.execute(command('media.control', { action: 'playPause' }));
    await executor.execute(command('window.control', { action: 'switchNext' }));
    const pingResult = await executor.execute(command('connection.ping', {}));

    expect(pingResult).toEqual({ ok: true });
    expect(adapter.calls).toEqual([
      { method: 'pressKey', args: ['Enter'] },
      { method: 'pressKey', args: ['F12'] },
      { method: 'pressShortcut', args: [['Ctrl', 'C']] },
      { method: 'pressShortcut', args: [['Ctrl', 'F5']] },
      { method: 'typeText', args: ['Hello'] },
      { method: 'mediaControl', args: ['playPause'] },
      { method: 'controlWindow', args: ['switchNext'] }
    ]);
    expect(overlay.events).toHaveLength(0);
    expect(overlay.activeCount).toBe(0);
    expect(overlay.hideCount).toBe(8);
  });

  it('executes non-movement no-response commands without coalescing', async () => {
    const { adapter, executor, overlay } = createExecutor();

    await executor.execute(command('mouse.click', { button: 'left' }, { responseMode: 'none' }));
    await executor.execute(command('mouse.scroll', { dx: 0, dy: -3 }, { responseMode: 'none' }));
    await executor.execute(command('keyboard.key', { key: 'F12' }, { responseMode: 'none' }));
    await executor.execute(command('window.control', { action: 'switchNext' }, { responseMode: 'none' }));

    expect(adapter.calls).toEqual([
      { method: 'clickMouse', args: ['left'] },
      { method: 'scrollMouse', args: [{ dx: 0, dy: -3 }] },
      { method: 'pressKey', args: ['F12'] },
      { method: 'controlWindow', args: ['switchNext'] }
    ]);
    expect(overlay.events).toEqual(['click']);
    expect(overlay.activeCount).toBe(2);
    expect(overlay.hideCount).toBe(2);
  });

  it('passes empty committed text to the adapter', async () => {
    const { adapter, executor } = createExecutor();

    await expect(executor.execute(command('keyboard.typeText', { text: '' }))).resolves.toEqual({ ok: true });

    expect(adapter.calls).toEqual([{ method: 'typeText', args: [''] }]);
  });

  it('hides the overlay for unsupported server-level non-mouse commands', async () => {
    const { executor, overlay } = createExecutor();

    await expect(executor.execute(command('connection.disconnecting', {}))).resolves.toMatchObject({
      ok: false,
      code: 'unsupported_command'
    });
    await expect(executor.execute(command('pointer.profile', {}))).resolves.toMatchObject({
      ok: false,
      code: 'unsupported_command'
    });

    expect(overlay.activeCount).toBe(0);
    expect(overlay.hideCount).toBe(2);
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
    expect(overlay.activeCount).toBe(2);
    expect(overlay.hideCount).toBe(2);
  });

  it('converts mouse adapter errors into structured failures without hiding the overlay', async () => {
    const { adapter, executor, overlay } = createExecutor();
    adapter.failNext = new DesktopInputError('adapter_failure', 'Native input failed.');

    await expect(executor.execute(command('mouse.click', { button: 'left' }))).resolves.toEqual({
      ok: false,
      code: 'adapter_failure',
      message: 'Native input failed.'
    });
    expect(overlay.events).toHaveLength(0);
    expect(overlay.activeCount).toBe(1);
    expect(overlay.hideCount).toBe(0);
  });

  it('converts non-mouse adapter errors into structured failures after hiding the overlay', async () => {
    const { adapter, executor, overlay } = createExecutor();
    adapter.failNext = new DesktopInputError('adapter_failure', 'Native input failed.');

    await expect(executor.execute(command('keyboard.key', { key: 'Enter' }))).resolves.toEqual({
      ok: false,
      code: 'adapter_failure',
      message: 'Native input failed.'
    });
    expect(overlay.events).toHaveLength(0);
    expect(overlay.activeCount).toBe(0);
    expect(overlay.hideCount).toBe(1);
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

  it('coalesces no-response pointer movement commands', async () => {
    const { adapter, executor } = createExecutor();
    adapter.mouseMoveDelayMs = 10;

    await Promise.all([
      executor.execute(command('mouse.move', { dx: 1, dy: 0 }, { responseMode: 'none' })),
      executor.execute(command('mouse.move', { dx: 2, dy: 0 }, { responseMode: 'none' })),
      executor.execute(command('mouse.move', { dx: 3, dy: 0 }, { responseMode: 'none' }))
    ]);

    expect(adapter.maxActiveMouseMoves).toBe(1);
    expect(adapter.calls.length).toBeLessThan(3);
    expect(adapter.calls).toEqual([
      { method: 'moveMouseBy', args: [{ dx: 6, dy: 0 }] }
    ]);
  });

  it('keeps drag actions ordered around no-response pointer movement', async () => {
    const { adapter, executor } = createExecutor();
    adapter.mouseMoveDelayMs = 10;

    await Promise.all([
      executor.execute(command('mouse.dragStart', { button: 'left' })),
      executor.execute(command('mouse.move', { dx: 1, dy: 0 }, { responseMode: 'none' })),
      executor.execute(command('mouse.move', { dx: 2, dy: 0 }, { responseMode: 'none' })),
      executor.execute(command('mouse.dragEnd', { button: 'left' }))
    ]);

    expect(adapter.maxActiveMouseMoves).toBe(1);
    expect(adapter.calls).toEqual([
      { method: 'setMouseButtonDown', args: ['left', true] },
      { method: 'moveMouseBy', args: [{ dx: 3, dy: 0 }] },
      { method: 'setMouseButtonDown', args: ['left', false] }
    ]);
  });

  it('maps drag start and end to held mouse button actions', async () => {
    const { adapter, executor, overlay } = createExecutor();

    await executor.execute(command('mouse.dragStart', { button: 'left' }));
    await executor.execute(command('mouse.move', { dx: 10, dy: 0 }));
    await executor.execute(command('mouse.dragEnd', { button: 'left' }));

    expect(adapter.calls).toEqual([
      { method: 'setMouseButtonDown', args: ['left', true] },
      { method: 'moveMouseBy', args: [{ dx: 10, dy: 0 }] },
      { method: 'setMouseButtonDown', args: ['left', false] }
    ]);
    expect(overlay.events).toEqual(['drag', 'drag', 'move']);
    expect(overlay.dragActiveChanges).toEqual([true, false]);
    expect(overlay.activeCount).toBe(3);
    expect(overlay.hideCount).toBe(0);
  });

  it('uses the drag overlay while moving with a held mouse button', async () => {
    const { executor, overlay } = createExecutor();

    await executor.execute(command('mouse.dragStart', { button: 'left' }));
    await executor.execute(command('mouse.move', { dx: 5, dy: 0 }));
    await executor.execute(command('mouse.dragEnd', { button: 'left' }));
    await executor.execute(command('mouse.move', { dx: 5, dy: 0 }));

    expect(overlay.events).toEqual(['drag', 'drag', 'move', 'move']);
    expect(overlay.dragActiveChanges).toEqual([true, false]);
  });

  it('serializes drag start, movement, and drag end pointer actions', async () => {
    const { adapter, executor } = createExecutor();
    adapter.mouseMoveDelayMs = 10;

    await Promise.all([
      executor.execute(command('mouse.dragStart', { button: 'left' })),
      executor.execute(command('mouse.move', { dx: 1, dy: 0 })),
      executor.execute(command('mouse.dragEnd', { button: 'left' }))
    ]);

    expect(adapter.maxActiveMouseMoves).toBe(1);
    expect(adapter.calls).toEqual([
      { method: 'setMouseButtonDown', args: ['left', true] },
      { method: 'moveMouseBy', args: [{ dx: 1, dy: 0 }] },
      { method: 'setMouseButtonDown', args: ['left', false] }
    ]);
  });

  it('treats repeated drag start and drag end without active drag as no-ops', async () => {
    const { adapter, executor } = createExecutor();

    await executor.execute(command('mouse.dragStart', { button: 'left' }));
    await executor.execute(command('mouse.dragStart', { button: 'left' }));
    await executor.execute(command('mouse.dragEnd', { button: 'left' }));
    await executor.execute(command('mouse.dragEnd', { button: 'left' }));

    expect(adapter.calls).toEqual([
      { method: 'setMouseButtonDown', args: ['left', true] },
      { method: 'setMouseButtonDown', args: ['left', false] }
    ]);
  });

  it('releases an active drag button before starting a different drag button', async () => {
    const { adapter, executor } = createExecutor();

    await executor.execute(command('mouse.dragStart', { button: 'left' }));
    await executor.execute(command('mouse.dragStart', { button: 'right' }));

    expect(adapter.calls).toEqual([
      { method: 'setMouseButtonDown', args: ['left', true] },
      { method: 'setMouseButtonDown', args: ['left', false] },
      { method: 'setMouseButtonDown', args: ['right', true] }
    ]);
  });

  it('releases the active drag button and clears the drag overlay during cleanup', async () => {
    const { adapter, executor, overlay } = createExecutor();

    await executor.execute(command('mouse.dragStart', { button: 'left' }));
    await executor.releaseHeldMouseButtons();
    await executor.releaseHeldMouseButtons();

    expect(adapter.calls).toEqual([
      { method: 'setMouseButtonDown', args: ['left', true] },
      { method: 'setMouseButtonDown', args: ['left', false] }
    ]);
    expect(overlay.dragActiveChanges).toEqual([true, false]);
    expect(overlay.hideCount).toBe(1);
  });

  it('converts drag adapter errors into structured failures', async () => {
    const { adapter, executor } = createExecutor();
    adapter.failNext = new DesktopInputError('adapter_failure', 'Native drag failed.');

    await expect(executor.execute(command('mouse.dragStart', { button: 'left' }))).resolves.toEqual({
      ok: false,
      code: 'adapter_failure',
      message: 'Native drag failed.'
    });

    expect(adapter.calls).toHaveLength(0);
  });
});

function createExecutor(): { adapter: FakeInputAdapter; overlay: FakeCursorOverlay; executor: DesktopCommandExecutor } {
  const adapter = new FakeInputAdapter();
  const overlay = new FakeCursorOverlay();
  return { adapter, overlay, executor: new DesktopCommandExecutor(adapter, overlay) };
}

function command<TType extends CommandRequest['type']>(
  type: TType,
  payload: Extract<CommandRequest, { type: TType }>['payload'],
  overrides: Partial<Extract<CommandRequest, { type: TType }>> = {}
): Extract<CommandRequest, { type: TType }> {
  return {
    version: PROTOCOL_VERSION,
    id: 'request-1',
    deviceId: 'android-1',
    timestamp: 1,
    type,
    payload,
    auth: 'proof',
    ...overrides
  } as Extract<CommandRequest, { type: TType }>;
}
