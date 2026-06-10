import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import {
  SWITCHIFY_FIREWALL_MDNS_RULE_NAME,
  SWITCHIFY_FIREWALL_RULE_GROUP,
  SWITCHIFY_FIREWALL_TCP_RULE_NAME,
  type FirewallDiagnostics,
  type FirewallRepairResult,
  type FirewallRuleProtocol,
  type FirewallRuleStatus,
  type NetworkProfileCategory,
  type NetworkProfileStatus
} from '../shared/firewall';

const execFileAsync = promisify(execFile);
const POWERSHELL_EXE = 'powershell.exe';
const MDNS_PORT = 5353;

export type WindowsFirewallOptions = {
  appPath: string;
  port: number;
};

type FirewallRuleSnapshot = {
  displayName: string;
  displayGroup: string | null;
  enabled: boolean;
  direction: string | null;
  action: string | null;
  profile: string | null;
  protocol: string | null;
  localPort: string | null;
  program: string | null;
};

type FirewallDiagnosticsSnapshot = {
  rules: FirewallRuleSnapshot[];
  networkProfiles: NetworkProfileStatus[];
};

type ExpectedRule = {
  displayName: string;
  protocol: FirewallRuleProtocol;
  localPort: number;
};

const EXPECTED_RULES: ExpectedRule[] = [
  { displayName: SWITCHIFY_FIREWALL_TCP_RULE_NAME, protocol: 'TCP', localPort: 0 },
  { displayName: SWITCHIFY_FIREWALL_MDNS_RULE_NAME, protocol: 'UDP', localPort: MDNS_PORT }
];

export async function getFirewallDiagnostics(options: WindowsFirewallOptions): Promise<FirewallDiagnostics> {
  if (process.platform !== 'win32') {
    return unsupportedDiagnostics(null);
  }

  try {
    return buildFirewallDiagnostics(await readFirewallSnapshot(), options);
  } catch (error) {
    return unsupportedDiagnostics(toErrorMessage(error));
  }
}

export async function repairFirewall(options: WindowsFirewallOptions): Promise<FirewallRepairResult> {
  if (process.platform !== 'win32') {
    return { ok: false, reason: 'unsupported_platform', diagnostics: unsupportedDiagnostics(null) };
  }

  const before = await getFirewallDiagnostics(options);
  if (!before.supported) {
    return { ok: false, reason: 'unsupported_platform', diagnostics: before };
  }

  const scriptDir = await mkdtemp(join(tmpdir(), 'switchify-firewall-'));
  const scriptPath = join(scriptDir, 'repair-firewall.ps1');

  try {
    await writeFile(scriptPath, createRepairScript(options), 'utf8');
    await runElevatedPowerShellScript(scriptPath);
    return { ok: true, diagnostics: await getFirewallDiagnostics(options) };
  } catch (error) {
    const diagnostics = await getFirewallDiagnostics(options);
    return {
      ok: false,
      reason: isElevationCancelled(error) ? 'elevation_cancelled' : 'repair_failed',
      diagnostics
    };
  } finally {
    await rm(scriptDir, { recursive: true, force: true });
  }
}

export function buildFirewallDiagnostics(
  snapshot: FirewallDiagnosticsSnapshot,
  options: WindowsFirewallOptions
): FirewallDiagnostics {
  const rules = createExpectedRules(options.port).map((rule) => toFirewallRuleStatus(snapshot.rules, rule, options.appPath));

  return {
    supported: true,
    rules,
    networkProfiles: snapshot.networkProfiles,
    needsRepair: rules.some((rule) => !rule.present),
    lastError: null
  };
}

function createExpectedRules(port: number): ExpectedRule[] {
  return EXPECTED_RULES.map((rule) => ({
    ...rule,
    localPort: rule.localPort === 0 ? port : rule.localPort
  }));
}

