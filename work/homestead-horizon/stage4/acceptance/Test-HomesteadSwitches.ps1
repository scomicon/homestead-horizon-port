#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Redstone switches: Pulse Repeater / Extender / Timer, Powered (Toggle) Latch, Analog Lever, Redstone Contact, Nixie Tube and the Redstone Link frequency items.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Switches')][string] $Phase='Switches',
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
$transcript=Join-Path $game ('stage20-switches-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage20-switches-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Pulse devices emit and end their pulses, latches hold their output, an analog lever steps its strength when clicked and powers a lamp, two facing contacts power each other, a Nixie tube stores the strength it reads, and a link keeps its frequency items. Sneak behaviour (lever decrement, link frequency reset) is not driven by the control harness and stays manual. Visuals are not compared.'
    coordinates=@{origin=@(100,80,100)}
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
$null=Command 'forceload add 90 90 160 120'
$null=Command 'tp @s 98.5 82 98.5'
Wait-Ticks 40
$null=Command 'fill 92 77 92 160 90 112 minecraft:air'
$null=Command 'fill 92 79 92 160 79 112 minecraft:smooth_stone'
Wait-Ticks 30
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Seen([int] $X,[int] $Y,[int] $Z,[string] $Pattern,[int] $Ticks=30) {
    for ($i=0;$i -lt $Ticks;$i++) { if ((State $X $Y $Z) -match $Pattern) { return $true };Wait-Ticks 1 }
    return $false
}
# A one-tick pulse can fall between two polls when the game ticks while the socket answers (45 data mods make
# every tick slower). Step the clock by hand so every tick is observed.
function SeenStepped([int] $X,[int] $Y,[int] $Z,[string] $Pattern,[int] $Ticks=40) {
    $null=Command 'tick freeze'
    try {
        for ($i=0;$i -lt $Ticks;$i++) {
            if ((State $X $Y $Z) -match $Pattern) { return $true }
            $null=Command 'tick step 1'
            Start-Sleep -Milliseconds 80
        }
        return $false
    } finally { $null=Command 'tick unfreeze' }
}function Touch([int] $X,[int] $Z) {
    $null=Command ('tp @s '+($X+0.5)+' 82 '+($Z+0.5))
    Wait-Ticks 8
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
    foreach ($face in 'up','north','south','west','east') {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,'80',[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,'80',[string]$Z,$face);Wait-Ticks 6;return } catch { }
    }
    throw ('No face of the block at '+$X+' '+$Z+' can be targeted.')
}
Passed 'Harness ready.'

# ---- Pulse Extender: facing=east takes its input from the east side and outputs to the west.
$null=Command 'setblock 99 80 100 minecraft:redstone_lamp'
$null=Command 'setblock 100 80 100 create:pulse_extender[facing=east]'
Wait-Ticks 10
Expect ((Block 100 80 100) -like 'create:pulse_extender*') ('The extender was not placed: '+(Block 100 80 100))
$null=Command 'setblock 101 80 100 minecraft:redstone_block'
Expect (Seen 100 80 100 'powering=true' 30) ('The extender never raised its output: '+(State 100 80 100))
Wait-Ticks 6
Expect ((State 100 80 100) -match 'powering=true') 'The extender dropped its output while the input is held.'
Expect ((State 99 80 100) -match 'lit=true') ('The lamp in front of the extender is dark: '+(State 99 80 100))
Passed 'Pulse Extender raised and held its output while powered.'
$null=Command 'setblock 101 80 100 minecraft:air'
Expect (Seen 100 80 100 'powering=false' 40) ('The extender kept its output after the input fell: '+(State 100 80 100))
Expect ((State 99 80 100) -notmatch 'lit=true') 'The lamp stayed lit after the extender ended.'
Passed 'Pulse Extender ended its output after the input fell.'

# ---- Pulse Repeater: one short pulse, then dark while the input is still held.
$null=Command 'setblock 99 80 104 minecraft:redstone_lamp'
$null=Command 'setblock 100 80 104 create:pulse_repeater[facing=east]'
Wait-Ticks 10
$null=Command 'setblock 101 80 104 minecraft:redstone_block'
# One retry: under a loaded full run the first edge can land before the freshly placed repeater has registered
# its input side; a real defect fails both attempts.
$pulsed=SeenStepped 100 80 104 'powering=true' 40
if (-not $pulsed) {
    $null=Command 'setblock 101 80 104 minecraft:air';Wait-Ticks 10
    $null=Command 'setblock 101 80 104 minecraft:redstone_block'
    $pulsed=SeenStepped 100 80 104 'powering=true' 40
}
Expect $pulsed ('The repeater never pulsed: '+(State 100 80 104))
Wait-Ticks 30
Expect ((State 100 80 104) -match 'powering=false') ('The repeater pulse did not end while the input is held: '+(State 100 80 104))
Passed 'Pulse Repeater emitted one pulse and ended it.'

# ---- Pulse Timer pulses by itself (no input).
$null=Command 'setblock 100 80 108 create:pulse_timer[facing=east]'
Expect (Seen 100 80 108 'powering=true' 60) ('The timer never pulsed: '+(State 100 80 108))
Passed 'Pulse Timer pulsed without input.'

