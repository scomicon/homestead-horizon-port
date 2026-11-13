[CmdletBinding()]
param(
    [string]$Resources,
    [string]$SourceInstance,
    [string]$World,
    [switch]$Review,
    [switch]$Creative,
    [switch]$Offscreen,
    [switch]$NoAudio
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$binary = Join-Path $root 'client/horizon-client.exe'
if (!(Test-Path -LiteralPath $binary)) { throw 'The native client artifact is missing.' }
if ($Review) {
    if ($World) { throw 'Choose -Review or -World.' }
    $World = 'HomesteadAutomationReview4'
    $Creative = $true
}
if ($World -and ($World -notmatch '^[A-Za-z0-9_-]{1,64}$')) { throw 'World must be one ASCII folder name.' }

# One copy at a time: a second client (or closing the first one while it builds its
# content cache) restarts the cache build from scratch and corrupts the shared instance.
$running = @(Get-Process -Name 'horizon-client' -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -eq [IO.Path]::GetFullPath($binary)) })
if ($running.Count -gt 0) {
    throw ('The game from this folder is already running (PID ' + ($running.Id -join ', ') + '). ' +
        'After the first start it can sit on the loading screen for several minutes while it builds its cache. ' +
        'Wait for it instead of starting another copy; close it only from the game window.')
}

$settingsFile = Join-Path $root 'local-settings.json'
if ((!$Resources -or !$SourceInstance) -and (Test-Path -LiteralPath $settingsFile)) {
    $settings = Get-Content -LiteralPath $settingsFile -Raw | ConvertFrom-Json
    if (!$Resources) { $Resources = $settings.resources }
    if (!$SourceInstance) { $SourceInstance = $settings.sourceInstance }
}
if (!$SourceInstance) {
    throw 'Укажите путь к вашему Homestead: запустите Install-Homestead.cmd или заполните sourceInstance в local-settings.json.'
}
if (!$Resources -or !(Test-Path -LiteralPath (Join-Path $Resources 'facts.json'))) {
    throw 'Pass -Resources with your prepared Horizon 26.3 resource folder.'
}
$privateRoot = Join-Path $root 'private'
$game = Join-Path $privateRoot 'instance'
$resourcesCopy = Join-Path $privateRoot 'resources-26.3'
New-Item -ItemType Directory -Force -Path $game,$resourcesCopy | Out-Null

