#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Packager and Factory Gauge: redstone packing of a chest into a package, hand-over to a second Packager that unpacks it into another chest, ejection of a finished box, and the gauge's satisfied state following the stock of the inventory behind it.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Packager')][string] $Phase='Packager',
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
$transcript=Join-Path $game ('stage20-packager-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage20-packager-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Chest -> Packager (redstone) -> Packager -> chest, a lone Packager ejecting its box, and a Factory Gauge on a chest. Visuals, sounds and the package item tooltip are not compared.'
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
$null=Command 'tp @s 100 90 100'
Wait-Ticks 40
$null=Command 'fill 92 77 92 118 90 112 minecraft:air'
$null=Command 'fill 92 78 92 118 78 112 minecraft:smooth_stone'

# --- 1. chest A -> Packager P1 (east) -> Packager P2 (west) -> chest B
$null=Command 'setblock 100 79 100 minecraft:chest'
$null=Command 'setblock 103 79 100 minecraft:chest'
$null=Command 'setblock 101 79 100 create:packager[facing=east]'
$null=Command 'setblock 102 79 100 create:packager[facing=west]'
Wait-Ticks 5
Expect ((Block 101 79 100) -eq 'create:packager' -and (Block 102 79 100) -eq 'create:packager') 'The Packagers were not placed.'
$null=Command 'item replace block 100 79 100 container.0 with minecraft:cobblestone 8'
$null=Command 'item replace block 100 79 100 container.1 with minecraft:sand 4'
Wait-Ticks 20
Assert-Empty (Counts 103 79 100) 'Chest B before the signal'
Expect ((Counts 100 79 100).Count -eq 2) 'Chest A must still hold its items without a signal.'
Passed 'Without a redstone signal the Packager leaves the chest alone.'
$null=Command 'setblock 101 80 100 minecraft:redstone_block'
$seenBox=$false
for ($i=0;$i -lt 12 -and -not $seenBox;$i++) {
    Wait-Ticks 5
    if ((Get-Data 101 79 100 HeldBox) -match 'cardboard_package') { $seenBox=$true }
    elseif ((Counts 100 79 100).Count -eq 0) { $seenBox=$true }
}
Expect $seenBox 'The Packager made no package after the signal.'
Passed 'A rising redstone edge made the Packager pack the chest into a package.'
for ($i=0;$i -lt 30;$i++) {
    Wait-Ticks 10
    if ((Counts 103 79 100).Count -ge 2) { break }
}
$b=Counts 103 79 100
Expect ($b.ContainsKey('minecraft:cobblestone') -and [int]$b['minecraft:cobblestone'] -eq 8 -and $b.ContainsKey('minecraft:sand') -and [int]$b['minecraft:sand'] -eq 4 -and $b.Count -eq 2) ('Chest B must hold 8 cobblestone and 4 sand, got '+($b|ConvertTo-Json -Compress))
Assert-Empty (Counts 100 79 100) 'Chest A after packing'
Passed 'The package travelled into the second Packager and was unpacked into chest B with every item.'
$null=Command 'setblock 101 80 100 minecraft:air'

# --- 2. a lone Packager keeps its box, then ejects it into open air
$null=Command 'setblock 99 79 106 minecraft:chest'
$null=Command 'setblock 100 79 106 create:packager[facing=east]'
Wait-Ticks 5
$null=Command 'item replace block 99 79 106 container.0 with minecraft:gold_nugget 3'
$null=Command 'setblock 100 80 106 minecraft:redstone_block'
$held=$false
for ($i=0;$i -lt 10 -and -not $held;$i++) {
    Wait-Ticks 5
    if ((Get-Data 100 79 106 HeldBox) -match 'cardboard_package') { $held=$true }
}
Expect $held 'The lone Packager did not hold a box.'
Assert-Empty (Counts 99 79 106) 'The chest behind the lone Packager'
Passed 'The lone Packager holds the finished box and emptied the chest.'
$gone=$false
for ($i=0;$i -lt 30 -and -not $gone;$i++) {
    Wait-Ticks 10
    if ((Get-Data 100 79 106 HeldBox) -notmatch 'cardboard_package') { $gone=$true }
}
Expect $gone 'The Packager never ejected its box.'
Passed 'The Packager ejected the box in front of open air (item entity not inspected).'

# --- 3. Factory Gauge on a chest
$null=Command 'setblock 110 79 100 minecraft:chest'
$null=Command 'item replace block 110 79 100 container.0 with minecraft:cobblestone 8'
$null=Command 'setblock 111 79 100 create:factory_gauge[facing=east]{Filter:{id:"minecraft:cobblestone",count:1},Count:10,UpTo:1b}'
Wait-Ticks 20
$state=[string](Invoke-Ctl @('block','111','79','100')).state
Expect ($state -match 'satisfied=false') ('8 of 10 is not satisfied: '+$state)
Passed 'The gauge is not satisfied at 8 of 10.'
$null=Command 'item replace block 110 79 100 container.0 with minecraft:cobblestone 12'
$ok=$false
for ($i=0;$i -lt 10 -and -not $ok;$i++) {
    Wait-Ticks 5
    $ok=([string](Invoke-Ctl @('block','111','79','100')).state) -match 'satisfied=true'
}
Expect $ok 'The gauge did not become satisfied at 12 of 10.'
Passed 'The gauge became satisfied when the stock reached the target.'
$null=Command 'item replace block 110 79 100 container.0 with minecraft:air'
$ok=$false
for ($i=0;$i -lt 10 -and -not $ok;$i++) {
    Wait-Ticks 5
    $ok=([string](Invoke-Ctl @('block','111','79','100')).state) -match 'satisfied=false'
}
Expect $ok 'The gauge stayed satisfied after the stock was removed.'
Passed 'The gauge dropped back to unsatisfied when the stock was removed.'

$report.passed=$true
Save-Report
Write-Output 'Packager acceptance passed.'
