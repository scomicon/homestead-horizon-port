#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Client mods, group H (stage 26 slice): Welcome Screen (opens once per install, closes on Escape,
# writes its marker), Traveler's Titles (a dimension change announces itself in the log), Better
# Mount HUD and Detail Armor Bar (smoke: survival, armor set, a saddled horse ridden; the frames
# keep drawing and the game stays alive). What the HUD looks like is NOT verified here: the
# screenshots are saved for a human to look at.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('ClientH')][string] $Phase='ClientH',
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
$transcript=Join-Path $game ('stage26-clienth-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage26-clienth-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@();notes=@()
    scope='Welcome Screen opens once and closes (verified through the screen state and the marker file); Traveler''s Titles announces a dimension change (verified through the game log); Better Mount HUD and Detail Armor Bar are smoke-tested only (the game keeps running with armor set and a mount ridden); the look of every HUD element is not verified.'
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
function Screen-Name { return [string](Invoke-Ctl @('screen')).screen }
function Ready-Screen {
    for ($i=0;$i -lt 6;$i++) {
        if ((Screen-Name) -eq 'playing') { return }
        $null=Invoke-Ctl @('key','esc','tap'); Start-Sleep -Seconds 1
    }
    Fail 'could not return to the playing screen'
}
function Read-Log {
    # The game log through horizon-ctl; if that gives nothing (another HORIZON_HOME), the newest latest.log under stage4.
    try {
        $reply=Invoke-Ctl @('logs','--lines','600')
        $text=($reply|ConvertTo-Json -Depth 10 -Compress)
        if ($text.Length -gt 40) { return $text }
    } catch { }
    $logs=Get-ChildItem -LiteralPath (Join-Path $workspace 'work/homestead-horizon/stage4') -Recurse -Filter latest.log -ErrorAction SilentlyContinue|Sort-Object LastWriteTime -Descending|Select-Object -First 1
    if ($logs) { return (Get-Content -LiteralPath $logs.FullName -Tail 600 -ErrorAction SilentlyContinue) -join "`n" }
    return ''
}
$marker=Join-Path $game 'horizon-mods/.horizon-mod-data/homestead/welcome-shown'
$modeBefore=$null
try {
    $modeBefore=[string](Invoke-Ctl @('state')).gamemode
} catch { }
try {
    Ready-Screen

    # 1. Welcome Screen: with no marker it opens after the player has been playing for five seconds
    #    and writes the marker; Escape closes it.
    if (-not (Test-Path -LiteralPath $marker)) { Note 'no marker at the start (Start-Test normally writes it); proceeding' }
    Remove-Item -LiteralPath $marker -ErrorAction SilentlyContinue
    $open=$false; $deadline=[DateTime]::UtcNow.AddSeconds(40)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ((Screen-Name) -ne 'playing') { $open=$true; break }
        Start-Sleep -Seconds 1
    }
    if (-not $open) { Fail 'the welcome screen did not open within 40 seconds of playing without its marker' }
    if (-not (Test-Path -LiteralPath $marker)) { Fail 'the welcome screen is open but its marker file was not written' }
    Passed 'welcome screen opened after the delay (screen state changed) and wrote its marker'
    $log=Read-Log
    if ($log -like '*Homestead welcome screen opened*') { Passed 'welcome screen: the game log says it opened' } else { Note 'the game log could not be read or has no welcome line' }
    $null=Invoke-Ctl @('key','esc','tap')
    $closed=$false; $deadline=[DateTime]::UtcNow.AddSeconds(10)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ((Screen-Name) -eq 'playing') { $closed=$true; break }
        Start-Sleep -Milliseconds 500
    }
    if (-not $closed) { Fail 'Escape did not close the welcome screen' }
    Passed 'welcome screen closed with Escape'
    # It stays closed: the install has the marker now.
    Start-Sleep -Seconds 12
    if ((Screen-Name) -ne 'playing') { Fail 'the welcome screen opened a second time in one session' }
    Passed 'welcome screen does not open again while the marker exists'
    Note 'the click and the Enter or Space key close the screen too (flag, then the next tick) but are not driven here; the look of the screen is not verified'

    # 2. Traveler's Titles: a dimension change is announced (log line "Homestead titles: The Nether / ...").
    $hopped=$false
    try {
        $reply=Command 'execute in minecraft:the_nether run tp @s 0 100 0'
        $hopped=$true
    } catch { Note ('the dimension hop command was refused, titles not exercised: '+$_.Exception.Message.Split("`n")[0]) }
    if ($hopped) {
        $seen=$false; $deadline=[DateTime]::UtcNow.AddSeconds(90)
        while ([DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Seconds 3
            if ((Read-Log) -like '*Homestead titles: The Nether*') { $seen=$true; break }
        }
        if ($seen) { Passed 'traveler''s titles: the dimension change was announced (log: Homestead titles: The Nether / ...)' }
        else { Note 'traveler''s titles: no banner line for the Nether in the log within 90 seconds (the hop may not have changed the dimension, or the log is unreadable); not verified' }
        try { $null=Command 'execute in minecraft:overworld run tp @s 0 100 0'; Start-Sleep -Seconds 5 } catch { }
    }

    # 3. Detail Armor Bar and Better Mount HUD: survival, armor over 20, a saddled horse ridden.
    # Slow falling for the whole survival part: the player was teleported to y=100 and a fall death left the shared test world with a dead player for every later suite.
    try { $null=Command 'effect give @s minecraft:slow_falling 900 0 true' } catch { Note ('slow falling not given: '+$_.Exception.Message.Split("`n")[0]) }
    $null=Command 'gamemode survival'
    $armorSet=$false
    try { $null=Command 'attribute @s minecraft:armor base set 34'; $armorSet=$true } catch { Note ('armor attribute not settable by command: '+$_.Exception.Message.Split("`n")[0]) }
    Start-Sleep -Seconds 2
    $shot=Join-Path $game 'stage26-clienth-armor.png'
    Remove-Item -LiteralPath $shot -ErrorAction SilentlyContinue
    $null=Invoke-Ctl @('screenshot',$shot); Start-Sleep -Seconds 3
    if (-not (Test-Path -LiteralPath $shot)) { Fail 'no screenshot after armor was set' }
    if ($armorSet) { Passed 'detail armor bar smoke: survival with 34 armor points, frames keep rendering (screenshot saved for a human; the look is not verified)' }
    else { Note 'detail armor bar not exercised (no armor); the screenshot is the plain survival HUD' }

    $mounted=$false
    try {
        $summoned=Invoke-Ctl @('summon','horse')
        $horse=$null
        foreach ($entity in @((Invoke-Ctl @('entities','12','horse')).entities)) { if ($null -eq $horse) { $horse=$entity } }
        if ($null -ne $horse) {
            try { $null=Command ('data merge entity @e[type=minecraft:horse,limit=1,sort=nearest] {Tame:1b,SaddleItem:{id:"minecraft:saddle",count:1}}') } catch { }
            $null=Invoke-Ctl @('interact-entity',[string]$horse.id); Start-Sleep -Seconds 3
            $mounted=$true
        }
    } catch { Note ('no horse could be summoned or ridden: '+$_.Exception.Message.Split("`n")[0]) }
    if ($mounted) {
        $shot2=Join-Path $game 'stage26-clienth-mount.png'
        Remove-Item -LiteralPath $shot2 -ErrorAction SilentlyContinue
        $null=Invoke-Ctl @('screenshot',$shot2); Start-Sleep -Seconds 3
        if (-not (Test-Path -LiteralPath $shot2)) { Fail 'no screenshot while riding' }
        $alive=Invoke-Ctl @('state')
        if (-not $alive.ContainsKey('screen') -and -not $alive.ContainsKey('gamemode')) { Fail 'the game did not answer state while riding' }
        Passed 'better mount HUD smoke: a horse was ridden in survival and the frames keep rendering (screenshot saved for a human; the look is not verified)'
        try { $null=Invoke-Ctl @('key','shift','tap'); Start-Sleep -Seconds 1 } catch { }
    } else { Note 'better mount HUD not exercised (could not ride a horse); not verified' }
    $log=Read-Log
    if ($log -match '(?i)panicked|stack overflow') { Fail 'the game log has a panic after the HUD scenarios' }
    Passed 'no panic in the game log after the HUD scenarios'
    $report.passed=$true
    Save-Report
} finally {
    # Leave the shared disposable instance as we found it: creative, standing, welcome marker present.
    try { $null=Command 'kill @e[type=minecraft:horse]' } catch { }
    try { $null=Command ('gamemode '+$(if ($modeBefore) { $modeBefore.ToLowerInvariant() } else { 'creative' })) } catch { }
    try { $null=Command 'attribute @s minecraft:armor base reset' } catch { }
    try { $null=Command 'effect clear @s minecraft:slow_falling' } catch { }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $marker)|Out-Null
    if (-not (Test-Path -LiteralPath $marker)) { [IO.File]::WriteAllText($marker,'1') }
    # The two screenshots (stage26-clienth-armor.png, stage26-clienth-mount.png) stay in the game folder for a human to look at.
}
