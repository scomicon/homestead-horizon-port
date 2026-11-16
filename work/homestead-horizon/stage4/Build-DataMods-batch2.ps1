#requires -Version 7.0
# Batch 2 (stage 23): packs Ocean's Delight, Crate Delight, Obsidian Equipment Reworked,
# Minecraft Accelerated, Create Deco and Sooty Chimneys as flat content mods (.hmod).
# Built strictly from the player's OWN jars (pack/inventory.json); nothing third-party is
# downloaded, committed or redistributed.
#
#   pack-batch2/            work dir: copies of pack.json/modrinth.json/fetch.json + filtered inventory,
#                           mods/<key>/carry (horizon-port-pack carry), overlay/<key>
#   extra-mods/<id>-<ver>-batch2.1.hmod
#                           one flat archive per mod. Does NOT match Build-DataMods.ps1 ('*-horizon.*.hmod')
#                           or Build-Batch1.ps1 ('*-batch1.*.hmod'), so scripts do not delete each other.
#
# Usage: pwsh Build-DataMods-batch2.ps1 [-Resources <prepared 26.3 resources>] [-Only key,key] [-NoNonPublic] [-DryRun]
[CmdletBinding()]
param(
    [string] $Resources,
    [string[]] $Only,
    [string] $Version = 'batch2.1',
    [switch] $NoNonPublic,
    [switch] $DryRun
)
$ErrorActionPreference = 'Stop'
if ($Only) { $Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ }) }
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$work = Join-Path $workspace 'work/homestead-horizon'
$create = (Get-Content -LiteralPath (Join-Path $work 'create-source-path.txt') -Raw).Trim()
$pack = Get-ChildItem (Join-Path $workspace 'work/horizon-source') -Directory | Select-Object -First 1
$tool = if ($pack) { Join-Path $pack.FullName 'target/fast/horizon-port-pack.exe' } else { '' }
$python = Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$env:PYTHONUTF8 = '1'
$policyFile = Join-Path $PSScriptRoot 'batch2-policy.json'
$policy = Get-Content -LiteralPath $policyFile -Raw | ConvertFrom-Json
$batch = Join-Path $work 'pack-batch2'
$extra = Join-Path $PSScriptRoot 'extra-mods'

# --- which mods ------------------------------------------------------------------------------------------
$mods = @($policy.mods | Where-Object { (-not $Only) -or ($Only -contains $_.key) })
if ($NoNonPublic) { $mods = @($mods | Where-Object { $_.publicRelease }) }
$carryMods = @($mods | Where-Object { $_.mode -eq 'carry' })
Write-Output ('Batch 2: ' + (($mods | ForEach-Object { $_.key }) -join ', '))
if ($DryRun) { Write-Output 'Dry run: nothing built.'; return }

# --- resources (prepared Horizon 26.3 resources with facts.json inside) ---------------------------------
if (-not $Resources) {
    $settings = Join-Path $workspace 'local-settings.json'
    if (Test-Path -LiteralPath $settings) { $Resources = (Get-Content -LiteralPath $settings -Raw | ConvertFrom-Json).resources }
}
if (-not $Resources) {
    $launcher = Join-Path $env:APPDATA 'horizon-launcher/versions'
    if (Test-Path -LiteralPath $launcher) {
        $Resources = Get-ChildItem -LiteralPath $launcher -Directory |
            ForEach-Object { Join-Path $_.FullName '.local/horizon-client/26.3' } |
            Where-Object { Test-Path -LiteralPath (Join-Path $_ 'facts.json') } |
            Sort-Object { (Get-Item -LiteralPath $_).LastWriteTime } -Descending | Select-Object -First 1
    }
}
if (-not $Resources -or -not (Test-Path -LiteralPath (Join-Path $Resources 'facts.json'))) {
    throw 'Pass -Resources <folder with facts.json> (prepared Horizon 26.3 resources) or set "resources" in local-settings.json.'
}
if (-not $tool -or -not (Test-Path -LiteralPath $tool)) {
    throw 'horizon-port-pack.exe is not built (work/horizon-source/*/target/fast).'
}

# --- carry (own work dir, so pack/mods and pack-batch1 stay untouched) -----------------------------------
New-Item -ItemType Directory -Force $batch | Out-Null
foreach ($f in 'pack.json', 'modrinth.json', 'fetch.json') {
    $src = Join-Path $work "pack/$f"
    if (Test-Path -LiteralPath $src) { Copy-Item $src $batch -Force }
}
$inventory = Get-Content (Join-Path $work 'pack/inventory.json') -Raw | ConvertFrom-Json -NoEnumerate
$wanted = @($carryMods | ForEach-Object { $_.key }) + @('farmersdelight', 'create', 'expandeddelight')
$missing = @($carryMods | Where-Object { $key = $_.key; -not ($inventory | Where-Object { $_.key -eq $key }) })
if ($missing.Count -gt 0) { throw ('Not in pack/inventory.json: ' + (($missing | ForEach-Object { $_.key }) -join ', ')) }
$keep = @($inventory | Where-Object { ($wanted -contains $_.key) -and -not ($_.key -eq 'farmersdelight' -and $_.version -like '1.20.1-1.4.3*') })
[IO.File]::WriteAllText((Join-Path $batch 'inventory.json'), (ConvertTo-Json -InputObject $keep -Depth 30), [Text.UTF8Encoding]::new($false))
Remove-Item (Join-Path $batch 'mods') -Recurse -Force -ErrorAction SilentlyContinue
if ($carryMods.Count -gt 0) {
    & $tool carry $batch --resources $Resources @($carryMods | ForEach-Object { $_.key })
    if ($LASTEXITCODE -ne 0) { throw 'carry failed' }
}

