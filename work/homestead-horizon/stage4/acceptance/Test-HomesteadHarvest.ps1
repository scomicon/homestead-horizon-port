#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Ordinary Chest/Barrel stock, real connector-built Belts and ordinary Funnel placement.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Harvest')][string] $Phase='Harvest',
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
$transcript=Join-Path $game ('stage6-harvest-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage6-harvest-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Right-click harvest of mature wheat with and without hoes, radius reach by hoe tier, immature crops and sugar cane columns. No hunger, experience, sneaking or Java comparison.'
    coordinates=@{field=@(150,84,36)}
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
function Age([int] $X,[int] $Y,[int] $Z) {
    $state=[string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state
    $m=[regex]::Match($state,'age=(\d+)'); if (-not $m.Success) { throw ('No age in '+$state) }
    return [int]$m.Groups[1].Value
}
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
function Item-Count([string] $Item) {
    $global:LastItemProbe=''
    # Dropped item entities near the field: the NBT selector first, then random samples.
    $near='x=150,y=84,z=36,distance=..14'
    $hit=((Command ('execute if entity @e[type=minecraft:item,'+$near+',nbt={Item:{id:"'+$Item+'"}}] run say stage6_drop')).chat -join ' ')
    if ($hit -match 'stage6_drop') { return $true }
    for ($try=0;$try -lt 16;$try++) {
        $text=((Command ('data get entity @e[type=minecraft:item,'+$near+',sort=random,limit=1]')).chat -join ' ')
        if ($text -match ('"'+[regex]::Escape($Item)+'"')) { return $true }
    }
    $global:LastItemProbe=$text
    # The player may already have picked the drop up.
    $counts=(Invoke-Ctl @('inventory')).counts
    if ($counts -and $counts.ContainsKey($Item) -and [int]$counts[$Item] -gt 0) { return $true }
    $global:LastItemProbe=($text+' | inventory '+($counts|ConvertTo-Json -Compress))
    return $false
}function Click([int] $X,[int] $Y,[int] $Z,[double] $Px,[double] $Pz,[string] $Face='up') {
    $null=Command ('tp @s '+$Px+' 85 '+$Pz)
    Wait-Ticks 2
    $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$Face)
    $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$Face)
    Wait-Ticks 6
}
function Plant-Field { $null=Command 'fill 146 83 33 154 83 39 minecraft:farmland'; foreach ($x in 148..152) { foreach ($z in 34..38) { $null=Command "setblock $x 84 $z minecraft:wheat[age=7]" } } }
function Unharvested([int[][]] $Cells) { return @($Cells | Where-Object { (Age $_[0] 84 $_[1]) -ne 0 }).Count }

$null=Command 'tick unfreeze';$null=Command 'gamemode survival'
$null=Command 'gamerule random_tick_speed 0'
$null=Command 'forceload add 144 32 159 47'
Wait-Ticks 40
$null=Command 'fill 146 83 33 158 90 39 minecraft:air'
$null=Command 'fill 146 82 33 158 82 39 minecraft:dirt'
$null=Command 'fill 146 83 33 158 83 39 minecraft:farmland'
$null=Command 'kill @e[type=minecraft:item]'
Wait-Ticks 2

# 1. A bare-handed click harvests one mature wheat plant and replants it.
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
$null=Command 'setblock 150 84 36 minecraft:wheat[age=7]'
Click 150 84 36 150.5 38.5
Expect ((Age 150 84 36) -eq 0) 'Mature wheat must be replanted at age 0.'
Expect (Item-Count 'minecraft:wheat') ('Harvested wheat must drop wheat. Probe: '+$global:LastItemProbe)
Passed 'A bare-handed click harvests mature wheat, drops wheat and replants it at age 0.'

# 2. An immature plant is left alone.
$null=Command 'kill @e[type=minecraft:item]'
$null=Command 'setblock 152 84 36 minecraft:wheat[age=3]'
Click 152 84 36 152.5 38.5
Expect ((Age 152 84 36) -eq 3) 'Immature wheat must stay at age 3.'
Passed 'An immature plant is not harvested.'

# 3. An iron hoe reaches four neighbours.
$null=Command 'kill @e[type=minecraft:item]'
Plant-Field
$null=Command 'item replace entity @s weapon.mainhand with minecraft:iron_hoe'
Click 150 84 36 150.5 39.5
$cross=@(,@(150,36)); $cross=@(@(150,36),@(149,36),@(151,36),@(150,35),@(150,37))
Expect ((Unharvested $cross) -eq 0) 'An iron hoe must replant the centre and four neighbours.'
Expect ((Age 148 84 36) -eq 7 -and (Age 152 84 36) -eq 7) 'The iron hoe must leave blocks beyond the cross untouched.'
Expect ((Age 149 84 35) -eq 7) 'The iron hoe must not reach diagonals.'
Passed 'An iron hoe harvests the clicked plant and its four orthogonal neighbours only.'

# 4. A diamond hoe reaches the full 3x3.
Plant-Field
$null=Command 'item replace entity @s weapon.mainhand with minecraft:diamond_hoe'
Click 150 84 36 150.5 39.5
$square=@(); foreach ($x in 149..151) { foreach ($z in 35..37) { $square+=,@($x,$z) } }
Expect ((Unharvested $square) -eq 0) 'A diamond hoe must replant a 3x3.'
Expect ((Age 148 84 36) -eq 7 -and (Age 150 84 34) -eq 7) 'A diamond hoe must not reach beyond the 3x3.'
Passed 'A diamond hoe harvests a 3x3 around the clicked plant.'

# 5. A netherite hoe reaches twelve neighbours (Manhattan distance two).
Plant-Field
$null=Command 'item replace entity @s weapon.mainhand with minecraft:netherite_hoe'
Click 150 84 36 150.5 39.5
$ring=@(); foreach ($x in 148..152) { foreach ($z in 34..38) { if ([Math]::Abs($x-150)+[Math]::Abs($z-36) -le 2) { $ring+=,@($x,$z) } } }
Expect ((Unharvested $ring) -eq 0) 'A netherite hoe must replant the diamond of radius two.'
Expect ((Age 148 84 34) -eq 7 -and (Age 152 84 38) -eq 7) 'A netherite hoe must leave the diamond corners alone.'
Passed 'A netherite hoe harvests twelve neighbours within Manhattan distance two.'

# 6. Sugar cane: the click breaks the segment above the base.
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
$null=Command 'kill @e[type=minecraft:item]'
$null=Command 'setblock 156 83 36 minecraft:sand'
$null=Command 'setblock 157 83 36 minecraft:water'
foreach ($y in 84..86) { $null=Command "setblock 156 $y 36 minecraft:sugar_cane" }
Click 156 86 36 156.5 38.5 south
Expect ((Block 156 84 36) -eq 'minecraft:sugar_cane') 'The base cane must remain.'
Expect ((Block 156 85 36) -eq 'minecraft:air') 'The segment above the base must be broken.'
Expect (Item-Count 'minecraft:sugar_cane') 'Breaking cane must drop sugar cane.'
Passed 'A click on a sugar cane column breaks the segment above the base and keeps the base.'

$report.passed=$true
Save-Report
Write-Output 'Right Click Harvest acceptance passed.'