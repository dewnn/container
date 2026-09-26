!define CONTAINER_HOOK_DIR "${__FILEDIR__}"

!macro NSIS_HOOK_POSTINSTALL
  SetShellVarContext current
  ; A versioned path prevents Explorer from reusing an icon cached for the old logo.
  SetOutPath "$INSTDIR"
  File "/oname=container-brand-rounded-${VERSION}.ico" "${CONTAINER_HOOK_DIR}\..\icons\icon.ico"
  File "/oname=container-cproj-v2-${VERSION}.ico" "${CONTAINER_HOOK_DIR}\..\icons\project.ico"
  ; Explorer caches icons by path. Give project documents a versioned icon path.
  ; Match the install mode's association hive, and replace stale per-user overrides.
  WriteRegStr SHELL_CONTEXT "Software\Classes\CONTAINER CPROJ\DefaultIcon" "" "$INSTDIR\container-cproj-v2-${VERSION}.ico,0"
  WriteRegStr HKCU "Software\Classes\CONTAINER CPROJ\DefaultIcon" "" "$INSTDIR\container-cproj-v2-${VERSION}.ico,0"
  ; Windows may use Applications\container-studio.exe after "Open with > Always".
  ; That class needs the document icon as well, or it falls back to the app icon.
  WriteRegStr SHELL_CONTEXT "Software\Classes\Applications\container-studio.exe\DefaultIcon" "" "$INSTDIR\container-cproj-v2-${VERSION}.ico,0"
  WriteRegStr HKCU "Software\Classes\Applications\container-studio.exe\DefaultIcon" "" "$INSTDIR\container-cproj-v2-${VERSION}.ico,0"
  ; The retired project extension must not survive an upgrade as our default.
  ReadRegStr $R0 SHELL_CONTEXT "Software\Classes\.containerproject" ""
  ${If} $R0 == "CONTAINER Project"
    DeleteRegValue SHELL_CONTEXT "Software\Classes\.containerproject" ""
  ${EndIf}
  Delete "$INSTDIR\container-project-v1.ico"
  Delete "$INSTDIR\container-project-v2.ico"
  CreateShortCut "$SENDTO\CONTAINER.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-rounded-${VERSION}.ico" 0

  ; Tauri skips shortcut creation during updates, so refresh existing links too.
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-rounded-${VERSION}.ico" 0
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-rounded-${VERSION}.ico" 0
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-rounded-${VERSION}.ico" 0
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  ${EndIf}
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  SetShellVarContext current
  Delete "$SENDTO\CONTAINER.lnk"
  Delete "$INSTDIR\container-brand-rounded-${VERSION}.ico"
  Delete "$INSTDIR\container-cproj-v2-${VERSION}.ico"
!macroend
