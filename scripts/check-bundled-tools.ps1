param(
    [string]$ConfigPath = (Join-Path $PSScriptRoot '..\config\bundled-tools.env'),
    [string]$Repository = $env:GITHUB_REPOSITORY,
    [string]$GitHubToken = $env:GITHUB_TOKEN,
    [string]$YtDlpLatestOverride = '',
    [string]$FfmpegLatestOverride = '',
    [switch]$NoIssue
)

$ErrorActionPreference = 'Stop'
$config = @{}
Get-Content -LiteralPath $ConfigPath | ForEach-Object {
    $line = $_.Trim()
    if ($line -and -not $line.StartsWith('#')) {
        $key, $value = $line -split '=', 2
        if (-not $key -or -not $value) { throw "Invalid bundled tool configuration line: $line" }
        $config[$key] = $value
    }
}
foreach ($required in 'FFMPEG_VERSION', 'YT_DLP_VERSION', 'YT_DLP_SHA256') {
    if (-not $config[$required]) { throw "$required is missing from $ConfigPath" }
}
if ($config.YT_DLP_SHA256 -notmatch '^[0-9a-f]{64}$') {
    throw 'YT_DLP_SHA256 must be a lowercase SHA-256 value.'
}

$headers = @{
    Accept = 'application/vnd.github+json'
    'User-Agent' = 'CONTAINER-bundled-tool-watch'
    'X-GitHub-Api-Version' = '2022-11-28'
}
$latestYtDlp = $YtDlpLatestOverride
if (-not $latestYtDlp) {
    $latestYtDlp = (Invoke-RestMethod -Uri 'https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest' -Headers $headers).tag_name
}
$latestFfmpeg = $FfmpegLatestOverride
if (-not $latestFfmpeg) {
    $line = choco search ffmpeg-full --exact --limit-output --source=https://community.chocolatey.org/api/v2/ |
        Where-Object { $_ -match '^ffmpeg-full\|(.+)$' } |
        Select-Object -First 1
    if (-not $line) { throw 'The latest ffmpeg-full version could not be read from Chocolatey.' }
    $latestFfmpeg = ($line -split '\|', 2)[1].Trim()
}
if (-not $latestYtDlp -or -not $latestFfmpeg) { throw 'An upstream version was empty.' }

$updates = @()
if ($config.YT_DLP_VERSION -ne $latestYtDlp) {
    $updates += "- yt-dlp: ``$($config.YT_DLP_VERSION)`` → ``$latestYtDlp``"
}
if ($config.FFMPEG_VERSION -ne $latestFfmpeg) {
    $updates += "- FFmpeg + FFprobe: ``$($config.FFMPEG_VERSION)`` → ``$latestFfmpeg``"
}

$summary = if ($updates.Count) {
    "Bundled media tool updates found:`n$($updates -join "`n")"
} else {
    "Bundled media tools are current: yt-dlp $latestYtDlp; FFmpeg/FFprobe $latestFfmpeg."
}
Write-Host $summary
if ($env:GITHUB_STEP_SUMMARY) { Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $summary }
if ($NoIssue) { return }
if (-not $Repository -or -not $GitHubToken) {
    throw 'Repository and GitHubToken are required when issue synchronization is enabled.'
}
$headers.Authorization = "Bearer $GitHubToken"
$title = '[toolchain] Bundled media tools update available'
$issues = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/issues?state=open&per_page=100" -Headers $headers
$existing = $issues | Where-Object { $_.title -eq $title -and -not $_.pull_request } | Select-Object -First 1

if (-not $updates.Count) {
    if ($existing) {
        $payload = @{ state = 'closed'; state_reason = 'completed' } | ConvertTo-Json
        Invoke-RestMethod -Method Patch -Uri $existing.url -Headers $headers -ContentType 'application/json' -Body $payload | Out-Null
    }
    return
}

$body = @"
The weekly supply-chain check found newer bundled media tooling:

$($updates -join "`n")

This check intentionally does **not** download or replace binaries automatically.

Release checklist:
- Review the upstream release notes and source.
- Update `config/bundled-tools.env`.
- For yt-dlp, record the official Windows executable SHA-256 and refresh bundled license files.
- Run frontend, Rust, FFmpeg/FFprobe, downloader, cancellation and package tests.
- Ship the verified binaries only through a signed CONTAINER release.

Sources:
- https://github.com/yt-dlp/yt-dlp/releases/latest
- https://community.chocolatey.org/packages/ffmpeg-full
"@
$payload = @{ title = $title; body = $body } | ConvertTo-Json
if ($existing) {
    Invoke-RestMethod -Method Patch -Uri $existing.url -Headers $headers -ContentType 'application/json' -Body $payload | Out-Null
} else {
    Invoke-RestMethod -Method Post -Uri "https://api.github.com/repos/$Repository/issues" -Headers $headers -ContentType 'application/json' -Body $payload | Out-Null
}
