#requires -Version 7.0
# Stage 4 belt-funnel acceptance in a disposable world. Keep this parent shell
# alive until horizon-ctl quit and the native process exit.
[CmdletBinding()]
param([switch] $ReloadOnly,[switch] $Vault,[switch] $Harvest,[switch] $Tweaks,[switch] $Rain,[switch] $Days,[switch] $FarmersDelight,[switch] $Decor,[switch] $Arm,[switch] $Link,[switch] $Points,[switch] $Tank,[switch] $Pipes,[switch] $Steam,[switch] $Board,[switch] $Stove,[switch] $Skillet,[switch] $Pot,[switch] $Switches,[switch] $Displays,[switch] $Cannon,[switch] $Packager,[switch] $Controller,[switch] $SchematicTools,[switch] $Stock,[switch] $DataMods,[switch] $Crops,[switch] $Feasts,[switch] $Motor,[switch] $LogicA,[switch] $LogicB,[switch] $LogicC,[switch] $LogicD,[switch] $ClientDefaults,[switch] $ClientT,[switch] $ClientH,[switch] $ClientS,[switch] $Batch1,[switch] $Engine20,[switch] $Fixes2,[switch] $FdStage23,[switch] $Batch2)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
# Game commands need '.' decimals whatever the Windows locale is.
[Threading.Thread]::CurrentThread.CurrentCulture=[Globalization.CultureInfo]::InvariantCulture
if ($ClientDefaults) { $env:HOMESTEAD_CLIENT_DEFAULTS='1' } else { Remove-Item Env:HOMESTEAD_CLIENT_DEFAULTS -ErrorAction SilentlyContinue }
# These flags must be inherited when Start-Test spawns the client.
if ($ClientS) { $env:HOMESTEAD_AUDIO='1' } else { Remove-Item Env:HOMESTEAD_AUDIO -ErrorAction SilentlyContinue }
if ($Motor -or $ClientDefaults -or $ClientT -or $ClientH -or $ClientS) { $env:HOMESTEAD_WINDOWED='1' } else { Remove-Item Env:HOMESTEAD_WINDOWED -ErrorAction SilentlyContinue }
$run=$null
$process=$null
$ctl=Join-Path $PSScriptRoot 'candidate/client/horizon-ctl.exe'
$script=Join-Path $PSScriptRoot $(if ($Vault) {'acceptance/Test-HomesteadItemVault.ps1'} else {'acceptance/Test-HomesteadBeltFunnels.ps1'})
$receiptPrefix=if ($Vault) {'stage5-itemvault-'} else {'stage4-beltfunnels-'}
$report=[ordered]@{passed=$false;process_id=0;reload_only=[bool]$ReloadOnly;phases=@();
    binary_sha256=$null;
    shutdown=[ordered]@{requested=$false;process_exited=$false}}
