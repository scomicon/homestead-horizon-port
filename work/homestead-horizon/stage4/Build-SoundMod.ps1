#requires -Version 7.0
# Builds the small Homestead Sound content mod (the placeholder sound events homestead_sound:tone.*
# used to try and test the client sound mods) from tools/sound_content.py into a flat .hmod in
# stage4/extra-mods. The name does not match Build-DataMods.ps1's cleanup pattern ('*-horizon.*.hmod').
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$work = Join-Path $workspace 'work/homestead-horizon'
$create = (Get-Content -LiteralPath (Join-Path $work 'create-source-path.txt') -Raw).Trim()
$python = Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$env:PYTHONUTF8 = '1'
$out = Join-Path $work 'tmp/sound-mod'
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
& $python -I (Join-Path $create 'tools/sound_content.py') $out
if ($LASTEXITCODE -ne 0) { throw 'sound content failed' }
$extra = Join-Path $PSScriptRoot 'extra-mods'
New-Item -ItemType Directory -Force $extra | Out-Null
Remove-Item (Join-Path $extra 'homestead_sound-*.hmod') -Force -ErrorAction SilentlyContinue
$archive = Join-Path $extra 'homestead_sound-0.1.0-homestead.1.hmod'
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($out, $archive)
"Built $archive"
