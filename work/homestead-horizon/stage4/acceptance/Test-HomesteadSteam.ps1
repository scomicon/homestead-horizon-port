#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Steam')][string] $Phase='Steam',
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
$transcript=Join-Path $game ('stage15-steam-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage15-steam-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Boiler, Steam Engine, Powered Shaft and Steam Whistle: a 2x2 tank over a Blaze Burner with water drives a Steam Engine; the Powered Shaft turns; a cold boiler, a removed burner and a removed engine stop it; a powered Whistle sounds only on a hot boiler. Each scenario is reported separately.'
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
function Num([string] $Text,[string] $Key) { $m=[regex]::Match($Text,$Key+':\s*(-?[\d.]+)'); if ($m.Success) { return [double]$m.Groups[1].Value } else { return 0.0 } }
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function Water-Bucket([int] $X,[int] $Y,[int] $Z) {
    $null=Command ('tp @s '+($X+0.5)+' '+($Y+3)+' '+($Z+0.5))
    Wait-Ticks 6
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:water_bucket 1'
    $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,'up')
    $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,'up')
    Wait-Ticks 8
}
# Layout: floor y=77, burner y=78, tank 2x2x1 at y=79 (x100-101, z100-101), engine east of it, shaft 2 blocks further.
$null=Command 'fill 92 77 92 112 90 112 minecraft:air'
$null=Command 'fill 92 77 92 112 77 112 minecraft:smooth_stone'
Wait-Ticks 30
foreach ($x in 100,101) { foreach ($z in 100,101) { $null=Command "setblock $x 79 $z create:fluid_tank" } }
$null=Command 'setblock 100 78 100 create:blaze_burner[blaze=smouldering]'
$null=Command 'setblock 102 79 100 create:steam_engine[face=wall,facing=east,waterlogged=false]'
Wait-Ticks 40
Scenario 'engine attaches and the shaft appears' {
    $d=Get-Data 100 79 100
    if ($d -notmatch 'Size:\s*2') { throw ('the tank did not form a 2x2: '+$d) }
    if ((State 102 79 100) -notmatch 'steam_engine') { throw ('engine state: '+(State 102 79 100)) }
    if ((Block 104 79 100) -ne 'create:powered_shaft') { throw ('no powered shaft at the shaft position, found '+(Block 104 79 100)+'; tank '+$d) }
    if ($d -notmatch 'Engines:\s*1') { throw ('boiler did not count the engine: '+$d) }
}
Scenario 'cold boiler keeps the shaft still' {
    $speed=Num (Get-Data 104 79 100) 'Speed'
    if ($speed -ne 0) { throw ('shaft turns without water: '+$speed) }
}
$null=Command 'setblock 101 80 101 create:steam_whistle[facing=north,wall=false,powered=false,size=medium]'
$null=Command 'setblock 102 80 101 minecraft:redstone_block'
# A plain shaft next to the powered shaft, along its axis, proves the speed reaches the network.
$axis=[regex]::Match((State 104 79 100),'axis=([xyz])').Groups[1].Value
$ext=switch ($axis) { 'x' { @(105,79,100) } 'y' { @(104,80,100) } default { @(104,79,101) } }
if ($axis) { $null=Command ('setblock '+($ext -join ' ')+' create:shaft[axis='+$axis+']') }
# Feed the burner like a player: coal on the burner makes it kindled.
$null=Command 'tp @s 97.5 78 100.5'
Wait-Ticks 8
$null=Command 'item replace entity @s weapon.mainhand with minecraft:coal 4'
$null=Invoke-Ctl @('look-at','block','100','78','100','west')
$null=Invoke-Ctl @('interact','100','78','100','west')
Wait-Ticks 10
Water-Bucket 100 79 100
Scenario 'water and heat turn the shaft' {
    $turning=$false;$detail=''
    for ($i=0;$i -lt 12;$i++) {
        Wait-Ticks 5
        $shaft=Get-Data 104 79 100
        if ((Num $shaft 'Speed') -ne 0 -or (Num $shaft 'GeneratedSpeed') -ne 0) { $turning=$true;$detail=$shaft;break }
    }
    if (-not $turning) { throw ('the shaft never turned; tank '+(Get-Data 100 79 100)+'; shaft '+(Get-Data 104 79 100)) }
    $network=$false
    for ($i=0;$i -lt 6;$i++) { if ((Num (Get-Data $ext[0] $ext[1] $ext[2]) 'Speed') -ne 0) { $network=$true;break }; Wait-Ticks 5 }
    if (-not $network) { throw ('the extension shaft stayed still: '+(Get-Data $ext[0] $ext[1] $ext[2])+'; powered '+(Get-Data 104 79 100)) }
    $level=Num (Get-Data 100 79 100) 'Level'
    if ($level -lt 1) { throw ('boiler level '+$level+'; burner '+(State 100 78 100)+' '+(Get-Data 100 78 100)+'; tank '+(Get-Data 100 79 100)) }
}
Scenario 'powered whistle sounds on a hot boiler' {
    $w=Get-Data 101 80 101
    if ($w -notmatch 'Sounding:\s*1') { throw ('whistle: '+$w+' state '+(State 101 80 101)) }
}
Scenario 'removing the burner stops the shaft' {
    $null=Command 'setblock 100 78 100 minecraft:air'
    $stopped=$false
    for ($i=0;$i -lt 30;$i++) { Wait-Ticks 10; if ((Num (Get-Data 104 79 100) 'Speed') -eq 0) { $stopped=$true;break } }
    if (-not $stopped) { throw ('shaft still turns: '+(Get-Data 104 79 100)) }
}
Scenario 'removing the engine restores a plain shaft' {
    $null=Command 'setblock 102 79 100 minecraft:air'
    $plain=$false
    for ($i=0;$i -lt 12;$i++) { Wait-Ticks 5; if ((Block 104 79 100) -eq 'create:shaft') { $plain=$true;break } }
    if (-not $plain) { throw ('powered shaft stayed: '+(Block 104 79 100)) }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)