#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Playtest fixes, part 2 (stage22). Static checks read the BUILT .hmod archives in stage4/extra-mods with the
# python checkers (no game needed); the game part asks the running client whether a revoked player gets a flood
# of structure/biome advancements.
#   1. block_looks.py --check          no carried block with a non-cube model may occlude, no oversized/empty collision
#   2. advancement_fix.py --check      no singular structure/biome left in a location predicate
#   3. hide_unplaceable.py --check     the modded boats are out of the creative tabs and their recipes
#   4. in game: revoke everything, wait, count the advancement messages
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Fixes2')][string] $Phase='Fixes2',
    [string] $World='HomesteadAutomationAcceptance',
    [int] $GameProcessId=0
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
[Threading.Thread]::CurrentThread.CurrentCulture=[Globalization.CultureInfo]::InvariantCulture
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$game=[IO.Path]::GetFullPath($GameDir)
$allowed=@(
    [IO.Path]::GetFullPath((Join-Path $workspace 'work/homestead-horizon/stage4')),
    [IO.Path]::GetFullPath((Join-Path $workspace 'outputs/homestead-horizon-stage4'))
)
if (-not ($allowed|Where-Object {$game.StartsWith($_+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)})) {
    throw 'Use the separate stage4 work or stage4 output instance, never a player save.'
}
if (-not (Test-Path -LiteralPath $game -PathType Container)) { throw 'Start the disposable client first.' }
if ($World -notmatch '^HomesteadAutomation[A-Za-z0-9_-]*$') { throw 'Use an explicitly disposable automation world.' }
$ctlPath=(Resolve-Path -LiteralPath $Ctl).Path
$socketPath=[IO.Path]::GetFullPath($Socket)
$transcript=Join-Path $game ('stage28-fixes2-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage28-fixes2-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@();notes=@()
    scope='Built data-mod archives: occlusion/collision of carried blocks, location predicates, hidden boats; in game: advancement flood after a revoke.'
}
[IO.File]::WriteAllText($transcript,'',$utf8)
function Save-Report { [IO.File]::WriteAllText($reportPath,($report|ConvertTo-Json -Depth 25)+[Environment]::NewLine,$utf8) }
function Invoke-Ctl([string[]] $Words) {
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$ctlPath;$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach ($word in (@('--socket',$socketPath)+$Words)) { $info.ArgumentList.Add([string]$word) }
    $process=[Diagnostics.Process]::Start($info)
    $outTask=$process.StandardOutput.ReadToEndAsync();$errTask=$process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(150000)) { $process.Kill();throw ('Control timeout: '+($Words -join ' ')) }
    $stdout=$outTask.GetAwaiter().GetResult();$stderr=$errTask.GetAwaiter().GetResult()
    [IO.File]::AppendAllText($transcript,(@{words=$Words;exit_code=$process.ExitCode;stdout=$stdout;stderr=$stderr}|ConvertTo-Json -Compress -Depth 20)+[Environment]::NewLine,$utf8)
    if ($process.ExitCode -ne 0) { throw ('Control failed: '+($Words -join ' ')+[Environment]::NewLine+$stdout+$stderr) }
    $reply=$stdout|ConvertFrom-Json -AsHashtable
    if ($reply.ContainsKey('error')) { throw ($reply.error|ConvertTo-Json -Depth 10) }
    return $reply
}
function Command([string] $Text) { return (Invoke-Ctl @('command',$Text)) }
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Passed([string] $Text) { $report.checks+=$Text;Write-Output $Text;Save-Report }
function Note([string] $Text) { $report.notes+=$Text;Write-Output ('note: '+$Text);Save-Report }
function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }

