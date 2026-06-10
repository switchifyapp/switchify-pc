!macro customInstall
  DetailPrint "Configuring Switchify PC firewall rules"
  nsExec::ExecToLog `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -Command "Get-NetFirewallRule -DisplayGroup 'Switchify PC' -ErrorAction SilentlyContinue | Remove-NetFirewallRule; Get-NetFirewallRule -Group 'Switchify PC' -ErrorAction SilentlyContinue | Remove-NetFirewallRule; Get-NetFirewallRule -DisplayName 'Switchify PC (TCP 7347)' -ErrorAction SilentlyContinue | Remove-NetFirewallRule; Get-NetFirewallRule -DisplayName 'Switchify PC (mDNS UDP 5353)' -ErrorAction SilentlyContinue | Remove-NetFirewallRule; New-NetFirewallRule -DisplayName 'Switchify PC (TCP 7347)' -Group 'Switchify PC' -Direction Inbound -Action Allow -Program '$INSTDIR\Switchify PC.exe' -Protocol TCP -LocalPort 7347 -Profile Any | Out-Null; New-NetFirewallRule -DisplayName 'Switchify PC (mDNS UDP 5353)' -Group 'Switchify PC' -Direction Inbound -Action Allow -Program '$INSTDIR\Switchify PC.exe' -Protocol UDP -LocalPort 5353 -Profile Any | Out-Null"`
!macroend

!macro customUnInstall
  DetailPrint "Removing Switchify PC firewall rules"
  nsExec::ExecToLog `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -Command "Get-NetFirewallRule -DisplayGroup 'Switchify PC' -ErrorAction SilentlyContinue | Remove-NetFirewallRule; Get-NetFirewallRule -Group 'Switchify PC' -ErrorAction SilentlyContinue | Remove-NetFirewallRule; Get-NetFirewallRule -DisplayName 'Switchify PC (TCP 7347)' -ErrorAction SilentlyContinue | Remove-NetFirewallRule; Get-NetFirewallRule -DisplayName 'Switchify PC (mDNS UDP 5353)' -ErrorAction SilentlyContinue | Remove-NetFirewallRule"`
!macroend
