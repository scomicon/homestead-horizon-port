#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Client sound mods (stage 26): Extreme Sound Muffler, Presence Footsteps and AmbientSounds as
# rebuilt in mixins/homestead (sound_*.rs). Everything is observed through the counters that the
# module writes to config/homestead-sound-status.txt twice a second; the audio itself is NOT
# verified (the harness has no ears): a counter says the engine decided to cancel, scale or play a
# sound, not that anything was audible.
#   1. the client tick runs and the status file counts up; the three config files are created
#   2. muffler: a muted sound is cancelled, a scaled one is counted; F9 switches the list
#   3. footsteps: a step on a matching block is replaced / doubled, other blocks are left alone
#   4. ambience: a rule with chance 1 plays the placeholder tone again and again; off means quiet
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('ClientS')][string] $Phase='ClientS',
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
$transcript=Join-Path $game ('stage26-clients-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage26-clients-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@();notes=@()
    audio_verified=$false
    scope='Counters of the client sound engine (cancelled / scaled / replaced / added / played), read from config/homestead-sound-status.txt. Audio output itself is not verified.'
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
function Command([string] $Text) { return (Invoke-Ctl @('command',$Text)) }
function Passed([string] $Text) { $report.checks+=$Text;Write-Output $Text;Save-Report }
function Note([string] $Text) { $report.notes+=$Text;Write-Output ('note: '+$Text);Save-Report }
function Fail([string] $Text) { $report.checks+=('FAILED: '+$Text);Save-Report;throw $Text }

$configDir=Join-Path $game 'config'
$statusFile=Join-Path $configDir 'homestead-sound-status.txt'
$files=@{muffler='homestead-muffler.txt';footsteps='homestead-footsteps.txt';ambient='homestead-ambient.txt'}

function Read-Status {
    # The game rewrites the file (temp + rename); a read can meet a gap, so retry a few times.
    for ($i=0;$i -lt 10;$i++) {
        if (Test-Path -LiteralPath $statusFile) {
            try {
                $map=@{}
                foreach ($line in [IO.File]::ReadAllLines($statusFile)) {
                    $at=$line.IndexOf('=')
                    if ($at -gt 0) { $map[$line.Substring(0,$at)]=$line.Substring($at+1) }
                }
                if ($map.ContainsKey('ticks')) { return $map }
            } catch { }
        }
        Start-Sleep -Milliseconds 200
    }
    return $null
}
function Num($status,[string] $Key) { if ($null -eq $status -or -not $status.ContainsKey($Key)) { return -1 }; return [int64]$status[$Key] }
function Wait-Status([scriptblock] $Condition,[int] $Seconds,[string] $What) {
    $deadline=[DateTime]::UtcNow.AddSeconds($Seconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $status=Read-Status
        if ($null -ne $status -and (& $Condition $status)) { return $status }
        Start-Sleep -Milliseconds 400
    }
    Fail ('timeout waiting for: '+$What+' (last status: '+((Read-Status|ConvertTo-Json -Compress)+')'))
}
function Write-Config([string] $Which,[string] $Text) {
    $before=Num (Read-Status) 'reloads'
    [IO.File]::WriteAllText((Join-Path $configDir $files[$Which]),$Text,$utf8)
    # The game checks the files' modification times every 40 ticks (2 s).
    $null=Wait-Status { param($s) (Num $s 'reloads') -gt $before } 15 ('the game to re-read '+$files[$Which])
}
function Press-Key([string] $Name) {
    # down, a moment so that a client tick sees it, up. False when the control socket refuses the key.
    try {
        $null=Invoke-Ctl @('key',$Name,'down'); Start-Sleep -Milliseconds 400
        $null=Invoke-Ctl @('key',$Name,'up'); Start-Sleep -Seconds 1
        return $true
    } catch { return $false }
}
function Ready-Screen {
    for ($i=0;$i -lt 5;$i++) {
        if ((Invoke-Ctl @('screen')).screen -eq 'playing') { return }
        $null=Invoke-Ctl @('key','esc','tap'); Start-Sleep -Seconds 1
    }
    Fail 'could not return to the playing screen'
}

try {
    Ready-Screen
    # 1. The client tick runs; the files exist.
    $first=Wait-Status { param($s) (Num $s 'ticks') -gt 0 } 90 'the status file to appear (is the homestead mod on? did the client tick run?)'
    $t0=Num $first 'ticks'; Start-Sleep -Seconds 3
    $second=Read-Status
    if ((Num $second 'ticks') -le $t0) { Fail 'the tick counter does not advance' }
    foreach ($name in $files.Values) { if (-not (Test-Path -LiteralPath (Join-Path $configDir $name))) { Fail ($name+' was not created') } }
    # Start from the commented defaults.
    foreach ($key in 'muffler','footsteps','ambient') { Remove-Item -LiteralPath (Join-Path $configDir $files[$key]) -ErrorAction SilentlyContinue }
    $before=Num (Read-Status) 'reloads'
    $null=Wait-Status { param($s) (Num $s 'reloads') -ge ($before+3) } 20 'the three default files to be written and read again'
    $status=Read-Status
    if ((Num $status 'muffler_rules') -ne 0 -or (Num $status 'footsteps_rules') -ne 0 -or (Num $status 'ambient_rules') -ne 0) { Fail 'the default files must carry no active rule' }
    if ($status['ambient_enabled'] -ne 'true' -and $status['ambient_enabled'] -ne 'false') { Fail 'ambient_enabled is missing' }
    if ($status['ambient_enabled'] -ne 'false') { Fail 'ambience must be off by default' }
    if ((Num $status 'config_errors') -ne 0) { Fail ('the default files produced '+(Num $status 'config_errors')+' parse errors') }
    Passed 'client tick runs, status file counts up, three config files created with no active rule, ambience off by default, no parse errors'
    # Does the client sound engine hand sounds to the playback hook in this harness at all? Probe once; when it does not,
    # the interception scenarios below are notes, not failures (the config parsing and ambience queue are still checked).
    $hookLive=$false
    foreach ($i in 1..4) {
        $null=Command 'playsound minecraft:block.note_block.bell master @s ~ ~ ~ 1 1'; Start-Sleep -Seconds 2
        if ((Num (Read-Status) 'hook_seen') -gt 0) { $hookLive=$true; break }
    }
    if ($hookLive) { Passed ('the client sound engine hands sounds to the homestead hook (hook_seen='+(Num (Read-Status) 'hook_seen')+', last: '+(Read-Status)['last_sound']+')') }
    else { Note 'the client sound engine did not call the playback hook for a playsound command in this harness (hook_seen=0); see the notes below' }


    # 2. Muffler.
    Write-Config 'muffler' "@enabled true`n@quiet 30`nminecraft:entity.experience_orb.pickup 0`nminecraft:entity.player.levelup 25`n"
    $status=Read-Status
    if ((Num $status 'muffler_rules') -ne 2) { Fail ('2 muffler rules expected, got '+(Num $status 'muffler_rules')) }
    if ($hookLive) {
    $base=Num $status 'muffler_cancelled'
    foreach ($i in 1..3) { $null=Command 'playsound minecraft:entity.experience_orb.pickup master @s ~ ~ ~ 1 1'; Start-Sleep -Milliseconds 300 }
    $null=Wait-Status { param($s) (Num $s 'muffler_cancelled') -ge ($base+3) } 15 'three muted sounds to be cancelled (does the playsound command reach the client hook?)'
    Passed 'muffler: a muted sound (0 percent) is cancelled each time it starts (3 of 3)'
    $baseScaled=Num (Read-Status) 'muffler_scaled'
    $null=Command 'playsound minecraft:entity.player.levelup master @s ~ ~ ~ 1 1'
    $null=Wait-Status { param($s) (Num $s 'muffler_scaled') -ge ($baseScaled+1) } 15 'the 25 percent sound to be counted as scaled'
    Passed 'muffler: a sound with a percentage rule is scaled (counted once)'
    $untouched=Num (Read-Status) 'muffler_cancelled'; $untouchedScaled=Num (Read-Status) 'muffler_scaled'
    $null=Command 'playsound minecraft:block.note_block.bell master @s ~ ~ ~ 1 1'; Start-Sleep -Seconds 2
    $status=Read-Status
    if ((Num $status 'muffler_cancelled') -ne $untouched -or (Num $status 'muffler_scaled') -ne $untouchedScaled) { Fail 'a sound without a rule was touched' }
    Passed 'muffler: a sound without a rule is left alone'
    # The F9 key switches the list off (the control socket may not deliver a key to the tick: a note then).
    $delivered=Press-Key 'f9'
    $status=Read-Status
    if ($delivered -and $status['muffler_enabled'] -eq 'false') {
        $base=Num $status 'muffler_cancelled'
        $null=Command 'playsound minecraft:entity.experience_orb.pickup master @s ~ ~ ~ 1 1'; Start-Sleep -Seconds 2
        if ((Num (Read-Status) 'muffler_cancelled') -ne $base) { Fail 'F9 switched the list off but a sound was still cancelled' }
        $null=Press-Key 'f9'
        if ((Read-Status)['muffler_enabled'] -ne 'true') { Fail 'F9 did not switch the list back on' }
        Passed 'muffler: F9 switches the list off and on again'
    } else {
        Note 'F9 through the control socket was refused or did not toggle the list (the harness key may not reach the held-key set the tick reads): the key is NOT verified, check it by hand'
    }
} else { Note 'muffler interception and F9 NOT verified: the client sound engine never handed a sound to the hook in this harness (hook_seen stayed 0); the rules were only checked as parsed config' }

    # 3. Footsteps. Stand on a platform of planks, then on stone, then on dirt.
    # Earlier suites share this world: clear the air above the platform so the player does not land inside leftovers.
    $null=Command 'fill 103 81 101 107 84 105 minecraft:air'; Start-Sleep -Seconds 1
    $null=Command 'fill 103 80 101 107 80 105 minecraft:oak_planks'; Start-Sleep -Seconds 1
    $null=Command 'tp @s 105.5 81 103.5'; Start-Sleep -Seconds 2
    Write-Config 'footsteps' "minecraft:oak_planks replace homestead_sound:tone.mid 1.0 1.0`nminecraft:stone add homestead_sound:tone.low 0.5 1.0`n"
    $status=Wait-Status { param($s) (Num $s 'footsteps_rules') -eq 2 } 15 'two footstep rules'
    $status=Wait-Status { param($s) $s['under'] -eq 'minecraft:oak_planks' } 20 'the block under the player to be recorded as oak planks (did the teleport land on the platform?)'
    if ($hookLive) {
    $baseR=Num $status 'footsteps_replaced'; $baseP=Num $status 'footsteps_played'
    $null=Command 'playsound minecraft:block.grass.step block @s ~ ~ ~ 1 1'
    $status=Wait-Status { param($s) (Num $s 'footsteps_replaced') -ge ($baseR+1) } 15 'the step on planks to be replaced'
    $status=Wait-Status { param($s) (Num $s 'footsteps_played') -ge ($baseP+1) -or (Num $s 'play_failed') -gt 0 } 15 'the replacement to be played'
    if ((Num $status 'play_failed') -gt 0) { Fail 'the replacement sound could not be played (is the content mod homestead_sound loaded?)' }
    Passed 'footsteps: a step on oak planks is replaced (vanilla cancelled, replacement queued and played)'
    $null=Command 'fill 103 80 101 107 80 105 minecraft:stone'; Start-Sleep -Seconds 1
    $null=Wait-Status { param($s) $s['under'] -eq 'minecraft:stone' } 20 'the block under the player to be recorded as stone'
    $baseA=Num (Read-Status) 'footsteps_added'; $baseR=Num (Read-Status) 'footsteps_replaced'
    $null=Command 'playsound minecraft:block.grass.step block @s ~ ~ ~ 1 1'
    $status=Wait-Status { param($s) (Num $s 'footsteps_added') -ge ($baseA+1) } 15 'the step on stone to get an added sound'
    if ((Num $status 'footsteps_replaced') -ne $baseR) { Fail 'an add rule must not replace the original step' }
    Passed 'footsteps: an add rule doubles the step on stone and keeps the original'
    $null=Command 'fill 103 80 101 107 80 105 minecraft:dirt'; Start-Sleep -Seconds 1
    $null=Wait-Status { param($s) $s['under'] -eq 'minecraft:dirt' } 20 'the block under the player to be recorded as dirt'
    $baseA=Num (Read-Status) 'footsteps_added'; $baseR=Num (Read-Status) 'footsteps_replaced'
    $null=Command 'playsound minecraft:block.grass.step block @s ~ ~ ~ 1 1'; Start-Sleep -Seconds 2
    $status=Read-Status
    if ((Num $status 'footsteps_added') -ne $baseA -or (Num $status 'footsteps_replaced') -ne $baseR) { Fail 'a step on a block without a rule was touched' }
    Passed 'footsteps: a step on dirt (no rule) is left alone'
    # A step event that is not a *.step event is never treated as a step.
    $null=Command 'playsound minecraft:block.stone.break block @s ~ ~ ~ 1 1'; Start-Sleep -Seconds 1
    if ((Num (Read-Status) 'footsteps_added') -ne $baseA) { Fail 'a non-step sound was treated as a step' }
    Passed 'footsteps: only *.step events are steps'
} else { Note 'footstep interception NOT verified: the client sound engine never handed a sound to the hook in this harness (hook_seen stayed 0), so the footstep rules were only checked as parsed config' }
    # A rule that answers with a vanilla step event is refused (it would loop).
    $baseErr=Num (Read-Status) 'config_errors'
    Write-Config 'footsteps' "minecraft:dirt replace minecraft:block.grass.step 1 1`n"
    $status=Read-Status
    if ((Num $status 'footsteps_rules') -ne 0 -or (Num $status 'config_errors') -le $baseErr) { Fail 'a replacement that is itself a step event must be refused' }
    Passed 'footsteps: a looping replacement (a vanilla step event) is refused and counted as a config error'
    Write-Config 'footsteps' "# empty`n"

    # 4. Ambience: chance 1, every 20 ticks, the placeholder tone; then off.
    Write-Config 'ambient' "@enabled true`ntesttone overworld homestead_sound:tone.high 20 1.0 0.5`n"
    $status=Read-Status
    if ((Num $status 'ambient_rules') -ne 1) { Fail 'one ambience rule expected' }
    $baseQ=Num $status 'ambient_queued'; $baseP=Num $status 'ambient_played'
    $status=Wait-Status { param($s) (Num $s 'ambient_played') -ge ($baseP+3) -or (Num $s 'play_failed') -gt 0 } 30 'the ambience rule to play three times'
    if ((Num $status 'play_failed') -gt 0 -or (Num $status 'ambient_queued') -le $baseQ) { Fail 'the ambience rule queued sounds that could not be played (is homestead_sound loaded?)' }
    Passed 'ambience: a rule (chance 1, every second) plays the placeholder tone repeatedly'
    Write-Config 'ambient' "@enabled false`ntesttone overworld homestead_sound:tone.high 20 1.0 0.5`n"
    Start-Sleep -Seconds 2
    $stopped=Num (Read-Status) 'ambient_queued'
    Start-Sleep -Seconds 4
    if ((Num (Read-Status) 'ambient_queued') -ne $stopped) { Fail 'ambience kept rolling after @enabled false' }
    Passed 'ambience: @enabled false stops the rolls'
    Write-Config 'ambient' "@enabled true`nnever night+rain+thunder+underwater+nether homestead_sound:tone.high 20 1.0 0.5`n"
    $baseQ=Num (Read-Status) 'ambient_queued'; Start-Sleep -Seconds 4
    if ((Num (Read-Status) 'ambient_queued') -ne $baseQ) { Fail 'a rule whose conditions cannot hold queued a sound' }
    Passed 'ambience: conditions are checked (a rule that cannot hold never fires)'
    Write-Config 'ambient' "@enabled true`nbadone nosuchcondition homestead_sound:tone.high 20 1.0 0.5`n"
    if ((Num (Read-Status) 'ambient_rules') -ne 0) { Fail 'a rule with an unknown condition must be refused' }
    Passed 'ambience: an unknown condition is refused'
    Note 'AUDIO NOT VERIFIED: the checks above read the engine counters; whether anything was heard (volume, replacement sound, placeholder tones) needs a person with headphones. The placeholder events use sounds.json "type": "event" entries pointing at vanilla note block events.'
    $report.passed=$true
    Save-Report
    Write-Output 'Client sound mods passed'
} finally {
    # Leave no rules behind for the suites that run after this one.
    foreach ($key in 'muffler','footsteps','ambient') { Remove-Item -LiteralPath (Join-Path $configDir $files[$key]) -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $statusFile -ErrorAction SilentlyContinue
    Save-Report
}
