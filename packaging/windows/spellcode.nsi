; Spellcode installer for Windows (x86_64).
;
; Build (cross-platform, from the repository root):
;   makensis \
;     /DVERSION=0.1.0 \
;     /DSRCEXE=/abs/path/target/release/spellcode-app.exe \
;     /DSRCICON=/abs/path/packaging/windows/spellcode.ico \
;     /DOUT=/abs/path/Setup-0.1.0-x64.exe \
;     packaging/windows/spellcode.nsi
;
; The two paths above are absolute on purpose: NSIS resolves relative File
; paths against its own working directory, which is easy to get wrong on a
; runner. The defaults below are only a courtesy for a manual run from the
; packaging/windows directory.

Unicode true

!include "MUI2.nsh"
!include "x64.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef SRCEXE
  !define SRCEXE "${__FILEDIR__}\..\..\target\release\spellcode-app.exe"
!endif
!ifndef SRCICON
  !define SRCICON "${__FILEDIR__}\spellcode.ico"
!endif
!ifndef OUT
  !define OUT "${__FILEDIR__}\..\..\Setup-Spellcode-${VERSION}-x64.exe"
!endif

!define APPNAME "Spellcode"
!define COMPANY "Spellcode"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"

Name "${APPNAME}"
OutFile "${OUT}"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\${APPNAME}" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show

!define MUI_ABORTWARNING
!define MUI_ICON "${SRCICON}"
!define MUI_UNICON "${SRCICON}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${APPNAME} requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Section "${APPNAME}" SecMain
  SectionIn RO

  SetOutPath "$INSTDIR"
  File /oname=Spellcode.exe "${SRCEXE}"
  File /oname=Spellcode.ico "${SRCICON}"

  WriteRegStr HKLM "Software\${APPNAME}" "InstallDir" "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" \
    "$INSTDIR\Spellcode.exe" "" "$INSTDIR\Spellcode.ico" 0
  CreateShortcut "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk" \
    "$INSTDIR\Uninstall.exe"
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" \
    "$INSTDIR\Spellcode.exe" "" "$INSTDIR\Spellcode.ico" 0

  WriteRegStr HKLM "${UNINSTKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINSTKEY}" "Publisher" "${COMPANY}"
  WriteRegStr HKLM "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\Spellcode.ico"
  WriteRegStr HKLM "${UNINSTKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINSTKEY}" "NoRepair" 1

  DetailPrint "Installed ${APPNAME} ${VERSION} to $INSTDIR"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\Spellcode.exe"
  Delete "$INSTDIR\Spellcode.ico"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"
  Delete "$DESKTOP\${APPNAME}.lnk"

  DeleteRegKey HKLM "${UNINSTKEY}"
  DeleteRegKey HKLM "Software\${APPNAME}"
SectionEnd
