#requires -Version 7.0
# Copies the freshly built stage4 native client into a disposable candidate folder.
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$bin=Join-Path $workspace 'work/horizon-source/horizon-5f7cadfff0fc073c0e4a68558754578c0d680a29/target/fast'
$stage=Join-Path $workspace 'work/homestead-horizon/staged-engine'
$dest=Join-Path $PSScriptRoot 'candidate/client'
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item (Join-Path $bin 'horizon-client.exe') $dest -Force
Copy-Item (Join-Path $bin 'horizon-ctl.exe') $dest -Force
Copy-Item (Join-Path $stage 'native-data') $dest -Recurse -Force
Copy-Item (Join-Path $PSScriptRoot '../../../outputs/homestead-horizon-stage3/client/native-mods.json') $dest -Force -ErrorAction SilentlyContinue
(Get-FileHash (Join-Path $dest 'horizon-client.exe') -Algorithm SHA256).Hash
