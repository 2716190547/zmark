!ifndef VERSION
  !define VERSION "0.2.1"
!endif
!ifndef EXE_PATH
  !define EXE_PATH "target\release\zmark_app.exe"
!endif
!ifndef OUT_PATH
  !define OUT_PATH "ZMark-${VERSION}-setup.exe"
!endif
!ifndef ICON
  !define ICON "packaging/windows/zmark.ico"
!endif

Name "ZMark"
OutFile "${OUT_PATH}"
InstallDir "$PROGRAMFILES64\ZMark"
InstallDirRegKey HKCU "Software\ZMark" "Install_Dir"
RequestExecutionLevel admin
Unicode true
SetCompressor /SOLID lzma

Icon "${ICON}"
UninstallIcon "${ICON}"

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "ZMark"
  SetOutPath "$INSTDIR"
  File /oname=ZMark.exe "${EXE_PATH}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\ZMark"
  CreateShortcut "$SMPROGRAMS\ZMark\ZMark.lnk" "$INSTDIR\ZMark.exe"
  CreateShortcut "$DESKTOP\ZMark.lnk" "$INSTDIR\ZMark.exe"

  WriteRegStr HKCU "Software\ZMark" "Install_Dir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ZMark" "DisplayName" "ZMark"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ZMark" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ZMark" "Publisher" "ZMark"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ZMark" "DisplayIcon" "$INSTDIR\ZMark.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ZMark" "UninstallString" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\ZMark.exe"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\ZMark\ZMark.lnk"
  RMDir "$SMPROGRAMS\ZMark"
  Delete "$DESKTOP\ZMark.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ZMark"
  DeleteRegKey HKCU "Software\ZMark"
SectionEnd