# ---- Powered Toggle Latch: rising edge flips the output, manual use flips it too.
$null=Command 'setblock 109 80 100 minecraft:redstone_lamp'
$null=Command 'setblock 110 80 100 create:powered_toggle_latch[facing=east]'
Wait-Ticks 10
$null=Command 'setblock 111 80 100 minecraft:redstone_block'
Expect (Seen 110 80 100 'powering=true' 30) ('The toggle latch did not switch on at the rising edge: '+(State 110 80 100))
Expect ((State 109 80 100) -match 'lit=true') 'The lamp did not light from the toggle latch.'
$null=Command 'setblock 111 80 100 minecraft:air'
Wait-Ticks 12
Expect ((State 110 80 100) -match 'powering=true') 'The toggle latch lost its output on the falling edge.'
$null=Command 'setblock 111 80 100 minecraft:redstone_block'
Expect (Seen 110 80 100 'powering=false' 30) ('The second rising edge did not switch it off: '+(State 110 80 100))
$null=Command 'setblock 111 80 100 minecraft:air'
Wait-Ticks 12
Touch 110 100
Expect ((State 110 80 100) -match 'powering=true') ('Empty-hand use did not toggle the latch: '+(State 110 80 100))
Passed 'Powered Toggle Latch toggled on the rising edge and by hand.'

# ---- Powered Latch: back input sets, side input resets.
$null=Command 'setblock 109 80 104 minecraft:redstone_lamp'
$null=Command 'setblock 110 80 104 create:powered_latch[facing=east]'
Wait-Ticks 10
$null=Command 'setblock 111 80 104 minecraft:redstone_block'
Expect (Seen 110 80 104 'powering=true' 30) ('The latch was not set by its back input: '+(State 110 80 104))
$null=Command 'setblock 111 80 104 minecraft:air'
Wait-Ticks 14
Expect ((State 110 80 104) -match 'powering=true') 'The latch forgot its state when the input dropped.'
$null=Command 'setblock 110 80 105 minecraft:redstone_block'
Expect (Seen 110 80 104 'powering=false' 30) ('The side input did not reset the latch: '+(State 110 80 104))
Passed 'Powered Latch was set by the back and reset by a side.'

# ---- Analog Lever: each use steps the strength and the lamp beside it lights.
$null=Command 'setblock 121 80 100 minecraft:redstone_lamp'
$null=Command 'setblock 120 80 100 create:analog_lever[facing=up,power=0]'
Wait-Ticks 10
Expect ((State 120 80 100) -match 'power=0') ('The lever was not placed: '+(State 120 80 100))
Touch 120 100
Expect ((State 120 80 100) -match 'power=1(,|\]|$)') ('One use did not raise the lever to 1: '+(State 120 80 100))
Touch 120 100
Expect ((State 120 80 100) -match 'power=2(,|\]|$)') ('A second use did not raise the lever to 2: '+(State 120 80 100))
Wait-Ticks 4
Expect ((State 121 80 100) -match 'lit=true') ('The lamp beside the lever is dark: '+(State 121 80 100))
$null=Command 'setblock 120 80 100 create:analog_lever[facing=up,power=15]'
Touch 120 100
Expect ((State 120 80 100) -match 'power=15') ('The lever went past 15: '+(State 120 80 100))
Passed 'Analog Lever stepped, powered a lamp and stopped at 15.'

# ---- Redstone Contact: two contacts facing each other power each other.
$null=Command 'setblock 130 80 100 create:redstone_contact[facing=east]'
Wait-Ticks 6
Expect ((State 130 80 100) -notmatch 'powered=true') 'A lone contact is powered.'
$null=Command 'setblock 131 80 100 create:redstone_contact[facing=west]'
Expect (Seen 130 80 100 'powered=true' 30) ('Facing contacts did not power: '+(State 130 80 100))
Expect (Seen 131 80 100 'powered=true' 30) ('The second contact did not power: '+(State 131 80 100))
$null=Command 'setblock 131 80 100 minecraft:air'
Expect (Seen 130 80 100 'powered=false' 30) ('The contact stayed powered after its partner left: '+(State 130 80 100))
Passed 'Redstone Contacts powered each other and released.'

# ---- Nixie Tube stores the strength it reads (the glyph itself is not compared).
$null=Command 'setblock 140 80 100 create:nixie_tube[mount=floor_south]'
Wait-Ticks 10
$null=Command 'setblock 141 80 100 minecraft:redstone_block'
Wait-Ticks 10
$nixie=Get-Data 140 80 100
Expect ($nixie -match 'RedstoneStrength' -and $nixie -match '15') ('The Nixie tube did not read 15: '+$nixie)
$null=Command 'setblock 141 80 100 minecraft:air'
Wait-Ticks 10
$nixie=Get-Data 140 80 100
Expect ($nixie -match 'RedstoneStrength' -and $nixie -notmatch '15') ('The Nixie tube kept 15: '+$nixie)
Passed 'Nixie Tube followed the redstone strength.'

# ---- Redstone Link keeps frequency items (reset by sneaking is manual).
function Set-Frequency([int] $X,[int] $Z,[string] $First,[string] $Last) {
    foreach ($item in @($First,$Last)) {
        $null=Command ('tp @s '+($X+0.5)+' 82 '+($Z+0.5))
        Wait-Ticks 8
        $null=Command ('item replace entity @s weapon.mainhand with '+$item+' 1')
        Touch2 $X $Z
        Wait-Ticks 6
    }
}
function Touch2([int] $X,[int] $Z) {
    foreach ($face in 'up','north','south','west','east') {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,'80',[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,'80',[string]$Z,$face);return } catch { }
    }
    throw ('No face of the link at '+$X+' '+$Z+' can be targeted.')
}
$null=Command 'setblock 150 80 100 create:redstone_link[facing=up,receiver=false]'
Wait-Ticks 10
Set-Frequency 150 100 'minecraft:stone' 'minecraft:dirt'
$tx=Get-Data 150 80 100
Expect ($tx -match 'stone' -and $tx -match 'dirt') ('The link did not take its frequency: '+$tx)
Passed 'The link holds a frequency; sneak + empty hand clears it (manual check).'
$report.passed=$true
Save-Report
Write-Output 'Switches acceptance passed.'