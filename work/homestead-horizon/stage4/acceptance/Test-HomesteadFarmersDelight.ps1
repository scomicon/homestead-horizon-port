#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Farmer's Delight: carried content with item components (food values, bowl remainders) and the Nourishment effect.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Items')][string] $Phase='Items',
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
$transcript=Join-Path $game ('stage8-fd-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage8-fd-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Farmers Delight content mod (carried blocks and items) with FoodValues components: a bowl food restores its nutrition, leaves a bowl and gives Nourishment; Nourishment stops hunger drain.'
    coordinates=@{}
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

function Food { return [int](Invoke-Ctl @('state')).player.food }
function Count([string] $Item) { $c=(Invoke-Ctl @('inventory')).counts; if ($c.ContainsKey($Item)) { return [int]$c[$Item] } else { return 0 } }
function Eat([string] $Item,[double] $Seconds) {
    $null=Invoke-Ctl @('select',$Item)
    $null=Invoke-Ctl @('use-item','--hold',[string]$Seconds)
    Wait-Ticks 10
}
$null=Command 'tick unfreeze';$null=Command 'gamemode survival';$null=Command 'effect clear @s'
$null=Command 'clear @s'
Wait-Ticks 5
# Control: hunger drains without Nourishment.
$full=Food
# Burn the saved saturation first, then food: a world kept between runs may start saturated.
$rounds=0
while ((Food) -gt 12 -and $rounds -lt 10) {
    $null=Command 'effect give @s minecraft:hunger 5 100'
    Wait-Ticks 110
    $rounds++
}
$null=Command 'effect clear @s minecraft:hunger'
$drained=Food
Expect ($drained -le 12) ('Hunger must drain without Nourishment: '+$full+' -> '+$drained)Passed ('Control: food '+$full+' -> '+$drained+' under Hunger.')
# A bowl food: nutrition 6, a bowl back, Nourishment given.
$null=Command 'give @s farmersdelight:cooked_rice 2'
$before=Food
Eat 'farmersdelight:cooked_rice' 1.9
$after=Food
Expect ($after -eq [math]::Min(20,$before+6)) ('Cooked rice must restore 6 food: '+$before+' -> '+$after)
Expect ((Count 'farmersdelight:cooked_rice') -eq 1) 'One cooked rice must be eaten.'
Expect ((Count 'minecraft:bowl') -eq 1) 'The bowl must come back.'
Passed ('Cooked rice: food '+$before+' -> '+$after+', bowl returned.')
# A fast food: bacon eats in 0.8 s (a short hold is enough).
$null=Command 'give @s farmersdelight:cooked_bacon 1'
$b2=Food
Eat 'farmersdelight:cooked_bacon' 1.0
Expect ((Count 'farmersdelight:cooked_bacon') -eq 0) 'Cooked bacon must be eaten in a 1.0 s hold.'
Expect ((Food) -ge [math]::Min(20,$b2+4)) ('Cooked bacon must restore 4 food: '+$b2+' -> '+(Food))
Passed 'Cooked bacon eaten at the fast speed.'
$effect=((Command 'effect clear @s farmersdelight:nourishment').chat -join ' ')
Expect ($effect -notmatch "doesn't have") ('Cooked rice must give Nourishment: '+$effect)
Passed 'Cooked rice gave Nourishment.'
# Nourishment: the same hunger no longer drains food.
$null=Command 'effect give @s farmersdelight:nourishment 60'
Wait-Ticks 10
$full2=Food
$null=Command 'effect give @s minecraft:hunger 4 100'
Wait-Ticks 100
$null=Command 'effect clear @s minecraft:hunger'
$held=Food
Expect ($held -ge $full2-1) ('Nourishment must stop hunger drain: '+$full2+' -> '+$held)
Passed ('Nourishment: food '+$full2+' -> '+$held+' under the same Hunger.')
$null=Command 'effect clear @s'
$report.passed=$true
Save-Report
Write-Output 'Farmers Delight items acceptance passed.'