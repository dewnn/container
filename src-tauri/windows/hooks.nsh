!macro NSIS_HOOK_POSTINSTALL
  SetShellVarContext current
  CreateShortCut "$SENDTO\CONTAINER.lnk" "$INSTDIR\container-studio.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  SetShellVarContext current
  Delete "$SENDTO\CONTAINER.lnk"
!macroend
