#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Farmer's Delight stage 23: baskets and cabinets (27 slots, menu, hoppers, placement, basket pickup),
# canvas rug, half tatami mat, rope, mushroom colonies (growth on rich soil, no plain mushrooms).
# Never run yet (written without building): see docs/fd-stage23.md, "Not verified".
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('FdStage23')][string] $Phase='FdStage23',
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
$transcript=Join-Path $game ('stage23-fd-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage23-fd-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Farmer''s Delight stage 23: baskets and cabinets keep 27 slots, open the 9x3 menu, give hoppers access, place facing the clicked face (basket) or the placer (cabinet), and a basket draws in dropped items; the canvas rug, half tatami mat and rope stand and the rope connects upward; mushroom colonies grow on rich soil without spawning plain mushrooms. Each scenario is reported separately.'
    coordinates=@{origin=@(205,79,205)}
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
function Prop([int] $X,[int] $Y,[int] $Z,[string] $Name) {
    $m=[regex]::Match((State $X $Y $Z),$Name+'=(\w+)'); if (-not $m.Success) { throw ('No '+$Name+' at '+$X+' '+$Y+' '+$Z+': '+(State $X $Y $Z)) }
    return $m.Groups[1].Value
}
function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }
function Poll([int] $Rounds,[int] $Step,[scriptblock] $Done) {
    for ($i=0;$i -lt $Rounds;$i++) { Wait-Ticks $Step; if (& $Done) { return $true } }
    return $false
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
function Count-Of([int] $X,[int] $Y,[int] $Z,[string] $Id) {
    $counts=Counts $X $Y $Z
    if ($counts.ContainsKey($Id)) { return [int]$counts[$Id] } else { return 0 }
}
function Slot([int] $X,[int] $Y,[int] $Z,[int] $Index,[string] $Spec) { $null=Command ('item replace block '+$X+' '+$Y+' '+$Z+' container.'+$Index+' with '+$Spec) }
function Have([string] $Id,[int] $X,[int] $Y,[int] $Z) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x='+$X+',y='+$Y+',z='+$Z+',distance=..8,nbt={Item:{id:"'+$Id+'"}}] run say stage23_item_found')).chat -join ' ')
    if ($text -match 'stage23_item_found') { return $true }
    $inv=(Invoke-Ctl @('inventory')).counts
    return ($inv.ContainsKey($Id) -and [int]$inv[$Id] -ge 1)
}
function Hold([string] $Item) {
    if ($Item) { $null=Command ('item replace entity @s weapon.mainhand with '+$Item+$(if ($Item -like '*shears') {''} else {' 4'})) } else { $null=Command 'item replace entity @s weapon.mainhand with minecraft:air' }
}
# Stands at ($Px,$Pz) on y 80, holds $Item and clicks the $Face of the block at $X $Y $Z.
function Use-Face([int] $X,[int] $Y,[int] $Z,[string] $Face,[double] $Px,[double] $Pz,[string] $Item) {
    $null=Command ('tp @s '+$Px+' 80 '+$Pz)
    Wait-Ticks 4
    Hold $Item
    $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$Face)
    $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$Face)
    Wait-Ticks 8
}
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
# A stone floor at y 79 over x 205-245, z 205-245; everything above it cleared.
function Reset-Area {
    $null=Command 'gamerule random_tick_speed 0'
    $null=Command 'time set day'
    $null=Command 'weather clear'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    try { $null=Invoke-Ctl @('close') } catch { }
    $null=Command 'fill 205 80 205 245 92 245 minecraft:air'
    $null=Command 'fill 205 79 205 245 79 245 minecraft:stone'
    Wait-Ticks 10
}
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 205 205 245 245'
$null=Command 'tp @s 210.5 85 210.5'
Wait-Ticks 40
Reset-Area
$woods='oak','spruce','birch','jungle','acacia','dark_oak','mangrove','cherry','bamboo','crimson','warped'

