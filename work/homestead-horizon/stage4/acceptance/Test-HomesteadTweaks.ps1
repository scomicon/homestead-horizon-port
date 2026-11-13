#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Ordinary Chest/Barrel stock, real connector-built Belts and ordinary Funnel placement.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Tweaks')][string] $Phase='Tweaks',
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
$transcript=Join-Path $game ('stage6-tweaks-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage6-tweaks-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Secure Crops (no trampling), Too Cheap (anvil results above 39 levels) and the hunger rules of Right Click Harvest. New-world rule defaults are not exercised here.'
    coordinates=@{farm=@(150,84,36);anvil=@(158,84,36)}
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

$null=Command 'tick unfreeze';$null=Command 'gamemode survival'
$null=Command 'gamerule random_tick_speed 0'
$null=Command 'forceload add 144 32 159 47'
Wait-Ticks 40
$null=Command 'fill 146 83 33 160 100 39 minecraft:air'
$null=Command 'fill 146 82 33 160 82 39 minecraft:dirt'
$null=Command 'fill 146 83 33 154 83 39 minecraft:farmland'
$null=Command 'effect clear @s'
$null=Command 'kill @e[type=minecraft:item]'
Wait-Ticks 2

# 1. Secure Crops: a player landing on a crop leaves farmland and crop in place.
$null=Command 'setblock 150 84 36 minecraft:wheat[age=3]'
$null=Command 'tp @s 150.5 90 36.5'
Wait-Ticks 80
Expect ((Block 150 83 36) -eq 'minecraft:farmland') 'Farmland under a landed player must stay farmland.'
Expect ((Block 150 84 36) -eq 'minecraft:wheat') 'The crop under a landed player must stay.'
$null=Command 'setblock 152 83 36 minecraft:farmland'
$null=Command 'tp @s 152.5 90 36.5'
Wait-Ticks 80
Expect ((Block 152 83 36) -eq 'minecraft:farmland') 'Empty farmland under a landed player must stay farmland.'
$null=Command 'effect give @s minecraft:instant_health 1 10 true'
Passed 'Landing on a crop or on empty farmland tramples nothing.'

# 2. Right Click Harvest hunger: nothing is harvested at food level 0, and a harvest tires the player.
$null=Command 'fill 146 83 33 154 83 39 minecraft:farmland'
$null=Command 'setblock 150 84 36 minecraft:wheat[age=7]'
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
$null=Command 'effect give @s minecraft:hunger 600 120 true'
for ($i=0;$i -lt 60 -and (Food) -gt 0;$i++) { Wait-Ticks 40 }
Expect ((Food) -eq 0) 'The hunger effect must empty the food bar.'
Click 150 84 36 150.5 38.5
Expect ((Age 150 84 36) -eq 7) 'A starving player must not harvest.'
Passed 'At food level 0 a right click does not harvest.'
$null=Command 'effect clear @s'
$null=Command 'effect give @s minecraft:saturation 1 10 true'
Wait-Ticks 20
Expect ((Food) -gt 0) 'Saturation must feed the player.'
Click 150 84 36 150.5 38.5
Expect ((Age 150 84 36) -eq 0) 'A fed player must harvest.'
Passed 'A fed player harvests again.'

# 3. Too Cheap: two heavily repaired swords still combine on an anvil.
$null=Command 'setblock 158 83 36 minecraft:anvil'
$null=Command 'item replace entity @s hotbar.0 with minecraft:iron_sword[minecraft:repair_cost=63,minecraft:damage=100]'
$null=Command 'item replace entity @s hotbar.1 with minecraft:iron_sword[minecraft:repair_cost=63,minecraft:damage=100]'
$null=Command 'tp @s 158.5 85 38.5'
Wait-Ticks 2
$null=Invoke-Ctl @('look-at','block','158','83','36','up')
$null=Invoke-Ctl @('interact','158','83','36','up')
Wait-Ticks 10
$null=Invoke-Ctl @('click-slot','0')
$null=Invoke-Ctl @('click-slot','600')
$null=Invoke-Ctl @('click-slot','1')
$null=Invoke-Ctl @('click-slot','601')
Wait-Ticks 10
$menu=Invoke-Ctl @('menu')
$report.anvil_menu=$menu
$cost=[int]$menu.info.anvil_cost
Expect ($cost -ge 40) ("The fixture must cost 40 levels or more, got $cost.")
Expect ([string]$menu.info.result -match 'iron_sword') ("Two swords at $cost levels must still give an anvil result, got: "+[string]$menu.info.result)
$report.anvil_cost=$cost
Passed 'An anvil still offers a result above 39 levels.'

$report.passed=$true
Save-Report
Write-Output 'Homestead tweaks acceptance passed.'