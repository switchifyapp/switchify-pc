!macro NSIS_HOOK_PREINSTALL
  ; Installing over an earlier version only adds and replaces files, so a file
  ; a newer TextSlinger runtime no longer ships (an older Python or numpy)
  ; would stay beside the new runtime. Remove the installed runtime first. This
  ; hook runs before Tauri checks for a running app, so check first: the
  ; runtime is removed only once the install will go ahead.
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  StrCmp $INSTDIR "" +2
  RMDir /r "$INSTDIR\resources\textslinger\runtime"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
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
; If that fails (no shell, Secondary Logon off), fall back to the template's
; launch: an elevated app is better than none for a switch user. Builds that
; still pass /R (1.16.1 and earlier) get the template's restart, elevated, on
; the update that installs this.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode = 1
    ClearErrors
    ${GetOptions} $CMDLINE "/SWITCHIFY_RELAUNCH" $R0
    ${IfNot} ${Errors}
      nsis_tauri_utils::RunAsUser "$WINDIR\explorer.exe" '"$INSTDIR\${MAINBINARYNAME}.exe"'
      Pop $R0
      ${If} $R0 <> 0
        nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
        Pop $R0
      ${EndIf}
      ${If} $R0 <> 0
        MessageBox MB_OK|MB_ICONINFORMATION "${PRODUCTNAME} was updated. Open it from Start." /SD IDOK
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend
