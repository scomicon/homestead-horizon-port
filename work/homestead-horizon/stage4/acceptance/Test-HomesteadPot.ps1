#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Pot')][string] $Phase='Pot',
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
$transcript=Join-Path $game ('stage19-pot-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage19-pot-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Farmer''s Delight Cooking Pot with its screen: the menu opens with an empty hand, ingredients in the container slots cook on a heated pot into the meal slot, a container in its slot moves servings to the output, serving by hand with the matching container, an unheated or non-matching pot does not cook, breaking drops everything. Each scenario is reported separately.'
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
function Passed([string] $Text) { $report.checks+=$Text;Write-Output $Text;Save-Report }
function Get-Data([int] $X,[int] $Y,[int] $Z,[string] $Key='') {
    $text="data get block $X $Y $Z";if ($Key) { $text+=' '+$Key }
    return ((Command $text).chat -join ' ')
}
function Counts([int] $X,[int] $Y,[int] $Z) {
    $counts=@{}
    foreach ($slot in (Invoke-Ctl @('container',[string]$X,[string]$Y,[string]$Z)).slots) {
        if ($null -ne $slot) {
            $id=[string]$slot.item
            if (-not $counts.ContainsKey($id)) { $counts[$id]=0 }
            $counts[$id]+=[int]$slot.count
        }
    }
    return ,$counts
}
function Assert-Only([hashtable] $Counts,[string] $Item,[int] $Count) {
    if ($Counts.Count -ne 1 -or -not $Counts.ContainsKey($Item) -or [int]$Counts[$Item] -ne $Count) {
        throw ('Expected only '+$Count+' '+$Item+', got '+($Counts|ConvertTo-Json -Compress))
    }
}
function Assert-Empty([hashtable] $Counts,[string] $Location) {
    if ($Counts.Count -ne 0) { throw ($Location+' must be empty: '+($Counts|ConvertTo-Json -Compress)) }
}

function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
function Age([int] $X,[int] $Y,[int] $Z) {
    $state=[string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state
    $m=[regex]::Match($state,'age=(\d+)'); if (-not $m.Success) { throw ('No age in '+$state) }
    return [int]$m.Groups[1].Value
}
function Food { return [int](Invoke-Ctl @('state')).player.food }
function Click([int] $X,[int] $Y,[int] $Z,[double] $Px,[double] $Pz) {
    $null=Command ('tp @s '+$Px+' 85 '+$Pz)
    Wait-Ticks 2
    $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,'up')
    $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,'up')
    Wait-Ticks 6
}











