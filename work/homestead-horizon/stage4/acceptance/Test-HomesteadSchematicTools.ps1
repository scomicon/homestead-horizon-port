#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Schematic tools: the Schematic and Quill selects two corners (by command and by a real right click),
# exports them to schematics/<name>.nbt with SchematicExport's naming (slug, _1, replace, fallback name),
# the export loads again (create schematic inspect / paste); a held Schematic is rotated, mirrored, moved,
# anchored, deployed and picked up by command and deployed by a right click on a block.
# The ghost preview, the name prompt screen, the tool wheel and the edit screen are not covered (not built).
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('SchematicTools')][string] $Phase='SchematicTools',
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
$transcript=Join-Path $game ('stage21-schematic-tools-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage21-schematic-tools-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Schematic and Quill: corners by command and by a real right click, discard, export to schematics/*.nbt with SchematicExport naming (slug, _1, replace, fallback name), refusal without a selection or with only air, the export loads (inspect) and pastes; the held Schematic: info, rotate, mirror, move, anchor, deploy, undeploy by command and deploy/pick-up by right click. Ghost preview, name prompt, tool wheel and edit screen are not built and not covered. Each scenario is reported separately.'
    coordinates=@{site=@(104,80,104);paste=@(110,80,104)}
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
function Say([string] $Text) { return ((Command $Text).chat -join ' ') }
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
function Inventory-Count([string] $Id) {
    $inv=(Invoke-Ctl @('inventory')).counts
    if ($inv.ContainsKey($Id)) { return [int]$inv[$Id] } else { return 0 }
}
function Need([string] $Text,[string] $Pattern) { if ($Text -notmatch $Pattern) { throw ('expected /'+$Pattern+'/ in: '+$Text) } }

$schematics=Join-Path $game 'schematics'
New-Item -ItemType Directory -Force -Path $schematics | Out-Null
foreach ($name in 'tool_export.nbt','tool_export_1.nbt','tool_export_2.nbt','my_schematic.nbt') {
    $path=Join-Path $schematics $name
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'
$null=Command 'tp @s 100.5 80 100.5'
Wait-Ticks 40
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
# A 3x3 cobblestone floor (x 104..106, z 104..106 at y 80) with one stone block above its middle,
# the same shape as the Schematicannon fixture; everything around it is air above a stone floor.
function Reset-Site {
    $null=Command 'fill 92 77 92 120 90 120 minecraft:air'
    $null=Command 'fill 92 79 92 120 79 120 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    $null=Command 'create schematool discard'
    $null=Command 'fill 104 80 104 106 80 106 minecraft:cobblestone'
    $null=Command 'setblock 105 81 105 minecraft:stone'
    Wait-Ticks 10
}
function Select-Box { $null=Command 'create schematool discard';$null=Command 'create schematool corner 104 80 104';$null=Command 'create schematool corner 106 81 106' }
function Click-Block([int] $X,[int] $Y,[int] $Z,[string] $Face='up') {
    $null=Command 'tp @s 105.5 82 100.5'
    Wait-Ticks 14
    $ok=$false;$last=''
    foreach ($try in @($Face,'north','south','east','west','up')) {
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$try);$null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$try);$ok=$true;break } catch { $last+=('['+$try+': '+$_.Exception.Message+'] ') }
    }
    if (-not $ok) { throw ('could not right click the block: '+$last) }
    Wait-Ticks 6
}
function Hold-Schematic([string] $File='tool_export.nbt') {
    $null=Command 'clear @s'
    $text=Say ('create cannon give '+$File)
    Need $text 'gave a Schematic'
    Wait-Ticks 6
    $null=Command 'item replace entity @s weapon.mainhand from entity @s container.0'
    Wait-Ticks 4
}

Reset-Site

