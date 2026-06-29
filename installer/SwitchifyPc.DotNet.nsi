!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
!define VERSION "0.2.0"
!endif

!ifndef SOURCE_DIR
!define SOURCE_DIR "..\dist-dotnet\win-unpacked"
!endif

!ifndef OUTPUT_EXE
!define OUTPUT_EXE "..\dist-dotnet\Switchify-PC-Setup-${VERSION}-x64.exe"
!endif

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

  Call PromptForRunningApp

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
SectionEnd

Section "Uninstall"
  SetShellVarContext all
  Call un.PromptForRunningApp

  Delete "$DESKTOP\Switchify PC.lnk"
  Delete "$SMPROGRAMS\Switchify PC\Switchify PC.lnk"
  RMDir "$SMPROGRAMS\Switchify PC"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC"
SectionEnd

Function PromptForRunningApp
  check:
  DetailPrint "Checking whether Switchify PC is running..."
  nsExec::ExecToStack 'tasklist /FI "IMAGENAME eq Switchify PC.exe" /NH'
  Pop $0
  Pop $1
  ${If} $1 != "INFO: No tasks are running which match the specified criteria."
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Switchify PC appears to be running. Close Switchify PC, then click Retry to continue installing." IDRETRY check IDCANCEL cancel
    cancel:
    Abort
  ${EndIf}
FunctionEnd

Function un.PromptForRunningApp
  check:
  DetailPrint "Checking whether Switchify PC is running..."
  nsExec::ExecToStack 'tasklist /FI "IMAGENAME eq Switchify PC.exe" /NH'
  Pop $0
  Pop $1
  ${If} $1 != "INFO: No tasks are running which match the specified criteria."
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Switchify PC appears to be running. Close Switchify PC, then click Retry to continue uninstalling." IDRETRY check IDCANCEL cancel
    cancel:
    Abort
  ${EndIf}
FunctionEnd
