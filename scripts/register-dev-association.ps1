$ErrorActionPreference = 'Stop'

$repository = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $repository 'src-tauri\target\release\container-studio-dev.exe'
$icon = Join-Path $repository 'src-tauri\icons\project.ico'

if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
  throw "Build CONTAINER DEV before registering it: $executable"
}
if (-not (Test-Path -LiteralPath $icon -PathType Leaf)) {
  throw "Project icon is missing: $icon"
}

$application = 'HKCU:\Software\Classes\Applications\container-studio-dev.exe'
New-Item -Path $application -Force | Out-Null
New-ItemProperty -Path $application -Name 'FriendlyAppName' -Value 'CONTAINER DEV' -PropertyType String -Force | Out-Null
New-Item -Path "$application\DefaultIcon" -Force | Out-Null
Set-Item -Path "$application\DefaultIcon" -Value ('"{0}",0' -f $icon)
New-Item -Path "$application\shell\open\command" -Force | Out-Null
Set-Item -Path "$application\shell\open\command" -Value ('"{0}" "%1"' -f $executable)
New-Item -Path "$application\SupportedTypes" -Force | Out-Null
New-ItemProperty -Path "$application\SupportedTypes" -Name '.containerproject' -Value '' -PropertyType String -Force | Out-Null

$progId = 'HKCU:\Software\Classes\CONTAINER.Project.Dev'
New-Item -Path $progId -Force | Out-Null
Set-Item -Path $progId -Value 'CONTAINER DEV'
New-Item -Path "$progId\DefaultIcon" -Force | Out-Null
Set-Item -Path "$progId\DefaultIcon" -Value ('"{0}",0' -f $icon)
New-Item -Path "$progId\shell\open\command" -Force | Out-Null
Set-Item -Path "$progId\shell\open\command" -Value ('"{0}" "%1"' -f $executable)

$openWith = 'HKCU:\Software\Classes\.containerproject\OpenWithProgids'
New-Item -Path $openWith -Force | Out-Null
New-ItemProperty -Path $openWith -Name 'CONTAINER.Project.Dev' -Value ([byte[]]@()) -PropertyType None -Force | Out-Null

Add-Type -Namespace Native -Name ShellAssociation -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("shell32.dll")]
public static extern void SHChangeNotify(int eventId, uint flags, System.IntPtr item1, System.IntPtr item2);
'@
[Native.ShellAssociation]::SHChangeNotify(0x08000000, 0x1003, [IntPtr]::Zero, [IntPtr]::Zero)

Write-Host "Registered CONTAINER DEV for .containerproject without changing the default application."
