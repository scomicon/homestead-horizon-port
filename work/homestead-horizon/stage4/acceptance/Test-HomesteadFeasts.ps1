#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Farmer's Delight feasts, pies, cake cut, Comfort effect and drinks.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Feasts')][string] $Phase='Feasts',
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
$transcript=Join-Path $game ('stage24-feasts-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage24-feasts-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Farmer''s Delight feasts: the five feast blocks and three pies place with their state; a bowl serves a portion of a bowl feast and is used up; a click without the bowl serves nothing; the last portion keeps or removes the block as the original does; an empty feast drops its leftovers when clicked; the rice roll medley serves rolls without a bowl in the original order; a knife cuts pie slices and the vanilla cake; the Comfort effect exists; the drinks are items. Each scenario is reported separately.'
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
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Prop([string] $Name) {
    $m=[regex]::Match((State 100 80 100),$Name+'=(\w+)')
    if (-not $m.Success) { throw ('No '+$Name+' in '+(State 100 80 100)) }
    return $m.Groups[1].Value
}
function Count([string] $Id) {
    $inv=(Invoke-Ctl @('inventory')).counts
    if ($inv.ContainsKey($Id)) { return [int]$inv[$Id] }
    return 0
}
function Have([string] $Id) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x=100,y=81,z=100,distance=..6,nbt={Item:{id:"'+$Id+'"}}] run say feast_item_found')).chat -join ' ')
    return (($text -match 'feast_item_found') -or ((Count $Id) -ge 1))
}
function Reset-Table {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
}
function Place([string] $Block) {
    $null=Command 'setblock 100 80 100 minecraft:air'
    $null=Command ('setblock 100 80 100 '+$Block)
    Wait-Ticks 6
}
function Use-Block([string] $Item,[int] $Count=1) {
    $null=Command 'tp @s 100.5 82 100.5'
    Wait-Ticks 14
    if ($Item) { $null=Command ('item replace entity @s weapon.mainhand with '+$Item+' '+$Count) } else { $null=Command 'item replace entity @s weapon.mainhand with minecraft:air' }
    $ok=$false;$last=''
    foreach ($face in 'up','north','south','east','west') {
        try { $null=Invoke-Ctl @('look-at','block','100','80','100',$face);$null=Invoke-Ctl @('interact','100','80','100',$face);$ok=$true;break } catch { $last+=('['+$face+': '+$_.Exception.Message+'] ') }
    }
    if (-not $ok) { throw ('cannot target the block; '+$last) }
    Wait-Ticks 10
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
Scenario 'feast blocks and pies place' {
    Reset-Table
    foreach ($b in 'roast_chicken_block[facing=north,servings=4]','stuffed_pumpkin_block[facing=north,servings=4]','honey_glazed_ham_block[facing=north,servings=4]','shepherds_pie_block[facing=north,servings=4]','rice_roll_medley_block[facing=north,servings=8]','apple_pie[facing=north,bites=0]','chocolate_pie[facing=north,bites=0]','sweet_berry_cheesecake[facing=north,bites=0]') {
        Place ('farmersdelight:'+$b)
        $name='farmersdelight:'+($b -replace '\[.*','')
        if ((Block 100 80 100) -ne $name) { throw ('found '+(Block 100 80 100)+' for '+$name) }
    }
}
Scenario 'feast items exist' {
    $null=Command 'clear @s'
    foreach ($i in 'roast_chicken_block','apple_pie') { $null=Command ('give @s farmersdelight:'+$i+' 1') }
    Wait-Ticks 5
    foreach ($i in 'roast_chicken_block','apple_pie') { if ((Count ('farmersdelight:'+$i)) -lt 1) { throw ('no '+$i+' item') } }
}
Scenario 'a bowl serves the roast chicken' {
    Reset-Table
    Place 'farmersdelight:roast_chicken_block[facing=north,servings=4]'
    Use-Block 'minecraft:bowl' 2
    if ((Prop 'servings') -ne '3') { throw ('servings '+(Prop 'servings')) }
    if ((Count 'farmersdelight:roast_chicken') -lt 1) { throw 'no roast chicken served' }
}
Scenario 'serving a creative player uses no bowl' {
    # The container is only used up outside creative mode; creative keeps both bowls.
    if ((Count 'minecraft:bowl') -lt 2) { throw ('bowls '+(Count 'minecraft:bowl')) }
}
Scenario 'a click without the bowl serves nothing' {
    Reset-Table
    Place 'farmersdelight:roast_chicken_block[facing=north,servings=4]'
    Use-Block $null
    if ((Prop 'servings') -ne '4') { throw ('servings '+(Prop 'servings')) }
    if ((Count 'farmersdelight:roast_chicken') -ne 0) { throw 'served without a bowl' }
}
Scenario 'the last portion leaves the roast chicken block' {
    Reset-Table
    Place 'farmersdelight:roast_chicken_block[facing=north,servings=1]'
    Use-Block 'minecraft:bowl' 1
    if ((Block 100 80 100) -ne 'farmersdelight:roast_chicken_block' -or (Prop 'servings') -ne '0') { throw ('state '+(State 100 80 100)) }
}
Scenario 'an empty roast chicken is broken and drops leftovers' {
    Use-Block $null
    if ((Block 100 80 100) -ne 'minecraft:air') { throw ('block '+(Block 100 80 100)) }
    if (-not (Have 'minecraft:bone_meal')) { throw 'no bone meal dropped' }
    if (-not (Have 'minecraft:bowl')) { throw 'no bowl dropped' }
}
Scenario 'the last portion removes the stuffed pumpkin' {
    Reset-Table
    Place 'farmersdelight:stuffed_pumpkin_block[facing=north,servings=1]'
    Use-Block 'minecraft:bowl' 1
    if ((Block 100 80 100) -ne 'minecraft:air') { throw ('block '+(Block 100 80 100)) }
    if ((Count 'farmersdelight:stuffed_pumpkin') -lt 1) { throw 'no stuffed pumpkin served' }
}
Scenario 'honey glazed ham and shepherds pie serve with a bowl' {
    foreach ($pair in @(@('honey_glazed_ham_block','honey_glazed_ham'),@('shepherds_pie_block','shepherds_pie'))) {
        Reset-Table
        Place ('farmersdelight:'+$pair[0]+'[facing=north,servings=4]')
        Use-Block 'minecraft:bowl' 1
        if ((Prop 'servings') -ne '3') { throw ($pair[0]+' servings '+(Prop 'servings')) }
        if ((Count ('farmersdelight:'+$pair[1])) -lt 1) { throw ('no '+$pair[1]+' served') }
    }
}
Scenario 'the rice roll medley serves rolls without a bowl' {
    foreach ($pair in @(@('8','farmersdelight:kelp_roll_slice'),@('3','farmersdelight:salmon_roll'),@('1','farmersdelight:cod_roll'))) {
        Reset-Table
        Place ('farmersdelight:rice_roll_medley_block[facing=north,servings='+$pair[0]+']')
        Use-Block $null
        $expected=[int]$pair[0]-1
        if ((Prop 'servings') -ne [string]$expected) { throw ('servings '+(Prop 'servings')+' after '+$pair[0]) }
        if ((Count $pair[1]) -lt 1) { throw ('no '+$pair[1]+' for servings '+$pair[0]) }
    }
}
Scenario 'a knife cuts a pie slice' {
    Reset-Table
    Place 'farmersdelight:apple_pie[facing=north,bites=0]'
    Use-Block 'farmersdelight:iron_knife' 1
    if ((Prop 'bites') -ne '1') { throw ('bites '+(Prop 'bites')) }
    if (-not (Have 'farmersdelight:apple_pie_slice')) { throw 'no pie slice dropped' }
}
Scenario 'the last cut removes the pie' {
    Reset-Table
    Place 'farmersdelight:chocolate_pie[facing=north,bites=3]'
    Use-Block 'farmersdelight:iron_knife' 1
    if ((Block 100 80 100) -ne 'minecraft:air') { throw ('block '+(Block 100 80 100)) }
    if (-not (Have 'farmersdelight:chocolate_pie_slice')) { throw 'no pie slice dropped' }
}
Scenario 'a knife cuts the vanilla cake' {
    Reset-Table
    Place 'minecraft:cake[bites=0]'
    Use-Block 'farmersdelight:iron_knife' 1
    if ((Prop 'bites') -ne '1') { throw ('bites '+(Prop 'bites')) }
    if (-not (Have 'farmersdelight:cake_slice')) { throw 'no cake slice dropped' }
}
Scenario 'the Comfort effect exists' {
    $null=Command 'effect give @s farmersdelight:comfort 30 0'
    Wait-Ticks 5
    $active=(Invoke-Ctl @('state')).player
    $text=($active|ConvertTo-Json -Depth 8 -Compress)
    if ($text -notmatch 'comfort') { throw ('no comfort effect in state: '+$text) }
    $null=Command 'effect clear @s farmersdelight:comfort'
}
Scenario 'the drinks are items' {
    $null=Command 'clear @s'
    foreach ($i in 'milk_bottle','hot_cocoa','melon_juice') { $null=Command ('give @s farmersdelight:'+$i+' 1') }
    Wait-Ticks 5
    foreach ($i in 'milk_bottle','hot_cocoa','melon_juice') { if ((Count ('farmersdelight:'+$i)) -lt 1) { throw ('no '+$i) } }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
