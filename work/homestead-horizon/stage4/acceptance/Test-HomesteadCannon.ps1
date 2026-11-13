#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Schematicannon, Schematic Table and Material Checklist: the screens open, a deployed schematic
# prints from the materials of an adjacent chest and from gunpowder, a missing material pauses
# or is skipped, a cold cannon waits for fuel, the table turns an Empty Schematic into a Schematic.
# Buttons are driven by the operator command `create cannon control` (the same code the screen's
# buttons call); the screen itself is only opened and its slots read.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Cannon')][string] $Phase='Cannon',
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
$transcript=Join-Path $game ('stage30-cannon-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage30-cannon-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Schematicannon with its screen, Schematic Table, Material Checklist: a deployed schematic from instance schematics/cannon_test.nbt prints into the world from an adjacent chest and gunpowder; a missing material pauses it, skip-missing continues, no gunpowder stops it; the table uploads an Empty Schematic; stopping resets; breaking drops the contents. The Schematic Tool preview and the buttons by mouse are not covered. Each scenario is reported separately.'
    coordinates=@{cannon=@(100,80,100);chest=@(101,80,100);table=@(96,80,100);anchor=@(104,80,104)}
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
function Get-Data([int] $X,[int] $Y,[int] $Z,[string] $Key='') {
    $text="data get block $X $Y $Z";if ($Key) { $text+=' '+$Key }
    return ((Command $text).chat -join ' ')
}
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
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
function Count-Of([hashtable] $Counts,[string] $Id) { if ($Counts.ContainsKey($Id)) { return [int]$Counts[$Id] } else { return 0 } }

# ---------------------------------------------------------------- fixture
# A 3x2x3 structure: a 3x3 floor of cobblestone with one stone block on top of its middle.
# Structure NBT as the vanilla/Create exporter writes it (gzip, big endian), DataVersion 3955.
function New-Fixture([string] $Path) {
    $ms=[IO.MemoryStream]::new()
    function U16($v) { $ms.WriteByte(($v -shr 8) -band 255); $ms.WriteByte($v -band 255) }
    function I32($v) { foreach ($shift in 24,16,8,0) { $ms.WriteByte(($v -shr $shift) -band 255) } }
    function Str($t) { $b=[Text.Encoding]::UTF8.GetBytes($t); U16 $b.Length; $ms.Write($b,0,$b.Length) }
    function Head($type,$name) { $ms.WriteByte($type); Str $name }
    function Ints3($a,$b,$c) { $ms.WriteByte(3); I32 3; I32 $a; I32 $b; I32 $c }
    $ms.WriteByte(10); Str ''
    Head 3 'DataVersion'; I32 3955
    Head 9 'size'; Ints3 3 2 3
    Head 9 'palette'; $ms.WriteByte(10); I32 2
    Head 8 'Name'; Str 'minecraft:cobblestone'; $ms.WriteByte(0)
    Head 8 'Name'; Str 'minecraft:stone'; $ms.WriteByte(0)
    Head 9 'blocks'; $ms.WriteByte(10); I32 10
    foreach ($x in 0..2) { foreach ($z in 0..2) {
        Head 9 'pos'; Ints3 $x 0 $z
        Head 3 'state'; I32 0
        $ms.WriteByte(0)
    } }
    Head 9 'pos'; Ints3 1 1 1
    Head 3 'state'; I32 1
    $ms.WriteByte(0)
    Head 9 'entities'; $ms.WriteByte(10); I32 0
    $ms.WriteByte(0)
    $out=[IO.MemoryStream]::new()
    $gz=[IO.Compression.GZipStream]::new($out,[IO.Compression.CompressionLevel]::Optimal,$true)
    $bytes=$ms.ToArray(); $gz.Write($bytes,0,$bytes.Length); $gz.Dispose()
    New-Item -ItemType Directory -Force -Path (Split-Path $Path) | Out-Null
    [IO.File]::WriteAllBytes($Path,$out.ToArray())
}
New-Fixture (Join-Path $game 'schematics/cannon_test.nbt')

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 98.5 80 98.5'
Wait-Ticks 40
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function Have([string] $Id) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x=100,y=81,z=100,distance=..6,nbt={Item:{id:"'+$Id+'"}}] run say cannon_item_found')).chat -join ' ')
    if ($text -match 'cannon_item_found') { return $true }
    $inv=(Invoke-Ctl @('inventory')).counts
    return ($inv.ContainsKey($Id) -and [int]$inv[$Id] -ge 1)
}
function Cannon-Status { return ((Command 'create cannon status 100 80 100').chat -join ' ') }
function Cannon-Control([string] $Action) { return ((Command ('create cannon control 100 80 100 '+$Action)).chat -join ' ') }
function Wait-Status([string] $Pattern,[int] $Rounds=40,[int] $Ticks=20) {
    $last=''
    for ($i=0;$i -lt $Rounds;$i++) {
        Wait-Ticks $Ticks
        $last=Cannon-Status
        if ($last -match $Pattern) { return $last }
    }
    throw ('status never matched /'+$Pattern+'/; last: '+$last)
}
function Reset-Site([int] $Cobble=20,[int] $Stone=5,[int] $Gunpowder=2,[int] $Turns=0) {
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    Wait-Ticks 20
    $null=Command 'setblock 100 80 100 create:schematicannon'
    $null=Command 'setblock 101 80 100 minecraft:chest'
    Wait-Ticks 10
    if ($Cobble -gt 0) { $null=Invoke-Ctl @('container','101','80','100','minecraft:cobblestone',[string]$Cobble) }
    if ($Stone -gt 0) { $null=Invoke-Ctl @('container','101','80','100','minecraft:stone',[string]$Stone) }
    if ($Gunpowder -gt 0) { $null=Command ('item replace block 100 80 100 container.4 with minecraft:gunpowder '+$Gunpowder) }
    $load='create cannon load 100 80 100 cannon_test.nbt 104 80 104'
    if ($Turns -gt 0) { $load+=' '+$Turns }
    $text=((Command $load).chat -join ' ')
    if ($text -notmatch 'loaded') { throw ('load failed: '+$text) }
    Wait-Ticks 30
}
# The footprint is offset from the anchor by the quarter turns: Create rotates around the zero pivot
# (SchematicItem.getSettings), so one clockwise quarter turn maps (x,z) to (-z,x) and the 3x3 floor
# covers x 102..104 instead of 104..106 (the Schematic Tool shifts the anchor to compensate).
function Floor-Count([int] $X0=104) { $n=0; foreach ($x in $X0..($X0+2)) { foreach ($z in 104..106) { if ((Block $x 80 $z) -eq 'minecraft:cobblestone') { $n++ } } }; return $n }
function Stone-Count([int] $X0=104) { $n=0; foreach ($x in $X0..($X0+2)) { foreach ($z in 104..106) { if ((Block $x 81 $z) -eq 'minecraft:stone') { $n++ } } }; return $n }
function Open-At([int] $X,[int] $Y,[int] $Z) {
    $null=Command 'tp @s 100.5 82 98.5'
    Wait-Ticks 14
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
    $ok=$false;$last=''
    foreach ($face in 'up','north','south','east','west') {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$face);$null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$face);$ok=$true;break } catch { $last+=('['+$face+': '+$_.Exception.Message+'] ') }
    }
    if (-not $ok) { throw ('cannot target the block; '+$last) }
    Wait-Ticks 8
}