function toFirewallRuleStatus(
  rules: FirewallRuleSnapshot[],
  expected: ExpectedRule,
  appPath: string
): FirewallRuleStatus {
  const matchingRules = rules.filter((rule) => rule.displayName === expected.displayName);
  const validRule = matchingRules.find((rule) => isValidRule(rule, expected, appPath));
  const bestRule = validRule ?? matchingRules[0];

  return {
    displayName: expected.displayName,
    protocol: expected.protocol,
    localPort: expected.localPort,
    present: Boolean(validRule),
    enabled: bestRule?.enabled ?? false,
    profile: bestRule?.profile ?? null,
    scopedToApp: bestRule ? isProgramScopedToApp(bestRule.program, appPath) : null
  };
}

function isValidRule(rule: FirewallRuleSnapshot, expected: ExpectedRule, appPath: string): boolean {
  return (
    rule.enabled &&
    equalsIgnoreCase(rule.direction, 'Inbound') &&
    equalsIgnoreCase(rule.action, 'Allow') &&
    equalsIgnoreCase(rule.protocol, expected.protocol) &&
    portMatches(rule.localPort, expected.localPort) &&
    profileMatches(rule.profile) &&
    isProgramScopedToApp(rule.program, appPath)
  );
}

function portMatches(value: string | null, expectedPort: number): boolean {
  if (!value) return false;
  return value
    .split(',')
    .map((part) => part.trim())
    .includes(String(expectedPort));
}

function profileMatches(value: string | null): boolean {
  if (!value) return false;
  const normalized = value.toLowerCase();
  return normalized === 'any' || normalized === 'domain, private, public' || normalized === 'domain,private,public';
}

function isProgramScopedToApp(value: string | null, appPath: string): boolean {
  if (!value) return false;
  return value.toLowerCase() === appPath.toLowerCase();
}

function equalsIgnoreCase(left: string | null, right: string): boolean {
  return left?.toLowerCase() === right.toLowerCase();
}

function unsupportedDiagnostics(lastError: string | null): FirewallDiagnostics {
  return {
    supported: false,
    rules: createExpectedRules(0).map((rule) => ({
      displayName: rule.displayName,
      protocol: rule.protocol,
      localPort: rule.localPort,
      present: false,
      enabled: false,
      profile: null,
      scopedToApp: null
    })),
    networkProfiles: [],
    needsRepair: false,
    lastError
  };
}

async function readFirewallSnapshot(): Promise<FirewallDiagnosticsSnapshot> {
  const { stdout } = await execFileAsync(POWERSHELL_EXE, [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-Command',
    createDiagnosticsScript()
  ]);
  return normalizeDiagnosticsSnapshot(JSON.parse(stdout));
}

function createDiagnosticsScript(): string {
  const group = psString(SWITCHIFY_FIREWALL_RULE_GROUP);
  const tcpName = psString(SWITCHIFY_FIREWALL_TCP_RULE_NAME);
  const mdnsName = psString(SWITCHIFY_FIREWALL_MDNS_RULE_NAME);

  return `
$ErrorActionPreference = 'Stop'
if (-not (Get-Command Get-NetFirewallRule -ErrorAction SilentlyContinue)) {
  throw 'NetSecurity unavailable'
}
$names = @(${tcpName}, ${mdnsName})
$rules = @(
  Get-NetFirewallRule -DisplayGroup ${group} -ErrorAction SilentlyContinue
  Get-NetFirewallRule -Group ${group} -ErrorAction SilentlyContinue
  foreach ($name in $names) {
    Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue
  }
) | Where-Object { $_ -ne $null } | Sort-Object -Property Name -Unique
$ruleViews = foreach ($rule in $rules) {
  $portFilter = $rule | Get-NetFirewallPortFilter -ErrorAction SilentlyContinue
  $appFilter = $rule | Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue
  [PSCustomObject]@{
    displayName = [string]$rule.DisplayName
    displayGroup = if ($rule.DisplayGroup) { [string]$rule.DisplayGroup } elseif ($rule.Group) { [string]$rule.Group } else { $null }
    enabled = $rule.Enabled -eq 'True'
    direction = [string]$rule.Direction
    action = [string]$rule.Action
    profile = [string]$rule.Profile
    protocol = if ($portFilter) { [string]$portFilter.Protocol } else { $null }
    localPort = if ($portFilter) { [string]$portFilter.LocalPort } else { $null }
    program = if ($appFilter -and $appFilter.Program -and $appFilter.Program -ne 'Any') { [string]$appFilter.Program } else { $null }
  }
}
$profiles = @(Get-NetConnectionProfile -ErrorAction SilentlyContinue | ForEach-Object {
  [PSCustomObject]@{
    name = if ($_.Name) { [string]$_.Name } else { 'Unknown network' }
    category = if ($_.NetworkCategory) { [string]$_.NetworkCategory } else { 'Unknown' }
  }
})
[PSCustomObject]@{
  rules = @($ruleViews)
  networkProfiles = @($profiles)
} | ConvertTo-Json -Depth 5
`;
}

