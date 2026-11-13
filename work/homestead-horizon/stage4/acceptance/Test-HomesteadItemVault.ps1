#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Ordinary Chest/Barrel stock, real connector-built Belts and ordinary Funnel placement.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Shared','Extras','Reload')][string] $Phase='Shared',
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
$transcript=Join-Path $game ('stage5-itemvault-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage5-itemvault-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Single and grown Item Vaults, shared multiblock inventory through the container view and a hopper under a different member. No brass/wet/sloped/Java parity claim.'
    coordinates=@{origin=@(150,84,35)}
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

function Slots([int] $X,[int] $Y,[int] $Z) { return @((Invoke-Ctl @('container',[string]$X,[string]$Y,[string]$Z)).slots).Count }
function Vault([int] $X,[int] $Y,[int] $Z,[string] $Axis='x') { $null=Command "setblock $X $Y $Z create:item_vault[axis=$Axis]" }
function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 144 32 159 47'
Wait-Ticks 40

function Total([object[]] $At,[string] $Item) {
    $sum=0
    foreach ($p in $At) { $c=Counts $p[0] $p[1] $p[2]; if ($c.ContainsKey($Item)) { $sum+=[int]$c[$Item] } }
    return $sum
}
function Fill-Member([int] $X,[int] $Y,[int] $Z) {
    foreach ($slot in 0..19) { $null=Command "item replace block $X $Y $Z container.$slot with minecraft:cobblestone 64" }
}
function Await-Total([object[]] $At,[string] $Item,[int] $Want) {
    for ($attempt=0;$attempt -lt 80;$attempt++) {
        Wait-Ticks 8
        if ((Total $At $Item) -eq $Want) { return }
    }
    throw ('Expected '+$Want+' '+$Item+', found '+(Total $At $Item))
}

function Signal([int] $X,[int] $Y,[int] $Z) {
    $text=((Command "data get block $X $Y $Z OutputSignal").chat -join ' ')
    $m=[regex]::Match($text,'(-?\d+)\D*$')
    if (-not $m.Success) { throw ('No comparator output in: '+$text) }
    return [int]$m.Groups[1].Value
}
function Seen([string] $Selector) { return (((Command ("execute if entity "+$Selector+" run say stage5_seen")).chat -join ' ') -match 'stage5_seen') }
if ($Phase -eq 'Shared') {
$null=Command 'fill 148 80 33 158 90 42 minecraft:air'
$null=Command 'fill 148 81 33 158 81 42 minecraft:stone'
Wait-Ticks 2
# 1. A lone vault has one block's capacity (the ctl container command lists one block entity).
Vault 150 84 35
Wait-Ticks 2
Expect ((Slots 150 84 35) -eq 20) 'A single Item Vault must expose 20 slots.'
Passed 'Single Item Vault exposes 20 slots.'

# 2. A hopper under the far member of a row empties stock held in the first member.
Vault 151 84 35
Vault 152 84 35
Wait-Ticks 2
$null=Command 'item replace block 150 84 35 container.0 with minecraft:cobblestone 5'
$null=Command 'setblock 152 83 35 minecraft:hopper[facing=down]'
$null=Command 'setblock 152 82 35 minecraft:chest'
Await-Total @(,@(152,82,35)) 'minecraft:cobblestone' 5
Assert-Empty (Counts 150 84 35) 'First member after the far hopper emptied the shared inventory'
Passed 'A hopper under the far member extracts stock held in the first member.'
$null=Command 'setblock 152 83 35 minecraft:air'
$null=Command 'setblock 152 82 35 minecraft:air'

# 3. With the first member full, a hopper above it inserts into the next member.
Fill-Member 150 84 35
$null=Command 'setblock 150 85 35 minecraft:hopper[facing=down]'
$null=Command 'setblock 150 86 35 minecraft:chest'
$null=Command 'item replace block 150 86 35 container.0 with minecraft:iron_ingot 9'
Await-Total @(@(151,84,35),@(152,84,35)) 'minecraft:iron_ingot' 9
Expect ((Total @(,@(150,84,35)) 'minecraft:iron_ingot') -eq 0) 'Iron must not enter the full first member.'
Passed 'A hopper above a full member inserts into the rest of the shared inventory.'
$null=Command 'setblock 150 85 35 minecraft:air'
$null=Command 'setblock 150 86 35 minecraft:air'

# 4. A 2x2x2 cuboid shares its inventory; an incomplete one does not.
$cells=@(); foreach ($x in 154,155) { foreach ($y in 84,85) { foreach ($z in 38,39) { $cells+=,@($x,$y,$z) } } }
$null=Command 'fill 154 84 38 155 85 39 create:item_vault[axis=x]'
Wait-Ticks 2
Fill-Member 154 84 38
$null=Command 'setblock 154 86 38 minecraft:hopper[facing=down]'
$null=Command 'setblock 154 87 38 minecraft:chest'
$null=Command 'item replace block 154 87 38 container.0 with minecraft:iron_ingot 9'
$others=$cells|Where-Object { -not ($_[0] -eq 154 -and $_[1] -eq 84 -and $_[2] -eq 38) }
Await-Total $others 'minecraft:iron_ingot' 9
Passed 'A 2x2x2 vault shares one inventory across its eight blocks.'
$null=Command 'setblock 154 86 38 minecraft:air'
$null=Command 'setblock 154 87 38 minecraft:air'

# 5. Removing the middle of the row splits it: the full first block now refuses more.
$null=Command 'setblock 151 84 35 minecraft:air'
Wait-Ticks 2
$null=Command 'setblock 150 85 35 minecraft:hopper[facing=down]'
$null=Command 'setblock 150 86 35 minecraft:chest'
$null=Command 'item replace block 150 86 35 container.0 with minecraft:iron_ingot 4'
Wait-Ticks 160
Expect ((Total @(@(150,86,35),@(150,85,35)) 'minecraft:iron_ingot') -eq 4 -and (Total @(,@(150,84,35)) 'minecraft:iron_ingot') -eq 0) 'A full single vault must refuse iron once the row is split.'
Passed 'Removing a block splits the row into single vaults.'
} elseif ($Phase -eq 'Extras') {
$null=Command 'fill 148 80 43 158 90 47 minecraft:air'
$null=Command 'fill 148 81 43 158 81 47 minecraft:stone'
Wait-Ticks 2

# 6. Comparator output follows the shared inventory fullness.
foreach ($x in 150,151,152) { Vault $x 84 45 }
Wait-Ticks 2
Fill-Member 150 84 45
$null=Command 'setblock 150 83 44 minecraft:stone'
$null=Command 'setblock 150 84 44 minecraft:comparator[facing=south]'
$null=Command 'setblock 150 84 43 minecraft:redstone_lamp'
Wait-Ticks 40
$shared=Signal 150 84 44
Expect ($shared -eq 5) ("A full block in a 3-block vault must read 5 (20 of 60 slots), read $shared.")
$null=Command 'setblock 151 84 45 minecraft:air'
Wait-Ticks 4
$null=Command 'setblock 150 84 44 minecraft:air'
$null=Command 'setblock 150 84 44 minecraft:comparator[facing=south]'
$null=Command 'setblock 150 84 43 minecraft:air'
$null=Command 'setblock 150 84 43 minecraft:redstone_lamp'
$null=Command 'setblock 150 84 43 minecraft:redstone_lamp'
Wait-Ticks 40
$single=Signal 150 84 44
Expect ($single -eq 15) ("A full single vault must read 15, read $single.")
$report.comparator=@{shared=$shared;single=$single}
Passed 'Comparator reads 5 for one full block of a three-block vault and 15 once the block stands alone.'
Vault 151 84 45
Wait-Ticks 2
$null=Command 'setblock 150 84 43 minecraft:air'
$null=Command 'setblock 150 84 43 minecraft:redstone_lamp'
Wait-Ticks 40
$merged=Signal 150 84 44
Expect ($merged -eq 5) ("Rejoining the row must read 5 again, read $merged.")
Passed 'Rejoining the row lowers the comparator reading back to 5.'

# 7. Destroying a stocked vault drops its contents and the vault item.
Vault 155 84 45
Fill-Member 155 84 45
$null=Command 'setblock 155 84 45 minecraft:air destroy'
Wait-Ticks 20
Expect (Seen '@e[type=minecraft:item,x=155,y=84,z=45,distance=..6,nbt={Item:{id:"minecraft:cobblestone"}}]') 'Destroying a stocked Item Vault must drop its contents.'
Expect (Seen '@e[type=minecraft:item,x=155,y=84,z=45,distance=..6,nbt={Item:{id:"create:item_vault"}}]') 'Destroying an Item Vault must drop the vault item.'
$null=Command 'kill @e[type=minecraft:item,x=155,y=84,z=45,distance=..6]'
Passed 'A destroyed Item Vault drops its stocked items and itself.'

# 8. Chutes pull from and insert into a vault.
$null=Command 'setblock 157 87 45 minecraft:chest'
$null=Command 'setblock 157 86 45 create:chute[facing=down]'
Vault 157 85 45
$null=Command 'setblock 157 84 45 create:chute[facing=down]'
$null=Command 'setblock 157 83 45 minecraft:chest'
Wait-Ticks 4
$null=Command 'item replace block 157 87 45 container.0 with minecraft:iron_ingot 10'
Await-Total @(,@(157,83,45)) 'minecraft:iron_ingot' 10
Passed 'A Chute inserts ten ingots into an Item Vault and a second Chute pulls them out into a Chest.'

# 9. Fixture kept for the restart check: a full first block and ingots in the last.
$null=Command 'item replace block 152 84 45 container.0 with minecraft:iron_ingot 7'
Expect ((Total @(,@(150,84,45)) 'minecraft:cobblestone') -eq 1280) 'The persistence fixture must hold 1280 cobblestone.'
Passed 'Persistence fixture saved: a three-block vault with 1280 cobblestone and 7 ingots.'
} else {
$before=Get-Content -LiteralPath (Join-Path $game 'stage5-itemvault-extras-result.json') -Raw|ConvertFrom-Json -AsHashtable
if (-not $before.passed -or $before.world -ne $World) { throw 'A passing Extras receipt is required before the cold reload.' }
if ($GameProcessId -le 0 -or [int]$before.process_id -le 0 -or $GameProcessId -eq [int]$before.process_id) { throw 'The reload check requires distinct native process IDs.' }
foreach ($x in 150,151,152) { Expect ((Invoke-Ctl @('block',[string]$x,'84','45')).block -eq 'create:item_vault') 'A cold-loaded Item Vault is missing.' }
Expect ((Total @(,@(150,84,45)) 'minecraft:cobblestone') -eq 1280) 'The first block must still hold 1280 cobblestone.'
Expect ((Total @(,@(152,84,45)) 'minecraft:iron_ingot') -eq 7) 'The last block must still hold 7 ingots.'
$signal=Signal 150 84 44
Expect ($signal -eq 5) ("The cold-loaded comparator must read 5, read $signal.")
Passed 'A distinct native process restores the three-block vault contents.'
$null=Command 'setblock 152 83 45 minecraft:hopper[facing=down]'
$null=Command 'setblock 152 82 45 minecraft:chest'
for ($attempt=0;$attempt -lt 60;$attempt++) {
    Wait-Ticks 8
    if ((Total @(,@(152,82,45)) 'minecraft:cobblestone') -gt 0) { break }
}
Expect ((Total @(,@(152,82,45)) 'minecraft:cobblestone') -gt 0) 'After reload a hopper under the last block must reach the first block''s stock.'
Passed 'After the restart the shared view still serves a hopper under another member.'
}
$report.passed=$true
Save-Report
Write-Output 'Item Vault acceptance passed.'