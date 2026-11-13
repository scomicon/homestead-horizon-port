#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Linked Controller: bound buttons power receivers of the same frequency through operator commands and right-click binding.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Controller')][string] $Phase='Controller',
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
$transcript=Join-Path $game ('stage23-controller-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage23-controller-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='A Linked Controller bound with create controller set/bind/right-click presses buttons; receivers with the same two frequency items power and time out after 30 ticks, others stay dark. Key input (G and Z X C V B N) and visuals are not exercised.'
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

function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Touch([int] $X,[int] $Z) {
    foreach ($face in 'up','north','south','west','east') {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,'80',[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,'80',[string]$Z,$face);return } catch { }
    }
    throw ('No face of the link at '+$X+' '+$Z+' can be targeted.')
}
function Status { return ((Command 'create controller status').chat -join ' ') }
function Set-LinkFrequency([int] $X,[int] $Z,[string] $First,[string] $Last) {
    foreach ($item in @($First,$Last)) {
        $null=Command ('tp @s '+($X+0.5)+' 82 '+($Z+0.5))
        Wait-Ticks 8
        $null=Command ('item replace entity @s weapon.mainhand with '+$item+' 1')
        Touch $X $Z
        Wait-Ticks 6
    }
}
function Hold-Controller { $null=Command 'item replace entity @s weapon.mainhand with create:linked_controller 1';Wait-Ticks 4 }

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 110 120 130'
$null=Command 'tp @s 98.5 80 118.5'
Wait-Ticks 40
$null=Command 'fill 92 77 112 115 90 130 minecraft:air'
$null=Command 'fill 92 79 112 115 79 130 minecraft:smooth_stone'
Wait-Ticks 30
# Two receivers with lamps: A (stone + dirt) and B (stone + gravel).
$null=Command 'setblock 100 79 120 minecraft:redstone_lamp'
$null=Command 'setblock 100 80 120 create:redstone_link[facing=up,receiver=true]'
$null=Command 'setblock 104 79 120 minecraft:redstone_lamp'
$null=Command 'setblock 104 80 120 create:redstone_link[facing=up,receiver=true]'
Wait-Ticks 10
Expect ((Block 100 80 120) -like 'create:redstone_link*') 'Receiver A was not placed.'
Expect ((Block 104 80 120) -like 'create:redstone_link*') 'Receiver B was not placed.'
Set-LinkFrequency 100 120 'minecraft:stone' 'minecraft:dirt'
Set-LinkFrequency 104 120 'minecraft:stone' 'minecraft:gravel'
Passed 'Two receivers exist with frequencies stone+dirt and stone+gravel.'

# The item exists, takes a frequency per button and reports it.
Hold-Controller
$null=Command 'create controller clear'
$null=Command 'create controller set 0 stone dirt'
$status=Status
Expect ($status -match 'stone' -and $status -match 'dirt') ('Button 0 did not take its frequency: '+$status)
Passed 'create controller set stores the two frequency items on the item.'

# A press powers only the receiver with the same frequency.
Expect ((State 100 79 120) -notmatch 'lit=true') 'Lamp A is lit before any press.'
$null=Command 'create controller press 0'
Wait-Ticks 6
Expect ((State 100 80 120) -match 'powered=true') ('Receiver A was not powered by the press: '+(Block 100 80 120))
Expect ((State 100 79 120) -match 'lit=true') 'Lamp A did not light.'
Expect ((State 104 80 120) -notmatch 'powered=true') ('Receiver B was powered by another frequency: '+(Block 104 80 120))
Passed 'A pressed button powered the matching receiver and left the other dark.'

# Held inputs time out after 30 ticks without a keep-alive.
Wait-Ticks 45
Expect ((State 100 80 120) -notmatch 'powered=true') ('Receiver A stayed powered after the timeout: '+(Block 100 80 120))
Expect ((State 100 79 120) -notmatch 'lit=true') 'Lamp A stayed lit after the timeout.'
Passed 'The held input expired after its timeout.'

# Release switches off at once (well before the timeout).
$null=Command 'create controller press 0'
Wait-Ticks 4
Expect ((State 100 80 120) -match 'powered=true') 'Receiver A did not power for the release test.'
$null=Command 'create controller release 0'
Wait-Ticks 6
Expect ((State 100 80 120) -notmatch 'powered=true') ('Receiver A stayed powered after release: '+(Block 100 80 120))
Passed 'Release unpowered the receiver before the timeout.'

# Binding from a link in the world.
$null=Command 'create controller bind 1 104 80 120'
$status=Status
Expect ($status -match 'gravel') ('Button 1 did not copy the link frequency: '+$status)
$null=Command 'create controller press 1'
Wait-Ticks 6
Expect ((State 104 80 120) -match 'powered=true') ('Receiver B was not powered by button 1: '+(Block 104 80 120))
Expect ((State 100 80 120) -notmatch 'powered=true') 'Receiver A powered with button 1.'
Passed 'create controller bind copied a link frequency to button 1 and it powered receiver B only.'
$null=Command 'create controller release 1'
Wait-Ticks 6

# Right-click binding: a cleared controller takes the link into button 0, then button 1.
$null=Command 'create controller clear'
$null=Command 'tp @s 100.5 82 120.5'
Wait-Ticks 8
Touch 100 120
Wait-Ticks 6
$status=Status
Expect ($status -match 'next=1' -and $status -match 'dirt') ('Right-click did not bind the link to button 0: '+$status)
$null=Command 'tp @s 104.5 82 120.5'
Wait-Ticks 8
Touch 104 120
Wait-Ticks 6
$status=Status
Expect ($status -match 'next=2' -and $status -match 'gravel') ('Right-click did not bind the second link to button 1: '+$status)
Expect ((State 100 80 120) -match 'receiver=true') 'Right-click with the controller toggled or re-keyed the link.'
Passed 'Right-clicking links with the controller bound them to buttons 0 and 1.'

# Sneak-use clears the button bound last: skipped (key state cannot be driven from the control socket).
$report.passed=$true
Save-Report
Write-Output 'Controller acceptance passed.'