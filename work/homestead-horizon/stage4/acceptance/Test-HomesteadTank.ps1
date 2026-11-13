#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Tank')][string] $Phase='Tank',
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
$transcript=Join-Path $game ('stage13-tank-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage13-tank-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Fluid Tank multiblock: a 3-high tower pools its fluid, buckets work through any member, breaking a member splits the tower, a 2x2x2 block forms, the Wrench toggles the windows. Each scenario is reported separately.'
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
function Clear-Area { $null=Command 'fill 92 77 92 112 95 112 minecraft:air'; $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'; Wait-Ticks 20 }
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function Use-Bucket([string] $Item,[int] $X,[int] $Y,[int] $Z,[bool] $Side=$false) {
    if ($Side) { $null=Command ('tp @s '+($X-2.5)+' '+$Y+' '+($Z+0.5)) } else { $null=Command ('tp @s '+($X+0.5)+' '+($Y+3)+' '+($Z+0.5)) }
    Wait-Ticks 6
    $null=Command ('item replace entity @s weapon.mainhand with '+$Item+' 1')
    $face = if ($Side) { 'west' } else { 'up' }
    $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$face)
    $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$face)
    Wait-Ticks 8
}
function Amount([int] $X,[int] $Y,[int] $Z) {
    $d=Get-Data $X $Y $Z
    $m=[regex]::Match($d,'amount:\s*(\d+)')
    if ($m.Success) { return [int]$m.Groups[1].Value } else { return 0 }
}
Scenario 'tower forms' {
    Clear-Area
    foreach ($y in 80..82) { $null=Command "setblock 100 $y 100 create:fluid_tank" }
    Wait-Ticks 30
    foreach ($y in 80..82) {
        $d=Get-Data 100 $y 100
        if ($d -notmatch 'Height:\s*3') { throw ('tank at y='+$y+' is not part of a 3-high tower: '+$d) }
    }
    if ((State 100 80 100) -notmatch 'bottom=true' -or (State 100 80 100) -match 'top=true') { throw ('bottom block state: '+(State 100 80 100)) }
    if ((State 100 81 100) -match 'top=true|bottom=true') { throw ('middle block state: '+(State 100 81 100)) }
    if ((State 100 82 100) -notmatch 'top=true') { throw ('top block state: '+(State 100 82 100)) }
}
Scenario 'bucket pools through any member' {
    foreach ($i in 1..3) { Use-Bucket 'minecraft:water_bucket' 100 82 100 }
    $a=Amount 100 80 100
    if ($a -ne 3000) { throw ('expected 3000 mB in the controller, found '+$a+': '+(Get-Data 100 80 100)) }
    Use-Bucket 'minecraft:bucket' 100 82 100
    $a=Amount 100 80 100
    if ($a -ne 2000) { throw ('expected 2000 mB after emptying one bucket, found '+$a) }
}
Scenario 'breaking a member splits the tower' {
    $null=Command 'setblock 100 81 100 minecraft:air'
    Wait-Ticks 30
    foreach ($y in 80,82) {
        $d=Get-Data 100 $y 100
        if ($d -notmatch 'Height:\s*1') { throw ('tank at y='+$y+' kept the tower height: '+$d) }
    }
}
Scenario 'lava does not merge with water' {
    Clear-Area
    $null=Command 'setblock 100 80 100 create:fluid_tank'
    $null=Command 'setblock 100 82 100 create:fluid_tank'
    Wait-Ticks 20
    Use-Bucket 'minecraft:water_bucket' 100 80 100 $true
    Use-Bucket 'minecraft:lava_bucket' 100 82 100 $true
    $null=Command 'setblock 100 81 100 create:fluid_tank'
    Wait-Ticks 40
    $heights=@(80,81,82|ForEach-Object { $m=[regex]::Match((Get-Data 100 $_ 100),'Height:\s*(\d+)'); [int]$m.Groups[1].Value })
    if ($heights -contains 3) { throw ('water and lava tanks merged into one tower: '+($heights -join ',')) }
    # The empty middle tank joins one neighbour; the pool lives in that controller.
    $water=(Get-Data 100 80 100); $upper=(Get-Data 100 81 100)+' '+(Get-Data 100 82 100)
    if ($water -notmatch 'water' -or $water -match 'lava') { throw ('water tank content: '+$water) }
    if ($upper -notmatch 'lava' -or $upper -match 'water') { throw ('lava tank content: '+$upper) }
}
Scenario '2x2x2 forms' {
    Clear-Area
    foreach ($x in 100,101) { foreach ($z in 100,101) { foreach ($y in 80,81) { $null=Command "setblock $x $y $z create:fluid_tank" } } }
    Wait-Ticks 40
    $d=Get-Data 100 80 100
    if ($d -notmatch 'Size:\s*2' -or $d -notmatch 'Height:\s*2') { throw ('2x2x2 did not form: '+$d) }
    if ((State 100 80 100) -notmatch 'shape=window_') { throw ('corner block shape: '+(State 100 80 100)) }
}
Scenario 'wrench toggles windows' {
    $null=Command 'tp @s 100.5 85 100.5'
    Wait-Ticks 6
    $null=Command 'item replace entity @s weapon.mainhand with create:wrench 1'
    $null=Invoke-Ctl @('look-at','block','100','81','100','up')
    $null=Invoke-Ctl @('interact','100','81','100','up')
    Wait-Ticks 10
    if ((State 100 81 100) -notmatch 'shape=plain') { throw ('windows stayed on: '+(State 100 81 100)) }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)