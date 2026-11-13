#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Crops Love Rain: wheat and sugar cane grow faster under rain than in clear weather.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Rain')][string] $Phase='Rain',
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
$transcript=Join-Path $game ('stage6-rain-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage6-rain-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Crops Love Rain (general speed 3): wheat and sugar cane random ticks gain extra growth steps in rain. Compared against clear weather on the same moist plots.'
    coordinates=@{wheat=@(146,84,44,160,84,50);cane=@(146,84,52,160,84,58)}
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
$null=Command 'gamerule random_tick_speed 0'
$null=Command 'gamerule advance_time false'
$null=Command 'time set 6000'
$null=Command 'forceload add 144 40 159 60'
$null=Command 'tp @s 153.5 100 51.5'
Wait-Ticks 40
$null=Command 'fill 146 83 43 160 120 59 minecraft:air'
$null=Command 'fill 146 82 43 160 82 59 minecraft:dirt'
# Wheat: farmland on both sides of a water row, so every plot is moist in both weathers.
$null=Command 'fill 146 83 44 160 83 50 minecraft:farmland'
$null=Command 'fill 146 83 47 160 83 47 minecraft:water'
# Sugar cane: sand rows either side of a water row, twice.
foreach ($sand in 52,54,56,58) { $null=Command ('fill 146 83 '+$sand+' 160 83 '+$sand+' minecraft:sand') }
foreach ($water in 53,57) { $null=Command ('fill 146 83 '+$water+' 160 83 '+$water+' minecraft:water') }
Wait-Ticks 10

$wheatRows=44,45,46,48,49,50
$caneRows=52,54,56,58
function Reset-Plots {
    foreach ($z in $wheatRows) { $null=Command ('fill 146 84 '+$z+' 160 84 '+$z+' minecraft:wheat[age=0]') }
    foreach ($z in $caneRows) { $null=Command ('fill 146 84 '+$z+' 160 84 '+$z+' minecraft:sugar_cane[age=0]'); $null=Command ('fill 146 85 '+$z+' 160 85 '+$z+' minecraft:air') }
}
function Sum-Ages {
    $wheat=0;$cane=0
    foreach ($z in $wheatRows) { foreach ($x in 146..160) { $wheat+=Age $x 84 $z } }
    foreach ($z in $caneRows) { foreach ($x in 146..160) {
        $cane+=Age $x 84 $z
        if ((Block $x 85 $z) -eq 'minecraft:sugar_cane') { $cane+=16 }
    } }
    return @{wheat=$wheat;cane=$cane}
}
function Trial([string] $Weather) {
    $null=Command ('weather '+$Weather+' 20000')
    Wait-Ticks 160
    Reset-Plots
    Wait-Ticks 4
    $null=Command 'gamerule random_tick_speed 100'
    Wait-Ticks 200
    $null=Command 'gamerule random_tick_speed 0'
    Wait-Ticks 4
    return Sum-Ages
}

$clear=Trial 'clear'
$rain=Trial 'rain'
$null=Command 'weather clear'
$report.measured=@{clear=$clear;rain=$rain;plants=@{wheat=90;cane=60}}
Save-Report
Expect ($clear.wheat -gt 0) ('Clear-weather wheat must still grow by itself: '+($clear|ConvertTo-Json -Compress))
Expect ($rain.wheat -ge ($clear.wheat*1.4)) ('Rain must speed up wheat: '+($report.measured|ConvertTo-Json -Compress -Depth 5))
Passed ('Wheat ages: clear '+$clear.wheat+', rain '+$rain.wheat+'.')
Expect ($rain.cane -ge ($clear.cane*1.12)) ('Rain must speed up sugar cane: '+($report.measured|ConvertTo-Json -Compress -Depth 5))
Passed ('Sugar cane growth: clear '+$clear.cane+', rain '+$rain.cane+'.')
$null=Command 'gamerule advance_time true'
$null=Command 'gamerule random_tick_speed 3'
$report.passed=$true
Save-Report
Write-Output 'Crops Love Rain acceptance passed.'