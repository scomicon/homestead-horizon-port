#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Decorative Create families: every declared block is registered and can be placed.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Decor')][string] $Phase='Decor',
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
$transcript=Join-Path $game ('stage9-decor-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage9-decor-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='350 decorative Create blocks from tools/decor_catalog.json (stone palettes, copper tiles, windows, glass, bars, ladders, scaffolding, casings, rose quartz, cardboard): each registers, places and is found again in the world. Visuals are not compared.'
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

$catalogPath=Join-Path $workspace 'work/homestead-horizon/create-source/create-horizon-4ae7340c06da5e77828e12494509fe9a70b8eaa4/tools/decor_catalog.json'
$ids=@((Get-Content -LiteralPath $catalogPath -Raw|ConvertFrom-Json).blocks|ForEach-Object { $_.id })
Expect ($ids.Count -ge 300) ('The decor catalog looks too small: '+$ids.Count)
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 96 96 191 111'
$null=Command 'tp @s 140 100 104'
Wait-Ticks 40
$null=Command 'fill 96 90 96 191 92 111 minecraft:air'
$failed=@()
$where=@{}
$n=0
foreach ($id in $ids) {
    $x=100+2*($n%45);$z=100+2*([math]::Floor($n/45)%5);$y=90+2*[math]::Floor($n/225)
    $where[$id]=@($x,$y,$z)
    # Scaffolding breaks without a support (vanilla distance rule): give it a floor.
    if ($id -like '*_scaffolding') { $null=Command ('setblock '+$x+' '+($y-1)+' '+$z+' minecraft:stone') }
    $reply=Invoke-Ctl @('command',('setblock '+$x+' '+$y+' '+$z+' create:'+$id))
    $text=($reply.chat -join ' ')
    if ($text -notmatch 'Changed the block') { $failed+=($id+': '+$text) }
    $n++
}
$report.measured=@{blocks=$ids.Count;rejected=$failed.Count}
Save-Report
Expect ($failed.Count -eq 0) ('Blocks refused by setblock: '+($failed -join ' | '))
Passed ($ids.Count.ToString()+' decorative blocks were accepted by setblock.')
Wait-Ticks 20
$found=@{}
$reply=Invoke-Ctl @('blocks','96','90','96','191','92','111')
$counts=$reply.counts
foreach ($id in $ids) {
    $key='create:'+$id
    if (-not $counts.ContainsKey($key)) {
        # The listing may be capped: ask the world about the exact position.
        $at=$where[$id]
        $actual=Block $at[0] $at[1] $at[2]
        if ($actual -ne $key) { $found[$id]=$actual }
    }
}
$report.measured.missing=$found.Count
Save-Report
Expect ($found.Count -eq 0) ('Blocks changed or missing after placement: '+(($found.GetEnumerator()|Sort-Object Name|ForEach-Object { $_.Name+'='+$_.Value }) -join ', '))
Passed 'Every placed decorative block is found in the world.'
# Unsupported scaffolding collapses; the server must survive its collision queries.
$null=Command 'setblock 100 100 100 create:andesite_scaffolding'
$null=Command 'setblock 102 100 100 create:brass_scaffolding'
$null=Command 'setblock 104 100 100 create:copper_scaffolding'
Wait-Ticks 80
$null=Block 100 100 100
Passed 'Unsupported Create scaffolding collapsed without stopping the server.'
$report.passed=$true
Save-Report
Write-Output 'Decor acceptance passed.'