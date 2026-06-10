export const SWITCHIFY_FIREWALL_RULE_GROUP = 'Switchify PC';
export const SWITCHIFY_FIREWALL_TCP_RULE_NAME = 'Switchify PC (TCP 7347)';
export const SWITCHIFY_FIREWALL_MDNS_RULE_NAME = 'Switchify PC (mDNS UDP 5353)';

export type FirewallRuleProtocol = 'TCP' | 'UDP';

export type FirewallRuleStatus = {
  displayName: string;
  protocol: FirewallRuleProtocol;
  localPort: number;
  present: boolean;
  enabled: boolean;
  profile: string | null;
  scopedToApp: boolean | null;
};

export type NetworkProfileCategory = 'Public' | 'Private' | 'DomainAuthenticated' | 'Unknown';

export type NetworkProfileStatus = {
  name: string;
  category: NetworkProfileCategory;
};

export type FirewallDiagnostics = {
  supported: boolean;
  rules: FirewallRuleStatus[];
  networkProfiles: NetworkProfileStatus[];
  needsRepair: boolean;
  lastError: string | null;
};

export type FirewallRepairResult =
  | { ok: true; diagnostics: FirewallDiagnostics }
  | {
      ok: false;
      reason: 'unsupported_platform' | 'elevation_cancelled' | 'repair_failed';
      diagnostics: FirewallDiagnostics | null;
    };
