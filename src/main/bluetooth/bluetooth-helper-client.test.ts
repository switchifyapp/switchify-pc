import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { describe, expect, it, vi } from 'vitest';
import { BLUETOOTH_FRAME_VERSION } from '../../shared/bluetooth-frame';
import { BluetoothHelperClient } from './bluetooth-helper-client';
import type { BluetoothHelperEvent } from './helper-protocol';

describe('BluetoothHelperClient', () => {
  it('writes start commands and parses helper events', () => {
    const helper = createFakeProcess();
    const events: BluetoothHelperEvent[] = [];
    const client = new BluetoothHelperClient({
      helperPath: 'package.json',
      spawnProcess: () => helper.process,
      onEvent: (event) => events.push(event)
    });

    expect(
      client.start({
        type: 'start',
        serviceUuid: 'service',
        rxCharacteristicUuid: 'rx',
        txCharacteristicUuid: 'tx',
        statusCharacteristicUuid: 'status',
        displayName: 'Switchify PC',
        desktopId: 'desktop-1'
      })
    ).toBe(true);
    helper.stdout.write(`${JSON.stringify({ type: 'ready' })}\n`);

    expect(helper.stdinText()).toContain('"type":"start"');
    expect(events).toEqual([{ type: 'ready' }]);
  });

  it('writes framed send commands', () => {
    const helper = createFakeProcess();
    const client = new BluetoothHelperClient({
      helperPath: 'package.json',
      spawnProcess: () => helper.process,
      onEvent: vi.fn()
    });

    client.start({
      type: 'start',
      serviceUuid: 'service',
      rxCharacteristicUuid: 'rx',
      txCharacteristicUuid: 'tx',
      statusCharacteristicUuid: 'status',
      displayName: 'Switchify PC',
      desktopId: 'desktop-1'
    });
    client.send('ble', {
      version: BLUETOOTH_FRAME_VERSION,
      messageId: 'message-1',
      sequence: 0,
      isFinal: true,
      totalBytes: 2,
      payloadBase64: 'e30='
    });

    expect(helper.stdinText()).toContain('"type":"send"');
    expect(helper.stdinText()).toContain('"connectionId":"ble"');
  });

  it('parses diagnostic and disconnected events', () => {
    const helper = createFakeProcess();
    const events: BluetoothHelperEvent[] = [];
    const client = new BluetoothHelperClient({
      helperPath: 'package.json',
      spawnProcess: () => helper.process,
      onEvent: (event) => events.push(event)
    });

    client.start({
      type: 'start',
      serviceUuid: 'service',
      rxCharacteristicUuid: 'rx',
      txCharacteristicUuid: 'tx',
      statusCharacteristicUuid: 'status',
      displayName: 'Switchify PC',
      desktopId: 'desktop-1'
    });
    helper.stdout.write(`${JSON.stringify({ type: 'diagnostic', event: 'subscribed' })}\n`);
    helper.stdout.write(`${JSON.stringify({ type: 'diagnostic', event: 'unsubscribed' })}\n`);
    helper.stdout.write(`${JSON.stringify({ type: 'diagnostic', event: 'unsubscribe_grace_timed_out' })}\n`);
    helper.stdout.write(`${JSON.stringify({ type: 'disconnected', connectionId: 'ble', reason: 'notification_unsubscribed' })}\n`);

    expect(events).toEqual([
      { type: 'diagnostic', event: 'subscribed' },
      { type: 'diagnostic', event: 'unsubscribed' },
      { type: 'diagnostic', event: 'unsubscribe_grace_timed_out' },
      { type: 'disconnected', connectionId: 'ble', reason: 'notification_unsubscribed' }
    ]);
  });

  it('treats malformed diagnostic events as safe helper failure', () => {
    const helper = createFakeProcess();
    const failures: string[] = [];
    const client = new BluetoothHelperClient({
      helperPath: 'package.json',
      spawnProcess: () => helper.process,
      onEvent: vi.fn(),
      onFailure: (message) => failures.push(message)
    });

    client.start({
      type: 'start',
      serviceUuid: 'service',
      rxCharacteristicUuid: 'rx',
      txCharacteristicUuid: 'tx',
      statusCharacteristicUuid: 'status',
      displayName: 'Switchify PC',
      desktopId: 'desktop-1'
    });
    helper.stdout.write(`${JSON.stringify({ type: 'diagnostic', event: 'payload:secret' })}\n`);

    expect(failures).toEqual(['Bluetooth helper returned malformed status output.']);
    expect(helper.kill).toHaveBeenCalledTimes(1);
  });
});

function createFakeProcess() {
  const stdin = new PassThrough();
  const stdout = new PassThrough();
  const stderr = new PassThrough();
  const chunks: string[] = [];
  const kill = vi.fn(() => {
    process.killed = true;
    return true;
  });
  stdin.on('data', (chunk) => chunks.push(String(chunk)));

  const process = Object.assign(new EventEmitter(), {
    stdin,
    stdout,
    stderr,
    killed: false,
    kill
  });

  return {
    process: process as never,
    stdout,
    kill,
    stdinText: () => chunks.join('')
  };
}