# Pre-built content cache shipped in cache/horizon-cache.zip (made by Build-Cache.ps1 from this
# exact client and mod set). It is restored once per private instance. If its hash does not
# match the installed mods the game simply builds its own cache next to it (slower, still correct).
$cacheZip = Join-Path $root 'cache/horizon-cache.zip'
$cacheDir = Join-Path $game '.horizon-cache'
$cacheMarker = Join-Path $cacheDir '.restored-from-package'
# The cache key depends on the folder the package sits in (verified: the same package in two
# folders gets two different hashes), so the zip is only used where it was built.
$builtFor = Join-Path $root 'cache/built-for.txt'
$cacheValid = (Test-Path -LiteralPath $cacheZip) -and (Test-Path -LiteralPath $builtFor) -and
    ((Get-Content -LiteralPath $builtFor -Raw).Trim() -ieq [IO.Path]::GetFullPath($root).TrimEnd('\'))
if ((Test-Path -LiteralPath $cacheZip) -and -not $cacheValid) {
    Write-Output 'The prepared cache belongs to another folder and is skipped; the game builds its own (3-5 minutes).'
}
if ($cacheValid -and !(Test-Path -LiteralPath $cacheMarker)) {
    Write-Output 'Restoring the prepared content cache (about 1 minute, one time)...'
    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
    & tar.exe -xf $cacheZip -C $cacheDir
    if ($LASTEXITCODE -ne 0) {
        Write-Warning 'The prepared cache could not be restored; the game will build its own.'
        Remove-Item -LiteralPath (Join-Path $cacheDir 'mod-content') -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        [IO.File]::WriteAllText($cacheMarker, (Get-Date -Format o))
    }
}
if (!(Test-Path -LiteralPath (Join-Path $resourcesCopy '.copy-complete'))) {
    Write-Output 'Preparing a private copy of your vanilla resources...'
    & robocopy $Resources $resourcesCopy /E /R:1 /W:1 /NFL /NDL /NJH /NJS
    if ($LASTEXITCODE -gt 7) { throw 'The private resource copy failed.' }
    [IO.File]::WriteAllText((Join-Path $resourcesCopy '.copy-complete'), '26.3')
}

$expectedHash = 'a613603fb08d967c5a4138d1adb7374c4a347b91e489fe21579a4cff4f40105a'
$inputs = Join-Path $game 'horizon-mods/create-resources'
$privateJar = Join-Path $inputs 'create.jar'
New-Item -ItemType Directory -Force -Path $inputs | Out-Null
if (!(Test-Path -LiteralPath $privateJar)) {
    $original = Join-Path $SourceInstance 'mods/create-fabric-6.0.8.1+build.1744-mc1.20.1.jar'
    if ((Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHash) {
        throw 'The supplied Create resource JAR differs from the verified Homestead version.'
    }
    Copy-Item -LiteralPath $original -Destination $privateJar
}
if ((Get-FileHash -LiteralPath $privateJar -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHash) {
    throw 'The private Create resource JAR changed.'
}
$addon = Join-Path $root 'mods/create_oxidized-0.1.1-horizon.1.hmod'
if (!(Test-Path -LiteralPath $addon)) { throw 'The bounded Create Oxidized content mod is missing.' }
# Every content mod shipped in mods/ is installed and enabled (id = file name up to the first '-').
Get-ChildItem -LiteralPath (Join-Path $game 'horizon-mods') -Filter *.hmod -ErrorAction SilentlyContinue | Remove-Item -Force
$enabled = @('mod.create:true')
foreach ($hmod in Get-ChildItem -LiteralPath (Join-Path $root 'mods') -Filter *.hmod) {
    Copy-Item -LiteralPath $hmod.FullName -Destination (Join-Path $game 'horizon-mods') -Force
    $enabled += ('mod.' + ($hmod.Name -replace '-.*$','') + ':true')
}
[IO.File]::WriteAllLines((Join-Path $game 'horizon-mods.txt'), $enabled, [Text.UTF8Encoding]::new($false))

if ($Review) {
    $reviewWorld = Join-Path $game ('saves/' + $World)
    if (!(Test-Path -LiteralPath $reviewWorld)) {
        $reviewZip = Join-Path $root 'review/world.zip'
        if (!(Test-Path -LiteralPath $reviewZip)) { throw 'The prepared review world is missing.' }
        Expand-Archive -LiteralPath $reviewZip -DestinationPath $reviewWorld -ErrorAction Stop
    }
}

# Horizon installs this template only into newly-created worlds.
$pack = Join-Path $root 'datapacks/homestead-create'
if (Test-Path -LiteralPath (Join-Path $pack 'pack.mcmeta')) {
    $templates = Join-Path $game 'new-world-datapacks'
    New-Item -ItemType Directory -Force -Path $templates | Out-Null
    Copy-Item -LiteralPath $pack -Destination $templates -Recurse -Force
}

$session = Join-Path $privateRoot ('runs/' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0,8))
New-Item -ItemType Directory -Force -Path $session | Out-Null
foreach ($pair in @(
    @('HORIZON_HOME','home'), @('HORIZON_LAUNCHER_HOME','launcher'),
    @('XDG_DATA_HOME','data'), @('XDG_CONFIG_HOME','config'),
    @('HORIZON_LAUNCHER_TRASH','trash'), @('HORIZON_LAUNCHER_CLIPBOARD','clipboard'),
    @('HORIZON_LAUNCHER_DESKTOP','desktop')
)) {
    $path = Join-Path $session $pair[1]
    New-Item -ItemType Directory -Path $path | Out-Null
    [Environment]::SetEnvironmentVariable($pair[0],$path,'Process')
}
$env:HORIZON_LAUNCHER_NO_SELF_UPDATE = '1'
if ($Offscreen -or $NoAudio) {
    $env:HORIZON_NO_AUDIO = '1'
} else {
    Remove-Item Env:HORIZON_NO_AUDIO -ErrorAction SilentlyContinue
}
$env:TEMP = Join-Path $session 'tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Path $env:TEMP | Out-Null
$arguments = @('--resources',$resourcesCopy,'--game-dir',$game,'--render-distance','6')
if ($World) { $arguments += @('--world',$World,'--seed','5') }
if ($Review) { $arguments += @('--name','HomesteadTest') }
if ($Creative) { $arguments += '--creative' }
if ($Offscreen) { $arguments += @('--offscreen','--control-socket',(Join-Path $session 'ctl.json')) }
$quoted = @($arguments | ForEach-Object {
    if ($_.Contains('"')) { throw 'A path contains a quote.' }
    '"' + $_ + '"'
})
$start = @{
    FilePath = $binary
    ArgumentList = $quoted
    WorkingDirectory = $game
    RedirectStandardOutput = (Join-Path $session 'stdout.log')
    RedirectStandardError = (Join-Path $session 'stderr.log')
    PassThru = $true
}
if ($Offscreen) { $start.WindowStyle = 'Hidden' }
$cacheReady = @(Get-ChildItem -Path (Join-Path $cacheDir 'mod-content/*/.copy-complete') -Force -ErrorAction SilentlyContinue).Count -gt 0
$process = Start-Process @start
Write-Output ('Native Create client started. PID: ' + $process.Id)
if (-not $cacheReady) {
    Write-Output ''
    Write-Output 'FIRST START: the game is building its content cache (about 100,000 files).'
    Write-Output 'This takes about 3-5 minutes and the window may stay on the loading screen.'
    Write-Output 'DO NOT close the game and do not start a second copy; later starts take seconds.'
    if (-not $Offscreen) { Start-Sleep -Seconds 15 }
}
Write-Output ('Private instance: ' + $game)
Write-Output ('Log folder: ' + $session)
exit 0
