#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Client defaults and resource packs (stage 32): the package's built-in client defaults are written
# once and never over a player's choice, the game reads them (Extras page), and a resource pack in
# resourcepacks/ is loaded and unloaded by options.txt's resourcePacks list (a language override and
# a one-texture override that WE generate; no third-party pack is used here).
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('ClientDefaults')][string] $Phase='ClientDefaults',
    [string] $World='HomesteadAutomationAcceptance',
    [int] $GameProcessId=0
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
[Threading.Thread]::CurrentThread.CurrentCulture=[Globalization.CultureInfo]::InvariantCulture
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$game=[IO.Path]::GetFullPath($GameDir)
$allowed=@(
    [IO.Path]::GetFullPath((Join-Path $workspace 'work/homestead-horizon/stage4')),
    [IO.Path]::GetFullPath((Join-Path $workspace 'outputs/homestead-horizon-stage4'))
)
if (-not ($allowed|Where-Object {$game.StartsWith($_+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)})) {
    throw 'Use the separate stage4 work or stage4 output instance, never a player save.'
}
if (-not (Test-Path -LiteralPath $game -PathType Container)) { throw 'Start the disposable client first.' }
if ($World -notmatch '^HomesteadAutomation[A-Za-z0-9_-]*$') { throw 'Use an explicitly disposable automation world.' }
$ctlPath=(Resolve-Path -LiteralPath $Ctl).Path
$socketPath=[IO.Path]::GetFullPath($Socket)
$transcript=Join-Path $game ('stage32-clientdefaults-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage32-clientdefaults-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Defaults file written by the package script (4 keys, never over a player choice, once per instance); the game reads it (Extras page shows Mouse Tweaks controls On); a generated resource pack loads (language + texture override) and unloads again.'
}
[IO.File]::WriteAllText($transcript,'',$utf8)
function Save-Report { [IO.File]::WriteAllText($reportPath,($report|ConvertTo-Json -Depth 25)+[Environment]::NewLine,$utf8) }
function Invoke-Ctl([string[]] $Words) {
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$ctlPath;$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach ($word in (@('--socket',$socketPath)+$Words)) { $info.ArgumentList.Add([string]$word) }
    $process=[Diagnostics.Process]::Start($info)
    $outTask=$process.StandardOutput.ReadToEndAsync();$errTask=$process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(150000)) { $process.Kill();throw ('Control timeout: '+($Words -join ' ')) }
    $stdout=$outTask.GetAwaiter().GetResult();$stderr=$errTask.GetAwaiter().GetResult()
    [IO.File]::AppendAllText($transcript,(@{words=$Words;exit_code=$process.ExitCode;stdout=$stdout;stderr=$stderr}|ConvertTo-Json -Compress -Depth 20)+[Environment]::NewLine,$utf8)
    if ($process.ExitCode -ne 0) { throw ('Control failed: '+($Words -join ' ')+[Environment]::NewLine+$stdout+$stderr) }
    $reply=$stdout|ConvertFrom-Json -AsHashtable
    if ($reply.ContainsKey('error')) { throw ($reply.error|ConvertTo-Json -Depth 10) }
    return $reply
}
function Command([string] $Text) { return (Invoke-Ctl @('command',$Text)) }
function Passed([string] $Text) { $report.checks+=$Text;Write-Output $Text;Save-Report }
function Fail([string] $Text) { $report.checks+=('FAILED: '+$Text);Save-Report;throw $Text }
function Ready-Screen {
    # Back to the playing screen (a previous step may have left an options page open).
    for ($i=0;$i -lt 5;$i++) {
        if ((Invoke-Ctl @('screen')).screen -eq 'playing') { return }
        $null=Invoke-Ctl @('key','esc','tap'); Start-Sleep -Seconds 1
    }
    Fail 'could not return to the playing screen'
}
$packsDir=Join-Path $game 'resourcepacks'
$packName='homestead-test-pack-'+[guid]::NewGuid().ToString('N').Substring(0,8)
$optionsBefore=(Invoke-Ctl @('options-get','resourcePacks')).options.resourcePacks.value
$markerText='HS CREATIVE '+$packName.Substring($packName.Length-4)
try {
    # 1. The package defaults are in the instance (Start-Test applied them like Start-Homestead does).
    $extrasFile=Join-Path $game 'horizon-extras.txt'
    if (-not (Test-Path -LiteralPath $extrasFile)) { Fail 'horizon-extras.txt was not written' }
    $extras=[IO.File]::ReadAllText($extrasFile)
    foreach ($key in 'dynamicLights','mouseTweaks','mouseTweaksWheel','threeDSkinLayers') {
        if ($extras -notmatch ('(?m)^'+$key+':true\s*$')) { Fail ('default '+$key+':true missing in horizon-extras.txt') }
    }
    if (-not (Test-Path -LiteralPath (Join-Path $game '.homestead-client-defaults'))) { Fail 'defaults marker missing' }
    Passed 'package defaults written: dynamicLights, mouseTweaks, mouseTweaksWheel, threeDSkinLayers (once per instance, marker set)'

    # 2. Merge rules on a scratch folder: a player's value is never overwritten, a deleted key is not re-added.
    . (Join-Path $PSScriptRoot '../package/Apply-ClientDefaults.ps1')
    $scratch=Join-Path $game 'client-defaults-scratch'
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $scratch|Out-Null
    [IO.File]::WriteAllText((Join-Path $scratch 'horizon-extras.txt'),"fullbright:true`r`ndynamicLights:false`r`nmouseTweaks:false",$utf8)
    $added=@(Set-HomesteadClientDefaults -GameDir $scratch)
    $text=[IO.File]::ReadAllText((Join-Path $scratch 'horizon-extras.txt'))
    if ($text -notmatch '(?m)^dynamicLights:false\s*$' -or $text -notmatch '(?m)^mouseTweaks:false\s*$' -or $text -notmatch '(?m)^fullbright:true\s*$') { Fail 'a player value was overwritten' }
    if (($added -join ',') -ne 'mouseTweaksWheel,threeDSkinLayers') { Fail ('unexpected keys added: '+($added -join ',')) }
    Remove-Item -LiteralPath (Join-Path $scratch 'horizon-extras.txt')
    [IO.File]::WriteAllText((Join-Path $scratch 'horizon-extras.txt'),"fullbright:true`n",$utf8)
    if (@(Set-HomesteadClientDefaults -GameDir $scratch).Count -ne 0) { Fail 'defaults applied twice to one instance' }
    Remove-Item -LiteralPath $scratch -Recurse -Force
    Passed 'merge rules: player values kept, missing keys added, second run does nothing'

    # 3. The game read the file: the Extras page shows the default.
    Ready-Screen
    $null=Invoke-Ctl @('options','extras'); Start-Sleep -Seconds 2
    $labels=@((Invoke-Ctl @('ui')).widgets|ForEach-Object { if ($_.ContainsKey('label')) { [string]$_['label'] } })
    $null=Invoke-Ctl @('click','done'); Start-Sleep -Seconds 1
    Ready-Screen
    if (-not ($labels -contains 'Mouse Tweaks controls: On')) { Fail ('Extras page does not show Mouse Tweaks controls On: '+($labels -join ' | ')) }
    Passed 'the game reads the defaults: Extras page shows "Mouse Tweaks controls: On"'

    # 4. A resource pack that we generate (language + one texture) is loaded from resourcepacks/.
    $pack=Join-Path $packsDir $packName
    New-Item -ItemType Directory -Force -Path (Join-Path $pack 'assets/minecraft/lang'),(Join-Path $pack 'assets/minecraft/textures/block')|Out-Null
    [IO.File]::WriteAllText((Join-Path $pack 'pack.mcmeta'),'{"pack":{"pack_format":15,"min_format":15,"max_format":999,"supported_formats":[15,999],"description":"Homestead client defaults test pack"}}',$utf8)
    [IO.File]::WriteAllText((Join-Path $pack 'assets/minecraft/lang/en_us.json'),('{"gameMode.creative":"'+$markerText+'"}'),$utf8)
    Add-Type -AssemblyName System.Drawing
    $bitmap=[Drawing.Bitmap]::new(16,16)
    for ($x=0;$x -lt 16;$x++) { for ($y=0;$y -lt 16;$y++) { $bitmap.SetPixel($x,$y,[Drawing.Color]::FromArgb(255,255,0,0)) } }
    $bitmap.Save((Join-Path $pack 'assets/minecraft/textures/block/stone.png'),[Drawing.Imaging.ImageFormat]::Png); $bitmap.Dispose()
    $null=Command 'setblock ~ ~2 ~ minecraft:stone'
    $null=Invoke-Ctl @('options-set','resourcePacks',('["vanilla","file/'+$packName+'"]'))
    # The reload runs in the background (up to a few minutes with all content mods): wait for the override.
    $seen=$false; $deadline=[DateTime]::UtcNow.AddMinutes(8)
    while ([DateTime]::UtcNow -lt $deadline) {
        $null=Command 'gamemode survival'
        $reply=Command 'gamemode creative'
        if (($reply.chat -join ' ') -like ('*'+$markerText+'*')) { $seen=$true; break }
        Start-Sleep -Seconds 5
    }
    if (-not $seen) { Fail 'the language override of the test pack was not applied within 8 minutes' }
    Passed ('resource pack loaded from resourcepacks/ ('+$packName+'): language override visible in game chat')
    $null=Invoke-Ctl @('look','0','-90'); Start-Sleep -Seconds 3
    $shot=Join-Path $game 'client-defaults-stone.png'
    Remove-Item -LiteralPath $shot -ErrorAction SilentlyContinue
    $null=Invoke-Ctl @('screenshot',$shot); Start-Sleep -Seconds 3
    $image=[Drawing.Bitmap]::FromFile($shot)
    try {
        $cx=[int]($image.Width/2); $cy=[int]($image.Height/2); $red=0
        foreach ($offset in @(@(-70,50),@(70,50),@(-70,-50),@(70,-50))) {
            $p=$image.GetPixel($cx+$offset[0],$cy+$offset[1])
            # The underside of a block is drawn dark (half brightness and shade), so test the hue (red, not grey stone), not full red.
            if ($p.R -gt 25 -and ($p.G*2.5) -lt $p.R -and ($p.B*2.5) -lt $p.R) { $red++ }
        }
    } finally { $image.Dispose() }
    if ($red -lt 3) { Fail ('the one-texture override is not visible on the stone above the player (red samples: '+$red+' of 4)') }
    Passed 'resource pack texture override visible: the stone block shows the generated red texture'
    $null=Command 'setblock ~ ~2 ~ minecraft:air'

    # 5. Unloading: with the list empty the vanilla text and texture come back.
    $null=Invoke-Ctl @('options-set','resourcePacks','[]')
    $back=$false; $deadline=[DateTime]::UtcNow.AddMinutes(8)
    while ([DateTime]::UtcNow -lt $deadline) {
        $null=Command 'gamemode survival'
        $reply=Command 'gamemode creative'
        if (($reply.chat -join ' ') -like '*Creative Mode*') { $back=$true; break }
        Start-Sleep -Seconds 5
    }
    if (-not $back) { Fail 'the vanilla text did not come back after the pack was removed' }
    Passed 'resource pack unloaded: vanilla text is back'
    $report.passed=$true
    Save-Report
} finally {
    # Leave the shared disposable instance as we found it.
    try { $null=Invoke-Ctl @('options-set','resourcePacks',([string]('['+(($optionsBefore|ForEach-Object { '"'+$_+'"' }) -join ',')+']'))) } catch { }
    Remove-Item -LiteralPath (Join-Path $packsDir $packName) -Recurse -Force -ErrorAction SilentlyContinue
    if ((Test-Path -LiteralPath $packsDir) -and -not (Get-ChildItem -LiteralPath $packsDir -Force)) { Remove-Item -LiteralPath $packsDir -Force }
    if ($report.passed) { Remove-Item -LiteralPath (Join-Path $game 'client-defaults-stone.png') -ErrorAction SilentlyContinue }
}
