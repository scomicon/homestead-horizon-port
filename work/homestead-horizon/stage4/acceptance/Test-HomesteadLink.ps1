#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Redstone Link: frequency by held items, wireless transmission with matching frequency only, analog strength, mode toggle and reload.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Link')][string] $Phase='Link',
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
$transcript=Join-Path $game ('stage11-link-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage11-link-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='A transmitter and a receiver share a frequency set with two held items; a redstone block powers the transmitter and the receiver lights a lamp; a different frequency stays dark; empty-hand use toggles the mode. Visuals are not compared.'
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
$null=Command 'fill 92 77 92 115 90 112 minecraft:air'
$null=Command 'fill 92 79 92 115 79 112 minecraft:smooth_stone'
# Let the registry sweep forget links of the previous run before placing new ones.
Wait-Ticks 30
$null=Command 'setblock 100 80 100 create:redstone_link[facing=up,receiver=false]'
$null=Command 'setblock 110 79 100 minecraft:redstone_lamp'
$null=Command 'setblock 110 80 100 create:redstone_link[facing=up,receiver=true]'
$null=Command 'setblock 120 80 100 minecraft:air'
Wait-Ticks 10
Expect ((Block 100 80 100) -like 'create:redstone_link*') ('The transmitter was not placed: '+(Block 100 80 100))
Expect ((Block 110 80 100) -like 'create:redstone_link*') ('The receiver was not placed: '+(Block 110 80 100))
Passed 'Transmitter and receiver exist.'
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Touch([int] $X,[int] $Z) {
    foreach ($face in 'up','north','south','west','east') {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,'80',[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,'80',[string]$Z,$face);return } catch { }
    }
    throw ('No face of the link at '+$X+' '+$Z+' can be targeted.')
}
function Set-Frequency([int] $X,[int] $Z,[string] $First,[string] $Last,[bool] $Side=$false) {
    foreach ($item in @($First,$Last)) {
        if ($Side) { $null=Command ('tp @s '+($X+1.5)+' 80 '+($Z+0.5)) } else { $null=Command ('tp @s '+($X+0.5)+' 82 '+($Z+0.5)) }
        Wait-Ticks 8
        $null=Command ('item replace entity @s weapon.mainhand with '+$item+' 1')
        Touch $X $Z
        Wait-Ticks 6
    }
}
Set-Frequency 100 100 'minecraft:stone' 'minecraft:dirt'
Set-Frequency 110 100 'minecraft:stone' 'minecraft:dirt'
$tx=Get-Data 100 80 100
Expect ($tx -match 'stone' -and $tx -match 'dirt') ('The transmitter did not take both frequency items: '+$tx)
$rx=Get-Data 110 80 100
Expect ($rx -match 'stone' -and $rx -match 'dirt') ('The receiver did not take both frequency items: '+$rx)
Passed 'Both links hold the frequency stone + dirt.'
Expect ((State 110 79 100) -notmatch 'lit=true') ('The lamp is lit before anything transmits: '+(State 100 80 100)+' '+(Get-Data 100 80 100)+' '+(State 110 80 100)+' '+(Get-Data 110 80 100))
$null=Command 'setblock 101 80 100 minecraft:redstone_block'
Wait-Ticks 20
Expect ((State 100 80 100) -match 'powered=true') ('The transmitter is not powered: '+(Block 100 80 100))
Expect ((State 110 80 100) -match 'powered=true') ('The receiver did not follow the transmitter: '+(Block 110 80 100))
Expect ((State 110 79 100) -match 'lit=true') ('The lamp did not light: '+(State 110 79 100))
Passed 'A matching receiver was powered wirelessly and lit the lamp.'
$null=Command 'setblock 101 80 100 minecraft:air'
Wait-Ticks 20
Expect ((State 110 80 100) -match 'powered=false') ('The receiver stayed powered: '+(Block 110 80 100))
Expect ((State 110 79 100) -notmatch 'lit=true') 'The lamp stayed lit.'
Passed 'The receiver switched off with the transmitter.'
# A second receiver with another frequency stays dark.
$null=Command 'setblock 105 79 106 minecraft:redstone_lamp'
$null=Command 'setblock 105 80 106 create:redstone_link[facing=up,receiver=true]'
Set-Frequency 105 106 'minecraft:stone' 'minecraft:gravel'
$null=Command 'setblock 101 80 100 minecraft:redstone_block'
Wait-Ticks 20
Expect ((State 110 80 100) -match 'powered=true') 'The matching receiver did not power again.'
Expect ((State 105 80 106) -notmatch 'powered=true') ('A receiver with another frequency was powered: '+(Block 105 80 106))
Passed 'A receiver with a different frequency was not powered.'
# Empty hand toggles the mode.
$null=Command 'tp @s 110.5 82 100.5'
Wait-Ticks 8
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
Touch 110 100
Wait-Ticks 8
Expect ((State 110 80 100) -match 'receiver=false') ('Empty-hand use did not switch the receiver to a transmitter: '+(Block 110 80 100))
Passed 'Empty-hand use toggled the mode.'
$report.passed=$true
Save-Report
Write-Output 'Link acceptance passed.'