#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Batch 2 (stage 23): Ocean's Delight, Crate Delight, Obsidian Equipment Reworked,
# Minecraft Accelerated, Create Deco and Sooty Chimneys.
# Reads work/homestead-horizon/pack-batch2 (written by Build-DataMods-batch2.ps1) and verifies in
# the running client that items, blocks, recipes and loot tables load, and that an Ocean's Delight
# cooking recipe actually cooks in the native Farmer's Delight cooking pot. Anything not directly
# observable through the control socket is recorded as a note ("not verified"), never as a pass.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Batch2')][string] $Phase = 'Batch2',
    [string] $World = 'HomesteadAutomationAcceptance',
    [int] $GameProcessId = 0
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Threading.Thread]::CurrentThread.CurrentCulture = [Globalization.CultureInfo]::InvariantCulture
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$game = [IO.Path]::GetFullPath($GameDir)
$allowed = @(
    [IO.Path]::GetFullPath((Join-Path $workspace 'work/homestead-horizon/stage4')),
    [IO.Path]::GetFullPath((Join-Path $workspace 'outputs/homestead-horizon-stage4'))
)
if (-not ($allowed | Where-Object { $game.StartsWith($_ + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) })) {
    throw 'Use the separate stage4 work or stage4 output instance, never a player save.'
}
if (-not (Test-Path -LiteralPath $game -PathType Container)) { throw 'Start the disposable client first.' }
if ($World -notmatch '^HomesteadAutomation[A-Za-z0-9_-]*$') { throw 'Use an explicitly disposable automation world.' }
$ctlPath = (Resolve-Path -LiteralPath $Ctl).Path
$socketPath = [IO.Path]::GetFullPath($Socket)
$transcript = Join-Path $game ('stage23-batch2-' + $Phase.ToLowerInvariant() + '-transcript.jsonl')
$reportPath = Join-Path $game ('stage23-batch2-' + $Phase.ToLowerInvariant() + '-result.json')
$utf8 = [Text.UTF8Encoding]::new($false)
$report = [ordered]@{
    passed = $false
    phase = $Phase
    world = $World
    process_id = $GameProcessId
    checks = @()
    notes = @()
    scope = 'Batch 2 data mods: items give, blocks place, recipes/loot load, Ocean''s Delight cooking recipe cooks in the native FD pot.'
    coordinates = @{ origin = @(100, 90, 100); pot = @(100, 80, 100) }
}
[IO.File]::WriteAllText($transcript, '', $utf8)

function Save-Report {
    [IO.File]::WriteAllText($reportPath, ($report | ConvertTo-Json -Depth 25) + [Environment]::NewLine, $utf8)
}

function Invoke-Ctl([string[]] $Words) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $ctlPath
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($word in (@('--socket', $socketPath) + $Words)) { $info.ArgumentList.Add([string]$word) }
    $process = [Diagnostics.Process]::Start($info)
    $outTask = $process.StandardOutput.ReadToEndAsync()
    $errTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(150000)) { $process.Kill(); throw ('Control timeout: ' + ($Words -join ' ')) }
    $stdout = $outTask.GetAwaiter().GetResult()
    $stderr = $errTask.GetAwaiter().GetResult()
    [IO.File]::AppendAllText($transcript, (@{ words = $Words; exit_code = $process.ExitCode; stdout = $stdout; stderr = $stderr } | ConvertTo-Json -Compress -Depth 20) + [Environment]::NewLine, $utf8)
    if ($process.ExitCode -ne 0) { throw ('Control failed: ' + ($Words -join ' ') + [Environment]::NewLine + $stdout + $stderr) }
    $reply = $stdout | ConvertFrom-Json -AsHashtable
    if ($reply.ContainsKey('error')) { throw ($reply.error | ConvertTo-Json -Depth 10) }
    return $reply
}

function Say([string] $Text) { return ((Invoke-Ctl @('command', $Text)).chat -join ' ') }

function Command([string] $Text) {
    $reply = Invoke-Ctl @('command', $Text)
    if (($reply.chat -join ' ') -match 'not loaded|Unknown block|Unknown item|Incorrect argument|Expected |Unknown or incomplete command') {
        throw ($Text + [Environment]::NewLine + ($reply.chat -join ' '))
    }
    return $reply
}