function normalizeDiagnosticsSnapshot(value: unknown): FirewallDiagnosticsSnapshot {
  const input = value as Partial<FirewallDiagnosticsSnapshot> | null;
  return {
    rules: asArray(input?.rules).map(normalizeRuleSnapshot),
    networkProfiles: asArray(input?.networkProfiles).map(normalizeNetworkProfile)
  };
}

function normalizeRuleSnapshot(value: unknown): FirewallRuleSnapshot {
  const rule = value as Partial<FirewallRuleSnapshot> | null;
  return {
    displayName: String(rule?.displayName ?? ''),
    displayGroup: nullableString(rule?.displayGroup),
    enabled: Boolean(rule?.enabled),
    direction: nullableString(rule?.direction),
    action: nullableString(rule?.action),
    profile: nullableString(rule?.profile),
    protocol: nullableString(rule?.protocol),
    localPort: nullableString(rule?.localPort),
    program: nullableString(rule?.program)
  };
}

function normalizeNetworkProfile(value: unknown): NetworkProfileStatus {
  const profile = value as Partial<NetworkProfileStatus> | null;
  return {
    name: String(profile?.name ?? 'Unknown network'),
    category: normalizeNetworkCategory(profile?.category)
  };
}

function normalizeNetworkCategory(value: unknown): NetworkProfileCategory {
  if (value === 'Public' || value === 'Private' || value === 'DomainAuthenticated') {
    return value;
  }
  return 'Unknown';
}

function nullableString(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function asArray<T>(value: T[] | T | undefined): T[] {
  if (!value) return [];
  return Array.isArray(value) ? value : [value];
}

function createRepairScript(options: WindowsFirewallOptions): string {
  const appPath = psString(options.appPath);
  const group = psString(SWITCHIFY_FIREWALL_RULE_GROUP);
  const tcpName = psString(SWITCHIFY_FIREWALL_TCP_RULE_NAME);
  const mdnsName = psString(SWITCHIFY_FIREWALL_MDNS_RULE_NAME);

  return `
$ErrorActionPreference = 'Stop'
$names = @(${tcpName}, ${mdnsName})
Get-NetFirewallRule -DisplayGroup ${group} -ErrorAction SilentlyContinue | Remove-NetFirewallRule
Get-NetFirewallRule -Group ${group} -ErrorAction SilentlyContinue | Remove-NetFirewallRule
foreach ($name in $names) {
  Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue | Remove-NetFirewallRule
}
New-NetFirewallRule -DisplayName ${tcpName} -Group ${group} -Direction Inbound -Action Allow -Program ${appPath} -Protocol TCP -LocalPort ${options.port} -Profile Any | Out-Null
New-NetFirewallRule -DisplayName ${mdnsName} -Group ${group} -Direction Inbound -Action Allow -Program ${appPath} -Protocol UDP -LocalPort ${MDNS_PORT} -Profile Any | Out-Null
`;
}

async function runElevatedPowerShellScript(scriptPath: string): Promise<void> {
  const command = [
    '$process = Start-Process',
    `-FilePath ${psString(POWERSHELL_EXE)}`,
    `-ArgumentList ${psString(`-NoProfile -ExecutionPolicy Bypass -File "${scriptPath}"`)}`,
    '-Verb RunAs',
    '-Wait',
    '-PassThru;',
    'exit $process.ExitCode'
  ].join(' ');

  await execFileAsync(POWERSHELL_EXE, ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', command]);
}

function psString(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
}

function isElevationCancelled(error: unknown): boolean {
  const message = toErrorMessage(error).toLowerCase();
  return message.includes('canceled') || message.includes('cancelled') || message.includes('operation was canceled');
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
