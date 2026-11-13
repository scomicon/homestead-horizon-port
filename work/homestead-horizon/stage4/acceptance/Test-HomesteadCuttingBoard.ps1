#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Board')][string] $Phase='Board',
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
$transcript=Join-Path $game ('stage16-board-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage16-board-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Farmer''s Delight Cutting Board: an item is placed with one click and taken back with an empty hand; a knife turns beef into minced beef; an axe strips a log and drops bark; breaking the board drops the item. Each scenario is reported separately.'
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
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function Item-Near([string] $Id) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x=100,y=80,z=100,distance=..5,nbt={Item:{id:"'+$Id+'"}}] run say board_item_found')).chat -join ' ')
    if ($text -match 'board_item_found') { return $true }
    # The player stands on the board and may already have picked the drop up.
    $inv=(Invoke-Ctl @('inventory')).counts
    return ($inv.ContainsKey($Id) -and [int]$inv[$Id] -ge 1)
}
function Use-Board([string] $Item) {
    $null=Command 'tp @s 100.5 81 100.5'
    Wait-Ticks 12
    if ($Item) { $null=Command ('item replace entity @s weapon.mainhand with '+$Item+' 1') } else { $null=Command 'item replace entity @s weapon.mainhand with minecraft:air' }
    $placed=$false;$last=''
    foreach ($face in 'up','east','north','south','west') {
        try { $null=Invoke-Ctl @('look-at','block','100','80','100',$face);$null=Invoke-Ctl @('interact','100','80','100',$face);$placed=$true;break } catch { $last+=('[' +$face+': '+$_.Exception.Message+'] ') }
    }
    if (-not $placed) { throw ('no face of the board can be targeted; last error '+$last) }
    Wait-Ticks 10
}
function Reset-Board {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
    $null=Command 'setblock 100 80 100 farmersdelight:cutting_board[facing=north]'
    Wait-Ticks 10
}
Scenario 'board places as a thin block' {
    Reset-Board
    if ((Block 100 80 100) -ne 'farmersdelight:cutting_board') { throw ('found '+(Block 100 80 100)) }
}
Scenario 'one click puts an item on the board and an empty hand takes it back' {
    Reset-Board
    Use-Board 'minecraft:beef'
    $d=Get-Data 100 80 100
    if ($d -notmatch 'beef') { throw ('the board did not take the beef: '+$d) }
    Use-Board $null
    $d=Get-Data 100 80 100
    if ($d -match 'beef') { throw ('the board still holds the beef: '+$d) }
    $inv=(Invoke-Ctl @('inventory')).counts
    if (-not ($inv.ContainsKey('minecraft:beef') -and [int]$inv['minecraft:beef'] -ge 1)) { throw ('the beef is not in the inventory: '+($inv|ConvertTo-Json -Compress)) }
}
Scenario 'a knife cuts beef into minced beef' {
    Reset-Board
    Use-Board 'minecraft:beef'
    Use-Board 'farmersdelight:iron_knife'
    if (-not (Item-Near 'farmersdelight:minced_beef')) { throw ('no minced beef dropped; board '+(Get-Data 100 80 100)) }
    if ((Get-Data 100 80 100) -match 'beef') { throw 'the board still holds the beef' }
}
Scenario 'an axe strips a log and drops bark' {
    Reset-Board
    Use-Board 'minecraft:acacia_log'
    Use-Board 'minecraft:iron_axe'
    if (-not (Item-Near 'minecraft:stripped_acacia_log')) { throw ('no stripped log dropped; board '+(Get-Data 100 80 100)) }
    if (-not (Item-Near 'farmersdelight:tree_bark')) { throw 'no tree bark dropped' }
}
Scenario 'a wrong tool does not cut' {
    Reset-Board
    Use-Board 'minecraft:beef'
    Use-Board 'minecraft:iron_pickaxe'
    if ((Get-Data 100 80 100) -notmatch 'beef') { throw 'a pickaxe cut the beef' }
}
Scenario 'breaking the board drops its item' {
    Reset-Board
    Use-Board 'minecraft:beef'
    $null=Command 'setblock 100 80 100 minecraft:air destroy'
    Wait-Ticks 10
    if (-not (Item-Near 'minecraft:beef')) { throw 'the stored beef was lost' }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)