param(
  [switch]$ApplyInDesktopSession,
  [string]$ResultPath
)

$ErrorActionPreference = 'Stop'

# A packaged development host can expose a private HKCU registry view to its
# children. Use a short-lived, non-elevated task so registration and verification
# run in the same registry view as Explorer. No scheduled task remains afterward.
if (-not $ApplyInDesktopSession) {
  $operationId = [Guid]::NewGuid().ToString('N')
  $taskName = "CONTAINER-DEV-Association-$operationId"
  $taskResult = Join-Path ([System.IO.Path]::GetTempPath()) "$taskName.clixml"
  $taskCreated = $false
  try {
    $arguments = '-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "{0}" -ApplyInDesktopSession -ResultPath "{1}"' -f $PSCommandPath, $taskResult
    $action = New-ScheduledTaskAction -Execute (Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe') -Argument $arguments
    $principal = New-ScheduledTaskPrincipal -UserId ([System.Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal | Out-Null
    $taskCreated = $true
    Start-ScheduledTask -TaskName $taskName
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
      Start-Sleep -Milliseconds 250
      $task = Get-ScheduledTask -TaskName $taskName
      $info = Get-ScheduledTaskInfo -TaskName $taskName
      $finished = $task.State -ne 'Running' -and $info.LastRunTime.Year -gt 2000
    } until ($finished -or [DateTime]::UtcNow -ge $deadline)
    if (-not $finished) { throw 'Windows project association registration timed out.' }
    if (-not (Test-Path -LiteralPath $taskResult -PathType Leaf)) {
      throw "Windows project association registration failed (task result $($info.LastTaskResult))."
    }
    $result = Import-Clixml -LiteralPath $taskResult
    if (-not $result.Success) { throw $result.Error }
    if ($info.LastTaskResult -ne 0) { throw "Windows registration task failed: $($info.LastTaskResult)" }
    Write-Host "Registered and verified CONTAINER DEV for .cproj in the Windows desktop session."
    Write-Host "Project icon: $($result.IconPath)"
  } finally {
    if ($taskCreated) {
      if ((Get-ScheduledTask -TaskName $taskName).State -eq 'Running') { Stop-ScheduledTask -TaskName $taskName }
      Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
    }
    if (Test-Path -LiteralPath $taskResult) { Remove-Item -LiteralPath $taskResult -Force }
  }
  return
}

trap {
  if ($ResultPath) { [pscustomobject]@{ Success = $false; Error = $_.Exception.Message } | Export-Clixml -LiteralPath $ResultPath }
  exit 1
}

function Ensure-RegistryKey([string]$Path) {
  # New-Item -Force can replace existing registry values. Preserve unrelated
  # associations and defaults when refreshing our icon and open command.
  if (-not (Test-Path -LiteralPath $Path)) { New-Item -Path $Path -Force | Out-Null }
}

$repository = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$executable = Join-Path $repository 'src-tauri\target\release\container-studio-dev.exe'

if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
  throw "Build CONTAINER DEV before registering it: $executable"
}
& (Join-Path $PSScriptRoot 'test-windows-icons.ps1') -Executable $executable
$icon = '"{0}",-32513' -f $executable

$application = 'HKCU:\Software\Classes\Applications\container-studio-dev.exe'
Ensure-RegistryKey $application
New-ItemProperty -Path $application -Name 'FriendlyAppName' -Value 'CONTAINER DEV' -PropertyType String -Force | Out-Null
Ensure-RegistryKey "$application\DefaultIcon"
Set-Item -Path "$application\DefaultIcon" -Value $icon
Ensure-RegistryKey "$application\shell\open\command"
Set-Item -Path "$application\shell\open\command" -Value ('"{0}" "%1"' -f $executable)
Ensure-RegistryKey "$application\SupportedTypes"
New-ItemProperty -Path "$application\SupportedTypes" -Name '.cproj' -Value '' -PropertyType String -Force | Out-Null
Remove-ItemProperty -Path "$application\SupportedTypes" -Name '.containerproject' -ErrorAction SilentlyContinue

$progId = 'HKCU:\Software\Classes\CONTAINER.CPROJ.Dev'
Ensure-RegistryKey $progId
Set-Item -Path $progId -Value 'CONTAINER DEV'
Ensure-RegistryKey "$progId\DefaultIcon"
Set-Item -Path "$progId\DefaultIcon" -Value $icon
Ensure-RegistryKey "$progId\shell\open\command"
Set-Item -Path "$progId\shell\open\command" -Value ('"{0}" "%1"' -f $executable)

$oldOpenWith = 'HKCU:\Software\Classes\.containerproject\OpenWithProgids'
if (Test-Path -LiteralPath $oldOpenWith) {
  Remove-ItemProperty -LiteralPath $oldOpenWith -Name 'CONTAINER.Project.Dev' -ErrorAction SilentlyContinue
}
$newExtension = 'HKCU:\Software\Classes\.cproj'
Ensure-RegistryKey $newExtension
$currentDefault = (Get-Item -Path $newExtension).GetValue('')
if (-not $currentDefault -or $currentDefault -eq 'CONTAINER.Project.Dev') { Set-Item -Path $newExtension -Value 'CONTAINER.CPROJ.Dev' }
Ensure-RegistryKey "$newExtension\OpenWithProgids"
New-ItemProperty -Path "$newExtension\OpenWithProgids" -Name 'CONTAINER.CPROJ.Dev' -Value ([byte[]]@()) -PropertyType None -Force | Out-Null
Remove-ItemProperty -Path "$newExtension\OpenWithProgids" -Name 'CONTAINER.Project.Dev' -ErrorAction SilentlyContinue

Add-Type -Namespace Native -Name ShellAssociation -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("shell32.dll")]
public static extern void SHChangeNotify(int eventId, uint flags, System.IntPtr item1, System.IntPtr item2);
'@
[Native.ShellAssociation]::SHChangeNotify(0x08000000, 0x1000, [IntPtr]::Zero, [IntPtr]::Zero)

$expectedIcon = $icon
$expectedCommand = '"{0}" "%1"' -f $executable
foreach ($key in @($application, $progId)) {
  if ((Get-Item -LiteralPath "$key\DefaultIcon").GetValue('') -ne $expectedIcon -or
      (Get-Item -LiteralPath "$key\shell\open\command").GetValue('') -ne $expectedCommand) {
    throw "Windows association verification failed: $key"
  }
}
if ($ResultPath) {
  [pscustomobject]@{ Success = $true; IconPath = $icon } | Export-Clixml -LiteralPath $ResultPath
}
