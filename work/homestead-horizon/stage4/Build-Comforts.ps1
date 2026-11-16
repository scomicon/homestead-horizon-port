#requires -Version 7.0
# Builds the small Comforts content mod (32 portable-bed blocks and items: comforts:sleeping_bag_<colour> and
# comforts:hammock_<colour>) from tools/comforts_content.py into a flat .hmod in stage4/extra-mods. The native
# behaviour (no respawn point from these beds) is mixins/homestead/comforts.rs. The name does not match
# Build-DataMods.ps1's cleanup pattern ('*-horizon.*.hmod').
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$work = Join-Path $workspace 'work/homestead-horizon'
$create = (Get-Content -LiteralPath (Join-Path $work 'create-source-path.txt') -Raw).Trim()
$python = Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$env:PYTHONUTF8 = '1'
$out = Join-Path $work 'tmp/comforts-mod'
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
& $python -I (Join-Path $create 'tools/comforts_content.py') $out
if ($LASTEXITCODE -ne 0) { throw 'comforts content failed' }
$extra = Join-Path $PSScriptRoot 'extra-mods'
New-Item -ItemType Directory -Force $extra | Out-Null
Remove-Item (Join-Path $extra 'comforts-*.hmod') -Force -ErrorAction SilentlyContinue
$archive = Join-Path $extra 'comforts-0.1.0-homestead.1.hmod'
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($out, $archive)
"Built $archive"
