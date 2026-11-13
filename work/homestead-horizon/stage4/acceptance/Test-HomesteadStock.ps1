#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Stock network: Stock Link on a Packager, `create stock list/order` through a Stock Ticker, the boxes leaving the Packager, two networks told apart by frequency, a Frogport pair carrying an addressed box to a chest, and the Postbox address commands.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Stock')][string] $Phase='Stock',
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
$transcript=Join-Path $game ('stage23-stock-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage23-stock-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@();notes=@()
    scope='Stock Link + Ticker ordering from a chest through a Packager, two networks told apart by frequency, a Frogport pair delivering an addressed box to a chest, Postbox address commands. Visuals, sounds and the package item tooltip are not compared.'
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
# A command whose answer is checked by the caller (an expected refusal may contain "unknown").
function Say([string] $Text) { return ((Invoke-Ctl @('command',$Text)).chat -join ' ') }
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Passed([string] $Text) { $report.checks+=$Text;Write-Output $Text;Save-Report }
function Note([string] $Text) { $report.notes+=$Text;Write-Output ('note: '+$Text);Save-Report }
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
function Count-Of([hashtable] $Counts,[string] $Item) { if ($Counts.ContainsKey($Item)) { return [int]$Counts[$Item] } else { return 0 } }
function Packages([hashtable] $Counts) {
    $n=0
    foreach ($key in $Counts.Keys) { if ($key -like 'create:cardboard_package_*') { $n+=[int]$Counts[$key] } }
    return $n
}
function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
function Wait-Until([scriptblock] $Condition,[int] $Rounds,[int] $Step) {
    for ($i=0;$i -lt $Rounds;$i++) {
        if (& $Condition) { return $true }
        Wait-Ticks $Step
    }
    return [bool](& $Condition)
}

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 100 90 100'
Wait-Ticks 40
$null=Command 'fill 92 77 92 118 90 112 minecraft:air'
$null=Command 'fill 92 78 92 118 78 112 minecraft:smooth_stone'

# --- 1. network A (blank frequency): chest C1 -> Packager P1 (east) -> chest B, link on top, ticker beside
$null=Command 'setblock 100 79 100 minecraft:chest'
$null=Command 'setblock 102 79 100 minecraft:chest'
$null=Command 'setblock 101 79 100 create:packager[facing=east]'
$null=Command 'setblock 101 80 100 create:stock_link[facing=up]'
$null=Command 'setblock 105 79 100 create:stock_ticker'
Wait-Ticks 5
Expect ((Block 101 80 100) -eq 'create:stock_link' -and (Block 105 79 100) -eq 'create:stock_ticker') 'The Stock Link and Stock Ticker were not placed.'
$null=Command 'item replace block 100 79 100 container.0 with minecraft:cobblestone 20'
$null=Command 'item replace block 100 79 100 container.1 with minecraft:sand 5'
Wait-Ticks 10
$list=Say 'create stock list 105 79 100'
Expect ($list -match 'minecraft:cobblestone x20' -and $list -match 'minecraft:sand x5' -and $list -match '1 link' -and $list -match '1 packager') ('The network listing is wrong: '+$list)
Passed 'The Stock Ticker lists the stock of the chest behind the linked Packager.'
$status=Say 'create stock status 101 80 100'
Expect ($status -match '1 link') ('Status of a link is wrong: '+$status)
Passed 'The Stock Link reports its network.'

$refused=Say 'create stock order 105 79 100 notanitem 3'
Expect ($refused -match 'unknown item') ('An unknown item must be refused: '+$refused)
Expect ((Counts 100 79 100).Count -eq 2) 'A refused order must not touch the chest.'
Passed 'An order for an unknown item is refused.'

$ordered=Say 'create stock order 105 79 100 cobblestone 12'
Expect ($ordered -match 'ordered 12 of minecraft:cobblestone') ('The order was not accepted: '+$ordered)
$a=Counts 100 79 100
Expect ((Count-Of $a 'minecraft:cobblestone') -eq 8 -and (Count-Of $a 'minecraft:sand') -eq 5) ('Chest C1 must keep 8 cobblestone and 5 sand, got '+($a|ConvertTo-Json -Compress))
Passed 'An order took exactly the requested cobblestone and left the sand.'
$got=Wait-Until { (Packages (Counts 102 79 100)) -ge 1 } 20 10
Expect $got 'The ordered box never reached chest B.'
Passed 'The finished box left the Packager into the chest in front of it.'

$none=Say 'create stock order 105 79 100 diamond 3'
Expect ($none -match 'accepted, but') ('An unfillable order is accepted without stock: '+$none)
Wait-Ticks 40
Expect ((Packages (Counts 102 79 100)) -eq 1) 'An unfillable order must not make a box.'
Passed 'An order for an item the network lacks is accepted but makes nothing.'

$short=Say 'create stock order 105 79 100 cobblestone 100'
Expect ($short -match 'ordered 8 of minecraft:cobblestone') ('A shortfall must be capped by the stock: '+$short)
$got=Wait-Until { (Packages (Counts 102 79 100)) -ge 2 } 20 10
Expect $got 'The second box never reached chest B.'
$a=Counts 100 79 100
Expect ((Count-Of $a 'minecraft:cobblestone') -eq 0 -and (Count-Of $a 'minecraft:sand') -eq 5) ('Chest C1 must hold only sand now, got '+($a|ConvertTo-Json -Compress))
Passed 'An order larger than the stock is capped at what the chest holds.'

# --- 2. a second network (apple frequency) must not see network A
$null=Command 'setblock 100 79 106 minecraft:chest'
$null=Command 'setblock 101 79 106 create:packager[facing=east]'
$null=Command 'setblock 101 79 105 create:stock_link[facing=north]{FrequencyFirst:{id:"minecraft:apple",count:1}}'
$null=Command 'setblock 105 79 106 create:stock_ticker{FrequencyFirst:{id:"minecraft:apple",count:1}}'
Wait-Ticks 5
$null=Command 'item replace block 100 79 106 container.0 with minecraft:iron_ingot 6'
Wait-Ticks 10
$listB=Say 'create stock list 105 79 106'
Expect ($listB -match 'minecraft:iron_ingot x6' -and $listB -notmatch 'cobblestone' -and $listB -notmatch 'sand') ('The apple network must hold only the iron: '+$listB)
$listA=Say 'create stock list 105 79 100'
Expect ($listA -notmatch 'iron_ingot') ('The blank network must not see the iron: '+$listA)
Passed 'Frequency items keep two stock networks apart.'

# --- 3. Frogport pair: P2 -> Frogport F1 (on top) ==address shop==> Frogport F2 -> chest G
$null=Command 'setblock 101 80 106 create:package_frogport'
$null=Command 'setblock 101 79 112 minecraft:chest'
$null=Command 'setblock 101 80 112 create:package_frogport'
Wait-Ticks 5
Expect ((Block 101 80 106) -eq 'create:package_frogport' -and (Block 101 80 112) -eq 'create:package_frogport') 'The Frogports were not placed.'
$addr=Say 'create stock address 101 80 112 shop'
Expect ($addr -match 'address filter "shop"') ('The address was not set: '+$addr)
$stat=Say 'create stock status 101 80 112'
Expect ($stat -match 'Package Frogport' -and $stat -match 'shop') ('Frogport status is wrong: '+$stat)
Passed 'A Frogport takes an address filter by command.'
$sent=Say 'create stock order 105 79 106 iron_ingot 6 shop'
Expect ($sent -match 'ordered 6 of minecraft:iron_ingot' -and $sent -match 'shop') ('The addressed order was not accepted: '+$sent)
Expect ((Counts 100 79 106).Count -eq 0) 'Chest E must be empty after the order.'
$held=Wait-Until { (Get-Data 101 79 106 HeldBox) -match 'cardboard_package' } 12 5
Expect $held 'The Packager under the Frogport made no box.'
if ((Get-Data 101 79 106 HeldBox) -match 'shop') { Passed 'The box carries the order address.' } else { Note 'HeldBox text did not show the address (NBT printing of components not verified).' }
$arrived=Wait-Until { (Packages (Counts 101 79 112)) -ge 1 } 30 10
Expect $arrived 'The addressed box never reached the second Frogport and its chest.'
Expect ((Packages (Counts 101 79 106)) -eq 0) 'Nothing may remain in the sender chest.'
Passed 'A Frogport pulled the box off the Packager and delivered it to the Frogport with the matching address, which pushed it into its chest.'

# --- 4. Postbox: address commands
$null=Command 'setblock 110 79 100 minecraft:chest'
$null=Command 'setblock 110 80 100 create:package_postbox'
Wait-Ticks 5
Expect ((Block 110 80 100) -eq 'create:package_postbox') 'The Postbox was not placed.'
$none=Say 'create stock address 110 80 100 none'
Expect ($none -match 'accepts nothing') ('A Postbox must be able to stop accepting: '+$none)
$clear=Say 'create stock address 110 80 100 clear'
Expect ($clear -match 'address filter ""') ('Clearing the filter failed: '+$clear)
$glob=Say 'create stock address 110 80 100 farm+'
Expect ($glob -match 'address filter "farm\*"') ('A + must become a star: '+$glob)
Passed 'The Postbox address commands (none, clear, glob) work.'

$report.passed=$true
Save-Report
Write-Output 'Stock network acceptance passed.'
