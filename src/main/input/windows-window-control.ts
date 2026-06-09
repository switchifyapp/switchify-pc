import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import type { WindowControlAction } from '../../shared/protocol';

const execFileAsync = promisify(execFile);

export type WindowsWindowControlStrategy =
  | { kind: 'taskView' }
  | { kind: 'showDesktop' }
  | { kind: 'foregroundWindow'; operation: 'close' | 'minimize' | 'maximizeOrRestore' };

export function toWindowsWindowControlStrategy(action: WindowControlAction): WindowsWindowControlStrategy | null {
  switch (action) {
    case 'switchNext':
    case 'switchPrevious':
      return null;
    case 'taskView':
      return { kind: 'taskView' };
    case 'showDesktop':
      return { kind: 'showDesktop' };
    case 'closeFocused':
      return { kind: 'foregroundWindow', operation: 'close' };
    case 'minimizeFocused':
      return { kind: 'foregroundWindow', operation: 'minimize' };
    case 'maximizeFocused':
      return { kind: 'foregroundWindow', operation: 'maximizeOrRestore' };
  }
}

export async function runWindowsWindowControlAction(action: WindowControlAction): Promise<void> {
  const strategy = toWindowsWindowControlStrategy(action);
  if (strategy === null) return;

  const script = createWindowControlScript(strategy);
  await execFileAsync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', script], {
    windowsHide: true,
    timeout: 2_000
  });
}

export function createWindowControlScript(strategy: WindowsWindowControlStrategy): string {
  switch (strategy.kind) {
    case 'taskView':
      return `Start-Process explorer.exe 'shell:::{3080F90E-D7AD-11D9-BD98-0000947B0257}'`;
    case 'showDesktop':
      return `(New-Object -ComObject Shell.Application).MinimizeAll()`;
    case 'foregroundWindow':
      return createForegroundWindowScript(strategy.operation);
  }
}

function createForegroundWindowScript(operation: 'close' | 'minimize' | 'maximizeOrRestore'): string {
  return `
$signature = @"
using System;
using System.Runtime.InteropServices;

public static class SwitchifyWindowControl {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}
"@
Add-Type -TypeDefinition $signature -ErrorAction SilentlyContinue
$handle = [SwitchifyWindowControl]::GetForegroundWindow()
if ($handle -eq [IntPtr]::Zero) {
  exit 0
}
${foregroundWindowOperationScript(operation)}
`.trim();
}

function foregroundWindowOperationScript(operation: 'close' | 'minimize' | 'maximizeOrRestore'): string {
  switch (operation) {
    case 'close':
      return '[SwitchifyWindowControl]::PostMessage($handle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null';
    case 'minimize':
      return '[SwitchifyWindowControl]::ShowWindow($handle, 6) | Out-Null';
    case 'maximizeOrRestore':
      return `
if ([SwitchifyWindowControl]::IsZoomed($handle)) {
  [SwitchifyWindowControl]::ShowWindow($handle, 9) | Out-Null
} else {
  [SwitchifyWindowControl]::ShowWindow($handle, 3) | Out-Null
}`.trim();
  }
}
