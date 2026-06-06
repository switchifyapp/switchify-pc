export const SWITCHIFY_MDNS_SERVICE_TYPE = 'switchify';
export const SWITCHIFY_MDNS_SERVICE_NAME = 'Switchify PC';
export const SWITCHIFY_DISCOVERY_KIND = 'switchify.pc';
export const SWITCHIFY_DISCOVERY_VERSION = '1';
export const SWITCHIFY_DISCOVERY_PROTOCOL_VERSION = '1';

export type SwitchifyDiscoveryTxt = {
  kind: typeof SWITCHIFY_DISCOVERY_KIND;
  version: typeof SWITCHIFY_DISCOVERY_VERSION;
  desktopId: string;
  protocolVersion: typeof SWITCHIFY_DISCOVERY_PROTOCOL_VERSION;
  pairing: 'approval';
};

export function createSwitchifyDiscoveryTxt(desktopId: string): SwitchifyDiscoveryTxt {
  return {
    kind: SWITCHIFY_DISCOVERY_KIND,
    version: SWITCHIFY_DISCOVERY_VERSION,
    desktopId,
    protocolVersion: SWITCHIFY_DISCOVERY_PROTOCOL_VERSION,
    pairing: 'approval'
  };
}
