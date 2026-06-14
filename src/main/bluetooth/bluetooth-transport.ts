import { join } from 'node:path';
import { app } from 'electron';
import {
  BluetoothFrameReassembler,
  createBluetoothFrames,
  type BluetoothFrame
} from '../../shared/bluetooth-frame';
import { type BluetoothStatus, type BluetoothUnavailableReason } from '../../shared/bluetooth-status';
import type { RemoteConnection } from '../transport/remote-connection';
import type { PcWebSocketServer } from '../websocket/server';
import {
  SWITCHIFY_BLE_RX_CHARACTERISTIC_UUID,
  SWITCHIFY_BLE_SERVICE_UUID,
  SWITCHIFY_BLE_STATUS_CHARACTERISTIC_UUID,
  SWITCHIFY_BLE_TX_CHARACTERISTIC_UUID
} from './constants';
import { BluetoothHelperClient } from './bluetooth-helper-client';
import type { BluetoothHelperEvent } from './helper-protocol';

export type WindowsBluetoothTransportOptions = {
  server: PcWebSocketServer;
  getDesktopId: () => Promise<string>;
  displayName: string;
  helperPath?: string;
  createHelper?: (options: ConstructorParameters<typeof BluetoothHelperClient>[0]) => BluetoothHelperClient;
  onStatusChange?: (status: BluetoothStatus) => void;
};

type BluetoothConnectionState = {
  connection: RemoteConnection;
  reassembler: BluetoothFrameReassembler;
};

export class WindowsBluetoothTransport {
  private readonly connections = new Map<string, BluetoothConnectionState>();
  private helper: BluetoothHelperClient | null = null;
  private status: BluetoothStatus = {
    status: process.platform === 'win32' ? 'stopped' : 'disabled',
    reason: null,
    connectedClientCount: 0,
    lastError: null
  };

  constructor(private readonly options: WindowsBluetoothTransportOptions) {}

  getStatus(): BluetoothStatus {
    return { ...this.status };
  }

  async start(): Promise<BluetoothStatus> {
    if (process.platform !== 'win32') {
      return this.setUnavailable('unsupported');
    }
    if (this.helper) return this.getStatus();

    this.setStatus({ status: 'starting', reason: null, lastError: null });
    this.helper = (this.options.createHelper ?? ((options) => new BluetoothHelperClient(options)))({
      helperPath: this.options.helperPath ?? defaultBluetoothHelperPath(),
      onEvent: (event) => {
        void this.handleEvent(event);
      },
      onFailure: (message) => {
        this.setStatus({ status: 'error', lastError: safeBluetoothError(message) });
      }
    });

    const started = this.helper.start({
      type: 'start',
      serviceUuid: SWITCHIFY_BLE_SERVICE_UUID,
      rxCharacteristicUuid: SWITCHIFY_BLE_RX_CHARACTERISTIC_UUID,
      txCharacteristicUuid: SWITCHIFY_BLE_TX_CHARACTERISTIC_UUID,
      statusCharacteristicUuid: SWITCHIFY_BLE_STATUS_CHARACTERISTIC_UUID,
      displayName: this.options.displayName,
      desktopId: await this.options.getDesktopId()
    });

    if (!started) {
      return this.setUnavailable('startup_failed');
    }
    return this.getStatus();
  }

  stop(): void {
    for (const connectionId of this.connections.keys()) {
      this.removeConnection(connectionId);
    }
    this.helper?.stop();
    this.helper?.destroy();
    this.helper = null;
    this.setStatus({ status: 'stopped', connectedClientCount: 0, reason: null, lastError: null });
  }

  private async handleEvent(event: BluetoothHelperEvent): Promise<void> {
    switch (event.type) {
      case 'ready':
        this.setStatus({ status: 'ready', reason: null, lastError: null });
        return;
      case 'unavailable':
        this.setUnavailable(event.reason);
        return;
      case 'connected':
        this.addConnection(event.connectionId, event.label);
        return;
      case 'message':
        await this.handleFrame(event.connectionId, event.frame);
        return;
      case 'disconnected':
        this.removeConnection(event.connectionId);
        return;
      case 'error':
        this.setStatus({ status: 'error', lastError: safeBluetoothError(event.reason) });
        return;
    }
  }

  private addConnection(connectionId: string, label: string): void {
    if (this.connections.has(connectionId)) return;

    const connection: RemoteConnection = {
      id: connectionId,
      kind: 'bluetooth',
      label,
      remoteAddress: null,
      send: (message) => {
        for (const frame of createBluetoothFrames(message)) {
          this.helper?.send(connectionId, frame);
        }
      },
      close: () => {
        this.helper?.disconnect(connectionId);
      }
    };

    this.connections.set(connectionId, {
      connection,
      reassembler: new BluetoothFrameReassembler()
    });
    this.options.server.addRemoteConnection(connection);
    this.setStatus({ status: 'connected', connectedClientCount: this.connections.size, reason: null });
  }

  private removeConnection(connectionId: string): void {
    if (!this.connections.delete(connectionId)) return;
    this.options.server.removeRemoteConnection(connectionId);
    this.setStatus({
      status: this.connections.size > 0 ? 'connected' : 'ready',
      connectedClientCount: this.connections.size
    });
  }

  private async handleFrame(connectionId: string, frame: BluetoothFrame): Promise<void> {
    const state = this.connections.get(connectionId);
    if (!state) return;

    const result = state.reassembler.accept(frame);
    if (!result.ok) {
      if (result.reason !== 'incomplete') {
        this.setStatus({ status: 'error', lastError: `Bluetooth frame rejected: ${result.reason}.` });
      }
      return;
    }

    await this.options.server.handleRemoteMessage(connectionId, result.message);
  }

  private setUnavailable(reason: BluetoothUnavailableReason): BluetoothStatus {
    return this.setStatus({ status: 'unavailable', reason, connectedClientCount: 0 });
  }

  private setStatus(update: Partial<BluetoothStatus>): BluetoothStatus {
    this.status = { ...this.status, ...update };
    this.options.server.setBluetoothStatus(this.status);
    this.options.onStatusChange?.(this.getStatus());
    return this.getStatus();
  }
}

function defaultBluetoothHelperPath(): string {
  return app.isPackaged
    ? join(process.resourcesPath, 'native', 'SwitchifyBluetoothTransport.exe')
    : join(process.cwd(), 'build', 'native', 'bluetooth-transport-helper', 'win-x64', 'SwitchifyBluetoothTransport.exe');
}

function safeBluetoothError(message: string): string {
  return message.length > 300 ? `${message.slice(0, 297)}...` : message;
}
