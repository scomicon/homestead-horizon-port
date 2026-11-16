$ErrorActionPreference='Stop'
$st=$PSScriptRoot
$save="$st\runtime\game\saves\HomesteadAutomationAcceptance"; if(Test-Path $save){ Rename-Item $save ("HomesteadAutomationAcceptance.old-"+(Get-Date -f HHmmss)) }
& "$st\Start-Test.ps1" | Out-Null
$run=Get-Content "$st\runtime\current-run.json" -Raw | ConvertFrom-Json
$sock=$run.socket; $ctl="$st\candidate\client\horizon-ctl.exe"; $dl=(Get-Date).AddMinutes(15); while(-not (Test-Path $sock)){ if((Get-Date) -gt $dl){ throw 'socket timeout' }; Start-Sleep 2 }
function Ctl([string[]]$w){ $o=& $ctl --socket $sock @w 2>&1; return ($o -join "`n") }
$null=Ctl @('wait-ready','300')
function Cmd($t){ return Ctl @('command',$t) }
function Food(){ $s=(Ctl @('state')) | ConvertFrom-Json; return $s.player }
$log=@()
function L($t){ $script:log+=$t; Write-Output $t }
L "full state player: $(((Ctl @('state')) | ConvertFrom-Json).player | ConvertTo-Json -Compress)"
$null=Cmd 'tick unfreeze'
foreach($case in @(
  @{n='A: survival normal, fresh';pre=@('gamemode survival','difficulty normal')},
  @{n='B: survival peaceful';pre=@('gamemode survival','difficulty peaceful')},
  @{n='C: creative->survival normal';pre=@('gamemode creative','gamemode survival','difficulty normal')} )){
  foreach($c in $case.pre){ $null=Cmd $c }
  $null=Cmd 'effect clear @s'; $null=Cmd 'tp @s 98.5 80 98.5'; $null=Ctl @('wait','40')
  $before=Food
  $null=Cmd 'effect give @s minecraft:hunger 20 255 true'; $null=Ctl @('wait','100'); $after=Food
  L "$($case.n): food $($before.food)->$($after.food) sat $($before.saturation)->$($after.saturation) exh? $($after.exhaustion) | abilities: $(Cmd 'data get entity @s abilities')"
  $null=Cmd 'effect clear @s'
}
L "status-like: $(Cmd 'data get entity @s foodExhaustionLevel') / $(Cmd 'data get entity @s foodSaturationLevel') / $(Cmd 'data get entity @s foodLevel')"
$null=Ctl @('save'); $null=Ctl @('quit')
$log | Set-Content "$st\stage22-hunger-experiment.log"

