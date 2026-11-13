$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
. (Join-Path $workspace 'work/rust-env.ps1')
$env:TEMP=Join-Path $workspace 'work/homestead-horizon/tmp'
$env:TMP=$env:TEMP
$create=(Get-Content -LiteralPath (Join-Path $PSScriptRoot '../create-source-path.txt') -Raw).Trim()
$engine=(Get-Content -LiteralPath (Join-Path $PSScriptRoot '../engine-source-path.txt') -Raw).Trim()
$mixins=(Get-Content -LiteralPath (Join-Path $PSScriptRoot '../mixins-source-path.txt') -Raw).Trim()
$stage=Join-Path $workspace 'work/homestead-horizon/staged-engine'
$python=Join-Path $workspace 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
$env:PYTHONUTF8='1'
& $python (Join-Path $create 'tools/content.py')
if ($LASTEXITCODE -ne 0) { throw 'Content generation failed.' }
& (Join-Path $workspace 'work/toolchain/bin/rustfmt.exe') --edition 2024 (Join-Path $create 'mixins/create/spout.rs')
if ($LASTEXITCODE -ne 0) { throw 'Spout formatting failed.' }

# Only timestamps of byte-identical staged inputs are restored. This preserves
# Cargo's cache without retaining stale transformed code or native data.
$prior=@{}
if (Test-Path -LiteralPath $stage) {
    foreach ($file in Get-ChildItem -LiteralPath $stage -Recurse -File) {
        $prior[$file.FullName.Substring($stage.Length+1)]=@{len=$file.Length;time=$file.LastWriteTimeUtc;hash=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash}
    }
    # The pinned official tool supports replacing an existing stage only on
    # Linux. Archive this owned stage, then create a fresh stage at the SAME
    # absolute path so Cargo can reuse its compiled source paths.
    $stageAbsolute=(Get-Item -LiteralPath $stage).FullName
    $backupRoot=Join-Path $PSScriptRoot 'stage-backups'
    New-Item -ItemType Directory -Force -Path $backupRoot | Out-Null
    $backup=[IO.Path]::GetFullPath((Join-Path $backupRoot ('staged-engine-'+[guid]::NewGuid().ToString('N'))))
    $prefix=$workspace.TrimEnd('\')+'\'
    if (!$stageAbsolute.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase) -or !$backup.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase) -or !(Test-Path -LiteralPath (Join-Path $stageAbsolute '.horizon-mixins-stage.json')) -or (Test-Path -LiteralPath $backup)) { throw 'Refusing to move an unverified source stage.' }
    Move-Item -LiteralPath $stageAbsolute -Destination $backup -ErrorAction Stop
    Write-Output ('Archived prior owned stage: '+$backup)
}
& (Join-Path $mixins 'target/debug/horizon-mixins.exe') apply --workspace $engine --mods (Join-Path $create 'mixins') --out $stage *> (Join-Path $PSScriptRoot 'apply.log')
if ($LASTEXITCODE -ne 0) { Get-Content -LiteralPath (Join-Path $PSScriptRoot 'apply.log') -Tail 30; throw 'Pinned mixin stage failed.' }
foreach ($p in Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'engine-patches') -Filter *.ps1) { & $p.FullName -Stage $stage }
$restored=0
foreach ($file in Get-ChildItem -LiteralPath $stage -Recurse -File) {
    $old=$prior[$file.FullName.Substring($stage.Length+1)]
    if ($null -ne $old -and $old.len -eq $file.Length -and (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash -eq $old.hash) {
        $file.LastWriteTimeUtc=$old.time
        $restored++
    }
}
Write-Output ('Restored timestamps for '+$restored+' unchanged staged files.')
$env:CARGO_TARGET_DIR=Join-Path $workspace 'work/horizon-source/horizon-5f7cadfff0fc073c0e4a68558754578c0d680a29/target'
# cmd redirection: Windows PowerShell 5.1 turns native stderr into terminating errors.
$cargo = Join-Path $workspace 'work/toolchain/bin/cargo.exe'
$manifest = Join-Path $stage 'Cargo.toml'
$log = Join-Path $PSScriptRoot 'game-build.log'
cmd /c "`"$cargo`" build --manifest-path `"$manifest`" --locked --offline --profile fast -p horizon-client -p horizon-client-lib --bins > `"$log`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content -LiteralPath (Join-Path $PSScriptRoot 'game-build.log') -Tail 75; throw 'Native stage3 client failed to build.' }
Get-Content -LiteralPath (Join-Path $PSScriptRoot 'game-build.log') -Tail 10