Scenario 'the command hands out a Schematic and Quill' {
    $text=Say 'create schematool quill'
    Need $text 'gave a Schematic and Quill'
    Wait-Ticks 6
    if ((Inventory-Count 'create:schematic_and_quill') -lt 1) { throw 'the Schematic and Quill is not in the inventory' }
}
Scenario 'two corners give the first corner, the second corner and the size' {
    Reset-Site
    Need (Say 'create schematool selection') 'no corner selected'
    Need (Say 'create schematool corner 104 80 104') 'first corner \[104, 80, 104\]'
    Need (Say 'create schematool selection') 'second corner not set'
    $text=Say 'create schematool corner 106 81 106'
    Need $text 'second corner \[106, 81, 106\], size 3 x 2 x 3'
    Need (Say 'create schematool selection') 'size 3 x 2 x 3'
}
Scenario 'saving without a name uses the fallback name and clears the selection' {
    $text=Say 'create schematool save'
    Need $text 'saved my_schematic\.nbt: 3 x 2 x 3, 10 blocks'
    if (-not (Test-Path -LiteralPath (Join-Path $schematics 'my_schematic.nbt'))) { throw 'my_schematic.nbt was not written' }
    Need (Say 'create schematool selection') 'no corner selected'
}
Scenario 'saving without a selection is refused' {
    $text=Say 'create schematool save'
    Need $text 'select two corners'
}
Scenario 'a selection of only air is refused' {
    $null=Command 'create schematool corner 112 85 112'
    $null=Command 'create schematool corner 113 86 113'
    $text=Say 'create schematool save airbox'
    Need $text 'only air'
    if (Test-Path -LiteralPath (Join-Path $schematics 'airbox.nbt')) { throw 'an air box was written' }
    $null=Command 'create schematool discard'
}
Scenario 'a named save is slugged, a second one gets _1, replace overwrites in place' {
    Select-Box
    Need (Say 'create schematool save tool_export') 'saved tool_export\.nbt'
    Select-Box
    Need (Say 'create schematool save Tool_Export') 'saved tool_export_1\.nbt'
    foreach ($name in 'tool_export.nbt','tool_export_1.nbt') {
        if (-not (Test-Path -LiteralPath (Join-Path $schematics $name))) { throw ($name+' was not written') }
    }
    $before=(Get-Item -LiteralPath (Join-Path $schematics 'tool_export.nbt')).LastWriteTimeUtc
    Select-Box
    Need (Say 'create schematool replace tool_export') 'saved tool_export\.nbt'
    if (Test-Path -LiteralPath (Join-Path $schematics 'tool_export_2.nbt')) { throw 'replace made a new numbered file' }
    $after=(Get-Item -LiteralPath (Join-Path $schematics 'tool_export.nbt')).LastWriteTimeUtc
    if ($after -lt $before) { throw 'the replaced file went back in time' }
}
Scenario 'the export is a valid structure file with the right blocks' {
    $text=Say 'create schematic inspect tool_export.nbt'
    Need $text '"ok":\s*true'
    Need $text '"blocks":\s*10'
}
Scenario 'the export pastes back as the same shape' {
    $null=Command 'fill 108 80 102 114 84 108 minecraft:air'
    $text=Say 'create schematic paste tool_export.nbt 110 80 104'
    Need $text '"ok":\s*true'
    Wait-Ticks 10
    foreach ($x in 110..112) { foreach ($z in 104..106) {
        if ((Block $x 80 $z) -ne 'minecraft:cobblestone') { throw ('floor block missing at '+$x+' 80 '+$z+': '+(Block $x 80 $z)) }
    } }
    if ((Block 111 81 105) -ne 'minecraft:stone') { throw 'the stone block is missing on top' }
}
Scenario 'sneak discard clears the corners' {
    $null=Command 'create schematool corner 104 80 104'
    $null=Command 'create schematool discard'
    Need (Say 'create schematool selection') 'no corner selected'
}
Scenario 'a real right click with the Schematic and Quill selects the neighbouring block of a top face' {
    Reset-Site
    $null=Command 'item replace entity @s weapon.mainhand with create:schematic_and_quill'
    Wait-Ticks 4
    Click-Block 104 80 104 'up'
    $text=Say 'create schematool selection'
    # Create selects the block next to a vertical face of a solid block: the top face of (104,80,104) gives (104,81,104).
    Need $text 'first corner \[104, 81, 104\]'
    $null=Command 'create schematool discard'
}
Scenario 'a held Schematic reports its file and bounds' {
    Reset-Site
    Hold-Schematic
    $text=Say 'create schematool info'
    Need $text 'file tool_export\.nbt'
    Need $text 'bounds \[3, 2, 3\]'
    Need $text 'not deployed'
    Need $text 'quarter turns 0, mirror none'
}
Scenario 'rotate and mirror change the stored settings' {
    $text=Say 'create schematool rotate cw'
    Need $text 'quarter turns 1'
    $text=Say 'create schematool rotate ccw'
    Need $text 'quarter turns 0'
    $text=Say 'create schematool mirror x'
    Need $text 'mirror left_right'
    $text=Say 'create schematool mirror x'
    Need $text 'mirror none'
    $text=Say 'create schematool mirror z'
    Need $text 'mirror front_back'
    $text=Say 'create schematool mirror z'
    Need $text 'mirror none'
    $bad=Say 'create schematool rotate sideways'
    Need $bad 'direction must be cw or ccw'
}
Scenario 'anchor, move and deploy set the stored anchor' {
    $text=Say 'create schematool anchor 110 80 104'
    Need $text 'anchor \[110, 80, 104\]'
    $text=Say 'create schematool move 2 0 1'
    Need $text 'anchor \[112, 80, 105\]'
    $text=Say 'create schematool deploy'
    Need $text 'deployed'
    Need $text 'anchor \[112, 80, 105\]'
    $text=Say 'create schematool undeploy'
    Need $text 'not deployed'
    $text=Say 'create schematool deploy 104 80 104'
    Need $text 'anchor \[104, 80, 104\]'
    Need $text ', deployed'
}
Scenario 'a right click on a block deploys a Schematic next to the clicked face' {
    $null=Command 'create schematool undeploy'
    Click-Block 104 80 104 'up'
    $text=Say 'create schematool info'
    Need $text 'anchor \[104, 81, 104\]'
    Need $text ', deployed'
}
Scenario 'undeploy picks a deployed Schematic up (the sneaking right click is not driven: the control socket has no sneak key)' {
    $null=Command 'create schematool deploy 104 80 104'
    $text=Say 'create schematool undeploy'
    Need $text 'not deployed'
}
Scenario 'a Schematic keeps its settings when the Schematicannon is clicked with it' {
    $null=Command 'create schematool deploy 104 80 104'
    $null=Command 'setblock 100 80 100 create:schematicannon'
    Wait-Ticks 10
    Click-Block 100 80 100 'up'
    $null=Invoke-Ctl @('close')
    $text=Say 'create schematool info'
    Need $text 'anchor \[104, 80, 104\]'
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
