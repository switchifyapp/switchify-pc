import { describe, expect, it } from 'vitest';
import { createToken, PairingManager } from './pairing-manager';
import { MemoryPairingStore } from './pairing-store';

describe('PairingManager', () => {
  it('creates a persistent desktop id on first load', async () => {
    const store = new MemoryPairingStore();
    const manager = new PairingManager(store);

    const first = await manager.getDesktopId();
    const second = await manager.getDesktopId();

    expect(first).toHaveLength(36);
    expect(second).toBe(first);
  });

  it('creates shared tokens', () => {
    expect(createToken().length).toBeGreaterThan(20);
    expect(createToken()).not.toBe(createToken());
  });
});