$reportName=if ($FdStage23) {'run-fdstage23-result.json'} elseif ($Batch2) {'run-batch2-result.json'} elseif ($Fixes2) {'run-fixes2-result.json'} elseif ($Batch1) {'run-batch1-result.json'} elseif ($Engine20) {'run-engine20-result.json'} elseif ($ClientT) {'run-clientt-result.json'} elseif ($ClientH) {'run-clienth-result.json'} elseif ($ClientS) {'run-clients-result.json'} elseif ($ClientDefaults) {'run-clientdefaults-result.json'} elseif ($LogicA) {'run-logica-result.json'} elseif ($LogicB) {'run-logicb-result.json'} elseif ($LogicC) {'run-logicc-result.json'} elseif ($LogicD) {'run-logicd-result.json'} elseif ($Motor) {'run-motor-result.json'} elseif ($DataMods) {'run-datamods-result.json'} elseif ($Crops) {'run-crops-result.json'} elseif ($Feasts) {'run-feasts-result.json'} elseif ($Controller) {'run-controller-result.json'} elseif ($SchematicTools) {'run-schematic-tools-result.json'} elseif ($Stock) {'run-stock-result.json'} elseif ($Switches) {'run-switches-result.json'} elseif ($Motor) {
$runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadMotor.ps1'),'Motor','stage31-motor-'))
} elseif ($Displays) {'run-displays-result.json'} elseif ($Cannon) {'run-cannon-result.json'} elseif ($Packager) {'run-packager-result.json'} elseif ($Pot) {'run-pot-result.json'} elseif ($Skillet) {'run-skillet-result.json'} elseif ($Stove) {'run-stove-result.json'} elseif ($Board) {'run-board-result.json'} elseif ($Steam) {'run-steam-result.json'} elseif ($Pipes) {'run-pipes-result.json'} elseif ($Tank) {'run-tank-result.json'} elseif ($Points) {'run-points-result.json'} elseif ($Link) {'run-link-result.json'} elseif ($Arm) {'run-arm-result.json'} elseif ($Decor) {'run-decor-result.json'} elseif ($FarmersDelight) {'run-fd-result.json'} elseif ($Days) {'run-days-result.json'} elseif ($Rain) {'run-rain-result.json'} elseif ($Tweaks) {'run-tweaks-result.json'} elseif ($Harvest) {'run-harvest-result.json'} elseif ($Vault -and $ReloadOnly) {'run-vault-reload-result.json'} elseif ($Vault) {'run-vault-result.json'} elseif ($ReloadOnly) {'run-reload-result.json'} else {'run-flow-result.json'}
function Invoke-Control([string[]] $Words,[int] $TimeoutMs=150000) {
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$ctl;$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach ($word in (@('--socket',$run.socket)+$Words)) { $info.ArgumentList.Add([string]$word) }
    $command=[Diagnostics.Process]::Start($info)
    $outTask=$command.StandardOutput.ReadToEndAsync();$errTask=$command.StandardError.ReadToEndAsync()
    if (-not $command.WaitForExit($TimeoutMs)) { $command.Kill();throw ('Native control timeout: '+($Words -join ' ')) }
    $stdout=$outTask.GetAwaiter().GetResult();$stderr=$errTask.GetAwaiter().GetResult()
    if ($command.ExitCode -ne 0) { throw ('Native control failed: '+($Words -join ' ')+[Environment]::NewLine+$stdout+$stderr) }
    $reply=$stdout|ConvertFrom-Json -AsHashtable
    if ($reply.ContainsKey('error')) { throw ($reply.error|ConvertTo-Json -Depth 8) }
    return $reply
}
$checksPassed=$false
try {
    # Invalidate an older PASS before startup, which can itself fail.
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot $reportName),($report|ConvertTo-Json -Depth 10)+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
    & (Join-Path $PSScriptRoot 'Start-Test.ps1')
    $run=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'runtime/current-run.json') -Raw|ConvertFrom-Json
    $process=Get-Process -Id $run.process_id -ErrorAction Stop
    $null=$process.Handle
    $report.process_id=$run.process_id
    $report.binary_sha256=(Get-FileHash -LiteralPath $run.binary -Algorithm SHA256).Hash.ToLowerInvariant()
    # A cold 50-mod resource initialization has taken over ten minutes.
    $deadline=[DateTime]::UtcNow.AddMinutes(15)
    while (-not (Test-Path -LiteralPath $run.socket)) {
        if ($process.HasExited) { throw 'The native stage4 client exited during startup.' }
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Native startup exceeded fifteen minutes.' }
        Start-Sleep -Seconds 1
    }
    $null=Invoke-Control @('wait-ready','180')
    Write-Output ('Isolated native stage4 world ready; PID '+$run.process_id)
    $vaultScript=Join-Path $PSScriptRoot 'acceptance/Test-HomesteadItemVault.ps1'
    $beltScript=Join-Path $PSScriptRoot 'acceptance/Test-HomesteadBeltFunnels.ps1'
    $runs=@()
    $harvestScript=Join-Path $PSScriptRoot 'acceptance/Test-HomesteadHarvest.ps1'
    if ($LogicA) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicA.ps1'),'LogicA','stage25-logica-'))
    } elseif ($LogicB) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicB.ps1'),'LogicB','stage25-logicb-'))
    } elseif ($LogicC) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicC.ps1'),'LogicC','stage25-logicc-'))
    } elseif ($LogicD) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicD.ps1'),'LogicD','stage25-logicd-'))
    } elseif ($Switches) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadSwitches.ps1'),'Switches','stage20-switches-'))
    } elseif ($Fixes2) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadFixes2.ps1'),'Fixes2','stage28-fixes2-'))
    } elseif ($DataMods) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadDataMods.ps1'),'DataMods','stage25-datamods-'))
    } elseif ($Crops) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadCrops.ps1'),'Crops','stage24-crops-'))
    } elseif ($Feasts) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadFeasts.ps1'),'Feasts','stage24-feasts-'))
    } elseif ($Controller) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadController.ps1'),'Controller','stage23-controller-'))
    } elseif ($SchematicTools) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadSchematicTools.ps1'),'SchematicTools','stage21-schematic-tools-'))
    } elseif ($Stock) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadStock.ps1'),'Stock','stage23-stock-'))
    } elseif ($FdStage23) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-Homestead-FdStage23.ps1'),'FdStage23','stage23-fd-'))
    } elseif ($Batch2) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-Homestead-Batch2.ps1'),'Batch2','stage23-batch2-'))
    } elseif ($Batch1) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadBatch1.ps1'),'Batch1','stage27-batch1-'))
    } elseif ($Engine20) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadEngine20.ps1'),'Engine20','stage27-engine20-'))
    } elseif ($ClientT) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadClientT.ps1'),'ClientT','stage26-clientt-'))
    } elseif ($ClientH) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadClientH.ps1'),'ClientH','stage26-clienth-'))
    } elseif ($ClientS) {
        $runs=@(
            @((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadClientS.ps1'),'ClientS','stage26-clients-'),
            @((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadSoundPlayback.ps1'),'Playback','stage33-soundplayback-')
        )
    } elseif ($ClientDefaults) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadClientDefaults.ps1'),'ClientDefaults','stage32-clientdefaults-'))
    } elseif ($Motor) {
$runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadMotor.ps1'),'Motor','stage31-motor-'))
} elseif ($Displays) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadDisplays.ps1'),'Displays','stage11-displays-'))
    } elseif ($Cannon) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadCannon.ps1'),'Cannon','stage30-cannon-'))
    } elseif ($Packager) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadPackager.ps1'),'Packager','stage20-packager-'))
    } elseif ($Pot) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadPot.ps1'),'Pot','stage19-pot-'))
    } elseif ($Skillet) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadSkillet.ps1'),'Skillet','stage18-skillet-'))
    } elseif ($Stove) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadStove.ps1'),'Stove','stage17-stove-'))
    } elseif ($Board) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadCuttingBoard.ps1'),'Board','stage16-board-'))
    } elseif ($Steam) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadSteam.ps1'),'Steam','stage15-steam-'))
    } elseif ($Pipes) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadPipes.ps1'),'Pipes','stage14-pipes-'))
    } elseif ($Tank) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadTank.ps1'),'Tank','stage13-tank-'))
    } elseif ($Points) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadArmPoints.ps1'),'Points','stage12-points-'))
    } elseif ($Link) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLink.ps1'),'Link','stage11-link-'))
    } elseif ($Arm) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadArm.ps1'),'Arm','stage10-arm-'))
    } elseif ($Decor) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadDecor.ps1'),'Decor','stage9-decor-'))
    } elseif ($FarmersDelight) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadFarmersDelight.ps1'),'Items','stage8-fd-'))
    } elseif ($Days) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadDays.ps1'),'Days','stage6-days-'))
    } elseif ($Rain) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadRain.ps1'),'Rain','stage6-rain-'))
    } elseif ($Tweaks) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadTweaks.ps1'),'Tweaks','stage6-tweaks-'))
    } elseif ($Harvest) {
        $runs=@(,@($harvestScript,'Harvest','stage6-harvest-'))
    } elseif ($Vault -and $ReloadOnly) {
        $runs=@(,@($vaultScript,'Reload','stage5-itemvault-'))
    } elseif ($Vault) {
        $runs=@(@($vaultScript,'Shared','stage5-itemvault-'),@($vaultScript,'Extras','stage5-itemvault-'),@($beltScript,'VaultBelt','stage4-beltfunnels-'))
    } elseif ($ReloadOnly) {
        $runs=@(,@($script,'Reload',$receiptPrefix))
    } else {
        $runs=@(@($script,'Flow',$receiptPrefix),@($script,'FrontBlocked',$receiptPrefix),@($script,'SidePass',$receiptPrefix))
    }    foreach ($run1 in $runs) {
        $phase=$run1[1]
        & $run1[0] -Ctl $ctl -Socket $run.socket -GameDir $run.game -World $run.world -GameProcessId $run.process_id -Phase $phase
        if (-not $?) { throw ('Acceptance phase failed: '+$phase) }
        $receipt=Join-Path $run.game ($run1[2]+$phase.ToLowerInvariant()+'-result.json')
        if (-not (Get-Content -LiteralPath $receipt -Raw|ConvertFrom-Json).passed) { throw ('Receipt failed: '+$phase) }
        $report.phases+=$phase
    }    $null=Invoke-Control @('save')
    $checksPassed=$true
} catch {
    $report.error=$_.Exception.Message
    Write-Output ('Stage4 live acceptance failed: '+$report.error)
} finally {
    try {
        if ($null -ne $process -and -not $process.HasExited) {
            $report.shutdown.requested=$true
            $null=Invoke-Control @('quit') 35000
            if (-not $process.WaitForExit(60000)) { throw 'Client did not exit after normal quit.' }
        }
        $report.shutdown.process_exited=($null -eq $process -or $process.HasExited)
    } catch {
        $report.shutdown.error=$_.Exception.Message
        $report.shutdown.process_exited=($null -eq $process -or $process.HasExited)
    } finally {
        $report.passed=$checksPassed -and $report.shutdown.requested -and $report.shutdown.process_exited -and -not $report.shutdown.Contains('error')
        [IO.File]::WriteAllText((Join-Path $PSScriptRoot $reportName),($report|ConvertTo-Json -Depth 10)+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
        Write-Output ('Runner receipt: '+(Join-Path $PSScriptRoot $reportName))
        if ($null -ne $run) { Get-Content -LiteralPath (Join-Path $run.session 'stderr.log') -Tail 12 -ErrorAction SilentlyContinue }
    }
}
if (-not $report.passed) { throw 'Stage4 live acceptance did not complete cleanly; inspect the runner and phase receipts.' }