Scenario 'the blocks and items exist' {
    Reset-Site 0 0 0 0
    if ((Block 100 80 100) -ne 'create:schematicannon') { throw ('found '+(Block 100 80 100)) }
    $null=Command 'setblock 96 80 100 create:schematic_table'
    if ((Block 96 80 100) -ne 'create:schematic_table') { throw ('found '+(Block 96 80 100)) }
    foreach ($item in 'create:empty_schematic','create:schematic','create:clipboard') {
        $null=Command ('give @s '+$item+' 1')
    }
    $inv=(Invoke-Ctl @('inventory')).counts
    foreach ($item in 'create:empty_schematic','create:schematic','create:clipboard') {
        if (-not $inv.ContainsKey($item)) { throw ($item+' was not given') }
    }
    $null=Command 'clear @s'
}
Scenario 'the cannon menu opens with its five slots' {
    try { $null=Invoke-Ctl @('close') } catch { }
    Reset-Site 0 0 1 0
    Open-At 100 80 100
    $screen=Invoke-Ctl @('screen')
    if ($screen.screen -ne 'container') { throw ('the container screen is not open: '+($screen|ConvertTo-Json -Compress -Depth 4)) }
    $ids=@($screen.slots|ForEach-Object { [int]$_.slot })
    try {
        foreach ($want in 300..304) { if ($ids -notcontains $want) { throw ('slot '+$want+' is missing from the screen: '+($ids -join ',')) } }
    } catch { $null=Invoke-Ctl @('close'); throw }
    $first=$screen.slots|Where-Object { [int]$_.slot -eq 300 }
    $shown=$first | ConvertTo-Json -Compress -Depth 4
    if ($shown -notmatch 'schematic') { throw ('slot 300 does not show the Schematic: '+$shown) }
    $null=Invoke-Ctl @('close')
}
Scenario 'a loaded schematic leaves the cannon ready, not running' {
    Reset-Site
    $status=Wait-Status 'status ready' 10 10
    if ($status -notmatch 'state 1') { throw ('expected paused: '+$status) }
    if ((Floor-Count) -ne 0) { throw 'blocks appeared before Play' }
}
Scenario 'play prints the schematic from the chest on one gunpowder' {
    Reset-Site
    $null=Wait-Status 'status ready' 10 10
    $null=Cannon-Control 'play'
    $status=Wait-Status 'status finished' 60 10
    if ((Floor-Count) -ne 9) { throw ('floor cobblestone: '+(Floor-Count)+'; '+$status) }
    if ((Stone-Count) -ne 1) { throw ('stone on top: '+(Stone-Count)+'; '+$status) }
    $chest=Counts 101 80 100
    if ((Count-Of $chest 'minecraft:cobblestone') -ne 11) { throw ('chest cobblestone should be 11: '+($chest|ConvertTo-Json -Compress)) }
    if ((Count-Of $chest 'minecraft:stone') -ne 4) { throw ('chest stone should be 4: '+($chest|ConvertTo-Json -Compress)) }
    $fuel=[regex]::Match($status,'fuel (\d+)')
    if (-not $fuel.Success -or [int]$fuel.Groups[1].Value -ne 390) { throw ('fuel after 10 shots should be 390: '+$status) }
    # Create's finishedPrinting calls resetPrinter, which zeroes AmountPlaced/AmountToPlace; ten
    # blocks in the world and ten shots of fuel are the evidence that ten blocks were placed.
    if ($status -notmatch 'placed 0 to_place 0') { throw ('counters should be reset after finishing: '+$status) }
}
Scenario 'a finished print returns an Empty Schematic' {
    $data=Get-Data 100 80 100
    if ($data -notmatch 'empty_schematic') { throw ('no Empty Schematic in the output slot: '+$data) }
}
Scenario 'a missing material pauses, skip-missing continues' {
    Reset-Site 20 0 2 0
    $null=Wait-Status 'status ready' 10 10
    $null=Cannon-Control 'play'
    $status=Wait-Status 'status missingBlock' 40 10
    if ($status -notmatch "missing 'minecraft:stone'") { throw ('the missing item is not named: '+$status) }
    $null=Cannon-Control 'skip_missing'
    $status=Wait-Status 'status finished' 60 10
    if ((Floor-Count) -ne 9) { throw ('floor cobblestone: '+(Floor-Count)) }
    if ((Stone-Count) -ne 0) { throw 'the skipped stone block was placed' }
}
Scenario 'an out-of-gunpowder cannon waits' {
    Reset-Site 20 5 0 0
    # As in Create, a loaded cannon without fuel goes Ready -> Out of Gunpowder on its own (the
    # paused-and-ready early return in tickPrinter needs remainingFuel > 0); no Play is needed.
    $status=Wait-Status 'status noGunpowder' 20 10
    if ($status -notmatch 'state 1') { throw ('expected paused: '+$status) }
    $null=Cannon-Control 'play'
    $status=Wait-Status 'status noGunpowder' 20 10
    if ((Floor-Count) -ne 0) { throw 'blocks were placed without fuel' }
    $null=Command 'item replace block 100 80 100 container.4 with minecraft:gunpowder 1'
    # Gunpowder alone does not resume a cannon that placed nothing yet (as in Create): Ready, paused;
    # press Play.
    $null=Wait-Status 'state 1 status ready' 10 10
    if ((Floor-Count) -ne 0) { throw 'blocks were placed before Play' }
    $null=Cannon-Control 'play'
    $status=Wait-Status 'status finished' 60 10
    if ((Floor-Count) -ne 9) { throw ('floor cobblestone after refuel: '+(Floor-Count)) }
}
Scenario 'a quarter turn prints the same blocks' {
    Reset-Site 20 5 2 1
    $null=Wait-Status 'status ready' 10 10
    $null=Cannon-Control 'play'
    $null=Wait-Status 'status finished' 60 10
    if ((Floor-Count 102) -ne 9) { throw ('floor cobblestone at x 102..104: '+(Floor-Count 102)) }
    if ((Stone-Count 102) -ne 1) { throw ('stone on top: '+(Stone-Count 102)) }
    if ((Block 103 81 105) -ne 'minecraft:stone') { throw ('the stone should be above the middle, at 103 81 105: '+(Block 103 81 105)) }
    if ((Block 105 80 105) -eq 'minecraft:cobblestone') { throw 'the unturned footprint was printed' }
}
Scenario 'stop resets a running print' {
    Reset-Site
    $null=Wait-Status 'status ready' 10 10
    $null=Cannon-Control 'play'
    Wait-Ticks 25
    $null=Cannon-Control 'stop'
    # As in Create, Stop resets the printer (counters zeroed, cursor back to the start) and the
    # cannon, with the Schematic still in its slot, initialises again: paused and Ready.
    Wait-Ticks 40
    $status=Cannon-Status
    if ($status -notmatch 'state 1 status ready') { throw ('not reset to paused/ready: '+$status) }
    if ($status -notmatch 'placed 0 ') { throw ('the counters were not reset: '+$status) }
    # Shots already in the air still land; after that nothing more is placed until Play.
    Wait-Ticks 40
    $landed=Floor-Count
    if ($landed -ge 9) { throw 'the print ran on after Stop' }
    Wait-Ticks 60
    if ((Floor-Count) -ne $landed) { throw 'blocks kept appearing after Stop' }
    # The print starts over from the beginning with Play and still completes.
    $null=Cannon-Control 'play'
    $null=Wait-Status 'status finished' 60 10
    if ((Floor-Count) -ne 9) { throw ('floor cobblestone after restarting: '+(Floor-Count)) }
    if ((Stone-Count) -ne 1) { throw ('stone on top after restarting: '+(Stone-Count)) }
}
Scenario 'the table lists the folder and uploads an Empty Schematic' {
    $null=Command 'setblock 96 80 100 create:schematic_table'
    Wait-Ticks 10
    $null=Command 'item replace block 96 80 100 container.0 with create:empty_schematic 1'
    Wait-Ticks 50
    $info=((Command 'create cannon control 96 80 100 next').chat -join ' ')
    if ($info -notmatch 'file \d+ of \d+') { throw ('table reply: '+$info) }
    $info=((Command 'create cannon control 96 80 100 upload').chat -join ' ')
    $data=Get-Data 96 80 100
    if ($data -notmatch 'create:schematic(?![_a-z])') { throw ('no Schematic in the table: '+$data+' / '+$info) }
    if ($data -match 'empty_schematic') { throw ('the Empty Schematic was not consumed: '+$data) }
}
Scenario 'the table menu opens' {
    try { $null=Invoke-Ctl @('close') } catch { }
    Open-At 96 80 100
    $screen=Invoke-Ctl @('screen')
    if ($screen.screen -ne 'container') { throw ('the container screen is not open: '+($screen|ConvertTo-Json -Compress -Depth 4)) }
    $ids=@($screen.slots|ForEach-Object { [int]$_.slot })
    try {
        foreach ($want in 300,301) { if ($ids -notcontains $want) { throw ('slot '+$want+' is missing from the screen: '+($ids -join ',')) } }
    } catch { $null=Invoke-Ctl @('close'); throw }
    $null=Invoke-Ctl @('close')
}
Scenario 'an undeployed Schematic leaves the cannon not positioned' {
    # The cannon still holds the previous scenario's deployed Schematic with a loaded printer; as in
    # Create, swapping the slot under a loaded printer does not reload it. Take the old one out
    # first so the cannon goes Idle and resets, then put the table's undeployed Schematic in.
    $null=Command 'item replace block 100 80 100 container.0 with minecraft:air'
    Wait-Ticks 30
    if ((Cannon-Status) -notmatch 'state 0') { throw ('the cannon did not stop: '+(Cannon-Status)) }
    $null=Command 'item replace block 100 80 100 container.0 from block 96 80 100 container.1'
    $deadline=0
    $last=''
    for ($i=0;$i -lt 20;$i++) {
        Wait-Ticks 10
        $last=Cannon-Status
        if ($last -match 'status schematicNotPlaced') { $deadline=1;break }
    }
    if ($deadline -ne 1) { throw ('status never became schematicNotPlaced; last: '+$last) }
}
Scenario 'breaking the cannon drops its gunpowder' {
    Reset-Site 0 0 3 0
    $null=Command 'clear @s'
    $null=Command 'tp @s 100.5 82 98.5'
    Wait-Ticks 14
    $null=Invoke-Ctl @('look-at','block','100','80','100','up')
    $null=Invoke-Ctl @('break','100','80','100')
    $null=Command 'tp @s 103.5 80 103.5'
    Wait-Ticks 14
    if (-not (Have 'minecraft:gunpowder')) { throw 'the gunpowder was lost' }
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
