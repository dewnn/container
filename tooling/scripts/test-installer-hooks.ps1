param(
  [string]$MakeNsis = (Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'),
  [string]$Executable = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'src-tauri\target\release\container-studio-dev.exe')
)

$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$operation = [Guid]::NewGuid().ToString('N')
$testParent = Join-Path $repository 'src-tauri\target\packaging-tests'
$testRoot = Join-Path $testParent $operation
$installRoot = Join-Path $testRoot 'Install With Spaces'
$registryRoot = "Software\CONTAINER-Packaging-Tests-$operation"
$utf8 = New-Object System.Text.UTF8Encoding($false)

# Compile the real hook, substituting only its external destinations. It cannot
# touch actual associations, desktop/start-menu links or the installed program.
$hooks = [IO.File]::ReadAllText((Join-Path $repository 'src-tauri\windows\hooks.nsh'))
$hooks = $hooks.Replace('Software\Classes', $registryRoot)
$hooks = $hooks.Replace('$DESKTOP', '$INSTDIR\shell\Desktop').Replace('$SMPROGRAMS', '$INSTDIR\shell\StartMenu').Replace('$SENDTO', '$INSTDIR\shell\SendTo')
if ($hooks.Contains('Software\Classes') -or $hooks -match '\$(DESKTOP|SMPROGRAMS|SENDTO)') { throw 'Hook sandbox is incomplete' }

try {
  foreach ($directory in @($testRoot, $installRoot, "$installRoot\shell\Desktop", "$installRoot\shell\StartMenu\CONTAINER TEST", "$installRoot\shell\SendTo")) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
  }
  Copy-Item -LiteralPath $Executable -Destination "$installRoot\container-studio.exe"
  [IO.File]::WriteAllText("$testRoot\hooks.nsh", $hooks, $utf8)
  $template = @'
Unicode true
Name "CONTAINER isolated packaging check"
OutFile "@ROOT@\check.exe"
RequestExecutionLevel user
SilentInstall silent
!include "LogicLib.nsh"
!define PRODUCTNAME "CONTAINER TEST"
!define SHELL_CONTEXT HKCU
Var AppStartMenuFolder
!macro SetLnkAppUserModelId Path
!macroend
!include "@ROOT@\hooks.nsh"
Section
  SetRegView 64
  StrCpy $INSTDIR "@INSTALL@"
  StrCpy $AppStartMenuFolder "CONTAINER TEST"
  WriteRegStr HKCU "@REG@\.containerproject" "" "CONTAINER Project"
  !insertmacro NSIS_HOOK_POSTINSTALL
  StrCpy $R9 0
  ReadRegStr $R0 HKCU "@REG@\CONTAINER CPROJ\DefaultIcon" ""
  ${If} $R0 != '$\"$INSTDIR\container-studio.exe$\",-32513'
    StrCpy $R9 1
  ${EndIf}
  ReadRegStr $R0 HKCU "@REG@\Applications\container-studio.exe\DefaultIcon" ""
  ${If} $R0 != '$\"$INSTDIR\container-studio.exe$\",-32513'
    StrCpy $R9 2
  ${EndIf}
  ReadRegStr $R0 HKCU "@REG@\.containerproject" ""
  ${If} $R0 != ""
    StrCpy $R9 3
  ${EndIf}
  DeleteRegKey HKCU "@REG@"
  SetErrorLevel $R9
SectionEnd
'@
  $template = $template.Replace('@ROOT@', $testRoot).Replace('@INSTALL@', $installRoot).Replace('@REG@', $registryRoot)
  [IO.File]::WriteAllText("$testRoot\check.nsi", $template, $utf8)
  & $MakeNsis /V2 "$testRoot\check.nsi"
  if ($LASTEXITCODE -ne 0) { throw "Hook compilation failed: $LASTEXITCODE" }

  $shell = New-Object -ComObject WScript.Shell
  $links = @('shell\SendTo\CONTAINER.lnk', 'shell\Desktop\CONTAINER TEST.lnk', 'shell\StartMenu\CONTAINER TEST.lnk', 'shell\StartMenu\CONTAINER TEST\CONTAINER TEST.lnk')
  $oldFiles = @('container-project-v1.ico', 'container-project-v2.ico', 'container-brand-rounded-0.18.5.ico', 'container-cproj-v2-0.18.5.ico', 'resources\FFmpeg-GPLv3.txt', 'resources\yt-dlp-LICENSE.txt', 'resources\yt-dlp-THIRD_PARTY_LICENSES.txt', 'resources\deno-LICENSE.md', 'resources\face_detection_yunet-LICENSE.txt', '_up_\docs\THIRD_PARTY_NOTICES.md')
  foreach ($scenario in @('fresh', 'upgrade')) {
    if ($scenario -eq 'upgrade') {
      foreach ($file in $oldFiles) {
        $path = Join-Path $installRoot $file
        New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
        [IO.File]::WriteAllText($path, 'old owned packaging asset', $utf8)
      }
      # Unknown/user files in old directories must never be removed recursively.
      [IO.File]::WriteAllText("$installRoot\resources\user-keep.txt", 'keep', $utf8)
      foreach ($relative in $links) {
        $link = $shell.CreateShortcut((Join-Path $installRoot $relative))
        $link.TargetPath = "$installRoot\container-studio.exe"
        $link.IconLocation = "$installRoot\container-brand-rounded-0.18.5.ico,0"
        $link.Save()
      }
    }
    $process = Start-Process -FilePath "$testRoot\check.exe" -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "$scenario association check failed: $($process.ExitCode)" }
    $expectedLinks = if ($scenario -eq 'fresh') { @($links[0]) } else { $links }
    foreach ($relative in $expectedLinks) {
      $link = $shell.CreateShortcut((Join-Path $installRoot $relative))
      if ($link.TargetPath -ne "$installRoot\container-studio.exe" -or $link.IconLocation -ne "$installRoot\container-studio.exe,0") {
        throw "$scenario shortcut mismatch: $relative"
      }
    }
    foreach ($file in $oldFiles) {
      if (Test-Path -LiteralPath (Join-Path $installRoot $file)) { throw "Old asset survived: $file" }
    }
    if ($scenario -eq 'upgrade' -and [IO.File]::ReadAllText("$installRoot\resources\user-keep.txt") -ne 'keep') { throw 'Unrelated file was changed' }
    Write-Host "Installer ${scenario}: embedded document icon records, app shortcuts and owned-file cleanup passed."
  }
} finally {
  # Only this invocation's generated test directory, never an installation root.
  if (Test-Path -LiteralPath $testRoot) {
    $resolved = (Resolve-Path -LiteralPath $testRoot).Path
    if ((Split-Path -Parent $resolved) -ne $testParent -or (Split-Path -Leaf $resolved) -ne $operation) { throw 'Unsafe test cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
  }
}
