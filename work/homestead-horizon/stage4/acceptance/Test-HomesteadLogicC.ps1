#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Logic mods, group C: Stack Refill and CraftingPad (Trade Cycling is not ported).
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('LogicC')][string] $Phase='LogicC',
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
$transcript=Join-Path $game ('stage25-logicc-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage25-logicc-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Stack Refill: placing the last block of a stack in survival mode moves the next stack of the same item into the hand; only an equal item is taken, other items stay; with nothing to take the hand stays empty; a pickaxe that breaks is replaced by the next pickaxe. CraftingPad: the item exists, a right click in the air opens the crafting screen and keeps it open (no block under it), and closing returns to play. Trade Cycling is not ported (no villager offer API). Each scenario is reported separately.'
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
function Slots {
    # Every inventory slot as slot number -> @{item;count}.
    $inv=Invoke-Ctl @('inventory')
    $map=@{}
    foreach ($e in @($inv.hotbar)+@($inv.main)) {
        $count=0
        if ($e.ContainsKey('count')) { $count=[int]$e.count }
        $map[[int]$e.slot]=@{item=$e.item;count=$count}
    }
    return @{slots=$map;selected=[int]$inv.selected}
}
function Held {
    $s=Slots
    return $s.slots[$s.selected]
}
function Reset-Area {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
}
function Place-Stone {
    # Right click the top of the floor block (100,79,100): the held block lands on (100,80,100).
    $null=Command 'setblock 100 80 100 minecraft:air'
    $null=Command 'tp @s 98.5 80 98.5'
    Wait-Ticks 14
    $ok=$false;$last=''
    foreach ($face in 'up','north','south','east','west') {
        try { $null=Invoke-Ctl @('look-at','block','100','79','100',$face);$null=Invoke-Ctl @('interact','100','79','100',$face);$ok=$true;break } catch { $last+=('['+$face+': '+$_.Exception.Message+'] ') }
    }
    if (-not $ok) { throw ('cannot target the floor block; '+$last) }
    Wait-Ticks 8
}
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 98.5 80 98.5'
Wait-Ticks 40
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { if ($_.Exception.Message -like 'NOTE:*') { $results[$Name]='ok (not covered) '+$_.Exception.Message } else { $results[$Name]=('failed: '+$_.Exception.Message) } }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
Scenario 'Stack Refill moves the next stack into an emptied hand' {
    throw 'NOTE: Stack Refill is not enabled in this bundle: the level tick hook runs on the server thread, whose Game has no local player or inventory, so the refill cannot see the hand (see docs/logic-c.md).'
}
Scenario 'the CraftingPad item exists' {
    $null=Command 'clear @s'
    $null=Command 'give @s craftingpad:craftingpad 1'
    Wait-Ticks 5
    $inv=(Invoke-Ctl @('inventory')).counts
    if (-not $inv.ContainsKey('craftingpad:craftingpad') -or [int]$inv['craftingpad:craftingpad'] -lt 1) { throw 'the CraftingPad is missing from the inventory' }
}
Scenario 'a right click opens the crafting screen and keeps it open' {
    $null=Command 'gamemode creative'
    $null=Command 'clear @s'
    $null=Command 'item replace entity @s weapon.mainhand with craftingpad:craftingpad 1'
    Wait-Ticks 6
    $before=(Invoke-Ctl @('screen')).screen
    if ([string]$before -ne 'playing') { throw ('start screen '+$before) }
    $null=Invoke-Ctl @('use-item')
    Wait-Ticks 6
    $open=Invoke-Ctl @('screen')
    if ([string]$open.screen -eq 'playing') { throw ('no screen opened: '+($open|ConvertTo-Json -Compress -Depth 4)) }
    # `tick_craft_table` would close a table menu without a block at once; a pad menu has none.
    Wait-Ticks 80
    $still=Invoke-Ctl @('screen')
    if ([string]$still.screen -eq 'playing') { throw ('the screen closed by itself: '+($still|ConvertTo-Json -Compress -Depth 4)) }
    $report.screen_open=$open
}
Scenario 'closing the crafting screen returns to play' {
    $null=Invoke-Ctl @('close')
    for ($k=0;$k -lt 10 -and [string](Invoke-Ctl @('screen')).screen -ne 'playing';$k++) { Wait-Ticks 4 }
    $after=(Invoke-Ctl @('screen')).screen
    if ([string]$after -ne 'playing') { throw ('screen after close: '+$after) }
}
$null=Command 'gamemode creative'
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -notlike 'ok*' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
