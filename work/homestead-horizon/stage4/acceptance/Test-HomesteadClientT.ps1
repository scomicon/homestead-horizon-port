#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Client tooltips (stage 20, group T): EnchantmentDescriptions, More Enchantment Info and Tooltip
# Overhaul as native tooltip hooks (module homestead_client_tooltips). The control socket cannot read
# what is drawn, so the operator command `homestead tooltip show` asks the window game for the same
# lines it draws (`stack_tooltip`, native tooltip hooks included) and `homestead tooltip color` calls
# the frame colour hook; the checks read those lines from the command's chat reply.
# NOT covered (not verified by this script): how the tooltip looks on screen (frame, spacing, line
# width), the language lookup of `.desc` keys, the Russian texts, and Shift held on a real keyboard
# (`homestead tooltip shift on` stands in for it).
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('ClientT')][string] $Phase='ClientT',
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
$transcript=Join-Path $game ('stage26-clientt-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage26-clientt-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Item tooltip lines through the native tooltip hook (the lines the window game draws): an enchanted book shows a description, max level, applies-to and incompatible lines under the enchantment; another item shows them only while Shift is held (forced by a test switch); the switches turn each part off; the frame colour hook gives rarity colours and vanilla colours for common items and when switched off. Not covered: the on-screen look, language files, Russian text, a real Shift key.'
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
function Command([string] $Text) {
    $reply=Invoke-Ctl @('command',$Text)
    if (($reply.chat -join ' ') -match 'not loaded|Unknown block|Unknown item|Incorrect argument|Expected |Unknown or incomplete command|Unknown component|Invalid') {
        throw ($Text+[Environment]::NewLine+($reply.chat -join ' '))
    }
    return $reply
}
function Say([string] $Text) { return ((Command $Text).chat -join ' ') }
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Need([string] $Text,[string] $Pattern) { if ($Text -notmatch $Pattern) { throw ('expected /'+$Pattern+'/ in: '+$Text) } }
function Refuse([string] $Text,[string] $Pattern) { if ($Text -match $Pattern) { throw ('did not expect /'+$Pattern+'/ in: '+$Text) } }
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
# Puts an item into the main hand. The component syntax of the stored/active enchantments changed
# between versions, so each variant is tried until the command is accepted and the enchantment shows.
function Hold-Item([string] $Id,[string[]] $Components,[string] $Expect) {
    $last=''
    foreach ($component in $Components) {
        try {
            $null=Command ('item replace entity @s weapon.mainhand with '+$Id+$component)
            Wait-Ticks 4
            $text=Say 'homestead tooltip show'
            if (-not $Expect -or $text -match $Expect) { return $text }
            $last=('['+$component+': '+$text+'] ')
        } catch { $last+=('['+$component+': '+$_.Exception.Message+'] ') }
    }
    throw ('could not hold '+$Id+': '+$last)
}
$bookForms=@('[stored_enchantments={"minecraft:sharpness":5}]','[stored_enchantments={levels:{"minecraft:sharpness":5}}]','[stored_enchantments={sharpness:5}]')
$swordForms=@('[enchantments={"minecraft:sharpness":3}]','[enchantments={levels:{"minecraft:sharpness":3}}]','[enchantments={sharpness:3}]')

$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'homestead tooltip descriptions on';$null=Command 'homestead tooltip info on'
$null=Command 'homestead tooltip borders on';$null=Command 'homestead tooltip shift off'
Wait-Ticks 20
$results=[ordered]@{}

Scenario 'the tooltip command answers on the window game' {
    $text=Say 'homestead tooltip status'
    Need $text 'homestead tooltip: descriptions on, info on, borders on, shift forced off'
}
Scenario 'an enchanted book shows a description under its enchantment' {
    $text=Hold-Item 'minecraft:enchanted_book' $bookForms 'Sharpness V'
    Need $text 'Sharpness V \|\s+Increases melee damage'
}
Scenario 'More Enchantment Info: max level, applies to and incompatible lines follow the description' {
    $text=Say 'homestead tooltip show'
    Need $text 'Max level: V'
    Need $text 'Applies to: \S'
    Need $text 'Incompatible with: \S'
}
Scenario 'a sword shows the extra lines only while Shift is held' {
    $text=Hold-Item 'minecraft:diamond_sword' $swordForms 'Sharpness III'
    Refuse $text 'Increases melee damage'
    Refuse $text 'Max level'
    $null=Command 'homestead tooltip shift on'
    $text=Say 'homestead tooltip show'
    Need $text 'Sharpness III \|\s+Increases melee damage'
    Need $text 'Max level: V'
    $null=Command 'homestead tooltip shift off'
    $text=Say 'homestead tooltip show'
    Refuse $text 'Increases melee damage'
}
Scenario 'the switches turn descriptions and info off separately' {
    $null=Hold-Item 'minecraft:enchanted_book' $bookForms 'Sharpness V'
    $null=Command 'homestead tooltip descriptions off'
    $text=Say 'homestead tooltip show'
    Refuse $text 'Increases melee damage'
    Need $text 'Max level: V'
    $null=Command 'homestead tooltip descriptions on';$null=Command 'homestead tooltip info off'
    $text=Say 'homestead tooltip show'
    Need $text 'Increases melee damage'
    Refuse $text 'Max level'
    $null=Command 'homestead tooltip info on'
    $text=Say 'homestead tooltip show'
    Need $text 'Increases melee damage'
    Need $text 'Max level: V'
}
Scenario 'the frame takes the rarity colour; common items and the off switch keep vanilla colours' {
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:dragon_egg'
    Wait-Ticks 4
    $text=Say 'homestead tooltip color'
    Need $text 'top F0FF55FF'
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:stick'
    Wait-Ticks 4
    $text=Say 'homestead tooltip color'
    Need $text 'background F0100010 top 505000FF bottom 5028007F'
    $null=Command 'homestead tooltip borders off'
    $null=Command 'item replace entity @s weapon.mainhand with minecraft:dragon_egg'
    Wait-Ticks 4
    $text=Say 'homestead tooltip color'
    Need $text 'top 505000FF'
    $null=Command 'homestead tooltip borders on'
}
Scenario 'both hooks ran in this process (their call counters moved)' {
    $text=Say 'homestead tooltip status'
    if ($text -notmatch 'text hook calls (\d+), color hook calls (\d+)') { throw ('no counters in: '+$text) }
    if ([int]$Matches[1] -lt 1) { throw ('the tooltip hook never ran: '+$text) }
    if ([int]$Matches[2] -lt 1) { throw ('the frame colour hook never ran: '+$text) }
}
$results['NOT VERIFIED: on-screen look, language files, Russian text, real Shift key']='note'
$report.checks+='note: on-screen look, language lookup of .desc keys, Russian text and a real Shift key are not covered by this script'
$null=Command 'homestead tooltip shift off'
$null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' -and $_.Value -ne 'note' })
$report.measured=@{scenarios=($results.Count-1);failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+($results.Count-1)+', failed: '+$failed.Count)
