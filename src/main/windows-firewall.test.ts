import { describe, expect, it } from 'vitest';
import {
  SWITCHIFY_FIREWALL_MDNS_RULE_NAME,
  SWITCHIFY_FIREWALL_TCP_RULE_NAME
} from '../shared/firewall';
import { buildFirewallDiagnostics } from './windows-firewall';

const APP_PATH = 'C:\\Program Files\\Switchify PC\\Switchify PC.exe';

describe('buildFirewallDiagnostics', () => {
  it('reports ready when expected rules are enabled and scoped to the app', () => {
    const diagnostics = buildFirewallDiagnostics(
      {
        rules: [tcpRule(), mdnsRule()],
        networkProfiles: [{ name: 'Wi-Fi', category: 'Private' }]
      },
      { appPath: APP_PATH, port: 7347 }
    );

    expect(diagnostics.supported).toBe(true);
    expect(diagnostics.needsRepair).toBe(false);
    expect(diagnostics.rules.every((rule) => rule.present)).toBe(true);
  });

  it('requires repair when the TCP rule is missing', () => {
    const diagnostics = buildFirewallDiagnostics(
      {
        rules: [mdnsRule()],
        networkProfiles: []
      },
      { appPath: APP_PATH, port: 7347 }
    );

    expect(diagnostics.needsRepair).toBe(true);
    expect(diagnostics.rules.find((rule) => rule.protocol === 'TCP')?.present).toBe(false);
  });

  it('requires repair when the mDNS rule is missing', () => {
    const diagnostics = buildFirewallDiagnostics(
      {
        rules: [tcpRule()],
        networkProfiles: []
      },
      { appPath: APP_PATH, port: 7347 }
    );

    expect(diagnostics.needsRepair).toBe(true);
    expect(diagnostics.rules.find((rule) => rule.protocol === 'UDP')?.present).toBe(false);
  });

  it('requires repair when a rule is disabled', () => {
    const diagnostics = buildFirewallDiagnostics(
      {
        rules: [tcpRule({ enabled: false }), mdnsRule()],
        networkProfiles: []
      },
      { appPath: APP_PATH, port: 7347 }
    );

    const tcp = diagnostics.rules.find((rule) => rule.protocol === 'TCP');
    expect(diagnostics.needsRepair).toBe(true);
    expect(tcp?.present).toBe(false);
    expect(tcp?.enabled).toBe(false);
  });

  it('requires repair when a rule is scoped to a stale app path', () => {
    const diagnostics = buildFirewallDiagnostics(
      {
        rules: [tcpRule({ program: 'C:\\Old\\Switchify PC.exe' }), mdnsRule()],
        networkProfiles: []
      },
      { appPath: APP_PATH, port: 7347 }
    );

    const tcp = diagnostics.rules.find((rule) => rule.protocol === 'TCP');
    expect(diagnostics.needsRepair).toBe(true);
    expect(tcp?.present).toBe(false);
    expect(tcp?.scopedToApp).toBe(false);
  });

  it('surfaces Public network profiles', () => {
    const diagnostics = buildFirewallDiagnostics(
      {
        rules: [tcpRule(), mdnsRule()],
        networkProfiles: [{ name: 'Home Wi-Fi', category: 'Public' }]
      },
      { appPath: APP_PATH, port: 7347 }
    );

    expect(diagnostics.networkProfiles).toEqual([{ name: 'Home Wi-Fi', category: 'Public' }]);
  });
});

function tcpRule(overrides = {}) {
  return rule({
    displayName: SWITCHIFY_FIREWALL_TCP_RULE_NAME,
    protocol: 'TCP',
    localPort: '7347',
    ...overrides
  });
}

function mdnsRule(overrides = {}) {
  return rule({
    displayName: SWITCHIFY_FIREWALL_MDNS_RULE_NAME,
    protocol: 'UDP',
    localPort: '5353',
    ...overrides
  });
}

function rule(overrides = {}) {
  return {
    displayName: SWITCHIFY_FIREWALL_TCP_RULE_NAME,
    displayGroup: 'Switchify PC',
    enabled: true,
    direction: 'Inbound',
    action: 'Allow',
    profile: 'Any',
    protocol: 'TCP',
    localPort: '7347',
    program: APP_PATH,
    ...overrides
  };
}
