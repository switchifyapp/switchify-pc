; The template's running-app check uses Restart Manager, which cannot close
; a copy open in another user's session, so an update or uninstall aborted
; with "Insufficient permissions to close Switchify PC" whenever another
; signed-in account had it open (#1559). The files are shared, so that copy
; has to close: this elevated installer stops copies of the installed
; executable (matched by path) in other sessions and waits for them to exit.
; It is a hard stop, not a request to close: Restart Manager cannot reach
; another session, so that copy's input cleanup does not run.
;
; An interactive install or uninstall asks first and stops only the copies it
; asked about. If finding them fails or times out (or PowerShell is blocked),
; nothing is closed and the template's check aborts as it did before. Passive
; and silent runs (in-app updates, deployments) stop them without asking, as
; the template does for the installing user's own copy. Copies in this
; session are left to the template. The path is passed in the environment,
; not in the script, and results come from exit codes because nsExec mixes
; stderr into the output.
!macro SWITCHIFY_CLOSE_OTHER_SESSIONS
  InitPluginsDir
  System::Call 'kernel32::SetEnvironmentVariable(t "SWITCHIFY_EXE", t "$INSTDIR\${MAINBINARYNAME}.exe")'
  System::Call 'kernel32::SetEnvironmentVariable(t "SWITCHIFY_PIDS", t "$PLUGINSDIR\switchify-other-sessions.txt")'
  ; Find copies in other sessions and save each one's process id and start
  ; time. Exit code 3 means some were found.
  nsExec::Exec /TIMEOUT=30000 `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -InputFormat None -OutputFormat Text -Command "$$ProgressPreference = 'SilentlyContinue'; $$own = (Get-Process -Id $$PID).SessionId; if ($$null -eq $$own) { exit 1 }; $$r = @(Get-CimInstance Win32_Process | Where-Object { $$_.SessionId -ne $$own -and $$_.ExecutablePath -eq $$env:SWITCHIFY_EXE } | ForEach-Object { try { '{0},{1}' -f $$_.ProcessId, (Get-Process -Id $$_.ProcessId).StartTime.ToFileTimeUtc() } catch {} }); Set-Content -LiteralPath $$env:SWITCHIFY_PIDS -Value $$r; if ($$r.Count -gt 0) { exit 3 }; exit 0"`
  Pop $0
  ${If} $0 == "3"
    ${IfNot} ${Silent}
    ${AndIf} $PassiveMode <> 1
      MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "${PRODUCTNAME} is open for another account on this computer. Continuing closes it for them, and they will need to start it again." /SD IDOK IDOK +2
      Abort "${PRODUCTNAME} is open for another account."
    ${EndIf}
    ; Stop only the saved copies. Each is opened first (skipped if that
    ; fails); with the handle held, its id cannot be reused, so its start
    ; time and path are checked then, and it is stopped and waited on through
    ; that handle. A process that took over a saved id is stopped only if it
    ; is itself a copy of the installed executable in another session, which
    ; has to close anyway; nothing else is. All are stopped before any wait.
    nsExec::Exec /TIMEOUT=30000 `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -InputFormat None -OutputFormat Text -Command "$$ProgressPreference = 'SilentlyContinue'; $$own = (Get-Process -Id $$PID).SessionId; if ($$null -eq $$own) { exit 1 }; $$h = @(foreach ($$l in @(Get-Content -LiteralPath $$env:SWITCHIFY_PIDS)) { $$id, $$t = $$l -split ','; try { $$q = Get-Process -Id ([int]$$id); if (!$$q.Handle) { continue }; if ($$q.SessionId -ne $$own -and [string]$$q.StartTime.ToFileTimeUtc() -eq $$t -and (Get-CimInstance Win32_Process -Filter ('ProcessId=' + $$q.Id)).ExecutablePath -eq $$env:SWITCHIFY_EXE) { $$q } } catch {} }); $$h | ForEach-Object { try { $$_.Kill() } catch {} }; $$h | ForEach-Object { try { $$null = $$_.WaitForExit(10000) } catch {} }"`
    Pop $0
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; Installing over an earlier version only adds and replaces files, so a file
  ; a newer TextSlinger runtime no longer ships (an older Python or numpy)
  ; would stay beside the new runtime. Remove the installed runtime first. This
  ; hook runs before Tauri checks for a running app, so check first: the
  ; runtime is removed only once the install will go ahead.
  !insertmacro SWITCHIFY_CLOSE_OTHER_SESSIONS
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  StrCmp $INSTDIR "" +2
  RMDir /r "$INSTDIR\resources\textslinger\runtime"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro SWITCHIFY_CLOSE_OTHER_SESSIONS
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "app.switchify.pc"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "app.switchify.pc"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Switchify PC"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Switchify PC"
  nsExec::ExecToStack 'schtasks.exe /Delete /TN "Switchify PC" /F'
  Pop $0
  Pop $1
!macroend

; In-app updates start this installer elevated with /UPDATE and, since #1560,
; /SWITCHIFY_RELAUNCH instead of /R, so the template does not start the app
; again itself. Its RunAsUser cannot create a UIAccess executable with the
; user's token (ERROR_ELEVATION_REQUIRED) and falls back to starting it from
; this elevated installer, so the app would run as an administrator, or as the
; admin whose credentials approved the update. Explorer is not UIAccess, so
; RunAsUser can start it as the signed-in user, and Explorer starts the app the
; way Start does: unelevated, with UIAccess granted by the Shell. Explorer
; cannot pass the app's launch arguments, so the window shows after an update.
; If that fails (no shell window, Secondary Logon off), RunAsUser cannot run
; the app either, so open it with ExecShell from this installer instead: an
; elevated app is better than none for a switch user. If even that fails, say
; so.
!macro SWITCHIFY_RELAUNCH_AS_USER
  nsis_tauri_utils::RunAsUser "$WINDIR\explorer.exe" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  Pop $R0
  ${If} $R0 <> 0
    ClearErrors
    ExecShell "open" "$INSTDIR\${MAINBINARYNAME}.exe"
    ${If} ${Errors}
      MessageBox MB_OK|MB_ICONINFORMATION "${PRODUCTNAME} was updated. Open it from Start." /SD IDOK
    ${EndIf}
  ${EndIf}
!macroend

; Builds up to 1.16.1 still pass /R (passive or silent), and the template's
; .onInstSuccess would restart the app the elevated way. For those, relaunch
; through Explorer here and Quit, which skips .onInstSuccess. Install work is
; done by now; only the passive page's auto-close is left. SetErrorLevel keeps
; the exit code 0 (the updater does not wait for it anyway: it exits right
; after starting the installer).
!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode = 1
    ClearErrors
    ${GetOptions} $CMDLINE "/SWITCHIFY_RELAUNCH" $R0
    ${IfNot} ${Errors}
      !insertmacro SWITCHIFY_RELAUNCH_AS_USER
    ${Else}
      ClearErrors
      ${GetOptions} $CMDLINE "/R" $R0
      ${IfNot} ${Errors}
        ${If} $PassiveMode = 1
        ${OrIf} ${Silent}
          !insertmacro SWITCHIFY_RELAUNCH_AS_USER
          SetErrorLevel 0
          Quit
        ${EndIf}
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend
