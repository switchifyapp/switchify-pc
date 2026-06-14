import { describe, expect, it } from 'vitest';
import { bluetoothPrimaryCopy } from './bluetooth-primary';

describe('bluetoothPrimaryCopy', () => {
  it('formats ready Bluetooth state without manual connection language', () => {
    expect(bluetoothPrimaryCopy('ready-to-pair', { status: 'ready', reason: null, connectedClientCount: 0, lastError: null })).toEqual({
      title: 'Ready for Bluetooth',
      body: 'Open Switchify on your Android device while you are near this PC.',
      tone: 'ready'
    });
  });

  it('formats starting state', () => {
    expect(bluetoothPrimaryCopy('starting', { status: 'starting', reason: null, connectedClientCount: 0, lastError: null }).title).toBe(
      'Getting Bluetooth ready...'
    );
  });

  it('formats adapter off state', () => {
    expect(
      bluetoothPrimaryCopy('ready-to-pair', { status: 'unavailable', reason: 'adapter_off', connectedClientCount: 0, lastError: null })
    ).toMatchObject({
      title: 'Bluetooth needs attention',
      body: 'Turn on Bluetooth on this PC, then refresh.'
    });
  });

  it('keeps adapter off copy when control status is already error', () => {
    expect(
      bluetoothPrimaryCopy('server-error', { status: 'unavailable', reason: 'adapter_off', connectedClientCount: 0, lastError: null })
    ).toMatchObject({
      title: 'Bluetooth needs attention',
      body: 'Turn on Bluetooth on this PC, then refresh.'
    });
  });

  it('formats permission denied state', () => {
    expect(
      bluetoothPrimaryCopy('ready-to-pair', {
        status: 'unavailable',
        reason: 'permission_denied',
        connectedClientCount: 0,
        lastError: null
      })
    ).toMatchObject({
      title: 'Bluetooth permission denied',
      body: 'Allow Bluetooth access for Switchify PC, then restart the app.'
    });
  });

  it('formats connected state', () => {
    expect(bluetoothPrimaryCopy('connected', { status: 'connected', reason: null, connectedClientCount: 1, lastError: null })).toMatchObject({
      title: 'Your device is connected',
      body: 'You can control this PC from Switchify over Bluetooth.'
    });
  });

  it('does not include QR or local-network copy', () => {
    const copy = bluetoothPrimaryCopy('ready-to-pair', { status: 'ready', reason: null, connectedClientCount: 0, lastError: null });

    expect(`${copy.title} ${copy.body}`).not.toMatch(/QR|manual|local network|address/i);
  });
});