# --- namespaces available in the port (including batch1 carried mods if built) ---------------------------
$available = [System.Collections.Generic.HashSet[string]]::new()
foreach ($n in 'minecraft', 'create', 'farmersdelight', 'homestead', 'c') { [void]$available.Add($n) }
foreach ($root in @((Join-Path $work 'pack/mods'), (Join-Path $work 'pack-batch1/mods'), (Join-Path $batch 'mods'))) {
    foreach ($carry in Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue | ForEach-Object { Join-Path $_.FullName 'carry' }) {
        foreach ($sub in 'data', 'assets') {
            foreach ($ns in Get-ChildItem -LiteralPath (Join-Path $carry $sub) -Directory -ErrorAction SilentlyContinue) { [void]$available.Add($ns.Name) }
        }
    }
}
$availableText = ($available | Sort-Object) -join ','

# --- overlay + creative tab + playtest fixes + flat archive ---------------------------------------------
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Force $extra | Out-Null
Get-ChildItem $extra -Filter "*-$Version.hmod" -ErrorAction SilentlyContinue | Remove-Item -Force
$tabs = Join-Path $work 'tmp/batch2-tabs'
$summary = @()
foreach ($mod in $carryMods) {
    $carry = Join-Path $batch ("mods/$($mod.key)/carry")
    $toml = Join-Path $carry 'horizon-mod.toml'
    if (-not (Test-Path -LiteralPath $toml)) {
        Write-Warning ("carry produced nothing for $($mod.key); skipped.")
        $summary += [ordered]@{ key = $mod.key; built = $false; reason = 'no carry output' }
        continue
    }
    $id = ((Get-Content $toml | Where-Object { $_ -match '^id = "(.+)"' }) | Select-Object -First 1) -replace '^id = "|"$', ''
    $ver = ((Get-Content $toml | Where-Object { $_ -match '^version = "(.+)"' }) | Select-Object -First 1) -replace '^version = "|"$', ''
    if ($id -match '-') { throw "Mod id '$id' contains '-': Start-Homestead.ps1 derives the enable key up to the first '-'." }
    $safe = ($ver -replace '[^A-Za-z0-9._]+', '_').Trim('_')
    $overlay = Join-Path $batch "overlay/$($mod.key)"
    $reportFile = Join-Path $batch "overlay-$($mod.key).json"
    & $python -I (Join-Path $create 'tools/batch2_content.py') overlay --carry $carry --out $overlay --key $mod.key --policy $policyFile --available $availableText --report $reportFile | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "overlay failed for $($mod.key)" }

    Remove-Item $tabs -Recurse -Force -ErrorAction SilentlyContinue
    & $python -I (Join-Path $create 'tools/creative_tab.py') --out $tabs $overlay | Out-Null
    $dropFix = @(& $python -I (Join-Path $create 'tools/hide_unplaceable.py') --out $tabs $overlay |
        Where-Object { $_ -like 'DROP *' } | ForEach-Object { $_.Substring(5) })
    & $python -I (Join-Path $create 'tools/advancement_fix.py') --out $tabs $overlay | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "advancement_fix failed for $id" }
    & $python -I (Join-Path $create 'tools/block_looks.py') --out $tabs $overlay | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "block_looks failed for $id" }

    foreach ($relDrop in $dropFix) {
        $p = Join-Path $overlay ($relDrop.Replace('/', '\'))
        if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Force }
    }
    if (Test-Path $tabs) {
        foreach ($g in Get-ChildItem $tabs -Recurse -File) {
            $rel = $g.FullName.Substring((Resolve-Path $tabs).Path.TrimEnd('\').Length + 1)
            $target = Join-Path $overlay $rel
            New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
            Copy-Item -LiteralPath $g.FullName -Destination $target -Force
        }
    }

    $archive = Join-Path $extra "$id-$safe-$Version.hmod"
    $zip = [IO.Compression.ZipFile]::Open($archive, 'Create')
    try {
        $root = (Resolve-Path $overlay).Path.TrimEnd('\') + '\'
        foreach ($file in Get-ChildItem $overlay -Recurse -File) {
            [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $file.FullName.Substring($root.Length).Replace('\', '/'), 'Optimal')
        }
    } finally { $zip.Dispose() }

    $report = Get-Content $reportFile -Raw | ConvertFrom-Json
    $summary += [ordered]@{
        key = $mod.key
        id = $id
        archive = (Split-Path -Leaf $archive)
        built = $true
        publicRelease = [bool]$mod.publicRelease
        licence = $mod.licence
        dropped_recipes = @($report.dropped_recipes).Count
        dropped_files = @($report.dropped_files).Count
        foods = @($report.foods).Count
        armor = @($report.armor).Count
        tools = @($report.tools).Count
        lights = @($report.lights).Count
        feasts = @($report.feasts).Count
    }
    Write-Output ("Built $archive")
}

[IO.File]::WriteAllText((Join-Path $batch 'batch2-summary.json'), (ConvertTo-Json -InputObject $summary -Depth 6), [Text.UTF8Encoding]::new($false))
Write-Output ("Batch 2 done: " + (@($summary | Where-Object { $_.built }).Count) + " built; summary in " + (Join-Path $batch 'batch2-summary.json'))
