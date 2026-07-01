!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
!define VERSION "0.2.0"
!endif

!ifndef SOURCE_DIR
!define SOURCE_DIR "..\dist\win-unpacked"
!endif

!ifndef OUTPUT_EXE
!define OUTPUT_EXE "..\dist\Switchify-PC-Setup-${VERSION}-x64.exe"
!endif

!define APP_EXE "Switchify PC.exe"
!define QUIT_FOR_INSTALL_ARG "--quit-for-install"
!define QUIT_WAIT_ATTEMPTS 20
!define QUIT_WAIT_SLEEP_MS 500

Name "Switchify PC"
OutFile "${OUTPUT_EXE}"
InstallDir "$PROGRAMFILES64\Switchify PC"
InstallDirRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "InstallLocation"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
Unicode true

!define MUI_ABORTWARNING
!define MUI_ICON "..\build\icon.ico"
!define MUI_UNICON "..\build\icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\Switchify PC.exe"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function .onInit
  SetRegView 64
FunctionEnd

Section "Switchify PC" SecMain
  SetShellVarContext all
  SetOutPath "$INSTDIR"

  Call CloseRunningAppForInstall

  File /r "${SOURCE_DIR}\*.*"

  CreateDirectory "$SMPROGRAMS\Switchify PC"
  CreateShortCut "$SMPROGRAMS\Switchify PC\Switchify PC.lnk" "$INSTDIR\Switchify PC.exe"
  CreateShortCut "$DESKTOP\Switchify PC.lnk" "$INSTDIR\Switchify PC.exe"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "DisplayName" "Switchify PC"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "Publisher" "Switchify"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "DisplayIcon" "$INSTDIR\Switchify PC.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC" "NoRepair" 1

  IfSilent 0 done
  Exec '"$INSTDIR\${APP_EXE}"'
  done:
SectionEnd

Section "Uninstall"
  SetShellVarContext all
  Call un.CloseRunningAppForInstall

  Delete "$DESKTOP\Switchify PC.lnk"
  Delete "$SMPROGRAMS\Switchify PC\Switchify PC.lnk"
  RMDir "$SMPROGRAMS\Switchify PC"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC"
SectionEnd

Function IsAppRunning
  nsExec::ExecToStack 'cmd /c tasklist /FI "IMAGENAME eq ${APP_EXE}" /NH | find /I "${APP_EXE}" >NUL'
  Pop $0
  Pop $1
FunctionEnd

Function WaitForAppExit
  StrCpy $2 ${QUIT_WAIT_ATTEMPTS}
  loop:
  Call IsAppRunning
  ${If} $0 != 0
    StrCpy $0 0
    Return
  ${EndIf}
  IntOp $2 $2 - 1
  ${If} $2 <= 0
    StrCpy $0 1
    Return
  ${EndIf}
  Sleep ${QUIT_WAIT_SLEEP_MS}
  Goto loop
FunctionEnd

Function CloseRunningAppForInstall
  check:
  DetailPrint "Checking whether Switchify PC is running..."
  Call IsAppRunning
  ${If} $0 != 0
    Return
  ${EndIf}

  DetailPrint "Requesting Switchify PC to close..."
  IfFileExists "$INSTDIR\${APP_EXE}" 0 force_prompt
  ExecWait '"$INSTDIR\${APP_EXE}" ${QUIT_FOR_INSTALL_ARG}'
  Call WaitForAppExit
  ${If} $0 == 0
    Return
  ${EndIf}

  IfSilent silent_force_close force_prompt

  silent_force_close:
  DetailPrint "Closing Switchify PC..."
  nsExec::ExecToStack 'cmd /c taskkill /IM "${APP_EXE}" /F /T >NUL 2>NUL'
  Pop $0
  Pop $1
  Call WaitForAppExit
  ${If} $0 == 0
    Return
  ${EndIf}
  SetErrorLevel 1
  Abort

  force_prompt:
  MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "Switchify PC needs to close before installation can continue.$\r$\n$\r$\nClick OK to close Switchify PC and continue installing, or Cancel to stop the installer." IDOK force_close IDCANCEL cancel

  force_close:
  DetailPrint "Closing Switchify PC..."
  nsExec::ExecToStack 'cmd /c taskkill /IM "${APP_EXE}" /F /T >NUL 2>NUL'
  Pop $0
  Pop $1
  Call WaitForAppExit
  ${If} $0 == 0
    Return
  ${EndIf}

  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Switchify PC could not be closed. Close it from the tray, then click Retry." IDRETRY check IDCANCEL cancel

  cancel:
  Abort
FunctionEnd

Function un.IsAppRunning
  nsExec::ExecToStack 'cmd /c tasklist /FI "IMAGENAME eq ${APP_EXE}" /NH | find /I "${APP_EXE}" >NUL'
  Pop $0
  Pop $1
FunctionEnd

Function un.WaitForAppExit
  StrCpy $2 ${QUIT_WAIT_ATTEMPTS}
  loop:
  Call un.IsAppRunning
  ${If} $0 != 0
    StrCpy $0 0
    Return
  ${EndIf}
  IntOp $2 $2 - 1
  ${If} $2 <= 0
    StrCpy $0 1
    Return
  ${EndIf}
  Sleep ${QUIT_WAIT_SLEEP_MS}
  Goto loop
FunctionEnd

Function un.CloseRunningAppForInstall
  check:
  DetailPrint "Checking whether Switchify PC is running..."
  Call un.IsAppRunning
  ${If} $0 != 0
    Return
  ${EndIf}

  DetailPrint "Requesting Switchify PC to close..."
  IfFileExists "$INSTDIR\${APP_EXE}" 0 force_prompt
  ExecWait '"$INSTDIR\${APP_EXE}" ${QUIT_FOR_INSTALL_ARG}'
  Call un.WaitForAppExit
  ${If} $0 == 0
    Return
  ${EndIf}

  IfSilent silent_force_close force_prompt

  silent_force_close:
  DetailPrint "Closing Switchify PC..."
  nsExec::ExecToStack 'cmd /c taskkill /IM "${APP_EXE}" /F /T >NUL 2>NUL'
  Pop $0
  Pop $1
  Call un.WaitForAppExit
  ${If} $0 == 0
    Return
  ${EndIf}
  SetErrorLevel 1
  Abort

  force_prompt:
  MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "Switchify PC needs to close before uninstalling can continue.$\r$\n$\r$\nClick OK to close Switchify PC and continue uninstalling, or Cancel to stop the uninstaller." IDOK force_close IDCANCEL cancel

  force_close:
  DetailPrint "Closing Switchify PC..."
  nsExec::ExecToStack 'cmd /c taskkill /IM "${APP_EXE}" /F /T >NUL 2>NUL'
  Pop $0
  Pop $1
  Call un.WaitForAppExit
  ${If} $0 == 0
    Return
  ${EndIf}

  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Switchify PC could not be closed. Close it from the tray, then click Retry." IDRETRY check IDCANCEL cancel

  cancel:
  Abort
FunctionEnd
