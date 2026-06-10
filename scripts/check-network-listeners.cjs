const { execFileSync } = require('node:child_process');

const PORT = 7347;
const FIREWALL_GROUP = 'Switchify PC';

function formatEndpoint(address, port) {
  const normalizedAddress = String(address ?? '');
  if (normalizedAddress.includes(':')) {
    const bracketedAddress = normalizedAddress.startsWith('[') ? normalizedAddress : `[${normalizedAddress}]`;
    return `${bracketedAddress}:${port}`;
  }

  return `${normalizedAddress}:${port}`;
}

if (process.platform !== 'win32') {
  console.log('Windows network diagnostics are only available on Windows.');
  process.exit(0);
}

const script = `
$ErrorActionPreference = 'Stop'

function To-JsonLine($value) {
  $value | ConvertTo-Json -Compress -Depth 6
}

$listeners = Get-NetTCPConnection -LocalPort ${PORT} -State Listen -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{
    LocalAddress = [string]$_.LocalAddress
    LocalPort = [int]$_.LocalPort
    State = $_.State.ToString()
    OwningProcess = [int]$_.OwningProcess
  }
}

$rules = Get-NetFirewallRule -DisplayGroup '${FIREWALL_GROUP}' -ErrorAction SilentlyContinue | ForEach-Object {
  $rule = $_
  $portFilter = $rule | Get-NetFirewallPortFilter -ErrorAction SilentlyContinue
  $appFilter = $rule | Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue
  [pscustomobject]@{
    DisplayName = $rule.DisplayName
    Enabled = [string]$rule.Enabled
    Direction = [string]$rule.Direction
    Action = [string]$rule.Action
    Profile = [string]$rule.Profile
    Protocol = if ($portFilter) { [string]$portFilter.Protocol } else { $null }
    LocalPort = if ($portFilter) { [string]$portFilter.LocalPort } else { $null }
    Program = if ($appFilter) { [string]$appFilter.Program } else { $null }
  }
}

$profiles = Get-NetConnectionProfile -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{
    Name = [string]$_.Name
    InterfaceAlias = [string]$_.InterfaceAlias
    NetworkCategory = $_.NetworkCategory.ToString()
    IPv4Connectivity = $_.IPv4Connectivity.ToString()
    IPv6Connectivity = $_.IPv6Connectivity.ToString()
  }
}

To-JsonLine ([pscustomobject]@{
  Listeners = @($listeners)
  FirewallRules = @($rules)
  NetworkProfiles = @($profiles)
})
`;

try {
  const output = execFileSync(
    'powershell.exe',
    ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', script],
    { encoding: 'utf8', windowsHide: true }
  ).trim();
  const diagnostics = JSON.parse(output || '{}');

  console.log('Switchify PC network diagnostics');
  console.log('Listeners:');
  const listeners = diagnostics.Listeners ?? [];
  if (listeners.length === 0) {
    console.log(`- No TCP listeners found on port ${PORT}`);
  } else {
    for (const listener of listeners) {
      const family = String(listener.LocalAddress ?? '').includes(':') ? 'IPv6' : 'IPv4';
      const state = String(listener.State).toLowerCase() === 'listen' ? 'listening' : String(listener.State).toLowerCase();
      console.log(`- ${family} ${formatEndpoint(listener.LocalAddress, listener.LocalPort)} ${state}`);
    }
  }

  console.log('');
  console.log('Firewall:');
  const rules = diagnostics.FirewallRules ?? [];
  if (rules.length === 0) {
    console.log(`- No rules found in display group "${FIREWALL_GROUP}"`);
  } else {
    for (const rule of rules) {
      console.log(
        `- ${rule.DisplayName}: enabled ${rule.Enabled}, profile ${rule.Profile}, protocol ${rule.Protocol}, port ${rule.LocalPort}, program ${rule.Program ?? 'Any'}`
      );
    }
  }

  console.log('');
  console.log('Network profiles:');
  const profiles = diagnostics.NetworkProfiles ?? [];
  if (profiles.length === 0) {
    console.log('- No active network profiles found');
  } else {
    for (const profile of profiles) {
      console.log(
        `- ${profile.InterfaceAlias}: ${profile.NetworkCategory} (${profile.IPv4Connectivity} IPv4, ${profile.IPv6Connectivity} IPv6)`
      );
    }
  }
} catch (error) {
  console.error('Could not collect Switchify PC network diagnostics.');
  if (error && error.message) {
    console.error(error.message);
  }
  process.exit(1);
}
