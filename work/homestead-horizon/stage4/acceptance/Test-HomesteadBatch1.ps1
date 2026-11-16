#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Batch 1 (stage 27): Expanded Delight, Farmer's Knives, Mo Glass, Perfect Homestead Plushies, Spud's Shops,
# Go Fish, Nether's Delight (data half) and the SparseStructures data pack.
# Reads work/homestead-horizon/pack-batch1 (written by Build-Batch1.ps1) to learn what each mod ships and checks
# in the running game that items, blocks, recipes, loot tables and tags LOAD, and that a carried Farmer's
# Delight cooking recipe actually cooks in the native pot. What this cannot observe is recorded as a note
# ("not verified"), never as a pass. Needs the batch's .hmod files in the client's horizon-mods folder
# (Start-Test.ps1 installs every extra-mods/*.hmod).
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Batch1')][string] $Phase='Batch1',
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
$transcript=Join-Path $game ('stage27-batch1-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage27-batch1-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@();notes=@()
    scope='Batch 1 data mods: items give, blocks place, recipes/loot load, tags match, one Expanded/Nether cooking recipe cooks in the native pot; SparseStructures only at file level (new worlds only).'
    coordinates=@{origin=@(100,90,100);pot=@(100,80,100)}
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
# A command whose chat reply is returned as text; never throws on a vanilla-style error reply.
function Say([string] $Text) { return ((Invoke-Ctl @('command',$Text)).chat -join ' ') }
function Command([string] $Text) {
    $reply=Invoke-Ctl @('command',$Text)
    if (($reply.chat -join ' ') -match 'not loaded|Unknown block|Unknown item|Incorrect argument|Expected |Unknown or incomplete command') {
        throw ($Text+[Environment]::NewLine+($reply.chat -join ' '))
    }
    return $reply
}
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Note([string] $Text) { $report.notes+=$Text;Write-Output ('note: '+$Text);Save-Report }
function Passed([string] $Text) { $report.checks+=$Text;Write-Output $Text;Save-Report }
function Get-Data([int] $X,[int] $Y,[int] $Z) { return ((Command "data get block $X $Y $Z").chat -join ' ') }
function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }

$batch=Join-Path $workspace 'work/homestead-horizon/pack-batch1'
$summaryPath=Join-Path $batch 'batch1-summary.json'
if (-not (Test-Path -LiteralPath $summaryPath)) { throw 'Run stage4/Build-Batch1.ps1 first (no pack-batch1/batch1-summary.json), then Start-Test.ps1 so the .hmod files are installed.' }
$summary=@(Get-Content -LiteralPath $summaryPath -Raw|ConvertFrom-Json)
$built=@($summary|Where-Object { $_.built -and $_.PSObject.Properties['archive'] })
Expect ($built.Count -ge 1) 'Build-Batch1.ps1 built no archive.'

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 140 140'
$null=Command 'tp @s 100.5 90 100.5'
Wait-Ticks 40
$null=Command 'fill 90 88 90 140 95 140 minecraft:air'
$null=Command 'fill 90 89 90 140 89 140 minecraft:smooth_stone'

$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
function Names([string] $Dir,[string] $Ns) {
    $path=Join-Path $Dir ('data/'+$Ns+'/horizon/'+$script:Kind)
    if (-not (Test-Path -LiteralPath $path)) { return @() }
    return @(Get-ChildItem -LiteralPath $path -Filter *.json -Recurse|Sort-Object Name|ForEach-Object { $_.BaseName })
}
function Sample([string[]] $List,[int] $Count) {
    if ($List.Count -le $Count) { return $List }
    $step=[math]::Max(1,[math]::Floor($List.Count/$Count));$pick=@()
    for ($k=0;$k -lt $List.Count -and $pick.Count -lt $Count;$k+=$step) { $pick+=$List[$k] }
    return $pick
}
$namespaces=[ordered]@{expandeddelight='expandeddelight';farmersknives='farmersknives';mo_glass='mo_glass';phomesteadplushies='phomesteadplushies';spuds_shops='spuds_shops';'go-fish'='gofish';nethersdelight='nethersdelight'}
$blockIndex=0

