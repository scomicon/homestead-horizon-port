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

$settingsFile = Join-Path $root 'local-settings.json'
$settings = $null
if (Test-Path -LiteralPath $settingsFile) { $settings = Get-Content -LiteralPath $settingsFile -Raw | ConvertFrom-Json }
if (!$Resources -and $settings) { $Resources = $settings.resources }
if (!$SourceInstance -and $settings) { $SourceInstance = $settings.sourceInstance }
if (!$Resources -or !(Test-Path -LiteralPath (Join-Path $Resources 'facts.json'))) {
    throw 'Pass -Resources with your prepared Horizon 26.3 resource folder.'
}
$privateRoot = Join-Path $root 'private'
$game = Join-Path $privateRoot 'instance'
$resourcesCopy = Join-Path $privateRoot 'resources-26.3'
New-Item -ItemType Directory -Force -Path $game,$resourcesCopy | Out-Null
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
    if (!$SourceInstance) { throw 'Set sourceInstance in local-settings.json (or pass -SourceInstance): the folder of your own Homestead installation.' }
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
$process = Start-Process @start
Write-Output ('Native Create client started. PID: ' + $process.Id)
Write-Output ('Private instance: ' + $game)
Write-Output ('Log folder: ' + $session)
exit 0
