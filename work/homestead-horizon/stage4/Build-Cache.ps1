#requires -Version 7.0
# Adds a pre-built content cache to an assembled package so the first launch does not spend
# minutes generating ~100,000 files. Run AFTER Build-Package.ps1 (one game at a time):
#   pwsh .\Build-Cache.ps1 -Name homestead-horizon-stage19
# It starts the packaged client hidden once, waits until the cache is complete, stops it,
# stores .horizon-cache as <package>/cache/horizon-cache.zip and removes the private folder.
# Start-Homestead.ps1 restores the zip into a fresh private instance. If the hash of the
# installed mods differs, the game builds its own cache (slow but correct).
param(
    [Parameter(Mandatory)] [string] $Name,
    [int] $TimeoutMinutes = 20
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$pkg = Join-Path $workspace ('outputs/' + $Name)
if (-not (Test-Path -LiteralPath (Join-Path $pkg 'Start-Homestead.ps1'))) { throw "Package not found: $pkg" }
$binary = [IO.Path]::GetFullPath((Join-Path $pkg 'client/horizon-client.exe'))
$busy = @(Get-Process -Name 'horizon-client' -ErrorAction SilentlyContinue)
if ($busy.Count -gt 0) { throw ('Another horizon-client is running (PID ' + ($busy.Id -join ', ') + '); one game at a time.') }
if (Test-Path -LiteralPath (Join-Path $pkg 'private')) { Remove-Item -LiteralPath (Join-Path $pkg 'private') -Recurse -Force }
$cacheOut = Join-Path $pkg 'cache'
Remove-Item -LiteralPath $cacheOut -Recurse -Force -ErrorAction SilentlyContinue

# 1. Start the packaged client hidden (-Review makes it load the review world, which fills every cache).
# Output goes to a file, not a pipe: the client inherits a pipe handle and would block the read.
$launchLog = Join-Path ([IO.Path]::GetTempPath()) ('build-cache-' + [guid]::NewGuid().ToString('N') + '.txt')
$null = Start-Process -FilePath (Get-Process -Id $PID).Path -NoNewWindow -Wait -RedirectStandardOutput $launchLog `
    -ArgumentList @('-NoProfile','-File',(Join-Path $pkg 'Start-Homestead.ps1'),'-Review','-Offscreen')
$line = Get-Content -LiteralPath $launchLog -ErrorAction SilentlyContinue | Where-Object { $_ -match 'PID: (\d+)' } | Select-Object -First 1
if (-not $line -or $line -notmatch 'PID: (\d+)') { throw 'The client did not start.' }
$procId = [int]$Matches[1]
Write-Output "Client started (PID $procId); building the cache..."

$cache = Join-Path $pkg 'private/instance/.horizon-cache'
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
$ready = $false
try {
    while ((Get-Date) -lt $deadline) {
        if (-not (Get-Process -Id $procId -ErrorAction SilentlyContinue)) { throw 'The client exited before the cache was complete.' }
        $done = @(Get-ChildItem -Path (Join-Path $cache 'mod-content/*/.copy-complete') -Force -ErrorAction SilentlyContinue).Count -gt 0
        if ($done -and (Test-Path (Join-Path $cache 'registry-v1.bin')) -and (Test-Path (Join-Path $cache 'behaviours-v2.bin')) -and
            (Test-Path (Join-Path $cache 'create-assets'))) { $ready = $true; break }
        Start-Sleep -Seconds 10
    }
    if (-not $ready) { throw "The cache was not complete after $TimeoutMinutes minutes." }
    Start-Sleep -Seconds 20   # let the last writers finish
} finally {
    Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 3
}

# 2. Store the cache. Absolute paths are not recorded in it (checked on stage18).
New-Item -ItemType Directory -Force -Path $cacheOut | Out-Null
$zip = Join-Path $cacheOut 'horizon-cache.zip'
& tar.exe -a -cf $zip -C $cache .
if ($LASTEXITCODE -ne 0) { throw 'tar failed.' }
Remove-Item -LiteralPath (Join-Path $pkg 'private') -Recurse -Force
# The cache key depends on the package folder, so record where it is valid.
[IO.File]::WriteAllText((Join-Path $cacheOut 'built-for.txt'), [IO.Path]::GetFullPath($pkg).TrimEnd('\'))
$mb = [int]((Get-Item -LiteralPath $zip).Length / 1MB)
Write-Output "Cache stored: $zip ($mb MB). The private folder was removed."
