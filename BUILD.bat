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
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\Temp\container_studio_release"
call pnpm tauri build
if errorlevel 1 exit /b %errorlevel%
set "DEST=%USERPROFILE%\Documents\ratiochangerexe\CONTAINER Studio"
if not exist "%DEST%" mkdir "%DEST%"
copy /y "%CARGO_TARGET_DIR%\release\container-studio.exe" "%DEST%\CONTAINER Studio.exe" >nul
if errorlevel 1 (
  echo.
  echo CONTAINER Studio.exe is currently open. Close the app and run BUILD.bat again.
  exit /b 1
)
copy /y "%CARGO_TARGET_DIR%\release\ffmpeg.exe" "%DEST%\ffmpeg.exe" >nul
copy /y "%CARGO_TARGET_DIR%\release\ffprobe.exe" "%DEST%\ffprobe.exe" >nul
for %%I in ("%CARGO_TARGET_DIR%\release\bundle\nsis\*.exe") do copy /y "%%~fI" "%DEST%\CONTAINER Studio Setup.exe" >nul
echo.
echo Build ready: %DEST%
endlocal
