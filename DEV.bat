@echo off
setlocal
set "VSWHERE=C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
for /f "usebackq tokens=*" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSPATH=%%I"
if not defined VSPATH (
  echo Visual C++ Build Tools not found.
  pause
  exit /b 1
)
call "%VSPATH%\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
call pnpm tauri dev
endlocal
