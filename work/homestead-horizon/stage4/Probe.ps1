#requires -Version 7.0
# Diagnostic probe: item placement of the Andesite Funnel against barrels (disposable stage4 world).
$ErrorActionPreference='Stop'
[Threading.Thread]::CurrentThread.CurrentCulture=[Globalization.CultureInfo]::InvariantCulture
& (Join-Path $PSScriptRoot 'Start-Test.ps1')
$run=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'runtime/current-run.json') -Raw|ConvertFrom-Json
$process=Get-Process -Id $run.process_id
$ctl=Join-Path $PSScriptRoot 'candidate/client/horizon-ctl.exe'
function Ctl([string[]] $Words) { & $ctl --socket $run.socket @Words }
$out=@()
function Try-Place([string]$label,[int]$bx,[int]$bz,[string]$face,[double]$sx,[double]$sz,[int]$fx,[int]$fz) {
    $script:out+="--- $label"
    $script:out+=Ctl @('command',"setblock $fx 83 $fz minecraft:air")
    $script:out+=Ctl @('command',"setblock $bx 83 $bz minecraft:barrel")
    $script:out+=Ctl @('command','item replace entity @s weapon.mainhand with create:andesite_funnel')
    $script:out+=Ctl @('command',"tp @s $sx 83 $sz")
    $script:out+=Ctl @('wait','2')
    $script:out+=Ctl @('look-at','block',"$bx",'83',"$bz",$face)
    $script:out+=Ctl @('interact',"$bx",'83',"$bz",$face)
    $script:out+=Ctl @('wait','1')
    $script:out+=Ctl @('block',"$fx",'83',"$fz")
}
try {
    while (-not (Test-Path -LiteralPath $run.socket)) { Start-Sleep 1 }
    $out+=Ctl @('wait-ready','180')
    $out+=Ctl @('command','tick unfreeze')
    $out+=Ctl @('command','gamemode creative')
    $out+=Ctl @('command','forceload add 128 32 143 47')
    Start-Sleep 5
    $out+=Ctl @('command','fill 130 81 34 143 87 46 minecraft:air')
    $out+=Ctl @('command','fill 130 80 34 143 80 46 minecraft:smooth_stone')
    # no-belt lane: ordinary funnel facing check, then a belt lane
    Try-Place 'nobelt west click (barrel 132,36)' 132 36 'west' 129.35 36.5 131 36
    Try-Place 'nobelt east click (barrel 132,38)' 132 38 'east' 134.65 38.5 133 38
    Try-Place 'nobelt south click (barrel 132,40)' 132 40 'south' 132.5 42.65 132 41
    Try-Place 'nobelt north click (barrel 132,44)' 132 44 'north' 132.5 41.35 132 43
    $out+=Ctl @('command','setblock 138 82 37 create:belt[facing=south,part=start,slope=horizontal,casing=false]')
    foreach ($z in 38..40) { $out+=Ctl @('command',"setblock 138 82 $z create:belt[facing=south,part=middle,slope=horizontal,casing=false]") }
    $out+=Ctl @('command','setblock 138 82 41 create:belt[facing=south,part=end,slope=horizontal,casing=false]')
    Start-Sleep 2
    Try-Place 'belt west click (barrel 139,39 -> funnel 138,39)' 139 39 'west' 136.35 39.5 138 39
    Try-Place 'belt east click (barrel 137,39 -> funnel 138,39)' 137 39 'east' 140.65 39.5 138 39
    Try-Place 'belt south click (barrel 138,38 -> funnel 138,39)' 138 38 'south' 138.5 41.65 138 39
} finally {
    try { $null=Ctl @('quit') } catch {}
    $null=$process.WaitForExit(60000)
    $out | Set-Content (Join-Path $PSScriptRoot 'probe-output.txt') -Encoding utf8
}
