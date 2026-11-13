#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Arm')][string] $Phase='Arm',
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
$transcript=Join-Path $game ('stage10-arm-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage10-arm-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='A creative motor drives an Arm through a cogwheel; the Arm item selects a chest (input) and a barrel (output); the placed Arm moves a stack. Visuals are not compared.'
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
$null=Command 'tp @s 100 90 100'
Wait-Ticks 40
$null=Command 'fill 92 77 92 112 90 112 minecraft:air'
$null=Command 'fill 92 78 92 112 78 112 minecraft:smooth_stone'
# Sources and sinks 3 blocks away on both sides of the future Arm.
$null=Command "setblock $ax $ay 97 minecraft:chest"
$null=Command "setblock $ax $ay 103 minecraft:barrel"
$null=Command "item replace block $ax $ay 97 container.0 with minecraft:cobblestone 8"
# Drive: motor under a cogwheel that meshes with the Arm cog.
$null=Command 'setblock 101 79 100 create:creative_motor[facing=up]{ScrollValue:64}'
$null=Command 'setblock 101 80 100 create:cogwheel[axis=y]'
Wait-Ticks 10
# Select the points with the Arm item: one click = take (input), two = deposit (output).
$null=Command 'item replace entity @s weapon.mainhand with create:mechanical_arm 1'
function Select-Point([int] $X,[int] $Y,[int] $Z,[int] $Times) {
    for ($i=0;$i -lt $Times;$i++) {
        $null=Command ('tp @s '+($X+0.5)+' '+($Y+3)+' '+($Z+0.5))
        Wait-Ticks 3
        $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,'up')
        $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,'up')
        Wait-Ticks 6
    }
}
Select-Point $ax $ay 97 1
Select-Point $ax $ay 103 2
# Place the Arm on a support block next to the cogwheel.
$null=Command ('setblock '+$ax+' 79 '+$az+' minecraft:smooth_stone')
$null=Command ('tp @s '+($ax+0.5)+' '+($ay+3)+' '+($az+0.5))
Wait-Ticks 3
for ($attempt=0;$attempt -lt 4;$attempt++) {
    # Stand beside the cell: falling from above would put the player inside it.
    $null=Command ('tp @s '+($ax-2.5)+' 79 '+($az+0.5))
    Wait-Ticks 8
    $null=Invoke-Ctl @('look-at','block',[string]$ax,'79',[string]$az,'up')
    $null=Invoke-Ctl @('interact',[string]$ax,'79',[string]$az,'up')
    Wait-Ticks 10
    if ((Block $ax $ay $az) -eq 'create:mechanical_arm') { break }
}
Expect ((Block $ax $ay $az) -eq 'create:mechanical_arm') ('The Arm was not placed, found '+(Block $ax $ay $az))
Expect ((Block $ax $ay 97) -eq 'minecraft:chest' -and (Block $ax $ay 103) -eq 'minecraft:barrel') 'The Arm item replaced an inventory instead of selecting it.'
Passed 'The Arm item selected the chest and the barrel without placing a block.'
Passed 'The Arm item placed the Arm block.'
$points=Get-Data $ax $ay $az Points
Expect ($points -match 'create:container' -and ([regex]::Matches($points,'Pos:')).Count -ge 2) ('The placed Arm did not take the selected points: '+$points)
Passed 'The placed Arm holds both selected points.'
$speed=Get-Data $ax $ay $az Speed
Expect ($speed -notmatch '\b0(\.0+)?[df]?\b' -or $speed -match '[1-9]') ('The Arm is not driven: '+$speed)
Passed ('The Arm is driven by the motor: '+$speed)
# Run until the stack has moved (two moves at 64 RPM plus searching).
for ($i=0;$i -lt 20;$i++) {
    Wait-Ticks 20
    $out=Counts $ax $ay 103
    if ($out.Count -ne 0) { break }
}
Assert-Only (Counts $ax $ay 103) 'minecraft:cobblestone' 8
Assert-Empty (Counts $ax $ay 97) 'The source chest'
Passed 'The Arm moved the whole stack from the chest to the barrel.'
$report.passed=$true
Save-Report
Write-Output 'Arm acceptance passed.'