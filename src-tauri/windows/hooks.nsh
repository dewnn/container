!macro NSIS_HOOK_POSTINSTALL
  SetShellVarContext current
  ; Both icons live in the executable. Negative index means resource ID,
  ; not the ordering of icon groups (32512 = app, 32513 = CPROJ).
  ; Match the install mode's association hive, and replace stale per-user overrides.
  WriteRegStr SHELL_CONTEXT "Software\Classes\CONTAINER CPROJ\DefaultIcon" "" '$\"$INSTDIR\container-studio.exe$\",-32513'
  WriteRegStr HKCU "Software\Classes\CONTAINER CPROJ\DefaultIcon" "" '$\"$INSTDIR\container-studio.exe$\",-32513'
  ; Windows may use Applications\container-studio.exe after "Open with > Always".
  ; That class needs the document icon as well, or it falls back to the app icon.
  WriteRegStr SHELL_CONTEXT "Software\Classes\Applications\container-studio.exe\DefaultIcon" "" '$\"$INSTDIR\container-studio.exe$\",-32513'
  WriteRegStr HKCU "Software\Classes\Applications\container-studio.exe\DefaultIcon" "" '$\"$INSTDIR\container-studio.exe$\",-32513'
  ; The retired project extension must not survive an upgrade as our default.
  ReadRegStr $R0 SHELL_CONTEXT "Software\Classes\.containerproject" ""
  ${If} $R0 == "CONTAINER Project"
    DeleteRegValue SHELL_CONTEXT "Software\Classes\.containerproject" ""
  ${EndIf}
  Delete "$INSTDIR\container-project-v1.ico"
  Delete "$INSTDIR\container-project-v2.ico"
  CreateShortCut "$SENDTO\CONTAINER.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-studio.exe" 0

  ; Tauri skips shortcut creation during updates, so refresh existing links too.
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-studio.exe" 0
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-studio.exe" 0
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\container-studio.exe" "" "$INSTDIR\container-studio.exe" 0
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  ${EndIf}
  ; Remove only our old loose icon assets, after all supported links are updated.
  Delete "$INSTDIR\container-brand-rounded-*.ico"
  Delete "$INSTDIR\container-cproj-v2-*.ico"
  ; Retire the previous license layout without recursively removing directories.
  Delete "$INSTDIR\resources\FFmpeg-GPLv3.txt"
  Delete "$INSTDIR\resources\yt-dlp-LICENSE.txt"
  Delete "$INSTDIR\resources\yt-dlp-THIRD_PARTY_LICENSES.txt"
  Delete "$INSTDIR\resources\deno-LICENSE.md"
  Delete "$INSTDIR\resources\face_detection_yunet-LICENSE.txt"
  Delete "$INSTDIR\_up_\docs\THIRD_PARTY_NOTICES.md"
  RMDir "$INSTDIR\resources"
  RMDir "$INSTDIR\_up_\docs"
  RMDir "$INSTDIR\_up_"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  SetShellVarContext current
  Delete "$SENDTO\CONTAINER.lnk"
!macroend
