#requires -Version 7.0
# Builds the CraftingPad content mod (tools/craftingpad_content.py) into a flat .hmod in
# stage4/extra-mods. The mod's behaviour is native (mixins/homestead/craftingpad.rs).
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$work = Join-Path $workspace 'work/homestead-horizon'
$create = (Get-Content -LiteralPath (Join-Path $work 'create-source-path.txt') -Raw).Trim()
$python = Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$env:PYTHONUTF8 = '1'
$version = 'horizon.1'
$tree = Join-Path $work 'tmp/craftingpad-mod'
Remove-Item $tree -Recurse -Force -ErrorAction SilentlyContinue
& $python -I (Join-Path $create 'tools/craftingpad_content.py') $tree
if ($LASTEXITCODE -ne 0) { throw 'craftingpad content failed' }
$extra = Join-Path $PSScriptRoot 'extra-mods'
New-Item -ItemType Directory -Force $extra | Out-Null
Remove-Item (Join-Path $extra 'craftingpad-*.hmod') -Force -ErrorAction SilentlyContinue
$archive = Join-Path $extra "craftingpad-1.0.12-$version.hmod"
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($tree, $archive)
"Built $archive"
