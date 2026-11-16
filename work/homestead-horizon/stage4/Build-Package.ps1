#requires -Version 7.0
# Assembles outputs/<Name> from the accepted candidate client.
# Refuses to run unless every acceptance receipt passed on the candidate binary.
# -AudioWaiver: accept a ClientS receipt that failed ONLY for lack of an audio output device;
# the package is then marked sound-unverified (provenance + SOUND-NOT-VERIFIED.ru.md).
param([string] $Name = 'homestead-horizon-stage22', [switch] $AudioWaiver)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$work = Join-Path $workspace 'work/homestead-horizon'
$candidate = Join-Path $PSScriptRoot 'candidate/client'
$final = Join-Path $workspace ('outputs/' + $Name)
$stage3 = @('stage3','stage18','stage17') | ForEach-Object { Join-Path $workspace ('outputs/homestead-horizon-'+$_) } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $stage3) { throw 'No base package (stage3, stage18 or stage17) found in outputs.' }
$stage3IsBase = (Split-Path $stage3 -Leaf) -eq 'homestead-horizon-stage3'
$create = (Get-Content -LiteralPath (Join-Path $work 'create-source-path.txt') -Raw).Trim()
$git = Join-Path $workspace 'work/mingit/cmd/git.exe'
$utf8 = [Text.UTF8Encoding]::new($false)
if (Test-Path -LiteralPath $final) { throw 'The output already exists; preserve it.' }
if (-not $final.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output escaped the workspace.' }

# 1. Receipts must belong to this very binary.
$binary = (Get-FileHash -LiteralPath (Join-Path $candidate 'horizon-client.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
$runs = @('run-flow-result.json','run-reload-result.json','run-vault-result.json','run-vault-reload-result.json','run-harvest-result.json','run-tweaks-result.json','run-rain-result.json','run-days-result.json','run-fd-result.json','run-decor-result.json','run-arm-result.json','run-points-result.json','run-link-result.json','run-tank-result.json','run-pipes-result.json','run-steam-result.json','run-board-result.json','run-stove-result.json','run-skillet-result.json','run-pot-result.json','run-switches-result.json','run-displays-result.json','run-cannon-result.json','run-packager-result.json','run-controller-result.json','run-schematic-tools-result.json','run-stock-result.json','run-crops-result.json','run-feasts-result.json','run-datamods-result.json','run-motor-result.json','run-clientdefaults-result.json','run-logica-result.json','run-logicb-result.json','run-logicd-result.json','run-clientt-result.json','run-clienth-result.json','run-clients-result.json','run-batch1-result.json','run-engine20-result.json','run-fixes2-result.json')
$audioWaived = $false
foreach ($name in $runs) {
    $r = Get-Content -LiteralPath (Join-Path $PSScriptRoot $name) -Raw | ConvertFrom-Json
    if ($name -eq 'run-clients-result.json' -and $AudioWaiver -and -not $r.passed) {
        $err = [string]$r.error
        if ($r.binary_sha256 -ne $binary) { throw 'ClientS receipt belongs to another client binary.' }
        if (@($r.phases) -notcontains 'ClientS') { throw 'AudioWaiver needs a passed ClientS phase on this binary.' }
        if ($err -notmatch 'no audio output device') { throw "AudioWaiver covers only a missing audio output device, not: $err" }
        $audioWaived = $true
        Write-Warning 'ClientS Playback waived (no audio output device); the package is marked sound-unverified.'
        continue
    }
    if (-not $r.passed) { throw "Receipt did not pass: $name" }
    if ($r.binary_sha256 -ne $binary) { throw "Receipt $name belongs to another client binary." }
    if ($name -eq 'run-clients-result.json') {
        # Older same-binary ClientS receipts only checked configuration/submission.
        if (@($r.phases).Count -ne 2 -or $r.phases -notcontains 'ClientS' -or $r.phases -notcontains 'Playback') {
            throw 'ClientS receipt lacks the mandatory live Playback phase; rerun -ClientS with audio output available.'
        }
        $playback=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'runtime/game/stage33-soundplayback-playback-result.json') -Raw | ConvertFrom-Json
        if (-not $playback.passed -or -not $playback.playback_hook_verified -or $playback.binary_sha256 -ne $binary -or $playback.process_id -ne $r.process_id) {
            throw 'Live sound-playback proof is missing, failed, or belongs to another client run.'
        }
    }
}

New-Item -ItemType Directory -Path $final | Out-Null
foreach ($d in 'client','mods','datapacks','review','acceptance','source','validation') { New-Item -ItemType Directory -Path (Join-Path $final $d) | Out-Null }

# 2. Client.
Copy-Item -LiteralPath (Join-Path $candidate 'horizon-client.exe') (Join-Path $final 'client')
Copy-Item -LiteralPath (Join-Path $candidate 'horizon-ctl.exe') (Join-Path $final 'client')
Copy-Item -LiteralPath (Join-Path $candidate 'native-data') (Join-Path $final 'client') -Recurse

# 3. Bundles: flat archives like the stage 3 one, plus their digests for native-mods.json.
function Get-BundleFiles([string] $Dir) {
    $items = @(Get-ChildItem -LiteralPath $Dir -Recurse -File -Force |
        Where-Object { $_.FullName -notmatch '\\(target)\\' -and $_.Name -ne 'Cargo.lock' } |
        ForEach-Object { [pscustomobject]@{ Name = [IO.Path]::GetRelativePath($Dir, $_.FullName).Replace('\','/'); Path = $_.FullName } })
    # Like the stage 3 archive, the bundle ships the licence and notices at its root.
    $items += [pscustomobject]@{ Name = 'LICENSE'; Path = (Join-Path $create 'LICENSE') }
    $items += [pscustomobject]@{ Name = 'NOTICE.md'; Path = (Join-Path $create 'THIRD_PARTY_NOTICES.md') }
    $names = [string[]]@($items | ForEach-Object { $_.Name })
    $order = [string[]]@($names)
    [Array]::Sort($order, [StringComparer]::Ordinal)
    foreach ($n in $order) { $items | Where-Object { $_.Name -ceq $n } | Select-Object -First 1 }
}
# Launcher digest (verified against the stage 3 archive): over every archive file in
# ordinal name order, name bytes, u64 LE length, bytes.
function Get-Digest($Files) {
    $hash = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
    foreach ($f in $Files) {
        $bytes = [IO.File]::ReadAllBytes($f.Path)
        $hash.AppendData($utf8.GetBytes($f.Name))
        $hash.AppendData([BitConverter]::GetBytes([uint64]$bytes.LongLength))
        $hash.AppendData($bytes)
    }
    [Convert]::ToHexString($hash.GetHashAndReset()).ToLowerInvariant()
}
$mods = @()
foreach ($id in 'create', 'homestead') {
    $dir = Join-Path $create "mixins/$id"
    $toml = Get-Content -LiteralPath (Join-Path $dir 'horizon-mixin.toml') -Raw
    $version = [regex]::Match($toml, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    $files = @(Get-BundleFiles $dir)
    $mods += [ordered]@{ sha256 = (Get-Digest $files); version = $version; id = $id }
    $archive = Join-Path $final "source/$id-homestead-$version.hnmod"
    Add-Type -AssemblyName System.IO.Compression
    $zip = [IO.Compression.ZipFile]::Open($archive, 'Create')
    try {
        foreach ($f in $files) {
            $entry = $zip.CreateEntry($f.Name, 'Optimal'); $entry.LastWriteTime = [DateTimeOffset]::new(2026, 11, 3, 0, 0, 0, [TimeSpan]::Zero)
            $s = $entry.Open(); $b = [IO.File]::ReadAllBytes($f.Path); $s.Write($b, 0, $b.Length); $s.Dispose()
        }
    } finally { $zip.Dispose() }
}$base = Get-Content -LiteralPath (Join-Path $stage3 'client/native-mods.json') -Raw | ConvertFrom-Json -AsHashtable
$base.mods = $mods
[IO.File]::WriteAllText((Join-Path $final 'client/native-mods.json'), ($base | ConvertTo-Json -Depth 6) + [Environment]::NewLine, $utf8)

# 4. Source archive of the repository (tracked files only), licences, notices.
$srcZip = Join-Path $final 'source/homestead-source.zip'
Push-Location $workspace
try { & $git archive --format=zip --output=$srcZip HEAD work/homestead-horizon/create-source work/homestead-horizon/stage4 docs 2>$null; if ($LASTEXITCODE -ne 0) { & $git archive --format=zip --output=$srcZip HEAD work/homestead-horizon/create-source work/homestead-horizon/stage4 } } finally { Pop-Location }
foreach ($n in 'CREATE-HORIZON-LICENSE.txt','HORIZON-LICENSE.txt') { Copy-Item -LiteralPath (Join-Path $stage3 "source/$n") (Join-Path $final 'source') }
Copy-Item -LiteralPath (Join-Path $create 'THIRD_PARTY_NOTICES.md') (Join-Path $final 'source/CREATE-HORIZON-NOTICES.md')

# 5. Launcher pieces carried over from stage 3.
if ($stage3IsBase) { Copy-Item -Path (Join-Path $stage3 'mods/*') (Join-Path $final 'mods') -Recurse } else { Copy-Item -Path (Join-Path $stage3 'mods/create_oxidized-*.hmod') (Join-Path $final 'mods') }
# Content mods built for this stage (Farmer's Delight); only what is accepted ships.
& (Join-Path $PSScriptRoot 'Build-SoundMod.ps1') | Out-Null
Copy-Item -Path (Join-Path $PSScriptRoot 'extra-mods/*.hmod') (Join-Path $final 'mods')
Copy-Item -Path (Join-Path $stage3 'datapacks/*') (Join-Path $final 'datapacks') -Recurse
# Batch 1: SparseStructures data pack (generated by Build-Batch1.ps1 from the player's own vanilla resources).
$batch1Packs = Join-Path $PSScriptRoot '../pack-batch1/datapacks'
if (Test-Path -LiteralPath $batch1Packs) { Copy-Item -Path (Join-Path $batch1Packs '*') (Join-Path $final 'datapacks') -Recurse }
Copy-Item -LiteralPath (Join-Path $stage3 'review/world.zip') (Join-Path $final 'review')
# No local-settings.json is shipped (it holds the user's own paths); Install-Homestead.cmd writes it.
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'package/local-settings.example.json') $final
foreach ($f in 'Install-Homestead.ps1','Install-Homestead.cmd','Apply-ClientDefaults.ps1') { Copy-Item -LiteralPath (Join-Path $PSScriptRoot ('package/' + $f)) $final }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'package/Play-Create.cmd') $final
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'package/Start-Homestead.ps1') $final

# 6. Acceptance harness and receipts.
foreach ($f in 'Run-Acceptance.ps1','Start-Test.ps1','Prepare-Candidate.ps1','Build-Package.ps1','Build-Cache.ps1') { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $f) (Join-Path $final 'acceptance') }
Copy-Item -Path (Join-Path $PSScriptRoot 'acceptance/Test-*.ps1') (Join-Path $final 'acceptance')
foreach ($name in $runs) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) (Join-Path $final 'validation') }
foreach ($f in Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'runtime/game') -Filter 'stage*-result.json') { Copy-Item -LiteralPath $f.FullName (Join-Path $final 'validation') }
foreach ($n in 'create-engine-tests.log') { if (Test-Path (Join-Path $PSScriptRoot $n)) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $n) (Join-Path $final 'validation') } }

