#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Logic mods group B: Panda's Falling Tree's (a player breaking a log fells the whole tree).
# Bridging Mod, Comforts and SimpleHats are not ported (docs/logic-b.md), so nothing tests them.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('LogicB')][string] $Phase='LogicB',
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
$transcript=Join-Path $game ('stage25-logicb-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage25-logicb-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Falling Trees: breaking the base of a leafy tree takes every log and leaf above it and drops the logs; a log pillar without leaves, a tree whose leaves a player placed (persistent) and a creative break fell nothing or nothing extra; breaking a middle log leaves the lower trunk; birch trees fall too; the axe is damaged once per extra log. Sneak mining cannot be driven from the control socket and is not tested. Each scenario is reported separately.'
    coordinates=@{origin=@(100,80,100)}
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
function Command([string] $Text) {
    $reply=Invoke-Ctl @('command',$Text)
    if (($reply.chat -join ' ') -match 'not loaded|Unknown block|Unknown item|Incorrect argument|Expected |Unknown or incomplete command') {
        throw ($Text+[Environment]::NewLine+($reply.chat -join ' '))
    }
    return $reply
}
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
function Count([string] $Id) {
    $inv=(Invoke-Ctl @('inventory')).counts
    if ($inv.ContainsKey($Id)) { return [int]$inv[$Id] }
    return 0
}
function Have([string] $Id) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x=100,y=82,z=100,distance=..8,nbt={Item:{id:"'+$Id+'"}}] run say logicb_item_found')).chat -join ' ')
    return (($text -match 'logicb_item_found') -or ((Count $Id) -ge 1))
}
function Reset-Grove {
    $null=Command 'fill 90 80 90 110 95 110 minecraft:air'
    $null=Command 'fill 90 79 90 110 79 110 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
}
# A tree of $Height logs at 100,80..,100: leaves in a 5x5 ring around the top two logs and a 3x3 cap
# above. Non-persistent leaves with distance 1 hold on without decaying.
function Plant-Tree([int] $Height,[string] $Wood='oak',[string] $Persistent='false') {
    $top=79+$Height
    $leaf='minecraft:'+$Wood+'_leaves[persistent='+$Persistent+',distance=1]'
    $null=Command ('fill 98 '+($top-1)+' 98 102 '+$top+' 102 '+$leaf)
    $null=Command ('fill 99 '+($top+1)+' 99 101 '+($top+1)+' 101 '+$leaf)
    $null=Command ('fill 100 80 100 100 '+$top+' 100 minecraft:'+$Wood+'_log')
    Wait-Ticks 6
}
function Chop([int] $Y=80) {
    $null=Command 'tp @s 100.5 80 97.5'
    Wait-Ticks 14
    $null=Invoke-Ctl @('look-at','block','100',[string]$Y,'100','north')
    $null=Invoke-Ctl @('break','100',[string]$Y,'100')
    Wait-Ticks 20
}
$null=Command 'tick unfreeze';$null=Command 'gamemode survival'
$null=Command 'gamerule random_tick_speed 0'
$null=Command 'forceload add 88 88 112 112'
$null=Command 'tp @s 100.5 80 97.5'
Wait-Ticks 40
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
Scenario 'breaking the base of a leafy oak fells the whole tree' {
    Reset-Grove
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
    Plant-Tree 5
    foreach ($y in 80..84) { if ((Block 100 $y 100) -ne 'minecraft:oak_log') { throw ('setup: '+(Block 100 $y 100)+' at y '+$y) } }
    Chop 80
    foreach ($y in 80..84) { if ((Block 100 $y 100) -ne 'minecraft:air') { throw ('log left at y '+$y+': '+(Block 100 $y 100)) } }
    foreach ($spot in @(@(98,83,98),@(102,84,102),@(99,84,101),@(100,85,100),@(101,85,99))) {
        if ((Block $spot[0] $spot[1] $spot[2]) -ne 'minecraft:air') { throw ('leaf left at '+($spot -join ' ')+': '+(Block $spot[0] $spot[1] $spot[2])) }
    }
}
Scenario 'the felled logs drop as items' {
    if (-not (Have 'minecraft:oak_log')) { throw 'no oak log item near the tree' }
}
Scenario 'a log pillar without leaves is not a tree' {
    Reset-Grove
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
    $null=Command 'fill 100 80 100 100 83 100 minecraft:oak_log'
    Wait-Ticks 6
    Chop 80
    if ((Block 100 80 100) -ne 'minecraft:air') { throw 'the base was not broken' }
    foreach ($y in 81..83) { if ((Block 100 $y 100) -ne 'minecraft:oak_log') { throw ('pillar log at y '+$y+' became '+(Block 100 $y 100)) } }
}
Scenario 'leaves a player placed (persistent) do not make a tree' {
    Reset-Grove
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
    Plant-Tree 5 'oak' 'true'
    Chop 80
    if ((Block 100 80 100) -ne 'minecraft:air') { throw 'the base was not broken' }
    if ((Block 100 81 100) -ne 'minecraft:oak_log') { throw ('the trunk above fell: '+(Block 100 81 100)) }
    if ((Block 98 83 98) -ne 'minecraft:oak_leaves') { throw ('a persistent leaf went: '+(Block 98 83 98)) }
}
Scenario 'breaking a middle log leaves the lower trunk' {
    Reset-Grove
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
    Plant-Tree 5
    Chop 82
    foreach ($y in 80,81) { if ((Block 100 $y 100) -ne 'minecraft:oak_log') { throw ('lower log at y '+$y+' is '+(Block 100 $y 100)) } }
    foreach ($y in 82..84) { if ((Block 100 $y 100) -ne 'minecraft:air') { throw ('log left at y '+$y+': '+(Block 100 $y 100)) } }
}
Scenario 'a birch tree falls too' {
    Reset-Grove
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
    Plant-Tree 6 'birch'
    Chop 80
    foreach ($y in 80..85) { if ((Block 100 $y 100) -ne 'minecraft:air') { throw ('birch log left at y '+$y+': '+(Block 100 $y 100)) } }
    if ((Block 98 84 98) -ne 'minecraft:air') { throw ('birch leaf left: '+(Block 98 84 98)) }
}
Scenario 'the axe is damaged once more for every extra log' {
    Reset-Grove
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:iron_axe'
    Plant-Tree 5
    Chop 80
    foreach ($y in 80..84) { if ((Block 100 $y 100) -ne 'minecraft:air') { throw ('log left at y '+$y) } }
    $text=((Command 'data get entity @s SelectedItem').chat -join ' ')
    $m=[regex]::Match($text,'(?i)damage\D{0,6}(\d+)')
    if (-not $m.Success) { throw ('no damage in the held item: '+$text) }
    # Four extra logs: at least four points (the break itself may add one more).
    if ([int]$m.Groups[1].Value -lt 4) { throw ('damage '+$m.Groups[1].Value+' in '+$text) }
}
Scenario 'a creative player fells the tree and gets no drops' {
    $null=Command 'gamemode creative'
    Reset-Grove
    Plant-Tree 5
    Chop 80
    foreach ($y in 80..84) { if ((Block 100 $y 100) -ne 'minecraft:air') { throw ('log left at y '+$y) } }
    if ((Block 98 83 98) -ne 'minecraft:air') { throw ('leaf left: '+(Block 98 83 98)) }
    if (Have 'minecraft:oak_log') { throw 'a creative break dropped a log' }
    $null=Command 'gamemode survival'
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
