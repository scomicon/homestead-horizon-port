#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Display Link + Display Board: board sizing, link-to-board text transfer, update interval, powered pause, source and label configuration. Block entity data is checked; the drawn text is not compared.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Displays')][string] $Phase='Displays',
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
$transcript=Join-Path $game ('stage11-displays-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage11-displays-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Two display boards form one board (controller + size); a Display Link attached to a stone block writes the redstone power of that block to the board; interval, powered pause, source cycling and label are checked on block entity data. Visuals (text drawing) are not compared.'
    coordinates=@{origin=@(100,81,100)}
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



function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Touch([int] $X,[int] $Y,[int] $Z) {
    foreach ($face in 'south','up','north','west','east') {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$face);return } catch { }
    }
    throw ('No face of the block at '+$X+' '+$Y+' '+$Z+' can be targeted.')
}
function Rows([int] $X,[int] $Y,[int] $Z) { return (Get-Data $X $Y $Z 'Rows') }

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 98.5 80 96.5'
Wait-Ticks 40
$null=Command 'fill 92 77 92 115 90 112 minecraft:air'
$null=Command 'fill 92 79 92 115 79 112 minecraft:smooth_stone'
Wait-Ticks 20

# --- Board: two blocks side by side, facing south. The row runs east, the controller is the west block.
$null=Command 'setblock 100 81 100 create:display_board[facing=south]'
$null=Command 'setblock 101 81 100 create:display_board[facing=south]'
Wait-Ticks 40
Expect ((Block 100 81 100) -like 'create:display_board*') ('The board was not placed: '+(Block 100 81 100))
$c=Get-Data 100 81 100
Expect ($c -match 'Controller:\s*1b') ('The west block is not the controller: '+$c)
Expect ($c -match 'XSize:\s*2') ('The board is not two blocks wide: '+$c)
Expect ($c -match 'YSize:\s*1') ('The board is not one block high: '+$c)
$n=Get-Data 101 81 100
Expect ($n -match 'Controller:\s*0b') ('The east block must not be a controller: '+$n)
Passed 'Two boards join: controller at the west block, size 2x1.'

# Stacking: a third board below the controller joins and doubles the line count.
$null=Command 'setblock 100 80 100 create:display_board[facing=south]'
Wait-Ticks 40
Expect ((State 100 81 100) -match 'down=true') ('The upper board did not get down=true: '+(State 100 81 100))
Expect ((State 100 80 100) -match 'up=true') ('The lower board did not get up=true: '+(State 100 80 100))
Expect ((Get-Data 100 81 100) -match 'YSize:\s*2') 'The board did not grow to two rows.'
Passed 'A board stacked below joins the controller (up/down states, YSize 2).'
$null=Command 'setblock 100 80 100 minecraft:air'
Wait-Ticks 40
Expect ((Get-Data 100 81 100) -match 'YSize:\s*1') 'The board did not shrink back to one row.'

# --- Link attached to a stone source. facing=south: attached to the north neighbour (105,81,100).
$null=Command 'setblock 105 81 100 minecraft:stone'
$null=Command 'setblock 105 81 101 create:display_link[facing=south]'
Wait-Ticks 10
Expect ((Block 105 81 101) -like 'create:display_link*') ('The link was not placed: '+(Block 105 81 101))
# Target = board controller: offset (-5,0,-1). Source forced to redstone power, quick interval.
$null=Command 'data merge block 105 81 101 {TargetOffset:[I;-5,0,-1],Source:"redstone_power",Mode:0,Interval:20}'
Wait-Ticks 40
$l=Get-Data 105 81 101
Expect ($l -match 'redstone_power' -and $l -match '-5') ('The link did not take its configuration: '+$l)
Expect ((Rows 100 81 100) -match '[''"]0[''"]') ('An unpowered source must show 0: '+(Rows 100 81 100)+' '+$l)
Passed 'The link wrote redstone power 0 to the board.'

# Power the stone from a redstone block that does not touch the link.
$null=Command 'setblock 106 81 100 minecraft:redstone_block'
Wait-Ticks 60
Expect ((Rows 100 81 100) -match '[''"]15[''"]') ('The board did not show 15: '+(Rows 100 81 100)+' '+(Get-Data 105 81 101))
Passed 'The board shows 15 after the source got powered (update interval 20 ticks).'

# Powered link pauses (DisplayLink.tick_source): a redstone block touching the link.
$null=Command 'setblock 106 81 101 minecraft:redstone_block'
Wait-Ticks 10
Expect ((State 105 81 101) -match 'powered=true') ('The link did not become powered: '+(State 105 81 101))
$null=Command 'setblock 106 81 100 minecraft:air'
Wait-Ticks 60
Expect ((Rows 100 81 100) -match '[''"]15[''"]') ('A powered link must not update: '+(Rows 100 81 100))
Passed 'A powered link keeps its last text.'
$null=Command 'setblock 106 81 101 minecraft:air'
Wait-Ticks 30
Expect ((State 105 81 101) -match 'powered=false') 'The link stayed powered.'
Expect ((Rows 100 81 100) -match '[''"]0[''"]') ('Losing power must refresh the text (onNoLongerPowered): '+(Rows 100 81 100))
Passed 'Losing power refreshed the board at once.'

# Source and label.
$null=Command 'data merge block 105 81 101 {Source:"time_of_day",Mode:0}'
Wait-Ticks 40
Expect ((Rows 100 81 100) -match '\d{1,2}:\d\d') ('The time of day was not written: '+(Rows 100 81 100))
$null=Command 'data merge block 105 81 101 {Label:"T"}'
Wait-Ticks 40
Expect ((Rows 100 81 100) -match 'T\s+\d{1,2}:\d\d') ('The label was not written in front of the time: '+(Rows 100 81 100))
Passed 'Time of day with a label reaches the board.'

# Second link, line 1 of the same board, game time.
$null=Command 'setblock 107 81 100 minecraft:stone'
$null=Command 'setblock 107 81 101 create:display_link[facing=south]'
$null=Command 'data merge block 107 81 101 {TargetOffset:[I;-7,0,-1],TargetLine:1,Source:"game_time",Interval:20}'
Wait-Ticks 60
$rows=Rows 100 81 100
Expect ($rows -match 'T\s+\d{1,2}:\d\d') ('Line 0 was overwritten by the second link: '+$rows)
Expect ($rows -match '\d{3,}') ('Line 1 does not show the game time: '+$rows)
Passed 'A second link wrote its own line without touching line 0.'

# Interactions (empty hand cycles the source). The look-at / interact commands may not reach every face.
$null=Command 'tp @s 105.5 81 103.5'
Wait-Ticks 8
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
$before=Get-Data 105 81 101 'Source'
Touch 105 81 101
Wait-Ticks 8
$after=Get-Data 105 81 101 'Source'
Expect ($before -ne $after) ('Empty-hand use did not cycle the source: '+$before+' -> '+$after)
Passed 'Empty-hand use cycled the link source.'

# Removing the attached source removes the link.
$null=Command 'setblock 107 81 100 minecraft:air'
Wait-Ticks 10
Expect ((Block 107 81 101) -notlike 'create:display_link*') ('The link survived losing its source block: '+(Block 107 81 101))
Passed 'A link without its attached block drops.'
$report.passed=$true
Save-Report
Write-Output 'Displays acceptance passed.'