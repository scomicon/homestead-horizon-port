Set-Location $PSScriptRoot
$ErrorActionPreference='Continue'
foreach($x in @(@('fd',@{FarmersDelight=$true}),@('decor',@{Decor=$true}))){ $n=$x[0]; $p=$x[1]; try { & .\Run-Acceptance.ps1 @p *> "regress-$n.log" } catch {}; Add-Content regress.txt ("$n done "+(Get-Date -f HH:mm:ss)) }
Add-Content regress.txt 'ALL DONE'