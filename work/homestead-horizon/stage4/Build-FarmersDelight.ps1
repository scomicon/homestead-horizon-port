#requires -Version 7.0
# Rebuilds the Farmer's Delight content mod from the player's own Refabricated jar:
# horizon-port-pack carry -> fd/overlay.py (components) -> a flat .hmod in stage4/extra-mods.
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$work = Join-Path $workspace 'work/homestead-horizon'
$create = (Get-Content -LiteralPath (Join-Path $work 'create-source-path.txt') -Raw).Trim()
$pack = Get-ChildItem (Join-Path $workspace 'work/horizon-source') -Directory | Select-Object -First 1
$tool = Join-Path $pack.FullName 'target/fast/horizon-port-pack.exe'
$python = Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$settingsFile = @(Get-ChildItem (Join-Path $workspace 'outputs') -Directory -Filter 'homestead-horizon-stage*' | Sort-Object Name -Descending | ForEach-Object { Join-Path $_.FullName 'local-settings.json' } | Where-Object { Test-Path -LiteralPath $_ })[0]
$resources = (Get-Content -LiteralPath $settingsFile -Raw | ConvertFrom-Json).resources
$version = 'horizon.10'
$fd = Join-Path $work 'fd-carry'
New-Item -ItemType Directory -Force $fd | Out-Null
foreach ($f in 'pack.json', 'modrinth.json', 'fetch.json') { Copy-Item (Join-Path $work "pack/$f") $fd -Force }
# The pack holds two jars with the id farmersdelight; Fabric loads the newer, 2.4.1.
$inventory = Get-Content (Join-Path $work 'pack/inventory.json') -Raw | ConvertFrom-Json -NoEnumerate
$keep = @($inventory | Where-Object { -not ($_.key -eq 'farmersdelight' -and $_.version -like '1.20.1-1.4.3*') })
[IO.File]::WriteAllText((Join-Path $fd 'inventory.json'), (ConvertTo-Json -InputObject $keep -Depth 30), [Text.UTF8Encoding]::new($false))
Remove-Item (Join-Path $fd 'mods') -Recurse -Force -ErrorAction SilentlyContinue
& $tool carry $fd --resources $resources farmersdelight
if ($LASTEXITCODE -ne 0) { throw 'carry failed' }
$env:PYTHONUTF8 = '1'
$overlay = Join-Path $work 'fd-overlay'
& $python -I (Join-Path $create 'fd/overlay.py') (Join-Path $fd 'mods/farmersdelight/carry') $overlay
if ($LASTEXITCODE -ne 0) { throw 'overlay failed' }
& $python -I (Join-Path $create 'tools/creative_tab.py') $overlay
if ($LASTEXITCODE -ne 0) { throw 'creative tab failed' }
$extra = Join-Path $PSScriptRoot 'extra-mods'
New-Item -ItemType Directory -Force $extra | Out-Null
Remove-Item (Join-Path $extra 'farmersdelight-*.hmod') -Force -ErrorAction SilentlyContinue
$archive = Join-Path $extra "farmersdelight-2.4.1-$version.hmod"
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($overlay, $archive)
"Built $archive"