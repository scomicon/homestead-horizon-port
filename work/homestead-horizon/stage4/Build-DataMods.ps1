#requires -Version 7.0
# Packs the already-carried data-tier mods (work/homestead-horizon/pack/mods/<key>/carry) into flat
# .hmod archives in stage4/extra-mods, one per mod, honouring stage4/data-mods-skip.json
# (per-mod list of relative paths dropped from the archive, with reasons).
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$mods = Join-Path $workspace 'work/homestead-horizon/pack/mods'
$extra = Join-Path $PSScriptRoot 'extra-mods'
$skipFile = Join-Path $PSScriptRoot 'data-mods-skip.json'
$version = 'horizon.2'
New-Item -ItemType Directory -Force $extra | Out-Null
$create = (Get-Content -LiteralPath (Join-Path $workspace 'work/homestead-horizon/create-source-path.txt') -Raw).Trim()
$python = Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$env:PYTHONUTF8 = '1'
$tabs = Join-Path $workspace 'work/homestead-horizon/tmp/data-mod-tabs'
$skip = @{}
if (Test-Path $skipFile) {
    $json = Get-Content $skipFile -Raw | ConvertFrom-Json
    foreach ($p in $json.PSObject.Properties) { $skip[$p.Name] = @($p.Value.paths) }
}
Get-ChildItem $extra -Filter '*-horizon.*.hmod' | Where-Object { $_.Name -notlike 'farmersdelight-*' } | Remove-Item -Force
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$built = @()
foreach ($dir in Get-ChildItem $mods -Directory | Sort-Object Name) {
    $carry = Join-Path $dir.FullName 'carry'
    $toml = Join-Path $carry 'horizon-mod.toml'
    if (-not (Test-Path $toml)) { continue }
    $id = ((Get-Content $toml | Where-Object { $_ -match '^id = "(.+)"' }) | Select-Object -First 1) -replace '^id = "|"$', ''
    $ver = ((Get-Content $toml | Where-Object { $_ -match '^version = "(.+)"' }) | Select-Object -First 1) -replace '^version = "|"$', ''
    $safe = ($ver -replace '[^A-Za-z0-9._]+', '_').Trim('_')
    $archive = Join-Path $extra "$id-$safe-$version.hmod"
    $drop = @($skip[$id])
    # Creative tab + title (tools/creative_tab.py): generated beside the carry, added over it.
    Remove-Item $tabs -Recurse -Force -ErrorAction SilentlyContinue
    & $python -I (Join-Path $create 'tools/creative_tab.py') --out $tabs $carry | Out-Null
    $over = @{}
    if (Test-Path $tabs) { foreach ($g in Get-ChildItem $tabs -Recurse -File) { $over[$g.FullName.Substring((Resolve-Path $tabs).Path.TrimEnd('\').Length + 1).Replace('\','/')] = $g.FullName } }
    $zip = [IO.Compression.ZipFile]::Open($archive, 'Create')
    try {
        $root = (Resolve-Path $carry).Path.TrimEnd('\') + '\'
        foreach ($file in Get-ChildItem $carry -Recurse -File) {
            $rel = $file.FullName.Substring($root.Length).Replace('\', '/')
            if ($drop -contains $rel -or $over.ContainsKey($rel)) { continue }
            [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $rel, 'Optimal')
        }
        foreach ($rel in $over.Keys) { [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $over[$rel], $rel, 'Optimal') }
    } finally { $zip.Dispose() }
    $built += $id
}
"Built $($built.Count) data mods in $extra"