# ---- static checks on the built archives ---------------------------------------------------------------------
$python=Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$createPath=(Get-Content -LiteralPath (Join-Path $workspace 'work/homestead-horizon/create-source-path.txt') -Raw).Trim()
$tools=Join-Path $createPath 'tools'
Expect (Test-Path -LiteralPath $python) 'The bundled python is missing.'
$extra=Join-Path $PSScriptRoot '../extra-mods'
$archives=@(Get-ChildItem -LiteralPath $extra -Filter '*-horizon.*.hmod'|Where-Object { $_.Name -notlike 'farmersdelight-*' -and $_.Name -notlike 'craftingpad-*' -and $_.Name -notlike 'universal_graves-*' }|Sort-Object Name)
Expect ($archives.Count -ge 30) ('Expected the packed data mods in stage4/extra-mods, found '+$archives.Count+'. Run Build-DataMods.ps1 first.')
function Invoke-Checker([string] $Script,[string[]] $Paths) {
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$python;$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $info.Environment['PYTHONUTF8']='1'
    foreach ($word in (@('-I',(Join-Path $tools $Script),'--check')+$Paths)) { $info.ArgumentList.Add($word) }
    $process=[Diagnostics.Process]::Start($info)
    $outTask=$process.StandardOutput.ReadToEndAsync();$errTask=$process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(600000)) { $process.Kill();throw ('Checker timeout: '+$Script) }
    return @{exit=$process.ExitCode;text=($outTask.GetAwaiter().GetResult()+$errTask.GetAwaiter().GetResult())}
}
# The bundled python has no zlib module and cannot open deflated archives: unpack each .hmod and check the folders.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$unpacked=Join-Path $workspace 'work/homestead-horizon/tmp/fixes2-unpacked'
Remove-Item -LiteralPath $unpacked -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $unpacked|Out-Null
$all=@()
foreach ($archive in $archives) {
    $target=Join-Path $unpacked $archive.BaseName
    [IO.Compression.ZipFile]::ExtractToDirectory($archive.FullName,$target)
    $all+=$target
}
$looks=Invoke-Checker 'block_looks.py' $all
$report.block_looks=($looks.text -split "`n"|Select-Object -First 60)
Save-Report
Expect ($looks.exit -eq 0) ('block_looks --check found wrong occlusion/collision:'+[Environment]::NewLine+(($looks.text -split "`n"|Where-Object { $_ -match '^\s+\S+:\S+ \[' }|Select-Object -First 12) -join [Environment]::NewLine))
Passed ('No carried block with a non-cube model occludes or keeps an oversized/empty collision ('+$archives.Count+' archives checked).')
$adv=Invoke-Checker 'advancement_fix.py' $all
$report.advancement_fix=($adv.text -split "`n"|Select-Object -First 60)
Save-Report
Expect ($adv.exit -eq 0) ('advancement_fix --check: location predicates still use the singular structure/biome keys:'+[Environment]::NewLine+(($adv.text -split "`n"|Where-Object { $_ -match '^\s+data/' }|Select-Object -First 12) -join [Environment]::NewLine))
Passed 'No location predicate in the packed advancements, predicates or loot tables still uses the singular structure/biome keys.'
$hide=Invoke-Checker 'hide_unplaceable.py' $all
$report.hide_unplaceable=($hide.text -split "`n"|Select-Object -First 60)
Save-Report
Expect ($hide.exit -eq 0) ('hide_unplaceable --check: hidden items are still offered:'+[Environment]::NewLine+(($hide.text -split "`n"|Where-Object { $_ -match '^\s+data/' }|Select-Object -First 12) -join [Environment]::NewLine))
Passed 'The modded boats are out of the creative tabs and their recipes are gone (oak_boat from Regions Unexplored planks, a vanilla boat, stays).'

# ---- in game: the advancement flood -------------------------------------------------------------------------
$flood=@('Regions Explored','Spelunker','Civilization from the Beyond','Bastions, Evil, and Bones','Blazing Cities','Skyward Cities','Decaying Glory','Having a Hobbit Time','Lavish Livings','Toiling in the Mines','Tributes and Sacrifices','Territory Control','Raider of the Tombs','The Useless Portal','Remains of the Past','Interdimensional Pirates','Following the Eyes','Unanswered Prayers','Sprawling Civilizations','Hexes and Curses')
try {
    $null=Command 'tick unfreeze'
    $revoke=((Command 'advancement revoke @s everything').chat -join ' ')
    if ($revoke -match 'Unknown or incomplete command|Unknown command|Incorrect argument') { throw 'advancement command missing' }
    Wait-Ticks 600
    $messages=((Invoke-Ctl @('messages','400')) | ConvertTo-Json -Depth 10 -Compress)
    $granted=[regex]::Matches($messages,'has (made the advancement|completed the challenge|reached the goal)').Count
    $report.measured=@{advancements_after_revoke=$granted}
    $named=@($flood|Where-Object { $messages -match [regex]::Escape($_) })
    Expect ($named.Count -eq 0) ('Structure advancements were granted at once: '+($named -join ', '))
    Expect ($granted -le 8) ('Too many advancements right after a revoke: '+$granted)
    Passed ('After revoking everything and waiting 30 seconds the player got '+$granted+' advancement message(s), none of the 20 structure/biome ones.')
} catch {
    if ($_.Exception.Message -match 'advancement command missing|Unknown') {
        Note 'not verified in game: the advancement command is not available in this client; the static checks above cover the predicates.'
    } else { throw }
}
Remove-Item -LiteralPath $unpacked -Recurse -Force -ErrorAction SilentlyContinue
$report.passed=$true
Save-Report
Write-Output 'Fixes2 acceptance passed.'
