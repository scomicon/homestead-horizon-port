#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Strict live playback gate: absence of actual sound callbacks is a failure.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Playback')][string] $Phase='Playback',
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
$transcript=Join-Path $game ('stage33-soundplayback-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage33-soundplayback-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@();notes=@()
    binary_sha256=(Get-FileHash -LiteralPath (Join-Path (Split-Path -Parent $ctlPath) 'horizon-client.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    playback_hook_verified=$false; audio_verified=$false
    scope='Actual client playback callback and rule decision counters. Numeric post-hook parameters and physical audio output are not measured.'
}
[IO.File]::WriteAllText($transcript,'',$utf8)
function Protect-Text([string] $Text) {
    return [regex]::Replace($Text,'(?i)[a-z]:[\\/]+users[\\/]+[^\\/\s"]+','<user-profile>')
}
function Save-Report { [IO.File]::WriteAllText($reportPath,(Protect-Text ($report|ConvertTo-Json -Depth 25))+[Environment]::NewLine,$utf8) }
function Read-StartupLog([string] $Path) {
    # Start-Process keeps stderr open while the client runs. Permit its writer
    # and log rotation while reading the completed audio-initialization message.
    $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
    $reader=[IO.StreamReader]::new($stream)
    try { return $reader.ReadToEnd() } finally { $reader.Dispose() }
}
function Assert-AudioBackend([string] $Path) {
    if (-not (Test-Path -LiteralPath $Path)) { Fail 'The isolated startup log is missing; audio-engine initialization cannot be checked.' }
    $failure=[regex]::Match((Read-StartupLog $Path),'(?im)^.*Horizon:\s*(?:no sound:|no sounds in |audio stream error:|audio device was lost|could not reopen the audio output:)[^\r\n]*')
    if ($failure.Success) {
        $report.audio_backend_error=Protect-Text $failure.Value.Trim()
        Fail ('The client has no healthy audio backend: '+$report.audio_backend_error+'. This cannot pass as sound-playback acceptance.')
    }
}
function Invoke-Ctl([string[]] $Words) {
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$ctlPath;$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach ($word in (@('--socket',$socketPath)+$Words)) { $info.ArgumentList.Add([string]$word) }
    $process=[Diagnostics.Process]::Start($info)
    $outTask=$process.StandardOutput.ReadToEndAsync();$errTask=$process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(150000)) { $process.Kill();throw ('Control timeout: '+($Words -join ' ')) }
    $stdout=$outTask.GetAwaiter().GetResult();$stderr=$errTask.GetAwaiter().GetResult()
    [IO.File]::AppendAllText($transcript,(Protect-Text (@{words=$Words;exit_code=$process.ExitCode;stdout=$stdout;stderr=$stderr}|ConvertTo-Json -Compress -Depth 20))+[Environment]::NewLine,$utf8)
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
    $null=Wait-Status { param($s) (Num $s 'reloads') -gt $before } 45 ('the game to re-read '+$files[$Which])
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

# A generic hook counter also includes updates of old loops. Establish a
# different last-started event before each bell, then observe the bell itself.
function Observe-Cue([string] $Identifier) {
    foreach ($attempt in 1..3) {
        $before=Num (Read-Status) 'hook_seen'
        $reply=Command ('playsound '+$Identifier+' master @s ~ ~ ~ 1 1')
        if (($reply.chat -join ' ') -match 'Unknown|Incorrect|Expected|No player|No sound') {
            Fail ('The controlled sound command was refused: '+$Identifier)
        }
        $deadline=[DateTime]::UtcNow.AddSeconds(15)
        do {
            $status=Read-Status
            if ($null -ne $status -and (Num $status 'hook_seen') -gt $before -and $status['last_sound'] -eq $Identifier) {
                return $status
            }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
    }
    Fail ('The actual start callback was not observed for '+$Identifier+' after three bounded cue attempts.')
}
function Observe-Bell {
    $markerStatus=Observe-Cue 'minecraft:block.note_block.hat'
    if ($markerStatus['last_sound'] -ne 'minecraft:block.note_block.hat') { Fail 'The distinct bell marker was not established.' }
    return (Observe-Cue 'minecraft:block.note_block.bell')
}
# This file is deliberately separate from the older config-only ClientS suite.
# It does not call a successful submission counter proof of actual playback.
$saved=@{}
$optionValues=@{}
$nl=[Environment]::NewLine
try {
    Save-Report
    Ready-Screen
    if ((Invoke-Ctl @('state')).world -ne $World) { Fail 'The current world does not match this disposable fixture.' }
    $startupLog=Join-Path (Split-Path -Parent $socketPath) 'stderr.log'
    Assert-AudioBackend $startupLog
    $null=Wait-Status {param($s) (Num $s 'ticks') -gt 0} 90 'a live client status writer'
    foreach ($file in $files.Values) {
        $path=Join-Path $configDir $file
        $saved[$file]=if(Test-Path -LiteralPath $path){[IO.File]::ReadAllBytes($path)}else{$null}
    }
    foreach ($pair in @(@('soundCategory_master','1'),@('soundCategory_block','1'),@('soundCategory_player','1'),@('soundCategory_music','0'))) {
        $key=$pair[0]
        $before=Invoke-Ctl @('options-get',$key)
        if (-not $before.ContainsKey('options') -or -not $before.options.ContainsKey($key)) { Fail ('The required audio option is missing: '+$key) }
        $optionValues[$key]=[string]$before.options[$key].value
        $null=Invoke-Ctl @('options-set',$key,$pair[1])
        $actual=(Invoke-Ctl @('options-get',$key)).options[$key].value
        if ([double]$actual -ne [double]$pair[1]) { Fail ('The required audio option was not applied: '+$key) }
    }
    Write-Config 'ambient' ('@enabled false'+$nl)
    Write-Config 'footsteps' ('# empty for controlled playback'+$nl)
    Write-Config 'muffler' (('@enabled true','@quiet 30' -join $nl)+$nl)
    $status=Read-Status
    if ($status['quiet'] -ne 'false') { Fail 'Quiet mode is enabled, so the untouched-sound baseline is not established.' }
    $hookBefore=Num $status 'hook_seen'
    $status=Observe-Bell
    $report.playback_hook_verified=$true
    $report.hook_probe=@{before=$hookBefore;after=(Num $status 'hook_seen');last_sound=$status['last_sound']}
    Passed 'The initialized audio engine delivered an actual vanilla playback event to the Homestead callback. Zero hook calls cannot pass.'

    Write-Config 'muffler' (('@enabled true','@quiet 30','minecraft:entity.experience_orb.pickup 0','minecraft:entity.player.levelup 25' -join $nl)+$nl)
    $status=Wait-Status {param($s) (Num $s 'muffler_rules') -eq 2} 45 'the two controlled muffler rules'
    $baseCancel=Num $status 'muffler_cancelled'
    foreach ($i in 1..3) { $null=Command 'playsound minecraft:entity.experience_orb.pickup master @s ~ ~ ~ 1 1' }
    $status=Wait-Status {param($s) (Num $s 'muffler_cancelled') -ge ($baseCancel+3)} 45 'three actual cancelled start callbacks'
    $report.cancelled=@{before=$baseCancel;after=(Num $status 'muffler_cancelled')}
    Passed 'Three commanded pickup sounds reached the callback and were counted as cancelled by the zero-percent rule.'

    $baseScale=Num $status 'muffler_scaled'
    $null=Command 'playsound minecraft:entity.player.levelup master @s ~ ~ ~ 1 1'
    $status=Wait-Status {param($s) (Num $s 'muffler_scaled') -gt $baseScale} 45 'an actual callback selecting the configured 25-percent rule'
    $report.scaled=@{before=$baseScale;after=(Num $status 'muffler_scaled');configured_percent=25}
    Passed 'An actual playback callback selected the 25-percent rule. Numeric output amplitude/pitch and physical audio output are not measured.'

    $baseHook=Num $status 'hook_seen';$baseCancel=Num $status 'muffler_cancelled';$baseScale=Num $status 'muffler_scaled'
    $status=Observe-Bell
    if ((Num $status 'muffler_cancelled') -ne $baseCancel -or (Num $status 'muffler_scaled') -ne $baseScale) { Fail 'An unruled sound changed the muffler decision counters.' }
    Passed 'An unruled sound traverses the callback while cancellation and scaling counters stay unchanged.'

    Write-Config 'muffler' ('@enabled true'+$nl)
    $status=Read-Status;$baseHook=Num $status 'hook_seen';$basePlay=Num $status 'ambient_played';$baseFailure=Num $status 'play_failed'
    Write-Config 'ambient' (('@enabled true','playbackcheck overworld homestead_sound:tone.high 20 1.0 0.5' -join $nl)+$nl)
    $status=Wait-Status {param($s) (Num $s 'ambient_played') -ge ($basePlay+2) -and (Num $s 'hook_seen') -ge ($baseHook+2) -and $s['last_sound'] -eq 'homestead_sound:tone.high'} 45 'the submitted ambience event appearing by identifier in the actual callback'
    if ((Num $status 'play_failed') -ne $baseFailure) { Fail 'An ambience event could not be submitted.' }
    $report.ambient=@{ambient_played=(Num $status 'ambient_played');hook_seen=(Num $status 'hook_seen');last_sound=$status['last_sound']}
    Passed 'The ambience event itself is visible in the playback callback, rather than only in a successful-submission counter.'
    Assert-AudioBackend $startupLog
    $report.notes+=@('No physical audio output was listened to or captured.','The configured volume percentage is verified through the decision counter, not a numeric post-hook parameter trace.','Actual walking-generated footsteps and F9/F10 keyboard toggles are separate checks and are not claimed here.')
    $report.passed=$true
} catch {
    $report.error=Protect-Text $_.Exception.Message
    throw
} finally {
    foreach ($file in $saved.Keys) {
        $path=Join-Path $configDir $file
        if ($null -eq $saved[$file]) { Remove-Item -LiteralPath $path -ErrorAction SilentlyContinue }
        else { [IO.File]::WriteAllBytes($path,$saved[$file]) }
    }
    foreach ($key in $optionValues.Keys) { try { $null=Invoke-Ctl @('options-set',$key,$optionValues[$key]) } catch { } }
    Save-Report
    Write-Output ('Sound playback receipt: '+[IO.Path]::GetFileName($reportPath))
}
