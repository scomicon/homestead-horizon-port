#requires -Version 7.0
# Diagnostic probe: does a placed Item Vault get a block entity and a container?
$ErrorActionPreference='Stop'
[Threading.Thread]::CurrentThread.CurrentCulture=[Globalization.CultureInfo]::InvariantCulture
& (Join-Path $PSScriptRoot 'Start-Test.ps1')
$run=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'runtime/current-run.json') -Raw|ConvertFrom-Json
$process=Get-Process -Id $run.process_id
$ctl=Join-Path $PSScriptRoot 'candidate/client/horizon-ctl.exe'
function Ctl([string[]] $Words) { & $ctl --socket $run.socket @Words }
$out=@()
try {
    while (-not (Test-Path -LiteralPath $run.socket)) { Start-Sleep 1 }
    $out+=Ctl @('wait-ready','180')
    $out+=Ctl @('command','tick unfreeze')
    $out+=Ctl @('command','forceload add 144 32 159 47')
    Start-Sleep 5
    $out+=Ctl @('command','fill 148 81 33 158 90 42 minecraft:air')
    $out+=Ctl @('command','gamemode survival')
    $out+=Ctl @('command','gamerule random_tick_speed 0')
    $out+=Ctl @('command','fill 146 83 33 160 100 39 minecraft:air')
    $out+=Ctl @('command','fill 146 82 33 160 82 39 minecraft:dirt')
    $out+=Ctl @('command','fill 146 83 33 154 83 39 minecraft:farmland')
    $out+=Ctl @('command','setblock 150 84 36 minecraft:wheat[age=3]')
    $out+="--- before"; $s=Ctl @('state'); $out+=([regex]::Match(($s -join ''),'"health":[0-9.]+').Value)
    $out+=Ctl @('command','tp @s 150.5 90 36.5')
    foreach ($n in 1..8) { $out+=Ctl @('wait','10'); $s=Ctl @('state'); $j=($s -join ''); $out+=("t+$($n*10): "+[regex]::Match($j,'"health":[0-9.]+').Value+' '+[regex]::Match($j,'"position":\[[^\]]*\]').Value+' '+[regex]::Match($j,'"on_ground":[a-z]+').Value) }
} finally {
    try { $null=Ctl @('quit') } catch {}
    $null=$process.WaitForExit(60000)
    $out | Set-Content (Join-Path $PSScriptRoot 'vault-probe-output.txt') -Encoding utf8
}
