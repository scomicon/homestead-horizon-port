#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Logic mods group D: Universal Graves (the grave block, its 54 slots, protection, breaking). SpawnFilter
# acts on a respawn and on a first login to a young world; the control socket cannot respawn the player, so
# it has no scenario here (see docs/logic-d.md for the manual check). Grind Enchantments is not ported.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('LogicD')][string] $Phase='LogicD',
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
$transcript=Join-Path $game ('stage25-logicd-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage25-logicd-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Universal Graves: the grave block places, holds 54 slots, keeps what is put in, breaks and drops its items once, an operator opens and breaks a protected grave of another owner. The death path (loot drops into a grave) and SpawnFilter need a death and a respawn, which the control socket cannot do: they are not covered.'
    coordinates=@{origin=@(100,90,100)}
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
function Counts([int] $X,[int] $Y,[int] $Z) {
    $counts=@{}
    foreach ($slot in (Invoke-Ctl @('container',[string]$X,[string]$Y,[string]$Z)).slots) {
        if ($null -ne $slot) {
            $id=[string]$slot.item
            if (-not $counts.ContainsKey($id)) { $counts[$id]=0 }
            $counts[$id]+=[int]$slot.count
        }
    }
    return $counts
}
function Slot-Count([int] $X,[int] $Y,[int] $Z) { return @((Invoke-Ctl @('container',[string]$X,[string]$Y,[string]$Z)).slots).Count }
function Have([string] $Id) {
    $inv=(Invoke-Ctl @('inventory')).counts
    if ($inv.ContainsKey($Id) -and [int]$inv[$Id] -ge 1) { return $true }
    $text=((Command ('execute if entity @e[type=minecraft:item,x=100,y=81,z=100,distance=..6,nbt={Item:{id:"'+$Id+'"}}] run say logicd_item_found')).chat -join ' ')
    return ($text -match 'logicd_item_found')
}
function Reset-Table {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
}
function Place-Grave {
    Reset-Table
    $null=Command 'setblock 100 80 100 universal_graves:grave'
    Wait-Ticks 8
}
function Break-Grave {
    $null=Command 'tp @s 100.5 82 100.5'
    Wait-Ticks 14
    $null=Invoke-Ctl @('look-at','block','100','80','100','up')
    $null=Invoke-Ctl @('break','100','80','100')
    $null=Command 'tp @s 103.5 80 103.5'
    Wait-Ticks 14
}
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 98.5 80 98.5'
Wait-Ticks 40
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
Scenario 'the grave block places' {
    Place-Grave
    if ((Block 100 80 100) -ne 'universal_graves:grave') { throw ('found '+(Block 100 80 100)) }
}
Scenario 'the grave has 54 slots' {
    Place-Grave
    # The control's container reply lists only the filled slots, so probe the first and the last slot.
    $null=Command 'item replace block 100 80 100 container.0 with minecraft:diamond 1'
    $null=Command 'item replace block 100 80 100 container.53 with minecraft:iron_ingot 1'
    try { $null=Command 'item replace block 100 80 100 container.54 with minecraft:gold_ingot 1' } catch { }
    Wait-Ticks 4
    $c=Counts 100 80 100
    if (-not $c.ContainsKey('minecraft:diamond') -or -not $c.ContainsKey('minecraft:iron_ingot')) { throw ('slots 0 and 53 do not both hold: '+($c|ConvertTo-Json -Compress)) }
    if ($c.ContainsKey('minecraft:gold_ingot')) { throw 'slot 54 exists: the grave has more than 54 slots' }
}
Scenario 'the grave keeps what is put in' {
    Place-Grave
    $null=Command 'item replace block 100 80 100 container.0 with minecraft:diamond 5'
    $null=Command 'item replace block 100 80 100 container.40 with minecraft:iron_ingot 3'
    Wait-Ticks 4
    $c=Counts 100 80 100
    if (-not $c.ContainsKey('minecraft:diamond') -or $c['minecraft:diamond'] -ne 5) { throw ('diamonds '+($c|ConvertTo-Json -Compress)) }
    if (-not $c.ContainsKey('minecraft:iron_ingot') -or $c['minecraft:iron_ingot'] -ne 3) { throw ('iron '+($c|ConvertTo-Json -Compress)) }
}
Scenario 'breaking an ownerless grave drops its items' {
    Place-Grave
    $null=Command 'item replace block 100 80 100 container.0 with minecraft:diamond 5'
    Break-Grave
    if (-not (Have 'minecraft:diamond')) { throw 'the diamonds were lost' }
    if ((Block 100 80 100) -eq 'universal_graves:grave') { throw 'the grave is still there' }
}
Scenario 'an operator breaks a protected grave of another owner' {
    Place-Grave
    $null=Command 'item replace block 100 80 100 container.0 with minecraft:emerald 2'
    # A fake owner and a creation time far in the future: younger than 900 s, so protected.
    $null=Command 'data merge block 100 80 100 {OwnerMost:1L,OwnerLeast:2L,Created:999999999L}'
    Wait-Ticks 4
    Break-Grave
    if (-not (Have 'minecraft:emerald')) { throw 'the operator could not break the protected grave' }
}
Scenario 'an expired grave of another owner opens too' {
    Place-Grave
    $null=Command 'item replace block 100 80 100 container.0 with minecraft:gold_ingot 4'
    $null=Command 'data merge block 100 80 100 {OwnerMost:1L,OwnerLeast:2L,Created:0L}'
    Wait-Ticks 4
    Break-Grave
    if (-not (Have 'minecraft:gold_ingot')) { throw 'the gold was lost' }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -notlike 'ok*' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