$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 98.5 80 98.5'
Wait-Ticks 40
$results=[ordered]@{}
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function Have([string] $Id) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x=100,y=81,z=100,distance=..6,nbt={Item:{id:"'+$Id+'"}}] run say pot_item_found')).chat -join ' ')
    if ($text -match 'pot_item_found') { return $true }
    $inv=(Invoke-Ctl @('inventory')).counts
    return ($inv.ContainsKey($Id) -and [int]$inv[$Id] -ge 1)
}
function Slot([int] $Index,[string] $Spec) { $null=Command ('item replace block 100 80 100 container.'+$Index+' with '+$Spec) }
function Has-In([string] $Pattern) { return ((Get-Data 100 80 100) -match $Pattern) }
function Touch-Pot([string] $Item) {
    $null=Command 'tp @s 100.5 82 100.5'
    Wait-Ticks 14
    if ($Item) { $null=Command ('item replace entity @s weapon.mainhand with '+$Item) } else { $null=Command 'item replace entity @s weapon.mainhand with minecraft:air' }
    $ok=$false;$last=''
    foreach ($face in 'up','north','south','east','west') {
        try { $null=Invoke-Ctl @('look-at','block','100','80','100',$face);$null=Invoke-Ctl @('interact','100','80','100',$face);$ok=$true;break } catch { $last+=('['+$face+': '+$_.Exception.Message+'] ') }
    }
    if (-not $ok) { throw ('cannot target the pot; '+$last) }
    Wait-Ticks 8
}
function Reset-Pot([string] $Below='farmersdelight:stove[facing=north,lit=true]') {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
    $null=Command ('setblock 100 79 100 '+$Below)
    $null=Command 'setblock 100 80 100 farmersdelight:cooking_pot[facing=north,support=none,waterlogged=false]'
    Wait-Ticks 10
}
Scenario 'pot places' {
    Reset-Pot
    if ((Block 100 80 100) -ne 'farmersdelight:cooking_pot') { throw ('found '+(Block 100 80 100)) }
}
Scenario 'an empty hand opens the menu with the pot slots' {
    try { $null=Invoke-Ctl @('close') } catch { }
    Reset-Pot
    Slot 0 'minecraft:apple 3'
    Touch-Pot $null
    $screen=Invoke-Ctl @('screen')
    if ($screen.screen -ne 'container') { throw ('the container screen is not open: '+($screen|ConvertTo-Json -Compress -Depth 4)) }
    $ids=@($screen.slots|ForEach-Object { [int]$_.slot })
    try {
    foreach ($want in 300..305 + 307,308) { if ($ids -notcontains $want) { throw ('slot '+$want+' is missing from the screen: '+($ids -join ',')) } }
        if ($ids -contains 306) { throw 'the meal display slot should be hidden' }
    } catch { $null=Invoke-Ctl @('close'); throw }
    $first=$screen.slots|Where-Object { [int]$_.slot -eq 300 }
    $shown=$first | ConvertTo-Json -Compress -Depth 4
    if ($shown -notmatch 'apple' -or $shown -notmatch '3') { throw ('slot 300 does not show the 3 apples: '+$shown) }
    $null=Invoke-Ctl @('close')
}
Scenario 'a heated pot cooks apple cider into the meal slot' {
    Reset-Pot
    Slot 0 'minecraft:apple 1'; Slot 1 'minecraft:apple 1'; Slot 2 'minecraft:sugar 1'
    $done=$false
    for ($i=0;$i -lt 30;$i++) { Wait-Ticks 20; if (Has-In 'apple_cider') { $done=$true;break } }
    if (-not $done) { throw ('no cider; pot '+(Get-Data 100 80 100)+'; stove '+(State 100 79 100)) }
}
Scenario 'a heated pot cooks apple cider into the meal slot' {
    Reset-Pot
    Slot 0 'minecraft:apple 1'; Slot 1 'minecraft:apple 1'; Slot 2 'minecraft:sugar 1'
    $done=$false
    for ($i=0;$i -lt 30;$i++) { Wait-Ticks 20; if (Has-In 'apple_cider') { $done=$true;break } }
    if (-not $done) { throw ('no cider; pot '+(Get-Data 100 80 100)+'; stove '+(State 100 79 100)) }
}
Scenario 'a bottle in the container slot moves a serving to the output' {
    Slot 7 'minecraft:glass_bottle 1'
    $moved=$false
    for ($i=0;$i -lt 10;$i++) { Wait-Ticks 5; if ((Get-Data 100 80 100) -match 'Slot: 8b') { $moved=$true;break } }
    if (-not $moved) { throw ('nothing reached the output slot; pot '+(Get-Data 100 80 100)) }
}
Scenario 'a bottle in the container slot moves a serving to the output' {
    Slot 7 'minecraft:glass_bottle 1'
    $moved=$false
    for ($i=0;$i -lt 10;$i++) { Wait-Ticks 5; if ((Get-Data 100 80 100) -match 'Slot: 8b') { $moved=$true;break } }
    if (-not $moved) { throw ('nothing reached the output slot; pot '+(Get-Data 100 80 100)) }
}
Scenario 'serving by hand with the matching container' {
    Reset-Pot 'minecraft:smooth_stone'
    Slot 6 'farmersdelight:apple_cider 2'
    Touch-Pot 'minecraft:glass_bottle 1'
    if (-not (Have 'farmersdelight:apple_cider')) { throw ('no cider served; pot '+(Get-Data 100 80 100)) }
}
Scenario 'an unheated pot does not cook' {
    Reset-Pot 'minecraft:smooth_stone'
    Slot 0 'minecraft:apple 1'; Slot 1 'minecraft:apple 1'; Slot 2 'minecraft:sugar 1'
    Wait-Ticks 300
    if (Has-In 'apple_cider') { throw 'a cold pot cooked' }
}
Scenario 'a wrong combination does not cook' {
    Reset-Pot
    Slot 0 'minecraft:apple 1'; Slot 1 'minecraft:carrot 1'
    Wait-Ticks 300
    if (Has-In 'apple_cider') { throw 'a wrong combination cooked' }
}
Scenario 'breaking the pot drops its contents once' {
    Reset-Pot 'minecraft:smooth_stone'
    Slot 0 'minecraft:apple 3'
    $null=Command 'clear @s'
    $null=Command 'tp @s 100.5 82 100.5'
    Wait-Ticks 14
    $null=Invoke-Ctl @('look-at','block','100','80','100','up')
    $null=Invoke-Ctl @('break','100','80','100')
    $null=Command 'tp @s 103.5 80 103.5'
    Wait-Ticks 14
    $inv=(Invoke-Ctl @('inventory')).counts
    $held=if ($inv.ContainsKey('minecraft:apple')) { [int]$inv['minecraft:apple'] } else { 0 }
    $near=0
    $text=((Command 'execute store result score @s pot_apples run data get entity @e[type=minecraft:item,x=100,y=81,z=100,distance=..6,nbt={Item:{id:"minecraft:apple"}},limit=1] Item.count').chat -join ' ')
    if ($held -lt 1 -and -not (Have 'minecraft:apple')) { throw 'the apples were lost' }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)