# 7. Provenance.
$provenance = Get-Content -LiteralPath (Join-Path $stage3 'provenance.json') -Raw | ConvertFrom-Json -AsHashtable
$provenance.build.mods = $mods
$provenance.game_sha256 = $binary
$provenance.source_digest = ($mods | Where-Object { $_.id -eq 'create' }).sha256
$provenance.local_changes = @(
    'Exact Homestead Fabric JAR resource hash profile; Spout, Item Drain, pumps and ancient debris crushing (stages 2-3)',
    'Andesite Belt Funnel on horizontal Belts, placed by the ordinary Funnel item (stage 4)',
    'Item Vault: exact-cuboid multiblock sharing one inventory through the container view, with Chute, Belt Funnel and comparator support (stage 5)',
    'homestead bundle: Right Click Harvest, Secure Crops, Too Cheap!, pack rule defaults (stage 6)',
    'homestead bundle: Crops Love Rain and Better Days with the pack configs (stage 7)',
    'Farmer''s Delight 2.4.1 content mod: blocks as shells, foods, drinks, knives, nourishment effect (stage 8)',
    'Create: 350 decorative blocks on vanilla templates; local engine patch for scaffolding collision (stage 9)',
    'Create: Mechanical Arm with point selection and transfers for 14 point types (stage 10)',
    'Create: Redstone Link with wireless frequencies and analog output (stage 11)',
    'Create: Fluid Tank multiblock, width 3 and height 32 with a pooled controller (stage 12)',
    'Create: general fluid pipe network with branching, several tanks, open-end sources and glass/encased/smart pipe blocks (stage 13)',
    'Create: Boiler on the multiblock tank, Steam Engine, Powered Shaft kinetic source and Steam Whistle with extenders (stage 14)',
    'Farmer''s Delight Cutting Board, Stove and Skillet as native behaviour in the homestead bundle (stage 15-16)',
    'Farmer''s Delight Cooking Pot: ingredients by right click, heated cooking by the farmersdelight:cooking recipes, servings by container (stage 17)',
    'Farmer''s Delight Cooking Pot screen: mod menu type, 9-slot block entity container, native screen with progress arrow and heat icon (stage 18)',
    'Create: redstone switches (Pulse Repeater/Extender/Timer, Powered Latch, Toggle Latch, Analog Lever, Redstone Contact), Nixie Tube, Link frequency reset (stage 19)',
    'Create: Display Link and Display Board with time, redstone, item, fluid and kinetic sources (stage 20)',
    'Create: Schematicannon, Schematic Table, Schematic items and Material Checklist, placement by operator command (stage 21)',
    'Create: Packager with packages and Mechanical Arm point, Factory Gauge as storage monitor (stage 22)',
    'Create: Linked Controller with link binding and key/command presses (stage 23)',
    'Create: Schematic and Quill selection and export, Schematic item deploy commands (stage 21)',
    'Create: Stock Link, Stock Ticker (commands), Package Frogport and Postbox over a virtual network (stage 23)',
    'Farmer''s Delight: crops (cabbage, onion, tomato, rice), rich soil, organic compost (stage 24)',
    'Farmer''s Delight: feasts, pies, drinks, Comfort effect (stage 24)',
'Create: value board admission fix (Creative Motor speed), original-asset import fix, creative tabs for all content mods (stage 31)',
    '45 carried data-tier mods packed as .hmod and loaded by the client (stage 25)',
    'Homestead logic A: Double Doors, Shulker Drops Two, Invisible Frames, Accelerated Decay, No Animal Tempt Delay (stage 25)',
    'Homestead logic B: Falling Trees (stage 25)',
    'Homestead logic D: Universal Graves, SpawnFilter (stage 25)',
    'Homestead client defaults: built-in dynamic lights, Mouse Tweaks and 3D skin layers switched on for new instances (stage 32)',
    'Homestead client tooltips: enchantment descriptions, enchantment info, rarity frames (stage 26)',
    'Homestead client HUD: Detail Armor Bar, Better Mount HUD, Traveler''s Titles, Welcome Screen (stage 26)',
    'Homestead client sound: Extreme Sound Muffler, Presence Footsteps, AmbientSounds engines, placeholder tones only (stage 26)',
    'Batch 1 data mods: Expanded Delight, Farmer''s Knives, Mo Glass, Perfect Homestead Plushies, Spud''s Shops, Go Fish (items only, fishing not ported), Nether''s Delight (data half), SparseStructures data pack for new worlds; food and tool values estimated (stage 27)',
    'Engine patches (sleep hooks for the single-player host, bed by block state, Peaceful Hunger switch, grindstone result take) with Comforts, Peaceful Hunger and Grind Enchantments (stage 27)',
    'Playtest fixes 2: advancement predicates (no structure advancements at once in new worlds), block occlusion and collision for small-model blocks, hidden modded boats (stage 22)'
)
$provenance.sound_playback_verified = -not $audioWaived
if ($audioWaived) {
    $provenance.sound_playback_waiver = 'Build machine has no audio output device (WASAPI). ClientS configuration phase passed on this binary; the live Playback phase did not run. Client sound mods are unverified in game.'
    $note = "# Звук не проверен`n`nПакет собран с флагом -AudioWaiver: на машине сборки нет аудиовыхода, поэтому фаза Playback набора -ClientS не запускалась.`nКлиентские звуковые моды (Extreme Sound Muffler, Presence Footsteps, AmbientSounds) в игре не подтверждены.`nПроверка: pwsh acceptance/Run-Acceptance.ps1 -ClientS на машине с колонками или наушниками.`n"
    [IO.File]::WriteAllText((Join-Path $final 'SOUND-NOT-VERIFIED.ru.md'), $note, [Text.UTF8Encoding]::new($true))
}
[IO.File]::WriteAllText((Join-Path $final 'provenance.json'), ($provenance | ConvertTo-Json -Depth 8) + [Environment]::NewLine, $utf8)
Write-Output "Assembled $final"
