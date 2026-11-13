#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Points')][string] $Phase='Points',
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
$transcript=Join-Path $game ('stage12-points-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage12-points-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='One Mechanical Arm scenario per point type: an input chest feeds the Arm and the target block must receive the item (or, for extraction, the chest must receive it). Each scenario is reported separately.'
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



$ax=100;$ay=80;$az=100
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 98.5 80 98.5'
Wait-Ticks 40
$results=[ordered]@{}
function Click-At([int] $X,[int] $Y,[int] $Z,[int] $Times) {
    for ($i=0;$i -lt $Times;$i++) {
        $null=Command ('tp @s '+($X+0.5)+' '+($Y+3)+' '+($Z+0.5))
        Wait-Ticks 4
        foreach ($face in 'up','north','south','west','east') {
            try { $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$face);break } catch { }
        }
        Wait-Ticks 6
    }
}
function Run-Scenario([string] $Name,[string[]] $Setup,[string] $Item,[int[]] $Source,[int[]] $Target,[int] $TargetClicks,[scriptblock] $Check,[int] $Timeout=300) {
    try {
        $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
        $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
        Wait-Ticks 30
        $null=Command ('setblock 101 79 100 create:creative_motor[facing=up]{ScrollValue:64}')
        $null=Command 'setblock 101 80 100 create:cogwheel[axis=y]'
        if ($Source) {
            $null=Command ('setblock '+($Source -join ' ')+' minecraft:chest')
            if ($Item) { $null=Command ('item replace block '+($Source -join ' ')+' container.0 with '+$Item) }
        }
        foreach ($command in $Setup) { $null=Command $command }
        Wait-Ticks 10
        $null=Command 'item replace entity @s weapon.mainhand with create:mechanical_arm 1'
        if ($Source) { Click-At $Source[0] $Source[1] $Source[2] 1 }
        Click-At $Target[0] $Target[1] $Target[2] $TargetClicks
        $null=Command 'setblock 100 79 100 minecraft:smooth_stone'
        for ($attempt=0;$attempt -lt 4;$attempt++) {
            $null=Command 'tp @s 97.5 80 100.5'
            Wait-Ticks 8
            $null=Invoke-Ctl @('look-at','block','100','79','100','up')
            $null=Invoke-Ctl @('interact','100','79','100','up')
            Wait-Ticks 10
            if ((Block 100 80 100) -eq 'create:mechanical_arm') { break }
        }
        if ((Block 100 80 100) -ne 'create:mechanical_arm') { throw 'the Arm was not placed' }
        $points=Get-Data 100 80 100 Points
        $done=$false
        for ($waited=0;$waited -lt $Timeout;$waited+=20) {
            Wait-Ticks 20
            if (& $Check) { $done=$true;break }
        }
        if ($done) { $results[$Name]='ok' } else { $results[$Name]=('no transfer; points '+$points+'; target '+(Get-Data $Target[0] $Target[1] $Target[2])+'; state '+(State $Target[0] $Target[1] $Target[2])+'; source '+(Get-Data 100 80 97)) }
    } catch {
        $results[$Name]=('error: '+$_.Exception.Message)
    }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Has([string] $Pattern,[int] $X,[int] $Y,[int] $Z) { return ((Get-Data $X $Y $Z) -match $Pattern) }

Run-Scenario 'container' @('setblock 100 80 103 minecraft:barrel') 'minecraft:cobblestone 8' @(100,80,97) @(100,80,103) 2 { Has 'cobblestone' 100 80 103 }
Run-Scenario 'basin' @('setblock 100 80 103 create:basin') 'minecraft:cobblestone 8' @(100,80,97) @(100,80,103) 2 { Has 'cobblestone' 100 80 103 }
Run-Scenario 'depot' @('setblock 100 80 103 create:depot') 'minecraft:cobblestone 8' @(100,80,97) @(100,80,103) 2 { Has 'cobblestone' 100 80 103 }
Run-Scenario 'millstone' @('setblock 100 80 103 create:millstone') 'minecraft:wheat 8' @(100,80,97) @(100,80,103) 2 { Has 'wheat' 100 80 103 }
Run-Scenario 'chute' @('setblock 100 80 103 create:chute[facing=down]') 'minecraft:cobblestone 8' @(100,80,97) @(100,80,103) 2 { Has 'cobblestone' 100 80 103 }
Run-Scenario 'funnel' @('setblock 100 80 104 minecraft:chest','setblock 100 80 103 create:andesite_funnel[facing=north]') 'minecraft:cobblestone 8' @(100,80,97) @(100,80,103) 1 { Has 'cobblestone' 100 80 104 }
Run-Scenario 'crafter' @('setblock 100 80 103 create:mechanical_crafter[facing=north]') 'minecraft:cobblestone 8' @(100,80,97) @(100,80,103) 2 { Has 'cobblestone' 100 80 103 }
Run-Scenario 'deployer' @('setblock 100 80 103 create:deployer[facing=down]') 'minecraft:bone_meal 4' @(100,80,97) @(100,80,103) 2 { Has 'bone_meal' 100 80 103 }
Run-Scenario 'blaze_burner' @('setblock 100 80 103 create:blaze_burner[blaze=smouldering]') 'minecraft:coal 4' @(100,80,97) @(100,80,103) 1 { (Get-Data 100 80 97) -notmatch 'count: 4' }
Run-Scenario 'campfire' @('setblock 100 80 103 minecraft:campfire') 'minecraft:beef 1' @(100,80,97) @(100,80,103) 1 { Has 'beef' 100 80 103 }
Run-Scenario 'composter' @('setblock 100 80 103 minecraft:composter') 'minecraft:wheat_seeds 8' @(100,80,97) @(100,80,103) 2 { (State 100 80 103) -match 'level=[1-9]' }
Run-Scenario 'jukebox' @('setblock 100 80 103 minecraft:jukebox') 'minecraft:music_disc_cat 1' @(100,80,97) @(100,80,103) 2 { (State 100 80 103) -match 'has_record=true' }
Run-Scenario 'respawn_anchor' @('setblock 100 80 103 minecraft:respawn_anchor') 'minecraft:glowstone 1' @(100,80,97) @(100,80,103) 1 { (State 100 80 103) -match 'charges=[1-9]' }
# Extraction: a depot holding items is the input, a chest the output.
Run-Scenario 'depot_extract' @('setblock 100 80 97 create:depot','item replace block 100 80 97 container.0 with minecraft:cobblestone 4','setblock 100 80 103 minecraft:chest') $null @(100,80,97) @(100,80,103) 2 { Has 'cobblestone' 100 80 103 } 
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)