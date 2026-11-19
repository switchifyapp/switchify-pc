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
