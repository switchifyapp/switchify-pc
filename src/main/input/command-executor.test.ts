import { describe, expect, it } from 'vitest';
import type { CommandRequest } from '../../shared/protocol';
import { PROTOCOL_VERSION } from '../../shared/protocol';
import type { DesktopInputAdapter } from './desktop-input-adapter';
import { DesktopInputError } from './desktop-input-adapter';
import { DesktopCommandExecutor } from './command-executor';

type RecordedCall = { method: keyof DesktopInputAdapter; args: unknown[] };

class FakeInputAdapter implements DesktopInputAdapter {
  readonly calls: RecordedCall[] = [];
  failNext: Error | null = null;

  async moveMouseBy(delta: { dx: number; dy: number }): Promise<void> {
    this.record('moveMouseBy', [delta]);
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
    const { adapter, executor } = createExecutor();

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

  it('rejects unsafe movement, scroll, shortcut, and text values', async () => {
    const { adapter, executor } = createExecutor();

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
  });

  it('converts adapter errors into structured failures', async () => {
    const { adapter, executor } = createExecutor();
    adapter.failNext = new DesktopInputError('adapter_failure', 'Native input failed.');

    await expect(executor.execute(command('mouse.click', { button: 'left' }))).resolves.toEqual({
      ok: false,
      code: 'adapter_failure',
      message: 'Native input failed.'
    });
  });
});

function createExecutor(): { adapter: FakeInputAdapter; executor: DesktopCommandExecutor } {
  const adapter = new FakeInputAdapter();
  return { adapter, executor: new DesktopCommandExecutor(adapter) };
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
