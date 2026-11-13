#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Creative Motor speed setting through the real value-board UI (hold use, pick a value, release), checked on ScrollValue and shaft speed.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Motor')][string] $Phase='Motor',
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
$transcript=Join-Path $game ('stage31-motor-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage31-motor-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='A Creative Motor is configured through the value board: hold use on the motor, move the cursor onto a value, release; ScrollValue and the driven shaft speed must follow.'
    coordinates=@{origin=@(100,81,100)}
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
function Touch([int] $X,[int] $Y,[int] $Z) {
    foreach ($face in 'south','up','north','west','east') {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$face);return } catch { }
    }
    throw ('No face of the block at '+$X+' '+$Y+' '+$Z+' can be targeted.')
}
function Rows([int] $X,[int] $Y,[int] $Z) { return (Get-Data $X $Y $Z 'Rows') }

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 103.5 80 100.5'
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
Wait-Ticks 40
$null=Command 'fill 92 77 92 115 90 112 minecraft:air'
$null=Command 'fill 92 79 92 115 79 112 minecraft:smooth_stone'
Wait-Ticks 20

# Motor facing east with a shaft behind its output, so the speed is observable on the shaft.
$null=Command 'setblock 100 80 100 create:creative_motor[facing=east]{ScrollValue:16}'
$null=Command 'setblock 101 80 100 create:shaft[axis=x]'
Wait-Ticks 20
$sv=Get-Data 100 80 100 'ScrollValue'
Expect ($sv -match '16') ('The motor did not start at 16 rpm: '+$sv)
Passed 'Motor placed with ScrollValue 16.'

$null=Command 'tp @s 103.5 80 100.5'
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
Wait-Ticks 6
$explore=[ordered]@{}
foreach ($face in 'east','up','north','south') {
    try {
        $null=Invoke-Ctl @('look-at','block','100','80','100',$face)
        $null=Invoke-Ctl @('wait','4')
        try { $null=Invoke-Ctl @('screenshot',(Join-Path $game ('motor-before-'+$face+'.png'))) } catch { }
        $null=Invoke-Ctl @('mouse','right','down')
        $null=Invoke-Ctl @('wait','12')
        $scr=Invoke-Ctl @('screen')
        for ($k=0;$k -lt 20 -and [string]$scr.screen -eq 'playing';$k++) { $null=Invoke-Ctl @('wait','2');$scr=Invoke-Ctl @('screen') }
        try { $null=Invoke-Ctl @('screenshot',(Join-Path $game ('motor-'+$face+'.png'))) } catch { $scr=@{screen=$scr.screen;shot_error=$_.Exception.Message} }
        $explore[$face]=$scr
        $report.explore=$explore;Save-Report
        if ([string]$scr.screen -ne 'playing') { $report.face=$face;break }
        $null=Invoke-Ctl @('mouse','right','up')
        $null=Invoke-Ctl @('wait','4')
    } catch { $explore[$face]='error: '+$_.Exception.Message;$report.explore=$explore;Save-Report }
}
Expect ($report.Contains('face')) ('The value board never opened: '+($explore|ConvertTo-Json -Compress -Depth 6))
Passed ('The value board opened when aiming at the '+$report.face+' face.')
function Num([string] $Text) { $m=[regex]::Match($Text,'block data:\s*(-?\d+(\.\d+)?)');if ($m.Success) { return [double]$m.Groups[1].Value };return [double]::NaN }
function Setting { return [int](Num (Get-Data 100 80 100 'ScrollValue')) }
function Speed { return (Num (Get-Data 100 80 100 'Speed')) }
# The panel is open (hold use ≥5 ticks). Release the right button over a cell of the top row: that confirms it.
$null=Invoke-Ctl @('drag-at','right','200','113','267','113')
Wait-Ticks 12
try { $null=Invoke-Ctl @('screenshot',(Join-Path $game 'motor-after-drag.png')) } catch { }
$report.screen_after=(Invoke-Ctl @('screen')).screen;Save-Report
$logText=((& $ctlPath --socket $socketPath logs --lines 400) -join [Environment]::NewLine)
$report.value_log=@([regex]::Matches($logText,'Create value[^\r\n]*')|ForEach-Object { $_.Value });Save-Report
$after=Setting
$report.after_top=$after;Save-Report
Expect ($after -ne 16) ('Releasing over the top row did not change ScrollValue (still '+$after+').')
Passed ('The value board confirmed a setting: ScrollValue 16 -> '+$after+'.')
Wait-Ticks 20
$speed=Speed
$report.speed=$speed;Save-Report
Expect ([math]::Abs($speed) -eq [math]::Abs($after)) ('The motor Speed '+$speed+' does not follow ScrollValue '+$after+'.')
Passed ('The motor speed follows the setting ('+$speed+' rpm).')
$shaft=Num (Get-Data 101 80 100 'Speed')
$report.shaft_speed=$shaft;Save-Report
Expect ([math]::Abs([double]$shaft) -eq [math]::Abs($after)) ('The shaft beside the motor runs at '+$shaft+', not '+$after+'.')
Passed ('The shaft follows the motor ('+$shaft+' rpm).')
# Second pass: the bottom row sets the opposite direction.
$null=Invoke-Ctl @('look-at','block','100','80','100','up')
Wait-Ticks 4
$null=Invoke-Ctl @('mouse','right','down')
Wait-Ticks 14
for ($k=0;$k -lt 20 -and [string](Invoke-Ctl @('screen')).screen -eq 'playing';$k++) { Wait-Ticks 2 }
try { $null=Invoke-Ctl @('screenshot',(Join-Path $game 'motor-second-panel.png')) } catch { }
$null=Invoke-Ctl @('drag-at','right','200','125','267','125')
Wait-Ticks 12
$second=Setting
Expect ($second -ne $after) ('The second setting did not change ScrollValue (still '+$second+').')
Expect ([math]::Sign($second) -ne [math]::Sign($after)) ('The bottom row must flip the direction: '+$after+' -> '+$second)
Passed ('The bottom row flipped the direction: '+$after+' -> '+$second+'.')
$report.passed=$true
Save-Report
Write-Output 'Motor acceptance passed.'