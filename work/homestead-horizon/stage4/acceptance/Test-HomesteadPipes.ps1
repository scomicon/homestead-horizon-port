#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Pipes')][string] $Phase='Pipes',
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
$transcript=Join-Path $game ('stage14-pipes-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage14-pipes-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Fluid pipe network: a Mechanical Pump moves water from a tank through a straight pipe into another tank, and through a tee into two tanks. Each scenario is reported separately.'
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
$results=[ordered]@{}
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Clear-Area { $null=Command 'fill 92 77 92 112 95 112 minecraft:air'; $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'; Wait-Ticks 30 }
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function Amount([int] $X,[int] $Y,[int] $Z) {
    $m=[regex]::Match((Get-Data $X $Y $Z),'amount:\s*(\d+)')
    if ($m.Success) { return [int]$m.Groups[1].Value } else { return 0 }
}
function Fill-Tank([int] $X,[int] $Y,[int] $Z,[int] $Buckets) {
    foreach ($i in 1..$Buckets) {
        $null=Command ('tp @s '+($X-2.5)+' '+$Y+' '+($Z+0.5))
        Wait-Ticks 6
        $null=Command 'item replace entity @s weapon.mainhand with minecraft:water_bucket 1'
        $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,'west')
        $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,'west')
        Wait-Ticks 8
    }
}
function Power-Pump {
    # Motor at the source tank's top shaft into a cogwheel that meshes with the pump (axis x).
    $null=Command 'setblock 100 81 100 create:creative_motor[facing=east]{ScrollValue:64}'
    $null=Command 'setblock 101 81 100 create:cogwheel[axis=x]'
}
Scenario 'straight line' {
    Clear-Area
    $null=Command 'setblock 100 80 100 create:fluid_tank'
    $null=Command 'setblock 104 80 100 create:fluid_tank'
    Wait-Ticks 15
    Fill-Tank 100 80 100 4
    $before=Amount 100 80 100
    if ($before -ne 4000) { throw ('source tank holds '+$before+' mB, expected 4000') }
    $null=Command 'setblock 101 80 100 create:mechanical_pump[facing=east]'
    $null=Command 'setblock 102 80 100 create:fluid_pipe[east=true,west=true]'
    $null=Command 'setblock 103 80 100 create:fluid_pipe[east=true,west=true]'
    Power-Pump
    $moved=0
    for ($t=0;$t -lt 600;$t+=20) { Wait-Ticks 20; $moved=Amount 104 80 100; if ($moved -gt 0) { break } }
    if ($moved -le 0) { throw ('nothing arrived; source '+(Amount 100 80 100)+'; pump '+(Get-Data 101 80 100)+'; pipe '+(State 102 80 100)+' '+(Get-Data 102 80 100)) }
    Wait-Ticks 200
    $source=Amount 100 80 100; $target=Amount 104 80 100
    if (($source+$target) -ne 4000) { throw ('fluid was lost or created: '+$source+' + '+$target) }
}
Scenario 'tee into two tanks' {
    Clear-Area
    $null=Command 'setblock 100 80 100 create:fluid_tank'
    $null=Command 'setblock 103 80 99 create:fluid_tank'
    $null=Command 'setblock 103 80 101 create:fluid_tank'
    Wait-Ticks 15
    Fill-Tank 100 80 100 4
    $null=Command 'setblock 101 80 100 create:mechanical_pump[facing=east]'
    $null=Command 'setblock 102 80 100 create:fluid_pipe[east=true,west=true]'
    $null=Command 'setblock 103 80 100 create:fluid_pipe[west=true,north=true,south=true]'
    Power-Pump
    $a=0;$b=0
    for ($t=0;$t -lt 800;$t+=20) { Wait-Ticks 20; $a=Amount 103 80 99; $b=Amount 103 80 101; if ($a -gt 0 -and $b -gt 0) { break } }
    if ($a -le 0 -or $b -le 0) { throw ('branches did not both fill: '+$a+' / '+$b+'; tee '+(State 103 80 100)) }
    Wait-Ticks 300
    $a=Amount 103 80 99; $b=Amount 103 80 101; $s=Amount 100 80 100
    if (($a+$b+$s) -ne 4000) { throw ('fluid was lost or created: '+$s+' + '+$a+' + '+$b) }
}
Scenario 'pump stops with the motor' {
    $null=Command 'setblock 101 81 100 minecraft:air'
    Wait-Ticks 80
    $a=Amount 103 80 99; $b=Amount 103 80 101
    Wait-Ticks 100
    if ((Amount 103 80 99) -ne $a -or (Amount 103 80 101) -ne $b) { throw 'fluid kept moving without power' }
}
Scenario 'open end places a water source' {
    Clear-Area
    $null=Command 'setblock 100 80 100 create:fluid_tank'
    Wait-Ticks 15
    Fill-Tank 100 80 100 3
    $null=Command 'setblock 101 80 100 create:mechanical_pump[facing=east]'
    $null=Command 'setblock 102 80 100 create:fluid_pipe[east=true,west=true]'
    Power-Pump
    $placed=$false
    for ($t=0;$t -lt 900;$t+=20) { Wait-Ticks 20; if ((Block 103 80 100) -eq 'minecraft:water') { $placed=$true;break } }
    if (-not $placed) { throw ('no water appeared at the open end: '+(Block 103 80 100)+'; tank '+(Amount 100 80 100)) }
    if ((Amount 100 80 100) -ge 3000) { throw 'the tank was not drained by the spill' }
}
Scenario 'glass and smart pipes place' {
    Clear-Area
    foreach ($name in 'glass_fluid_pipe','smart_fluid_pipe','encased_fluid_pipe') {
        $null=Command ('setblock 100 80 100 create:'+$name)
        Wait-Ticks 6
        if ((Block 100 80 100) -ne ('create:'+$name)) { throw ($name+' was not placed: '+(Block 100 80 100)) }
    }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)