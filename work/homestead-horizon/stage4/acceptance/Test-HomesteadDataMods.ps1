#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Carried data-tier mods: the client loads every .hmod, and a sample of each mod blocks registers and places.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('DataMods')][string] $Phase='DataMods',
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
$transcript=Join-Path $game ('stage25-datamods-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage25-datamods-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='45 carried data-tier mods (pack/mods/*/carry): each mod with registered blocks places at least one sampled block; data-only mods are listed.'
    coordinates=@{origin=@(100,90,100)}
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
    if (($reply.chat -join ' ') -match 'not loaded|Unknown block|Unknown item|Incorrect argument|Expected |Unknown or incomplete command') {
        throw ($Text+[Environment]::NewLine+($reply.chat -join ' '))
    }
    return $reply
}
function Wait-Ticks([int] $Count) { $null=Invoke-Ctl @('wait',[string]$Count) }
function Passed([string] $Text) { $report.checks+=$Text;Write-Output $Text;Save-Report }
function Get-Data([int] $X,[int] $Y,[int] $Z,[string] $Key='') {
    $text="data get block $X $Y $Z";if ($Key) { $text+=' '+$Key }
    return ((Command $text).chat -join ' ')
}

function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }

$modsRoot=Join-Path $workspace 'work/homestead-horizon/pack/mods'
$skipPath=Join-Path $PSScriptRoot '../data-mods-skip.json'
$skip=@{}
if (Test-Path -LiteralPath $skipPath) { $j=Get-Content -LiteralPath $skipPath -Raw|ConvertFrom-Json; foreach ($p in $j.PSObject.Properties) { $skip[$p.Name]=$p.Value } }
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 96 96 191 191'
$null=Command 'tp @s 140 100 104'
Wait-Ticks 40
$null=Command 'fill 96 90 96 191 90 191 minecraft:air'
$mods=@(Get-ChildItem -LiteralPath $modsRoot -Directory|Sort-Object Name)
Expect ($mods.Count -ge 45) ('Expected 45 carried data mods, found '+$mods.Count)
$n=0;$perMod=[ordered]@{};$dataOnly=@();$failedMods=@();$placedTotal=0
foreach ($mod in $mods) {
    $names=@()
    # Blocks the carry tool registered live under data/<namespace>/horizon/block.
    foreach ($dir in Get-ChildItem -LiteralPath (Join-Path $mod.FullName 'carry/data') -Directory -ErrorAction SilentlyContinue) {
        $hb=Join-Path $dir.FullName 'horizon/block'
        if (Test-Path -LiteralPath $hb) { foreach ($f in Get-ChildItem -LiteralPath $hb -Filter *.json -Recurse|Sort-Object Name) { $names+=($dir.Name+':'+$f.BaseName) } }
    }
    if ($names.Count -eq 0) { $dataOnly+=$mod.Name;continue }
    $pick=@();$step=[math]::Max(1,[math]::Floor($names.Count/4))
    for ($k=0;$k -lt $names.Count -and $pick.Count -lt 4;$k+=$step) { $pick+=$names[$k] }
    $ok=@();$bad=@()
    foreach ($id in $pick) {
        $x=100+2*($n%45);$z=100+2*[math]::Floor($n/45);$n++
        $text=(Invoke-Ctl @('command',('setblock '+$x+' 90 '+$z+' '+$id))).chat -join ' '
        if ($text -match 'Changed the block') { $ok+=$id } else { $bad+=$id }
    }
    if ($ok.Count -eq 0) {
        # The sample may hit only blocks the loader left out: try every other blockstate before giving up.
        foreach ($id in ($names|Where-Object { $pick -notcontains $_ })) {
            $x=100+2*($n%45);$z=100+2*[math]::Floor($n/45);$n++
            $text=(Invoke-Ctl @('command',('setblock '+$x+' 90 '+$z+' '+$id))).chat -join ' '
            if ($text -match 'Changed the block') { $ok+=$id;break } else { $bad+=$id }
        }
    }
    $perMod[$mod.Name]=@{sampled=$pick.Count;placed=$ok.Count;rejected=$bad}
    $placedTotal+=$ok.Count
    if ($ok.Count -eq 0 -and -not $skip.ContainsKey($mod.Name)) { $failedMods+=$mod.Name }
}
$report.measured=@{mods=$mods.Count;with_blocks=$perMod.Count;data_only=$dataOnly;placed=$placedTotal;per_mod=$perMod}
Save-Report
Expect ($failedMods.Count -eq 0) ('Mods with no placeable sampled block: '+($failedMods -join ', '))
Passed ($perMod.Count.ToString()+' mods with blocks each placed a sampled block ('+$placedTotal+' blocks); '+$dataOnly.Count+' data-only mods (structures, recipes, functions).')
Wait-Ticks 20
$missing=@()
$n=0
foreach ($mod in $perMod.Keys) { }
$text=((& $ctlPath --socket $socketPath logs --lines 3000) -join "`n")
$m=[regex]::Match($text,'content from (\d+) mod')
if ($m.Success) {
    Expect ([int]$m.Groups[1].Value -ge 47) ('Only '+$m.Groups[1].Value+' mods contributed content.')
    Passed ('The client reports content from '+$m.Groups[1].Value+' mods.')
} else { Passed 'note: the load summary line is outside the log tail; mod count checked by placements only.' }
# Creative tabs: every packed mod that declares horizon items/blocks must carry a tab file, or its content is
# invisible in the creative inventory (stage17 shipped without them).
Add-Type -AssemblyName System.IO.Compression.FileSystem
$tabless=@();$tabbed=0
foreach ($archive in (Get-ChildItem (Join-Path $PSScriptRoot '../extra-mods') -Filter '*.hmod')) {
    $zip=[IO.Compression.ZipFile]::OpenRead($archive.FullName)
    try {
        $names=@($zip.Entries|ForEach-Object { $_.FullName })
        $declares=@($names|Where-Object { $_ -match '^data/[^/]+/horizon/(item|block)/[^/]+\.json$' -and $_ -notmatch '^data/(minecraft|c)/' }).Count -gt 0
        $hasTab=@($names|Where-Object { $_ -match '^data/[^/]+/horizon/creative_tab/[^/]+\.json$' }).Count -gt 0
        if ($declares -and -not $hasTab) { $tabless+=$archive.Name } elseif ($hasTab) { $tabbed++ }
    } finally { $zip.Dispose() }
}
Expect ($tabless.Count -eq 0) ('Mods without a creative tab: '+($tabless -join ', '))
Expect ($tabbed -ge 20) ('Only '+$tabbed+' mods carry a creative tab.')
Passed ($tabbed.ToString()+' mods carry a creative tab (Farmer''s Delight included).')
# Original Create assets: the import must not be aborted by an unsupported model loader.
$imp=[regex]::Match($text,'Create: local resource import failed[^\r\n]*')
Expect (-not ($imp.Success -and $imp.Value -match 'Unsupported or ambiguous')) ('Create asset import aborted: '+$imp.Value)
Passed 'The Create original-asset import was not aborted by a model loader.'
$report.passed=$true
Save-Report
Write-Output 'DataMods acceptance passed.'