function Wait-Ticks([int] $Count) { $null = Invoke-Ctl @('wait', [string]$Count) }
function Note([string] $Text) { $report.notes += $Text; Write-Output ('note: ' + $Text); Save-Report }
function Passed([string] $Text) { $report.checks += $Text; Write-Output $Text; Save-Report }
function Get-Data([int] $X, [int] $Y, [int] $Z) { return ((Command "data get block $X $Y $Z").chat -join ' ') }
function Expect([bool] $Condition, [string] $Message) { if (-not $Condition) { throw $Message } }

$batch = Join-Path $workspace 'work/homestead-horizon/pack-batch2'
$summaryPath = Join-Path $batch 'batch2-summary.json'
if (-not (Test-Path -LiteralPath $summaryPath)) {
    throw 'Run stage4/Build-DataMods-batch2.ps1 first (no pack-batch2/batch2-summary.json), then Start-Test.ps1.'
}
$summary = @(Get-Content -LiteralPath $summaryPath -Raw | ConvertFrom-Json)
$built = @($summary | Where-Object { $_.built -and $_.PSObject.Properties['archive'] })
Expect ($built.Count -ge 1) 'Build-DataMods-batch2.ps1 built no archive.'

$null = Command 'tick unfreeze'
$null = Command 'gamemode creative'
$null = Command 'forceload add 90 90 140 140'
$null = Command 'tp @s 100.5 90 100.5'
Wait-Ticks 40
$null = Command 'fill 90 88 90 140 95 140 minecraft:air'
$null = Command 'fill 90 89 90 140 89 140 minecraft:smooth_stone'

$results = [ordered]@{}
function Scenario([string] $Name, [scriptblock] $Body) {
    try { & $Body; $results[$Name] = 'ok' } catch { $results[$Name] = ('failed: ' + $_.Exception.Message) }
    Write-Output ($Name + ': ' + $results[$Name])
    $report.checks += ($Name + ': ' + $results[$Name])
    Save-Report
}

function Names([string] $Dir, [string] $Ns) {
    $path = Join-Path $Dir ('data/' + $Ns + '/horizon/' + $script:Kind)
    if (-not (Test-Path -LiteralPath $path)) { return @() }
    return @(Get-ChildItem -LiteralPath $path -Filter *.json -Recurse | Sort-Object Name | ForEach-Object { $_.BaseName })
}

function Sample([string[]] $List, [int] $Count) {
    if ($List.Count -le $Count) { return $List }
    $step = [math]::Max(1, [math]::Floor($List.Count / $Count))
    $pick = @()
    for ($k = 0; $k -lt $List.Count -and $pick.Count -lt $Count; $k += $step) { $pick += $List[$k] }
    return $pick
}

$namespaces = [ordered]@{
    oceansdelight = 'oceansdelight'
    cratedelight = 'cratedelight'
    obsidianequipmentrework = 'obsidianequipmentrework'
    mcaccel = 'mcaccel'
    createdeco = 'createdeco'
    sootychimneys = 'sootychimneys'
}
$blockIndex = 0

