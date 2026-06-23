export {};

import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type { PairedDeviceView, PcControlStatus } from '../shared/server-status';
import type { CursorOverlaySettings } from '../shared/cursor-overlay-settings';
import type { PointerMovementSettings } from '../shared/pointer-movement-settings';
import type { SettingsSectionId } from '../shared/settings';
import type { SystemStartupSettings } from '../shared/system-startup';
import type { UpdateState } from '../shared/update';

declare global {
  interface Window {
    switchifyPc: {
      appName: string;
      minimizeWindow: () => Promise<void>;
      closeWindow: () => Promise<void>;
      getServerStatus: () => Promise<PcControlStatus>;
      getPairedDevices: () => Promise<PairedDeviceView[]>;
      disconnectClients: () => Promise<PcControlStatus>;
      forgetPairedDevice: (
        deviceId: string
      ) => Promise<
        { ok: true; pairedDevices: PairedDeviceView[]; status: PcControlStatus } | { ok: false; reason: string }
      >;
      getCursorOverlayEnabled: () => Promise<boolean>;
      setCursorOverlayEnabled: (enabled: boolean) => Promise<boolean>;
      getCursorOverlaySettings: () => Promise<CursorOverlaySettings>;
      setCursorOverlaySettings: (settings: CursorOverlaySettings) => Promise<CursorOverlaySettings>;
      getPointerMovementSettings: () => Promise<PointerMovementSettings>;
      setPointerMovementSettings: (settings: PointerMovementSettings) => Promise<PointerMovementSettings>;
      openSettingsWindow: (section?: SettingsSectionId) => Promise<void>;
      onShowSettingsSection: (handler: (section: SettingsSectionId) => void) => () => void;
      openExternalUrl: (url: string) => Promise<{ ok: boolean }>;
      getPendingPairingRequests: () => Promise<PendingPairingApprovalView[]>;
      respondToPairingRequest: (
        requestId: string,
        decision: PairingApprovalDecision
      ) => Promise<{ ok: boolean; reason?: string }>;
      getUpdateState: () => Promise<UpdateState>;
      checkForUpdates: () => Promise<UpdateState>;
      downloadUpdate: () => Promise<UpdateState>;
      getSystemStartupSettings: () => Promise<SystemStartupSettings>;
      setStartWithSystem: (enabled: boolean) => Promise<SystemStartupSettings>;
      installDownloadedUpdate: () => Promise<{ ok: boolean; reason?: string }>;
    };
  }
}