Scenario 'stage 23 items exist' {
    $items='farmersdelight:basket','farmersdelight:oak_cabinet','farmersdelight:rope','farmersdelight:canvas_rug','farmersdelight:half_tatami_mat','farmersdelight:brown_mushroom_colony','farmersdelight:red_mushroom_colony'
    foreach ($item in $items) { $null=Command ('give @s '+$item+' 1') }
    Wait-Ticks 5
    $inv=(Invoke-Ctl @('inventory')).counts
    foreach ($item in $items) { Expect ($inv.ContainsKey($item)) ('no '+$item+' item: '+($inv|ConvertTo-Json -Compress)) }
    $null=Command 'clear @s'
}
Scenario 'all eleven cabinets stand with every horizontal facing and the open state' {
    foreach ($wood in $woods) {
        foreach ($facing in 'north','south','east','west') {
            $null=Command ('setblock 206 80 206 farmersdelight:'+$wood+'_cabinet[facing='+$facing+',open=false]')
            Expect ((Block 206 80 206) -eq ('farmersdelight:'+$wood+'_cabinet')) ('the '+$wood+' cabinet did not stand: '+(Block 206 80 206))
            Expect ((Prop 206 80 206 'facing') -eq $facing) ('facing '+$facing+' read back as '+(Prop 206 80 206 'facing'))
        }
    }
    $null=Command 'setblock 206 80 206 minecraft:air'
}
Scenario 'a basket keeps items in its slots' {
    Reset-Area
    $null=Command 'setblock 210 80 210 farmersdelight:basket[facing=up]'
    Wait-Ticks 10
    Slot 210 80 210 0 'minecraft:apple 5'
    Slot 210 80 210 26 'minecraft:carrot 3'
    Wait-Ticks 5
    Expect ((Count-Of 210 80 210 'minecraft:apple') -eq 5) ('apples: '+((Counts 210 80 210)|ConvertTo-Json -Compress))
    Expect ((Count-Of 210 80 210 'minecraft:carrot') -eq 3) 'the last slot (26) did not hold the carrots'
}
Scenario 'a right click opens the 27 slot menu of a basket' {
    Reset-Area
    $null=Command 'setblock 210 80 210 farmersdelight:basket[facing=up]'
    Wait-Ticks 10
    Slot 210 80 210 0 'minecraft:apple 3'
    $null=Command 'tp @s 210.5 82 210.5'
    Wait-Ticks 14
    Hold $null
    $null=Invoke-Ctl @('look-at','block','210','80','210','up')
    $null=Invoke-Ctl @('interact','210','80','210','up')
    Wait-Ticks 10
    $screen=Invoke-Ctl @('screen')
    try {
        Expect ($screen.screen -eq 'container') ('the container screen is not open: '+($screen|ConvertTo-Json -Compress -Depth 4))
        Expect (@($screen.slots).Count -ge 27) ('the menu shows only '+@($screen.slots).Count+' slots')
        Expect (($screen.slots|ConvertTo-Json -Compress -Depth 5) -match 'apple') 'the apples are not shown in the menu'
    } finally { try { $null=Invoke-Ctl @('close') } catch { } }
}
Scenario 'a right click opens the menu of a cabinet' {
    Reset-Area
    $null=Command 'setblock 212 80 210 farmersdelight:oak_cabinet[facing=north,open=false]'
    Wait-Ticks 10
    Slot 212 80 210 4 'minecraft:bread 2'
    $null=Command 'tp @s 212.5 82 213.5'
    Wait-Ticks 14
    Hold $null
    $null=Invoke-Ctl @('look-at','block','212','80','210','south')
    $null=Invoke-Ctl @('interact','212','80','210','south')
    Wait-Ticks 10
    $screen=Invoke-Ctl @('screen')
    try {
        Expect ($screen.screen -eq 'container') ('the container screen is not open: '+($screen|ConvertTo-Json -Compress -Depth 4))
        Expect (($screen.slots|ConvertTo-Json -Compress -Depth 5) -match 'bread') 'the bread is not shown in the menu'
    } finally { try { $null=Invoke-Ctl @('close') } catch { } }
}
Scenario 'a basket placed on a top face opens upward and on a side face sideways' {
    Reset-Area
    $null=Command 'setblock 215 80 215 minecraft:stone'
    Use-Face 215 80 215 'up' 215.5 212.5 'farmersdelight:basket'
    Expect ((Block 215 81 215) -eq 'farmersdelight:basket') ('no basket on top: '+(Block 215 81 215))
    Expect ((Prop 215 81 215 'facing') -eq 'up') ('top face placement faces '+(Prop 215 81 215 'facing'))
    Use-Face 215 80 215 'east' 218.5 215.5 'farmersdelight:basket'
    Expect ((Block 216 80 215) -eq 'farmersdelight:basket') ('no basket on the east side: '+(Block 216 80 215))
    Expect ((Prop 216 80 215 'facing') -eq 'east') ('east face placement faces '+(Prop 216 80 215 'facing'))
}
Scenario 'a cabinet faces the placer and is never placed up or down' {
    Reset-Area
    $null=Command 'setblock 220 79 215 minecraft:stone'
    # Stand north of the block, looking south: the cabinet faces north (back to the placer's look).
    Use-Face 220 79 215 'up' 220.5 211.5 'farmersdelight:oak_cabinet'
    Expect ((Block 220 80 215) -eq 'farmersdelight:oak_cabinet') ('no cabinet: '+(Block 220 80 215))
    $facing=Prop 220 80 215 'facing'
    Expect (@('north','south','east','west') -contains $facing) ('a cabinet faces '+$facing)
    Expect ($facing -eq 'north') ('placed looking south, the cabinet faces '+$facing+', not north')
}
Scenario 'a basket draws in dropped items in front of its opening' {
    Reset-Area
    $null=Command 'setblock 225 80 225 farmersdelight:basket[facing=up]'
    Wait-Ticks 10
    $null=Command 'summon minecraft:item 225.5 81.5 225.5 {Item:{id:"minecraft:apple",count:3}}'
    $drawn=Poll 12 10 { (Count-Of 225 80 225 'minecraft:apple') -eq 3 }
    Expect $drawn ('the basket drew in '+(Count-Of 225 80 225 'minecraft:apple')+' of 3 apples')
}
Scenario 'a basket facing away does not draw in items on its top' {
    Reset-Area
    $null=Command 'setblock 227 80 225 farmersdelight:basket[facing=down]'
    Wait-Ticks 10
    $null=Command 'summon minecraft:item 227.5 81.5 225.5 {Item:{id:"minecraft:apple",count:3}}'
    Wait-Ticks 120
    Expect ((Count-Of 227 80 225 'minecraft:apple') -eq 0) 'a downward basket took items from its back'
}
Scenario 'a hopper under a basket takes its items' {
    Reset-Area
    $null=Command 'setblock 230 79 225 minecraft:chest'
    $null=Command 'setblock 230 80 225 minecraft:hopper[facing=down]'
    $null=Command 'setblock 230 81 225 farmersdelight:basket[facing=up]'
    Wait-Ticks 10
    Slot 230 81 225 0 'minecraft:apple 4'
    $moved=Poll 20 10 { (Count-Of 230 79 225 'minecraft:apple') -gt 0 }
    Expect $moved ('nothing reached the chest through the hopper; basket '+((Counts 230 81 225)|ConvertTo-Json -Compress))
}
Scenario 'a hopper above a cabinet fills it' {
    Reset-Area
    $null=Command 'setblock 233 80 225 farmersdelight:oak_cabinet[facing=north,open=false]'
    $null=Command 'setblock 233 81 225 minecraft:hopper[facing=down]'
    Wait-Ticks 10
    Slot 233 81 225 0 'minecraft:apple 4'
    $moved=Poll 20 10 { (Count-Of 233 80 225 'minecraft:apple') -gt 0 }
    Expect $moved ('the hopper put nothing into the cabinet; cabinet '+((Counts 233 80 225)|ConvertTo-Json -Compress))
}
Scenario 'breaking a basket drops its items' {
    Reset-Area
    $null=Command 'setblock 236 80 225 farmersdelight:basket[facing=up]'
    Wait-Ticks 10
    Slot 236 80 225 0 'minecraft:apple 3'
    $null=Command 'clear @s'
    $null=Command 'tp @s 236.5 82 225.5'
    Wait-Ticks 14
    $null=Invoke-Ctl @('look-at','block','236','80','225','up')
    $null=Invoke-Ctl @('break','236','80','225')
    Wait-Ticks 20
    Expect ((Block 236 80 225) -eq 'minecraft:air') 'the basket was not broken'
    Expect (Have 'minecraft:apple' 236 80 225) 'the apples were lost'
}
Scenario 'a canvas rug and a half tatami mat stand' {
    Reset-Area
    $null=Command 'setblock 240 80 215 farmersdelight:canvas_rug'
    $null=Command 'setblock 241 80 215 farmersdelight:half_tatami_mat[facing=north]'
    Wait-Ticks 40
    Expect ((Block 240 80 215) -eq 'farmersdelight:canvas_rug') ('the rug is '+(Block 240 80 215))
    Expect ((Block 241 80 215) -eq 'farmersdelight:half_tatami_mat') ('the mat is '+(Block 241 80 215))
}
Scenario 'a rope connects to the rope above it' {
    Reset-Area
    $null=Command 'setblock 240 83 220 minecraft:stone'
    $null=Command 'setblock 240 82 220 farmersdelight:rope'
    $null=Command 'setblock 240 81 220 farmersdelight:rope'
    Wait-Ticks 20
    Expect ((Block 240 81 220) -eq 'farmersdelight:rope') ('the lower rope is '+(Block 240 81 220))
    Expect ((Prop 240 81 220 'up') -eq 'true') ('the lower rope is not connected upward: '+(State 240 81 220))
    Expect ((Prop 240 82 220 'tied_to_bell') -eq 'false') 'tied_to_bell should start false'
}
Scenario 'a brown mushroom colony grows on rich soil and spawns no plain mushrooms' {
    Reset-Area
    $null=Command 'setblock 228 79 232 farmersdelight:rich_soil'
    $null=Command 'setblock 228 80 232 farmersdelight:brown_mushroom_colony[age=0]'
    Wait-Ticks 20
    Expect ((Block 228 80 232) -eq 'farmersdelight:brown_mushroom_colony') ('the colony did not stand on rich soil: '+(Block 228 80 232))
    $null=Command 'gamerule random_tick_speed 300'
    $grew=Poll 20 60 { (Block 228 80 232) -eq 'farmersdelight:brown_mushroom_colony' -and [int](Prop 228 80 232 'age') -gt 0 }
    $null=Command 'gamerule random_tick_speed 0'
    Expect $grew ('the colony never aged: '+(State 228 80 232))
    $plain=0
    foreach ($x in 224..232) { foreach ($z in 228..236) { foreach ($y in 79..81) {
        if ((Block $x $y $z) -eq 'minecraft:brown_mushroom') { $plain++ }
    } } }
    Expect ($plain -eq 0) ($plain.ToString()+' plain brown mushrooms spread around the colony')
}
Scenario 'a colony on stone does not grow' {
    Reset-Area
    $null=Command 'setblock 230 79 232 minecraft:stone'
    $null=Command 'setblock 230 80 232 farmersdelight:red_mushroom_colony[age=0]'
    Wait-Ticks 10
    if ((Block 230 80 232) -eq 'farmersdelight:red_mushroom_colony') {
        $null=Command 'gamerule random_tick_speed 300'
        Wait-Ticks 300
        $null=Command 'gamerule random_tick_speed 0'
        if ((Block 230 80 232) -eq 'farmersdelight:red_mushroom_colony') {
            Expect ((Prop 230 80 232 'age') -eq '0') ('a colony off rich soil grew to age '+(Prop 230 80 232 'age'))
        }
    }
}
Scenario 'a ripe colony broken with shears drops itself' {
    Reset-Area
    $null=Command 'setblock 232 79 232 farmersdelight:rich_soil'
    $null=Command 'setblock 232 80 232 farmersdelight:brown_mushroom_colony[age=3]'
    Wait-Ticks 10
    $null=Command 'clear @s'
    $null=Command 'tp @s 232.5 82 232.5'
    Wait-Ticks 14
    # Block loot is only rolled for a survival player (creative breaking drops nothing).
    $null=Command 'gamemode survival'
    Hold 'minecraft:shears'
    $null=Invoke-Ctl @('look-at','block','232','80','232','up')
    $null=Invoke-Ctl @('break','232','80','232')
    Wait-Ticks 20
    $null=Command 'gamemode creative'
    Expect (Have 'farmersdelight:brown_mushroom_colony' 232 80 232) 'no colony item dropped (shears, age 3, survival)'
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
