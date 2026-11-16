#requires -Version 7.0
[CmdletBinding()]
param([string] $World='HomesteadAutomationAcceptance')
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$run=Join-Path $PSScriptRoot 'runtime'
$game=Join-Path $run 'game'
$resources=Join-Path $workspace 'work/homestead-horizon/runtime-01/resources'
$candidate=Join-Path $PSScriptRoot 'candidate'
$binary=Join-Path $candidate 'client/horizon-client.exe'
if (-not (Test-Path -LiteralPath $binary)) { throw 'Run Prepare-Candidate.ps1 first.' }
if (-not (Test-Path -LiteralPath (Join-Path $resources 'facts.json'))) { throw 'The owned vanilla resource snapshot is missing.' }
if ($World -notmatch '^HomesteadAutomation[A-Za-z0-9_-]*$') { throw 'Use a disposable automation world.' }
$save=Join-Path $game ('saves/'+$World)
New-Item -ItemType Directory -Force -Path $game|Out-Null
if (-not (Test-Path -LiteralPath $save)) {
    $worldZip=@('stage22','stage20','stage18','stage17','stage2') | ForEach-Object { Join-Path $workspace ('outputs/homestead-horizon-'+$_+'/review/world.zip') } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $worldZip) { throw 'No review world.zip found in outputs (stage2, stage18 or stage17).' }
    Expand-Archive -LiteralPath $worldZip -DestinationPath $save
}
# The stage2 review save has the 27-recipe pack already enabled. Update only
# this owned copy before startup so the new zero-duration recipe is loaded.
$mainPack=Join-Path $PSScriptRoot '../stage3/recipes/homestead-main-v3/datapack'
if (-not (Test-Path -LiteralPath (Join-Path $mainPack 'data/homestead_create/recipe/ancient_debris_processing_crushing.json'))) {
    throw 'Generate the complete 28-recipe stage4 pack before starting acceptance.'
}
$worldPack=Join-Path $save 'datapacks/homestead-create'
New-Item -ItemType Directory -Force -Path $worldPack|Out-Null
foreach ($entry in Get-ChildItem -LiteralPath $mainPack) {
    Copy-Item -LiteralPath $entry.FullName -Destination $worldPack -Recurse -Force
}
$mods=Join-Path $game 'horizon-mods'
$resourceInputs=Join-Path $mods 'create-resources'
New-Item -ItemType Directory -Force -Path $resourceInputs|Out-Null
$privateJar=Join-Path $resourceInputs 'create.jar'
if (-not (Test-Path -LiteralPath $privateJar)) {
    $oldJar=Join-Path $workspace 'work/homestead-horizon/runtime-01/game/horizon-mods/create-resources/create.jar'
    Copy-Item -LiteralPath $oldJar -Destination $privateJar
}
if ((Get-FileHash -LiteralPath $privateJar -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'a613603fb08d967c5a4138d1adb7374c4a347b91e489fe21579a4cff4f40105a') {
    throw 'The owned private Create resource input does not match the verified Homestead JAR.'
}
$addon=@('stage22','stage20','stage18','stage17','stage2') | ForEach-Object { Join-Path $workspace ('outputs/homestead-horizon-'+$_+'/mods/create_oxidized-0.1.1-horizon.1.hmod') } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $addon) { throw 'create_oxidized hmod not found in outputs.' }
Copy-Item -LiteralPath $addon -Destination $mods -Force
$enabled=@('mod.create:true','mod.create_oxidized:true')
$extra=Join-Path $PSScriptRoot 'extra-mods'
Get-ChildItem -LiteralPath $mods -Filter *.hmod | Where-Object { $_.Name -notlike 'create_oxidized*' } | Remove-Item -Force
if (Test-Path -LiteralPath $extra) {
    foreach ($hmod in Get-ChildItem -LiteralPath $extra -Filter *.hmod) {
        Copy-Item -LiteralPath $hmod.FullName -Destination $mods -Force
        $enabled+=('mod.'+($hmod.Name -replace '-.*$','')+':true')
    }
}
[IO.File]::WriteAllLines((Join-Path $game 'horizon-mods.txt'),$enabled,[Text.UTF8Encoding]::new($false))
# Client defaults suite (-ClientDefaults): start from a clean slate and apply the package defaults,
# exactly as Start-Homestead.ps1 does. Every other suite runs with no extras file (vanilla behaviour).
$extrasFile=Join-Path $game 'horizon-extras.txt'
$defaultsMarker=Join-Path $game '.homestead-client-defaults'
Remove-Item -LiteralPath $extrasFile,$defaultsMarker -ErrorAction SilentlyContinue
if ($env:HOMESTEAD_CLIENT_DEFAULTS) {
    . (Join-Path $PSScriptRoot 'package/Apply-ClientDefaults.ps1')
    $null=Set-HomesteadClientDefaults -GameDir $game
}
# Client sound suite leftovers: an interrupted -ClientS run must not leave rules for the next suites.
foreach ($f in 'homestead-muffler.txt','homestead-footsteps.txt','homestead-ambient.txt','homestead-sound-status.txt') {
    Remove-Item -LiteralPath (Join-Path (Join-Path $game 'config') $f) -ErrorAction SilentlyContinue
}
# Welcome Screen (clientH): other suites must never see it; -ClientH deletes the marker itself.
$welcomeDir=Join-Path $game 'horizon-mods/.horizon-mod-data/homestead'
New-Item -ItemType Directory -Force -Path $welcomeDir | Out-Null
[IO.File]::WriteAllText((Join-Path $welcomeDir 'welcome-shown'),'1')
$session=Join-Path $run ('session-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $session|Out-Null
foreach ($pair in @(@('HORIZON_HOME','home'),@('HORIZON_LAUNCHER_HOME','launcher'),@('XDG_DATA_HOME','data'),@('XDG_CONFIG_HOME','config'),@('HORIZON_LAUNCHER_TRASH','trash'),@('HORIZON_LAUNCHER_CLIPBOARD','clipboard'),@('HORIZON_LAUNCHER_DESKTOP','desktop'))) {
    $path=Join-Path $session $pair[1]
    New-Item -ItemType Directory -Path $path|Out-Null
    [Environment]::SetEnvironmentVariable($pair[0],$path,'Process')
}
foreach ($name in @('CREATE_HORIZON_TRACE','CREATE_HORIZON_TRACE_POSES','CREATE_HORIZON_PARTICLE_TRACE')) { Remove-Item -LiteralPath ('Env:'+$name) -ErrorAction SilentlyContinue }
# -ClientS needs a live sound engine (the playback hook only runs when a sound actually starts), so it keeps audio on.
if ($env:HOMESTEAD_AUDIO) { Remove-Item Env:HORIZON_NO_AUDIO -ErrorAction SilentlyContinue } else { $env:HORIZON_NO_AUDIO='1' }
$env:HORIZON_NO_DISCORD='1';$env:HORIZON_LAUNCHER_NO_SELF_UPDATE='1'
$env:HORIZON_API_DISCOVERY='http://127.0.0.1:9/'
$env:TEMP=Join-Path $session 'tmp';$env:TMP=$env:TEMP
New-Item -ItemType Directory -Path $env:TEMP|Out-Null
$socket=Join-Path $session 'ctl.json'
$screenArg=if ($env:HOMESTEAD_WINDOWED) { @() } else { @('--offscreen') }
$arguments=@('--resources',$resources,'--game-dir',$game,'--world',$World,'--seed','5','--creative','--name','HomesteadTest','--render-distance','2','--width','1280','--height','720','--control-socket',$socket)+$screenArg
$quoted=@($arguments|ForEach-Object { if ($_.Contains('"')) { throw 'A native argument contains a quote.' }; '"'+$_+'"' })
$process=Start-Process -FilePath $binary -ArgumentList $quoted -WorkingDirectory $game -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $session 'stdout.log') -RedirectStandardError (Join-Path $session 'stderr.log')
$record=@{process_id=$process.Id;socket=$socket;game=$game;session=$session;binary=$binary;resources=$resources;world=$World}
[IO.File]::WriteAllText((Join-Path $run 'current-run.json'),($record|ConvertTo-Json)+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
Write-Output ('Started isolated stage4 native client PID '+$process.Id)

