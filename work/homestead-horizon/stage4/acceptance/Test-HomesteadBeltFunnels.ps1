#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Ordinary Chest/Barrel stock, real connector-built Belts and ordinary Funnel placement.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Flow','FrontBlocked','SidePass','Reload','VaultBelt')][string] $Phase='Flow',
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
$transcript=Join-Path $game ('stage4-beltfunnels-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage4-beltfunnels-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Dry bare horizontal Belts, ordinary Andesite Funnel item placement and Wrench shapes, Chest/Trapped Chest/Barrel IO, redstone pause, stopped/reversed movement, inline blockage versus side bypass and a cold process restart. No Java runtime, brass/wet/sloped/custom-container parity claim.'
    coordinates=@{
        inline_belt_start=@(136,82,35);inline_belt_end=@(136,82,43)
        inline_input=@(136,83,34);inline_input_funnel=@(136,83,35)
        inline_receiver=@(136,83,44);inline_receiving_funnel=@(136,83,43);inline_motor=@(135,82,35)
        side_belt_start=@(140,82,35);side_belt_end=@(140,82,43)
        side_input=@(140,83,34);side_input_funnel=@(140,83,35)
        side_funnel=@(140,83,39);side_receiver=@(141,83,39)
        side_end_receiver=@(140,83,44);side_end_funnel=@(140,83,43);side_motor=@(139,82,35)
    }
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
function Belt-Snapshot([int] $X) {
    $raw=Get-Data $X 82 35 Inventory
    $counts=@{}
    foreach ($match in [regex]::Matches($raw,'\bItem:\s*\{(?<payload>[^{}]*)\}')) {
        $payload=$match.Groups['payload'].Value
        $id=[regex]::Match($payload,'\bid:\s*"([^"]+)"')
        $amount=[regex]::Match($payload,'\bcount:\s*(\d+)')
        if (-not $id.Success -or -not $amount.Success) { throw ('Unsupported transported stack snapshot: '+$raw) }
        $item=$id.Groups[1].Value
        if (-not $counts.ContainsKey($item)) { $counts[$item]=0 }
        $counts[$item]+=[int]$amount.Groups[1].Value
    }
    if ($raw -match '\bItem:\s*\{' -and $counts.Count -eq 0) { throw ('No complete supported cargo parsed from '+$raw) }
    return @{raw=$raw;counts=$counts}
}
function Assert-NoLooseItems {
    if (((Command 'execute if entity @e[type=minecraft:item,x=132,y=80,z=33,dx=10,dy=8,dz=13] run say stage4_loose_item').chat -join ' ') -match 'stage4_loose_item') {
        throw 'A transported item escaped the fixture inventories.'
    }
}
function Set-Motor([int] $X,[int] $Rpm) {
    $motor=$X-1
    $null=Command "setblock $motor 82 35 minecraft:air"
    $null=Command ("setblock $motor 82 35 create:creative_motor[facing=east]{ScrollValue:"+$Rpm+'}')
    Wait-Ticks 5
}
function Connect-Belt([int] $X) {
    $null=Command 'item replace entity @s weapon.mainhand with create:belt_connector'
    foreach ($z in @(35,43)) {
        $null=Command ('tp @s '+($X+0.5)+' 85 '+($z+0.5))
        Wait-Ticks 2
        $null=Invoke-Ctl @('look-at','block',[string]$X,'82',[string]$z,'up')
        $null=Invoke-Ctl @('interact',[string]$X,'82',[string]$z,'up')
        Wait-Ticks 6
    }
    foreach ($z in 35..43) {
        if ((Invoke-Ctl @('block',[string]$X,'82',[string]$z)).block -ne 'create:belt') { throw 'The real connector did not form the complete nine-segment Belt.' }
    }
}
function Assert-Shape([int] $X,[int] $Z,[string] $Shape) {
    $test="execute if block $X 83 $Z create:andesite_belt_funnel[shape=$Shape] run say stage4_shape_ok"
    if (((Command $test).chat -join ' ') -notmatch 'stage4_shape_ok') {
        $block=Invoke-Ctl @('block',[string]$X,'83',[string]$Z)
        throw ('Expected Andesite Belt Funnel '+$Shape+' at '+$X+',83,'+$Z+': '+($block|ConvertTo-Json -Compress -Depth 6))
    }
}
function Place-Funnel([int] $X,[int] $Z,[ValidateSet('north','south','west')][string] $Face) {
    $null=Command 'item replace entity @s weapon.mainhand with create:andesite_funnel'
    switch ($Face) {
        'south' { $stand=@(($X+0.5),83,($Z+2.65));$target=@($X,83,($Z+1)) }
        'north' { $stand=@(($X+0.5),83,($Z-2.65));$target=@($X,83,($Z-1)) }
        'west' { $stand=@(($X-2.65),83,($Z+0.5));$target=@(($X-1),83,$Z) }
    }
    # Slow falling: the player must not drop away while the slow ctl calls run.
    $null=Command 'effect give @s minecraft:slow_falling 60 0 true'
    $null=Command ('tp @s '+($stand -join ' '))
    Wait-Ticks 2
    $null=Invoke-Ctl @('look-at','block',[string]$X,'83',[string]$Z,$Face)
    $null=Invoke-Ctl @('interact',[string]$X,'83',[string]$Z,$Face)
    Wait-Ticks 5
    if ((Invoke-Ctl @('block',[string]$target[0],'83',[string]$target[2])).block -ne 'create:andesite_belt_funnel') {
        throw 'Ordinary use of the Andesite Funnel item on the inventory did not create its real Belt Funnel variant.'
    }
}
function Wrench([int] $X,[int] $Z,[double] $Aim=-1) {
    $null=Command 'item replace entity @s weapon.mainhand with create:wrench'
    $null=Command ('tp @s '+($X+0.5)+' 85.5 '+($Z+0.5))
    Wait-Ticks 2
    if ($Aim -ge 0) {
        # Retracted/extended plates do not cover the block centre: aim at a point
        # on the top of the plate instead of the centre of the block face.
        $null=Invoke-Ctl @('look-at',([string]($X+0.5)),'83.99',([string]($Z+$Aim)))
        # interact needs the block-centre face to be visible, so right-click the
        # crosshair target with the held Wrench instead.
        $null=Invoke-Ctl @('use-item')
    } else {
        $null=Invoke-Ctl @('look-at','block',[string]$X,'83',[string]$Z,'up')
        $null=Invoke-Ctl @('interact',[string]$X,'83',[string]$Z,'up')
    }
    Wait-Ticks 4
}
function Setup-Lane([int] $X,[bool] $Side) {
    $min=$X-1;$max=$X+1
    $null=Command "fill $min 81 33 $max 87 46 minecraft:air"
    $null=Command "fill $min 80 33 $max 80 46 minecraft:smooth_stone"
    $null=Command "kill @e[type=minecraft:item,x=$min,y=80,z=33,dx=2,dy=8,dz=13]"
    foreach ($z in @(35,43)) { $null=Command "setblock $X 82 $z create:shaft[axis=x]" }
    Set-Motor $X 0
    Connect-Belt $X
    $input=if ($Side) {'minecraft:trapped_chest'} else {'minecraft:chest'}
    $receiver=if ($Side) {'minecraft:chest'} else {'minecraft:barrel'}
    if ($Phase -eq 'VaultBelt') { $input='create:item_vault[axis=x]';$receiver='create:item_vault[axis=x]' }
    $null=Command "setblock $X 83 34 $input"
    $null=Command "setblock $X 83 44 $receiver"
    Place-Funnel $X 34 south
    Place-Funnel $X 44 north
    Assert-Shape $X 35 retracted
    Assert-Shape $X 43 retracted
    if ($Side) {
        $sideX=$X+1
        $null=Command "setblock $sideX 83 39 minecraft:barrel"
        Place-Funnel $sideX 39 west
        # BeltFunnelBlock.getShapeForPosition: an item placed without sneaking
        # extracts (PUSHING); the Wrench turns it into the taking PULLING shape.
        Assert-Shape $X 39 pushing
        Wrench $X 39
        Assert-Shape $X 39 pulling
    }
    Assert-Empty (Counts $X 83 34) 'New source inventory'
    Assert-Empty (Counts $X 83 44) 'New end inventory'
    Assert-Empty (Belt-Snapshot $X).counts 'New Belt'
}
function Stock([int] $X,[int] $Y,[int] $Z,[string] $Item,[int] $Count) {
    $null=Command "item replace block $X $Y $Z container.0 with $Item $Count"
}
function Add-Blockers([int] $X,[int] $Z) {
    foreach ($slot in 0..26) { $null=Command "item replace block $X 83 $Z container.$slot with minecraft:cobblestone 64" }
    Assert-Only (Counts $X 83 $Z) 'minecraft:cobblestone' 1728
}
function Remove-Blockers([int] $X,[int] $Z) {
    # Only remove the previously declared fixture stock after verifying that
    # the inventory still consists of exactly those 27 full blocker stacks.
    Assert-Only (Counts $X 83 $Z) 'minecraft:cobblestone' 1728
    foreach ($slot in 0..26) { $null=Command "item replace block $X 83 $Z container.$slot with minecraft:air" }
    Assert-Empty (Counts $X 83 $Z) 'Receiver after removal of declared blockers'
}
function Await-Only([int] $X,[int] $Y,[int] $Z,[string] $Item,[int] $Count,[int] $Attempts=150) {
    for ($attempt=0;$attempt -lt $Attempts;$attempt++) {
        Wait-Ticks 4
        $counts=Counts $X $Y $Z
        if ($counts.Count -eq 1 -and $counts.ContainsKey($Item) -and [int]$counts[$Item] -eq $Count) { return }
    }
    throw ('Ordinary Belt Funnel transport did not deliver '+$Count+' '+$Item+' into '+$X+','+$Y+','+$Z+': '+($counts|ConvertTo-Json -Compress))
}
function Assert-LaneEmpty([int] $X) {
    Assert-Empty (Counts $X 83 34) 'Source inventory after delivery'
    Assert-Empty (Belt-Snapshot $X).counts 'Belt after delivery'
}
try {
    Save-Report
    if ((Invoke-Ctl @('state')).world -ne $World) { throw 'The current world is not the intended disposable fixture world.' }
    $null=Command 'tick unfreeze';$null=Command 'gamemode creative'
    $null=Command 'tp @s 137.5 85 45.5'
    $null=Command 'forceload add 128 32 143 47'
    $null=Invoke-Ctl @('wait-ready','120')
    $loaded=$false
    for ($attempt=0;$attempt -lt 50;$attempt++) {
        if (((Command 'execute if loaded 140 83 44 run say stage4_loaded').chat -join ' ') -match 'stage4_loaded') { $loaded=$true;break }
        Wait-Ticks 5
    }
    if (-not $loaded) { throw 'The stage4 fixture chunk is not authoritative.' }
    if ($Phase -eq 'Flow') {
        Setup-Lane 136 $false
        Setup-Lane 140 $true
        # The input funnel faces south: its retracted plate spans z 2..8/16 and its
        # extended plate z 10..16/16 of the block.
        Wrench 136 35 0.3;Assert-Shape 136 35 extended
        Wrench 136 35 0.8;Assert-Shape 136 35 retracted
        Passed 'An ordinary Andesite Funnel item on each inventory becomes a Belt Funnel; ordinary Wrench clicks toggle the inline retracted and extended shapes.'
        Stock 136 83 34 'minecraft:copper_ingot' 3
        Wait-Ticks 30
        Assert-Only (Counts 136 83 34) 'minecraft:copper_ingot' 3
        Assert-Empty (Belt-Snapshot 136).counts 'Stopped Belt'
        Assert-Empty (Counts 136 83 44) 'Stopped receiver'
        Passed 'A stationary Belt leaves all three declared source ingots in the ordinary Chest and does not accept cargo.'
        $null=Command 'setblock 135 83 35 minecraft:redstone_block'
        Set-Motor 136 32
        Wait-Ticks 35
        Assert-Only (Counts 136 83 34) 'minecraft:copper_ingot' 3
        Assert-Empty (Belt-Snapshot 136).counts 'Belt behind a powered input Funnel'
        Assert-Empty (Counts 136 83 44) 'Receiver behind a powered input Funnel'
        Passed 'Redstone disables the input Funnel while its Belt is genuinely powered; source stock is unchanged.'
        $null=Command 'setblock 135 83 35 minecraft:air'
        Await-Only 136 83 44 'minecraft:copper_ingot' 3
        Set-Motor 136 0
        Assert-LaneEmpty 136;Assert-NoLooseItems
        Passed 'Clearing redstone sends three real Chest inputs through the connector-built Belt and the receiving Funnel into the Barrel, without loose items or duplication.'
        Set-Motor 136 -32
        Await-Only 136 83 34 'minecraft:copper_ingot' 3
        Set-Motor 136 0
        Assert-Empty (Counts 136 83 44) 'Barrel after reverse delivery'
        Assert-Empty (Belt-Snapshot 136).counts 'Reversed Belt after delivery'
        Assert-NoLooseItems
        Passed 'Reversing real RPM reverses the inline Funnel roles and returns the same three ingots from Barrel to Chest.'
        Set-Motor 136 32
        Await-Only 136 83 44 'minecraft:copper_ingot' 3
        Set-Motor 136 0
        Assert-LaneEmpty 136
        $report.output=(Counts 136 83 44);$report.declared_input_count=3
    } elseif ($Phase -eq 'VaultBelt') {
        Setup-Lane 136 $false
        Stock 136 83 34 'minecraft:copper_ingot' 3
        Set-Motor 136 32
        Await-Only 136 83 44 'minecraft:copper_ingot' 3
        Set-Motor 136 0
        Assert-LaneEmpty 136;Assert-NoLooseItems
        Passed 'Belt Funnels take three ingots out of an Item Vault and deliver them into another Item Vault without loose items or duplication.'
        Set-Motor 136 -32
        Await-Only 136 83 34 'minecraft:copper_ingot' 3
        Set-Motor 136 0
        Assert-Empty (Counts 136 83 44) 'Receiving Item Vault after reverse delivery'
        Assert-NoLooseItems
        Passed 'Reversing the Belt returns the same three ingots between the two Item Vaults.'
    } elseif ($Phase -eq 'FrontBlocked') {
        Setup-Lane 136 $false
        Add-Blockers 136 44
        Stock 136 83 34 'minecraft:copper_ingot' 3
        Set-Motor 136 32
        Wait-Ticks 220
        Assert-Empty (Counts 136 83 34) 'Blocked line source'
        $snapshot=Belt-Snapshot 136
        Assert-Only $snapshot.counts 'minecraft:copper_ingot' 3
        Assert-Only (Counts 136 83 44) 'minecraft:cobblestone' 1728
        Wait-Ticks 40
        Assert-Only (Belt-Snapshot 136).counts 'minecraft:copper_ingot' 3
        Assert-NoLooseItems
        Set-Motor 136 0
        Wait-Ticks 8
        $report.saved_belt=Belt-Snapshot 136
        $report.blocker_count=1728;$report.declared_input_count=3
        Passed 'A full inline receiver holds exactly three real ingots on the Belt instead of ejecting or deleting them; the declared blocker stock remains unchanged.'
        Passed 'The blocked line is saved with its motor stopped and all three transported ingots for the distinct-process cold-load check.'
    } elseif ($Phase -eq 'SidePass') {
        Setup-Lane 140 $true
        Wrench 140 39;Assert-Shape 140 39 pushing
        Wrench 140 39;Assert-Shape 140 39 pulling
        Add-Blockers 141 39
        Stock 140 83 34 'minecraft:copper_ingot' 3
        Set-Motor 140 32
        Await-Only 140 83 44 'minecraft:copper_ingot' 3
        Set-Motor 140 0
        Assert-Only (Counts 141 83 39) 'minecraft:cobblestone' 1728
        Assert-LaneEmpty 140;Assert-NoLooseItems
        Passed 'A full perpendicular side collector lets all three ordinary Trapped Chest inputs pass into the end Chest; it does not impose inline backpressure.'
        Remove-Blockers 141 39
        Stock 140 83 34 'minecraft:gold_ingot' 3
        Set-Motor 140 32
        Await-Only 141 83 39 'minecraft:gold_ingot' 3
        Set-Motor 140 0
        Assert-Only (Counts 140 83 44) 'minecraft:copper_ingot' 3
        Assert-LaneEmpty 140;Assert-NoLooseItems
        $report.side_output=Counts 141 83 39;$report.end_output=Counts 140 83 44
        $report.declared_copper=3;$report.declared_gold=3
        Passed 'After removal of only the declared side blockers, the same ordinary side Funnel collects three newly declared gold ingots into its Barrel while the three earlier copper ingots remain in the end Chest.'
    } else {
        $before=Get-Content -LiteralPath (Join-Path $game 'stage4-beltfunnels-frontblocked-result.json') -Raw|ConvertFrom-Json -AsHashtable
        $side=Get-Content -LiteralPath (Join-Path $game 'stage4-beltfunnels-sidepass-result.json') -Raw|ConvertFrom-Json -AsHashtable
        if (-not $before.passed -or -not $side.passed -or $before.world -ne $World -or $side.world -ne $World) { throw 'Passing blocked-front and side-flow receipts are required before cold reload.' }
        if ($GameProcessId -le 0 -or [int]$before.process_id -le 0 -or $GameProcessId -eq [int]$before.process_id) { throw 'The reload check requires distinct actual native process IDs.' }
        foreach ($x in @(136,140)) { foreach ($z in 35..43) {
            if ((Invoke-Ctl @('block',[string]$x,'82',[string]$z)).block -ne 'create:belt') { throw 'A cold-loaded Belt segment is missing.' }
        } }
        foreach ($pos in @(@(136,35),@(136,43),@(140,35),@(140,43))) { Assert-Shape $pos[0] $pos[1] retracted }
        Assert-Shape 140 39 pulling
        Assert-Only (Counts 136 83 44) 'minecraft:cobblestone' 1728
        Assert-Only (Belt-Snapshot 136).counts 'minecraft:copper_ingot' 3
        Assert-Empty (Counts 136 83 34) 'Cold-loaded blocked source'
        Assert-Only (Counts 141 83 39) 'minecraft:gold_ingot' 3
        Assert-Only (Counts 140 83 44) 'minecraft:copper_ingot' 3
        Assert-NoLooseItems
        Passed 'A distinct native process restores both nine-segment Belts, real Funnel shapes, three blocked inline ingots, all side-line outputs and the unchanged blocker stock.'
        Remove-Blockers 136 44
        Set-Motor 136 32
        Await-Only 136 83 44 'minecraft:copper_ingot' 3
        Set-Motor 136 0
        Wait-Ticks 25
        Assert-Only (Counts 136 83 44) 'minecraft:copper_ingot' 3
        Assert-LaneEmpty 136
        Assert-Only (Counts 141 83 39) 'minecraft:gold_ingot' 3
        Assert-Only (Counts 140 83 44) 'minecraft:copper_ingot' 3
        Assert-NoLooseItems
        $report.previous_process_id=[int]$before.process_id
        $report.inline_output=Counts 136 83 44;$report.side_output=Counts 141 83 39;$report.end_output=Counts 140 83 44
        Passed 'Removing only the declared front blockers after restart releases the three saved transported ingots through the real receiving Funnel into the Barrel, exactly once.'
    }
    $null=Command 'tp @s 137.5 84 46.5'
    $null=Invoke-Ctl @('save')
    $report.passed=$true
} catch { $report.error=$_.Exception.Message;throw }
finally { Save-Report;Write-Output ('Belt Funnel acceptance receipt: '+$reportPath) }
