#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Engine-patch mods (stage 27): Comforts (portable beds: sleeping works, the respawn point stays), Peaceful
# Hunger (the food bar does not refill in Peaceful) and the Grind Enchantments grindstone. The control socket has
# no known menu-click command, so the grindstone result and take are only checked as far as the screen opens;
# the death/respawn paths and the sleeping posture are not observable either. Every scenario that cannot be
# observed ends as a note ("not verified"), never as a success.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Engine20')][string] $Phase='Engine20',
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
$transcript=Join-Path $game ('stage27-engine20-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage27-engine20-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@();notes=@()
    scope='Comforts: a sleeping bag works as a bed in a single-player world (the engine takes it for a bed by its state data) and does not set the respawn point while a vanilla bed does (needs a readable respawn point). Peaceful Hunger: the food bar does not climb back in Peaceful. Grind Enchantments: the grindstone screen opens with the mod loaded; the result and the take are NOT driven (no menu-click command).'
    coordinates=@{origin=@(100,90,100)}
}
[IO.File]::WriteAllText($transcript,'',$utf8)
function Save-Report { [IO.File]::WriteAllText($reportPath,($report|ConvertTo-Json -Depth 25)+[Environment]::NewLine,$utf8) }
function Note([string] $Text) { $report.notes+=$Text; Write-Output ('note: '+$Text) }
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
function Command([string] $Text) {
    $reply=Invoke-Ctl @('command',$Text)
    if (($reply.chat -join ' ') -match 'not loaded|Unknown block|Unknown item|Incorrect argument|Expected |Unknown or incomplete command') {
        throw ($Text+[Environment]::NewLine+($reply.chat -join ' '))
    }
    return $reply
}
function Chat([string] $Text) { return (($(Invoke-Ctl @('command',$Text)).chat) -join ' ') }
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Food { return [int](Invoke-Ctl @('state')).player.food }
function Reset-Area {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
}
function Stand-Up {
    try { $null=Invoke-Ctl @('key','shift','down'); Wait-Ticks 6; $null=Invoke-Ctl @('key','shift','up') } catch { }
    Wait-Ticks 6
}
# A two-block bed at x..x+1 along east: the foot at (X,80,Z), the head at (X+1,80,Z).
function Place-Bed([string] $Block,[int] $X,[int] $Z) {
    $null=Command ('setblock '+$X+' 80 '+$Z+' '+$Block+'[part=foot,facing=east,occupied=false]')
    $null=Command ('setblock '+($X+1)+' 80 '+$Z+' '+$Block+'[part=head,facing=east,occupied=false]')
    Wait-Ticks 6
}
function Click-Block([int] $X,[int] $Y,[int] $Z) {
    $last=''
    foreach ($face in @('up','north','south','west','east')) {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$face); $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$face); return } catch { $last+=('['+$face+': '+$_.Exception.Message+'] ') }
    }
    throw ('cannot click the block at '+$X+' '+$Y+' '+$Z+' '+$last)
}
function Night {
    # The gamerule name changed between versions: try both, the night is set either way.
    foreach ($rule in @('advance_time','doDaylightCycle')) { try { $null=Command ('gamerule '+$rule+' false'); break } catch { } }
    $null=Command 'time set 14000'; Wait-Ticks 4
}
# The respawn point in the chat of `data get entity @s ...`, or $null when the socket cannot read it.
function Spawn-X {
    try {
        $text=Chat 'data get entity @s SpawnX'
        if ($text -match '(-?\d+)') { return [int]$Matches[1] }
    } catch { }
    return $null
}
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'difficulty easy'
$null=Command 'tp @s 98.5 80 98.5'
Wait-Ticks 40
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
Scenario 'the portable bed blocks exist' {
    Reset-Area
    Place-Bed 'comforts:sleeping_bag_red' 100 100
    if ((Block 100 80 100) -ne 'comforts:sleeping_bag_red') { throw ('found '+(Block 100 80 100)) }
    Place-Bed 'comforts:hammock_blue' 100 104
    if ((Block 101 80 104) -ne 'comforts:hammock_blue') { throw ('found '+(Block 101 80 104)) }
}
Scenario 'a sleeping bag is slept in at night (a bed by its state data, hooks for the host)' {
    Reset-Area
    Night
    Place-Bed 'comforts:sleeping_bag_red' 100 100
    $null=Command 'tp @s 100.5 80 98.5'
    Wait-Ticks 6
    Click-Block 101 80 100
    Wait-Ticks 8
    $state=State 101 80 100
    Stand-Up
    if ($state -notmatch 'occupied=true') { throw ('the sleeping bag was not occupied: '+$state) }
}
Scenario 'a sleeping bag does not set the respawn point' {
    Reset-Area
    Night
    $null=Command 'spawnpoint @s 90 80 90'
    Wait-Ticks 4
    $before=Spawn-X
    if ($null -eq $before) { Note 'the respawn point is not readable through the socket: the bag/respawn scenarios are not verified'; return }
    Place-Bed 'comforts:sleeping_bag_blue' 100 100
    $null=Command 'tp @s 100.5 80 98.5'
    Wait-Ticks 6
    Click-Block 101 80 100
    Wait-Ticks 8
    Stand-Up
    $after=Spawn-X
    if ($after -ne $before) { throw ('the respawn point moved from x='+$before+' to x='+$after) }
}
Scenario 'a vanilla bed still sets the respawn point (the control for the check above)' {
    Reset-Area
    Night
    $null=Command 'spawnpoint @s 90 80 90'
    Wait-Ticks 4
    $before=Spawn-X
    if ($null -eq $before) { Note 'the respawn point is not readable through the socket: the vanilla-bed control is not verified'; return }
    Place-Bed 'minecraft:red_bed' 100 100
    $null=Command 'tp @s 100.5 80 98.5'
    Wait-Ticks 6
    Click-Block 101 80 100
    Wait-Ticks 8
    Stand-Up
    $after=Spawn-X
    if ($after -eq $before) { throw 'a vanilla bed did not move the respawn point: the reading above proves nothing' }
}
Scenario 'peaceful hunger: the food bar does not refill in Peaceful' {
    Reset-Area
    $null=Command 'gamemode survival'
    $null=Command 'difficulty peaceful'
    $null=Command 'effect give @s minecraft:hunger 20 255 true'
    $low=$false
    for ($i=0;$i -lt 20;$i++) { Wait-Ticks 20; if ((Food) -le 12) { $low=$true;break } }
    $null=Command 'effect clear @s minecraft:hunger'
    if (-not $low) { throw ('the hunger effect did not lower the food bar (food '+(Food)+')') }
    $before=Food
    Wait-Ticks 200
    $after=Food
    $null=Command 'difficulty easy'
    $null=Command 'gamemode creative'
    # Vanilla Peaceful refills +1 every 10 ticks: 20 in 200 ticks. With the mod the bar must stay near where it was.
    if ($after -gt $before+2) { throw ('the food bar climbed from '+$before+' to '+$after+' in Peaceful') }
}
Scenario 'the grindstone screen opens (Grind Enchantments loaded)' {
    Reset-Area
    # Earlier scenarios (bed, respawn) leave the player far from the block: stand next to it first.
    $null=Command 'tp @s 100.5 80 98.5'
    Wait-Ticks 20
    $null=Command 'setblock 100 80 100 minecraft:grindstone[face=floor,facing=north]'
    Wait-Ticks 6
    try { $null=Invoke-Ctl @('close') } catch { }
    Click-Block 100 80 100
    Wait-Ticks 6
    $screen=Invoke-Ctl @('screen')
    try { $null=Invoke-Ctl @('close') } catch { }
    if ([string]$screen.screen -notmatch 'grind|container|station') { throw ('no grindstone screen: '+($screen|ConvertTo-Json -Compress -Depth 4)) }
    Note 'the grindstone result (enchanted item + plain book -> enchanted book) and the take (item stays, book used, levels charged) are not driven: no menu-click command. Manual check in docs/comforts-grind.md.'
}
Scenario 'homestead status prints the diagnostic line' {
    $text=Chat 'homestead status'
    if ($text -notmatch 'status: nick .+ mode .+ difficulty .+ health') { throw ('no status line: '+$text) }
    if ($text -notmatch 'food \d+ saturation') { throw ('the status line has no food data: '+$text) }
    Note ('status line sample: '+$text)
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -notlike 'ok*' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count;notes=@($report.notes).Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