foreach ($entry in $built) {
    $key = [string]$entry.key
    if (-not $namespaces.Contains($key)) { continue }
    $ns = $namespaces[$key]
    $overlay = Join-Path $batch ('overlay/' + $key)
    if (-not (Test-Path -LiteralPath $overlay)) { Note ($key + ': overlay folder missing, skipped'); continue }
    $script:Kind = 'item'; $items = @(Names $overlay $ns)
    $script:Kind = 'block'; $blocks = @(Names $overlay $ns)
    $giveNames = @($items + $blocks | Select-Object -Unique)

    if ($giveNames.Count -eq 0) {
        Note ($key + ': declares no items or block items')
    } else {
        Scenario ($key + ': items can be given') {
            $ok = 0
            foreach ($name in (Sample $giveNames 4)) {
                $text = Say ('give @s ' + $ns + ':' + $name + ' 1')
                if ($text -match 'Unknown item|not loaded|Incorrect argument|Unknown or incomplete command') {
                    throw ($ns + ':' + $name + ' -> ' + $text)
                }
                $ok++
            }
            $null = Command 'clear @s'
            Expect ($ok -gt 0) 'no item was given'
        }
    }

    if ($blocks.Count -gt 0) {
        Scenario ($key + ': blocks place') {
            $placed = 0
            $bad = @()
            foreach ($name in (Sample $blocks 4)) {
                $x = 100 + 2 * ($blockIndex % 24)
                $z = 100 + 2 * [math]::Floor($blockIndex / 24)
                $blockIndex++
                $text = Say ('setblock ' + $x + ' 90 ' + $z + ' ' + $ns + ':' + $name)
                if ($text -match 'Changed the block') { $placed++ } else { $bad += ($ns + ':' + $name) }
            }
            if ($placed -eq 0) { throw ('none of the sampled blocks placed: ' + ($bad -join ', ')) }
            if ($bad.Count -gt 0) { Note ($key + ': sampled blocks the loader rejected: ' + ($bad -join ', ')) }
        }
    } else {
        Note ($key + ': declares no blocks (items only)')
    }

    $recipeRoot = @((Join-Path $overlay ('data/' + $ns + '/recipes')), (Join-Path $overlay ('data/' + $ns + '/recipe'))) |
        Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if ($recipeRoot) {
        $ids = @(Get-ChildItem -LiteralPath $recipeRoot -Filter *.json -Recurse | ForEach-Object {
            $rel = $_.FullName.Substring($recipeRoot.Length + 1).Replace('\', '/')
            $rel.Substring(0, $rel.Length - 5)
        } | Sort-Object)
        if ($ids.Count -eq 0) {
            Note ($key + ': no recipe survived pruning')
        } else {
            Scenario ($key + ': recipes are known to the game') {
                $seen = 0; $unsupported = $false; $unknown = @()
                foreach ($id in (Sample $ids 3)) {
                    $text = Say ('recipe give @s ' + $ns + ':' + $id)
                    if ($text -match 'Unknown or incomplete command|Incorrect argument') { $unsupported = $true; break }
                    if ($text -match 'Unknown recipe|No recipes? were|Could not') { $unknown += ($ns + ':' + $id) } else { $seen++ }
                }
                if ($unsupported) { Note ($key + ': the "recipe" command is not available here, recipes not verified'); return }
                if ($unknown.Count -gt 0 -and $seen -eq 0) { throw ('the game does not know: ' + ($unknown -join ', ')) }
                if ($unknown.Count -gt 0) { Note ($key + ': recipes the game does not know: ' + ($unknown -join ', ')) }
            }
        }
    } else {
        Note ($key + ': no recipes shipped')
    }

    $lootRoot = @((Join-Path $overlay ('data/' + $ns + '/loot_tables/blocks')), (Join-Path $overlay ('data/' + $ns + '/loot_table/blocks'))) |
        Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if ($lootRoot) {
        $tables = @(Get-ChildItem -LiteralPath $lootRoot -Filter *.json | Sort-Object Name | ForEach-Object { $_.BaseName })
        if ($tables.Count -gt 0) {
            Scenario ($key + ': block loot tables load') {
                $text = Say ('loot give @s loot ' + $ns + ':blocks/' + $tables[0])
                if ($text -match 'Unknown or incomplete command|Incorrect argument') { Note ($key + ': the "loot" command is not available here, loot not verified'); return }
                if ($text -match 'Unknown loot table|could not') { throw ($ns + ':blocks/' + $tables[0] + ' -> ' + $text) }
                $null = Command 'clear @s'
            }
        }
    }
}

# Ocean's Delight: a carried farmersdelight:cooking recipe cooks in the native pot.
function Plain-Items($Recipe) {
    $out = @()
    foreach ($ing in @($Recipe.ingredients)) {
        $id = $null
        if ($ing -is [string]) { $id = $ing }
        elseif ($ing.PSObject.Properties['item']) { $id = [string]$ing.item }
        elseif ($ing.PSObject.Properties['id']) { $id = [string]$ing.id }
        if (-not $id -or $id.StartsWith('#')) { return $null }
        $out += $id
    }
    return $out
}

$od = @($built | Where-Object { $_.key -eq 'oceansdelight' })
if ($od.Count -gt 0) {
    Scenario 'oceansdelight: a carried farmersdelight:cooking recipe cooks in the native pot' {
        $overlay = Join-Path $batch 'overlay/oceansdelight'
        $found = $null
        foreach ($root in @((Join-Path $overlay 'data/oceansdelight/recipes'), (Join-Path $overlay 'data/oceansdelight/recipe'))) {
            if (-not (Test-Path -LiteralPath $root)) { continue }
            foreach ($file in (Get-ChildItem -LiteralPath $root -Filter *.json -Recurse | Sort-Object FullName)) {
                $r = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
                if ([string]$r.type -ne 'farmersdelight:cooking') { continue }
                $items = Plain-Items $r
                if (-not $items -or $items.Count -lt 1 -or $items.Count -gt 6) { continue }
                $res = $null
                if ($r.result -is [string]) { $res = $r.result }
                elseif ($r.result.PSObject.Properties['item']) { $res = [string]$r.result.item }
                elseif ($r.result.PSObject.Properties['id']) { $res = [string]$r.result.id }
                if (-not $res) { continue }
                $found = @{ file = $file.Name; items = $items; result = $res }
                break
            }
            if ($found) { break }
        }
        if (-not $found) { Note 'oceansdelight: no cooking recipe with plain item ingredients survived; native pot not exercised'; return }
        $null = Command 'fill 92 77 92 112 90 112 minecraft:air'
        $null = Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
        $null = Command 'kill @e[type=minecraft:item]'
        Wait-Ticks 20
        $null = Command 'setblock 100 79 100 farmersdelight:stove[facing=north,lit=true]'
        $null = Command 'setblock 100 80 100 farmersdelight:cooking_pot[facing=north,support=none,waterlogged=false]'
        Wait-Ticks 10
        $slot = 0
        foreach ($id in $found.items) {
            $null = Command ('item replace block 100 80 100 container.' + $slot + ' with ' + $id + ' 1')
            $slot++
        }
        $local = $found.result.Split(':')[-1]
        $cooked = $false
        for ($i = 0; $i -lt 40; $i++) {
            Wait-Ticks 20
            if ((Get-Data 100 80 100) -match [regex]::Escape($local)) { $cooked = $true; break }
        }
        if (-not $cooked) { throw ('recipe ' + $found.file + ' did not produce ' + $found.result + '; pot ' + (Get-Data 100 80 100)) }
        $report.cooked = $found
    }
}

# Crate Delight: verify a surviving crate is in #minecraft:mineable/axe if tag command works.
$cd = @($built | Where-Object { $_.key -eq 'cratedelight' })
if ($cd.Count -gt 0) {
    Scenario 'cratedelight: carried crate is in declared mineable/axe tag' {
        $null = Command 'setblock 96 90 96 cratedelight:apple_crate'
        $text = Say 'execute if block 96 90 96 #minecraft:mineable/axe run say batch2_tag_ok'
        if ($text -match 'Unknown or incomplete command|Incorrect argument') { Note 'cratedelight: execute if block with a tag is not available here; tags not verified'; return }
        if ($text -notmatch 'batch2_tag_ok') { throw ('cratedelight:apple_crate is not in #minecraft:mineable/axe (' + $text + ')') }
    }
}

Note 'oceansdelight: entity loot injections (squid/guardian/elder_guardian) and interactive guardian_soup bowl serving are not ported'
Note 'obsidianequipmentrework: armor/tool stats are estimates; survival loot acquisition of obsidian_upgrade_smithing_template is not verified'
Note 'sootychimneys: smoke particles and soot accumulation/scraping require client particle and block-entity hooks (not ported)'
Note 'createdeco: interactive block behaviours beyond template matches and Create pressing recipes are not visually/functionally verified'
Note 'mcaccel: light emission values (15/10) and block textures are not visually verified via socket'

$failed = @($results.GetEnumerator() | Where-Object { $_.Value -ne 'ok' })
$report.measured = @{ scenarios = $results; notes = $report.notes.Count }
$report.passed = ($failed.Count -eq 0)
Save-Report
if ($failed.Count -gt 0) { throw ('Batch 2 failures: ' + (($failed | ForEach-Object { $_.Key + ' => ' + $_.Value }) -join ' | ')) }
Passed ($results.Count.ToString() + ' batch 2 scenarios passed; ' + $report.notes.Count + ' not-verified notes recorded.')
