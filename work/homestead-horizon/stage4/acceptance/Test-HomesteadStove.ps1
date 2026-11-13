#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Mechanical Arm: point selection with the Arm item, placement, kinetic drive and an item transfer between two inventories.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Stove')][string] $Phase='Stove',
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
$transcript=Join-Path $game ('stage17-stove-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage17-stove-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Farmer''s Delight Stove: flint and steel lights it, a food item with a campfire recipe goes on it and cooks while it is lit, an unlit stove does not cook, breaking it drops the contents. Each scenario is reported separately.'
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
function Have([string] $Id) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x=100,y=81,z=100,distance=..6,nbt={Item:{id:"'+$Id+'"}}] run say stove_item_found')).chat -join ' ')
    if ($text -match 'stove_item_found') { return $true }
    $inv=(Invoke-Ctl @('inventory')).counts
    return ($inv.ContainsKey($Id) -and [int]$inv[$Id] -ge 1)
}
function Use-Stove([string] $Item,[int] $Count=1) {
    $null=Command 'tp @s 100.5 81 100.5'
    Wait-Ticks 12
    $null=Command ('item replace entity @s weapon.mainhand with '+$Item+' '+$Count)
    $ok=$false;$last=''
    foreach ($face in 'up','north','south','east','west') {
        try { $null=Invoke-Ctl @('look-at','block','100','80','100',$face);$null=Invoke-Ctl @('interact','100','80','100',$face);$ok=$true;break } catch { $last+=('['+$face+': '+$_.Exception.Message+'] ') }
    }
    if (-not $ok) { throw ('cannot target the stove; '+$last) }
    Wait-Ticks 10
}
function Reset-Stove([string] $Lit='false') {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
    $null=Command ('setblock 100 80 100 farmersdelight:stove[facing=north,lit='+$Lit+']')
    Wait-Ticks 10
}
Scenario 'stove places' {
    Reset-Stove
    if ((Block 100 80 100) -ne 'farmersdelight:stove') { throw ('found '+(Block 100 80 100)) }
}
Scenario 'flint and steel lights the stove' {
    Reset-Stove
    Use-Stove 'minecraft:flint_and_steel'
    if ((State 100 80 100) -notmatch 'lit=true') { throw ('state '+(State 100 80 100)) }
}
Scenario 'food goes on the stove' {
    Reset-Stove
    Use-Stove 'minecraft:flint_and_steel'
    Use-Stove 'minecraft:beef' 2
    $d=Get-Data 100 80 100
    if ($d -notmatch 'beef') { throw ('the stove did not take the beef: '+$d) }
}
Scenario 'a lit stove cooks the food' {
    $done=$false
    for ($i=0;$i -lt 45;$i++) { Wait-Ticks 20; if (Have 'minecraft:cooked_beef') { $done=$true;break } }
    if (-not $done) { throw ('no cooked beef; stove '+(Get-Data 100 80 100)) }
}
Scenario 'an unlit stove does not cook' {
    Reset-Stove
    Use-Stove 'minecraft:beef' 1
    Wait-Ticks 700
    if (Have 'minecraft:cooked_beef') { throw 'an unlit stove cooked the beef' }
    if ((Get-Data 100 80 100) -notmatch 'beef') { throw ('the beef is gone: '+(Get-Data 100 80 100)) }
}
Scenario 'breaking the stove drops its contents' {
    $null=Command 'clear @s'
    $null=Command 'tp @s 100.5 82 100.5'
    Wait-Ticks 14
    $null=Invoke-Ctl @('look-at','block','100','80','100','up')
    $null=Invoke-Ctl @('break','100','80','100')
    $null=Command 'tp @s 103.5 80 103.5'
    Wait-Ticks 12
    if (-not (Have 'minecraft:beef')) { throw 'the stored beef was lost' }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)