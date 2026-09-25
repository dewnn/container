!define CONTAINER_HOOK_DIR "${__FILEDIR__}"

!macro NSIS_HOOK_POSTINSTALL
  SetShellVarContext current
  ; A versioned path prevents Explorer from reusing an icon cached for the old logo.
  SetOutPath "$INSTDIR"
  File "/oname=container-brand-${VERSION}.ico" "${CONTAINER_HOOK_DIR}\..\icons\icon.ico"
  CreateShortCut "$SENDTO\CONTAINER.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-${VERSION}.ico" 0

  ; Tauri skips shortcut creation during updates, so refresh existing links too.
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-${VERSION}.ico" 0
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-${VERSION}.ico" 0
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-brand-${VERSION}.ico" 0
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  ${EndIf}
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  SetShellVarContext current
  Delete "$SENDTO\CONTAINER.lnk"
  Delete "$INSTDIR\container-brand-${VERSION}.ico"
!macroend