foreach ($entry in $built) {
    $key=[string]$entry.key
    if (-not $namespaces.Contains($key)) { continue }
    $ns=$namespaces[$key]
    $overlay=Join-Path $batch ('overlay/'+$key)
    if (-not (Test-Path -LiteralPath $overlay)) { Note ($key+': overlay folder missing, skipped'); continue }
    $script:Kind='item';$items=@(Names $overlay $ns)
    $script:Kind='block';$blocks=@(Names $overlay $ns)

    if ($items.Count -eq 0) { Note ($key+': declares no items (blocks and data only)') }
    else { Scenario ($key+': items can be given') {
        $ok=0
        foreach ($name in (Sample $items 4)) {
            $text=Say ('give @s '+$ns+':'+$name+' 1')
            if ($text -match 'Unknown item|not loaded|Incorrect argument|Unknown or incomplete command') { throw ($ns+':'+$name+' -> '+$text) }
            $ok++
        }
        $null=Command 'clear @s'
        Expect ($ok -gt 0) 'no item was given'
    } }
    if ($blocks.Count -gt 0) {
        Scenario ($key+': blocks place') {
            $placed=0;$bad=@()
            foreach ($name in (Sample $blocks 4)) {
                $x=100+2*($blockIndex%24);$z=100+2*[math]::Floor($blockIndex/24);$blockIndex++
                $text=Say ('setblock '+$x+' 90 '+$z+' '+$ns+':'+$name)
                if ($text -match 'Changed the block') { $placed++ } else { $bad+=($ns+':'+$name) }
            }
            if ($placed -eq 0) { throw ('none of the sampled blocks placed: '+($bad -join ', ')) }
            if ($bad.Count -gt 0) { Note ($key+': sampled blocks the loader rejected: '+($bad -join ', ')) }
        }
    } else { Note ($key+': declares no blocks (items only)') }

    # Recipes that survived the pruning: ask the game for them by id (vanilla "recipe give").
    $recipeRoot=@((Join-Path $overlay ('data/'+$ns+'/recipes')),(Join-Path $overlay ('data/'+$ns+'/recipe')))|Where-Object { Test-Path -LiteralPath $_ }|Select-Object -First 1
    if ($recipeRoot) {
        $ids=@(Get-ChildItem -LiteralPath $recipeRoot -Filter *.json -Recurse|ForEach-Object {
            $rel=$_.FullName.Substring($recipeRoot.Length+1).Replace('\','/');$rel.Substring(0,$rel.Length-5)
        }|Sort-Object)
        $shipsRecipes=@('expandeddelight','farmersknives','nethersdelight') -contains $key
        if ($ids.Count -eq 0 -and -not $shipsRecipes) { Note ($key+': no recipe survived the pruning (nothing to check)') }
        else { Scenario ($key+': recipes are known to the game') {
            Expect ($ids.Count -gt 0) 'no recipe survived the pruning'
            $seen=0;$unsupported=$false;$unknown=@()
            foreach ($id in (Sample $ids 3)) {
                $text=Say ('recipe give @s '+$ns+':'+$id)
                if ($text -match 'Unknown or incomplete command|Incorrect argument') { $unsupported=$true;break }
                if ($text -match 'Unknown recipe|No recipes? were|Could not') { $unknown+=($ns+':'+$id) } else { $seen++ }
            }
            if ($unsupported) { Note ($key+': the "recipe" command is not available here, recipes not verified'); return }
            if ($unknown.Count -gt 0 -and $seen -eq 0) { throw ('the game does not know: '+($unknown -join ', ')) }
            if ($unknown.Count -gt 0) { Note ($key+': recipes the game does not know: '+($unknown -join ', ')) }
        } }
    } else { Note ($key+': no recipes shipped') }

    # Loot tables of blocks (self-drop): ask the game to roll one.
    $lootRoot=@((Join-Path $overlay ('data/'+$ns+'/loot_tables/blocks')),(Join-Path $overlay ('data/'+$ns+'/loot_table/blocks')))|Where-Object { Test-Path -LiteralPath $_ }|Select-Object -First 1
    if ($lootRoot) {
        $tables=@(Get-ChildItem -LiteralPath $lootRoot -Filter *.json|Sort-Object Name|ForEach-Object { $_.BaseName })
        Scenario ($key+': block loot tables load') {
            $text=Say ('loot give @s loot '+$ns+':blocks/'+$tables[0])
            if ($text -match 'Unknown or incomplete command|Incorrect argument') { Note ($key+': the "loot" command is not available here, loot not verified'); return }
            if ($text -match 'Unknown loot table|could not') { throw ($ns+':blocks/'+$tables[0]+' -> '+$text) }
            $null=Command 'clear @s'
        }
    }
}

# Tags: a carried Mo Glass block must be in the tags the mod declares (c:glass_blocks and friends).
$mo=@($built|Where-Object { $_.key -eq 'mo_glass' })
if ($mo.Count -gt 0) {
    Scenario 'mo_glass: its blocks are in the declared block tags' {
        $overlay=Join-Path $batch 'overlay/mo_glass'
        $tagFile=@(Get-ChildItem -LiteralPath (Join-Path $overlay 'data') -Recurse -Filter *.json -ErrorAction SilentlyContinue|Where-Object {
            $_.FullName -match '[\\/]tags[\\/]blocks?[\\/]' -and (Get-Content -LiteralPath $_.FullName -Raw) -match '"mo_glass:'
        })|Select-Object -First 1
        if (-not $tagFile) { Note 'mo_glass: no block tag with its blocks found in the overlay; tags not verified'; return }
        $m=[regex]::Match((Get-Content -LiteralPath $tagFile.FullName -Raw),'"(mo_glass:[a-z0-9_/]+)"')
        $block=$m.Groups[1].Value
        $rel=[regex]::Match($tagFile.FullName,'[\\/]data[\\/](.+)$').Groups[1].Value.Replace('\','/')
        $parts=$rel.Split('/');$tagNs=$parts[0];$tagName=($parts[3..($parts.Count-1)] -join '/') -replace '\.json$',''
        $null=Command ('setblock 96 90 96 '+$block)
        $text=Say ('execute if block 96 90 96 #'+$tagNs+':'+$tagName+' run say batch1_tag_ok')
        if ($text -match 'Unknown or incomplete command|Incorrect argument') { Note 'mo_glass: execute if block with a tag is not available here; tags not verified'; return }
        if ($text -notmatch 'batch1_tag_ok') { throw ($block+' is not in #'+$tagNs+':'+$tagName+' ('+$text+')') }
    }
}

# A Farmer's Delight cooking recipe carried by Expanded Delight or Nether's Delight cooks in the native pot.
function Plain-Items($Recipe) {
    $out=@()
    foreach ($ing in @($Recipe.ingredients)) {
        $id=$null
        if ($ing -is [string]) { $id=$ing } elseif ($ing.PSObject.Properties['item']) { $id=[string]$ing.item } elseif ($ing.PSObject.Properties['id']) { $id=[string]$ing.id }
        if (-not $id -or $id.StartsWith('#')) { return $null }
        $out+=$id
    }
    return $out
}
Scenario 'a carried farmersdelight:cooking recipe cooks in the native pot' {
    $found=$null
    foreach ($key in 'expandeddelight','nethersdelight') {
        $ns=$namespaces[$key];$overlay=Join-Path $batch ('overlay/'+$key)
        foreach ($root in @((Join-Path $overlay ('data/'+$ns+'/recipes')),(Join-Path $overlay ('data/'+$ns+'/recipe')))) {
            if (-not (Test-Path -LiteralPath $root)) { continue }
            foreach ($file in (Get-ChildItem -LiteralPath $root -Filter *.json -Recurse|Sort-Object FullName)) {
                $r=Get-Content -LiteralPath $file.FullName -Raw|ConvertFrom-Json
                if ([string]$r.type -ne 'farmersdelight:cooking') { continue }
                $items=Plain-Items $r
                if (-not $items -or $items.Count -lt 1 -or $items.Count -gt 6) { continue }
                $res=$null;if ($r.result -is [string]) { $res=$r.result } elseif ($r.result.PSObject.Properties['item']) { $res=[string]$r.result.item } elseif ($r.result.PSObject.Properties['id']) { $res=[string]$r.result.id }
                if (-not $res) { continue }
                $container=$null
                if ($r.PSObject.Properties['container'] -and $r.container) { if ($r.container -is [string]) { $container=$r.container } elseif ($r.container.PSObject.Properties['item']) { $container=[string]$r.container.item } }
                $found=@{file=$file.Name;items=$items;result=$res;container=$container}
                break
            }
            if ($found) { break }
        }
        if ($found) { break }
    }
    if (-not $found) { Note 'no cooking recipe with only plain item ingredients (<= 6) survived; native pot not exercised with batch 1 data'; return }
    $null=Command 'fill 92 77 92 112 90 112 minecraft:air'
    $null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
    $null=Command 'kill @e[type=minecraft:item]'
    Wait-Ticks 20
    $null=Command 'setblock 100 79 100 farmersdelight:stove[facing=north,lit=true]'
    $null=Command 'setblock 100 80 100 farmersdelight:cooking_pot[facing=north,support=none,waterlogged=false]'
    Wait-Ticks 10
    $slot=0
    foreach ($id in $found.items) { $null=Command ('item replace block 100 80 100 container.'+$slot+' with '+$id+' 1'); $slot++ }
    $local=$found.result.Split(':')[-1];$cooked=$false
    for ($i=0;$i -lt 40;$i++) { Wait-Ticks 20; if ((Get-Data 100 80 100) -match [regex]::Escape($local)) { $cooked=$true;break } }
    if (-not $cooked) { throw ('recipe '+$found.file+' did not produce '+$found.result+'; pot '+(Get-Data 100 80 100)) }
    $report.cooked=$found
}

# SparseStructures: a data pack for NEW worlds only. The disposable world already exists, so only the generated files are checked.
$sp=@($summary|Where-Object { $_.key -eq 'sparsestructures' -and $_.built })
if ($sp.Count -gt 0) {
    Scenario 'sparsestructures: the generated data pack lengthens structure spacing' {
        $dp=[string]$sp[0].datapack
        Expect (Test-Path -LiteralPath (Join-Path $dp 'pack.mcmeta')) 'pack.mcmeta missing'
        $rep=Get-Content -LiteralPath (Join-Path $dp 'sparsestructures-report.json') -Raw|ConvertFrom-Json
        Expect (@($rep.changed).Count -gt 0) 'no structure set was changed'
        foreach ($c in @($rep.changed)) { if ($c.factor -ge 1 -and $c.spacing[1] -lt $c.spacing[0]) { throw ($c.set+' got closer together') }; if ($c.separation[1] -ge $c.spacing[1]) { throw ($c.set+': separation must stay below spacing') } }
        Note 'sparsestructures: effect on world generation not verified in the game (data packs apply to newly created worlds only)'
    }
}
Note 'go-fish: catching the fish is Java (loot injection and bobber mixins) and is NOT ported; only items, crates and recipes load'
Note 'ARR/GPL/LGPL mods (Perfect Homestead Plushies, Mo Glass, Nether''s Delight) are local-only content from the player''s own jars'

$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results;notes=$report.notes.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
if ($failed.Count -gt 0) { throw ('Batch 1 failures: '+(($failed|ForEach-Object { $_.Key+' => '+$_.Value }) -join ' | ')) }
Passed ($results.Count.ToString()+' batch 1 scenarios passed; '+$report.notes.Count+' not-verified notes recorded.')
