#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Diagnostic acceptance. UI stacks are NOT evidence of rendered icons.
# Does not launch or stop the game. An incomplete visual check never passes.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('CreativeTabs')][string] $Phase='CreativeTabs',
    [string] $World='HomesteadAutomationAcceptance',
    [int] $GameProcessId=0,
    [ValidateRange(1,64)][int] $MaxTabPages=16,
    [ValidateRange(1,512)][int] $MaxScrollRows=512
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$cultureBefore=[Threading.Thread]::CurrentThread.CurrentCulture
[Threading.Thread]::CurrentThread.CurrentCulture=[Globalization.CultureInfo]::InvariantCulture
$stage=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$game=[IO.Path]::GetFullPath($GameDir)
if (-not $game.StartsWith((Join-Path $stage 'runtime')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
    throw 'Use the disposable stage4 instance only.'
}
if ($World -notmatch '^HomesteadAutomation[A-Za-z0-9_-]+$') { throw 'Use a disposable automation world.' }
if (-not (Test-Path -LiteralPath $game -PathType Container)) { throw 'Start the disposable client first.' }
# Do not follow a directory junction outside the workspace when saving evidence.
for ($dir=[IO.DirectoryInfo]::new($game); $null -ne $dir; $dir=$dir.Parent) {
    if ($dir.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse-point game directories are not supported.' }
}
$utf8=[Text.UTF8Encoding]::new($false)
$prefix='stage23-creative-tabs-'+$Phase.ToLowerInvariant()
$reportPath=Join-Path $game ($prefix+'-result.json')
$report=[ordered]@{
    schema_version=1; suite='stage23-creative-tabs'; phase=$Phase
    run_id=[guid]::NewGuid().ToString('N'); started_utc=[DateTime]::UtcNow.ToString('o')
    completed_utc=$null; passed=$false; automation_passed=$false; status='running'
    world=$World; process_id=$GameProcessId; binary_sha256=$null; script_sha256=$null
    checks=@(); tabs=@(); tooltip=[ordered]@{status='not_verified'; text=$null}
    not_verified=@('Rendered icons: socket UI exposes stacks, not baked geometry or visible pixels.',
        'Hovered creative tooltip appearance: held-item text is checked separately.',
        'Cold cache: first traversal does not prove an empty model/sprite/OS cache.',
        'Lag fix: measurements are diagnostic, with no established performance threshold.')
    error=$null; cleanup_errors=@()
}
function Save-Report {
    [IO.File]::WriteAllText($reportPath,($report|ConvertTo-Json -Depth 40)+[Environment]::NewLine,$utf8)
}
# Invalidate a previous PASS before resolving tools or reading the run record.
Save-Report
function Invoke-Ctl([string[]] $Words) {
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$ctlPath; $info.UseShellExecute=$false; $info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
    foreach ($word in (@('--socket',$socketPath)+$Words)) { $info.ArgumentList.Add([string]$word) }
    $process=[Diagnostics.Process]::Start($info)
    try {
        $outTask=$process.StandardOutput.ReadToEndAsync()
        $errTask=$process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(45000)) { $process.Kill(); throw 'Control timeout.' }
        $stdout=$outTask.GetAwaiter().GetResult()
        $null=$errTask.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw 'Control process failed.' }
        $reply=ConvertFrom-Json -InputObject $stdout -AsHashtable
        if ($reply -isnot [Collections.IDictionary] -or $reply.Contains('error')) { throw 'Control rejected the request.' }
        return $reply
    } finally { $process.Dispose() }
}
function Command([string] $Text) {
    $reply=Invoke-Ctl @('command',$Text)
    if (($reply.chat -join ' ') -match 'Unknown |not loaded|Incorrect argument|Expected |Invalid|incomplete command') {
        throw 'Game command was not accepted.'
    }
    return $reply
}
function Check([string] $Name,[bool] $Condition) {
    $report.checks+=@{name=$Name; status=$(if ($Condition) {'passed'} else {'failed'})}
    Save-Report
    if (-not $Condition) { throw ('Check failed: '+$Name) }
}
function Read-Ui {
    $ui=Invoke-Ctl @('ui')
    if ($ui.screen -ne 'creative' -or $null -eq $ui.gui_size -or $ui.gui_size.Count -ne 2) {
        throw 'Rendered creative screen required.'
    }
    return $ui
}
function Grid([Collections.IDictionary] $Ui) {
    $slots=@{}
    foreach ($node in $Ui.widgets) {
        if ($node.id -match '^slot-(9[0-3][0-9]|94[0-4])$') { $slots[[int]$node.id.Substring(5)]=$node }
    }
    if ($slots.Count -ne 45) { throw 'Expected display slots 900..944.' }
    return ,@(900..944 | ForEach-Object { $slots[$_] })
}
function Grid-Ids([Collections.IDictionary] $Ui) {
    return ,@((Grid $Ui) | ForEach-Object {
        if ($null -ne $_.stack -and $_.stack.count -gt 0) { [string]$_.stack.item } else { '' }
    })
}
function After-Frame([long] $Frame) {
    $deadline=[DateTime]::UtcNow.AddSeconds(10)
    do {
        $ui=Read-Ui
        if ([long]$ui.frame -gt $Frame) { return $ui }
        Start-Sleep -Milliseconds 30
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'No newly rendered creative frame.'
}
function Origin([Collections.IDictionary] $Ui) {
    return @([Math]::Truncate(($Ui.gui_size[0]-195)/2),[Math]::Truncate(($Ui.gui_size[1]-136)/2))
}
function Click-Point([double] $X,[double] $Y) {
    $before=Read-Ui
    $null=Invoke-Ctl @('click-at',[string]$X,[string]$Y)
    return (After-Frame $before.frame)
}
function Scroll([Collections.IDictionary] $Ui,[int] $Steps) {
    $o=Origin $Ui
    # The scrollbar track avoids hovering an item (tooltip work) during measurement.
    $null=Invoke-Ctl @('scroll-at',[string]($o[0]+182),[string]($o[1]+70),[string]$Steps)
    return (After-Frame $Ui.frame)
}
function Match-Grid([Collections.IDictionary] $Ui,[string[]] $Items,[int] $Offset) {
    $ids=Grid-Ids $Ui
    for ($i=0; $i -lt 45; $i++) {
        $expected=if ($Offset+$i -lt $Items.Count) { $Items[$Offset+$i] } else { '' }
        if ($ids[$i] -cne $expected) { return $false }
    }
    return $true
}
function Measure-Scroll([string] $Kind,[string[]] $Items,[Collections.IDictionary] $Ui) {
    $rows=[Math]::Max(0,[int][Math]::Ceiling($Items.Count/9.0)-5)
    if ($rows -eq 0) {
        return @{kind=$Kind;status='not_applicable';rows=0;stats=$null;reason='Tab fits in 45 slots.'}
    }
    if ($rows -gt $MaxScrollRows) { throw 'Scroll row limit exceeded; no partial traversal is accepted.' }
    $null=Invoke-Ctl @('stats','reset')
    $timer=[Diagnostics.Stopwatch]::StartNew()
    for ($row=1; $row -le $rows; $row++) {
        $Ui=Scroll $Ui -1
        if (-not (Match-Grid $Ui $Items ($row*9))) { throw 'Scrolling did not advance exactly one expected row.' }
    }
    $stats=Invoke-Ctl @('stats','raw')
    $timer.Stop()
    if (-not $stats.ContainsKey('frame_ms') -or $stats.frames -lt $rows -or
        @($stats.frame_ms).Count -ne [int]$stats.frames) { throw 'Missing or incomplete frame statistics.' }
    foreach ($key in 'p50_ms','p95_ms','p99_ms') {
        if (-not $stats.ContainsKey($key) -or $null -eq $stats[$key] -or
            -not [double]::IsFinite([double]$stats[$key])) { throw 'Invalid frame percentiles.' }
    }
    foreach ($value in $stats.frame_ms) {
        if (-not [double]::IsFinite([double]$value) -or [double]$value -lt 0) { throw 'Invalid frame duration.' }
    }
    return @{kind=$Kind;status='measured';rows=$rows;elapsed_ms=$timer.Elapsed.TotalMilliseconds
        stats=$stats;cache_cold_verified=$false
        window='Whole traversal, including controller and UI polling overhead; not per-event latency.'}
}
function Read-TabInputs {
    $tabs=@{}
    foreach ($archive in (Get-ChildItem -LiteralPath (Join-Path $game 'horizon-mods') -Filter '*.hmod' -File)) {
        $zip=[IO.Compression.ZipFile]::OpenRead($archive.FullName)
        try {
            foreach ($entry in $zip.Entries) {
                if ($entry.FullName -notmatch '^data/(mcwholidays|regions_unexplored|perfectplushies|phomesteadplushies)/horizon/creative_tab/[^/]+\.json$') { continue }
                $ns=$Matches[1]
                if ($tabs.ContainsKey($ns)) { throw 'Multiple tab declarations for one target namespace.' }
                $reader=[IO.StreamReader]::new($entry.Open())
                try { $data=ConvertFrom-Json -InputObject $reader.ReadToEnd() -AsHashtable } finally { $reader.Dispose() }
                $items=@($data.items)
                if ($items.Count -eq 0 -or @($items | Where-Object { $_ -isnot [string] -or $_ -notmatch ('^'+$ns+':[a-z0-9_./-]+$') }).Count) {
                    throw 'Expected a nonempty namespace-local string item list.'
                }
                $tabs[$ns]=@{items=$items;archive=$archive.Name;sha256=(Get-FileHash -LiteralPath $archive.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
            }
        } finally { $zip.Dispose() }
    }
    return $tabs
}
$bound=$false; $screenChanged=$false; $selectedBefore=$null; $testSlot=$null; $active='preflight'
try {
    $ctlPath=(Resolve-Path -LiteralPath $Ctl).Path
    $socketPath=[IO.Path]::GetFullPath($Socket)
    $run=Get-Content -Encoding UTF8 -LiteralPath (Join-Path $stage 'runtime/current-run.json') -Raw | ConvertFrom-Json -AsHashtable
    Check 'run identity matches disposable instance' (
        [IO.Path]::GetFullPath($run.game) -eq $game -and
        [IO.Path]::GetFullPath($run.socket) -eq $socketPath -and $run.world -eq $World -and
        ($GameProcessId -eq 0 -or $GameProcessId -eq $run.process_id))
    $gameProcess=Get-Process -Id $run.process_id
    Check 'live process matches recorded binary' ($gameProcess.Path -eq $run.binary)
    $report.process_id=[int]$run.process_id
    $report.binary_sha256=(Get-FileHash -LiteralPath $run.binary -Algorithm SHA256).Hash.ToLowerInvariant()
    $report.script_sha256=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $state=Invoke-Ctl @('state')
    Check 'rendered creative-mode world is playing' ($state.screen -eq 'playing' -and $state.creative -and $state.frames -gt 0)
    $bound=$true
    $inputs=Read-TabInputs
    Check 'all four target tabs have unique declarations' ($inputs.Count -eq 4)
    $inventory=Invoke-Ctl @('inventory')
    Check 'no carried stack before navigation' ($null -eq $inventory.carried)
    $selectedBefore=[int]$inventory.selected
    $screenChanged=$true
    $null=Invoke-Ctl @('open-inventory')
    $ui=Read-Ui
    # This pinned screen does not expose creative tab headers/page numbers in ui.
    # Use its source-defined GUI geometry and verify identity by all first 45 IDs.
    $o=Origin $ui
    if ($o[1]-40 -lt 0 -or $o[1]+148 -ge $ui.gui_size[1]) { throw 'GUI too small for the creative page controls.' }
    for ($page=0; $page -lt $MaxTabPages; $page++) { $ui=Click-Point ($o[0]+10) ($o[1]-40) }
    $seen=@{}
    for ($page=0; $page -lt $MaxTabPages -and $seen.Count -lt 4; $page++) {
        foreach ($top in @($true,$false)) {
            for ($col=0; $col -lt 5; $col++) {
                $active='tab navigation and measurement'
                $ui=Click-Point ($o[0]+$col*27+13) ($o[1]+$(if ($top) {-14} else {148}))
                # Select resets scroll for a different tab; force the top for an already selected one.
                $ui=Scroll $ui ($MaxScrollRows+5)
                $ids=Grid-Ids $ui
                if (-not $ids[0].Contains(':')) { continue }
                $ns=$ids[0].Split(':')[0]
                if (-not $inputs.ContainsKey($ns) -or $seen.ContainsKey($ns)) { continue }
                $inputTab=$inputs[$ns]
                Check ($ns+': first 45 stack IDs and padding match installed hmod') (Match-Grid $ui $inputTab.items 0)
                $seen[$ns]=$true
                $tab=[ordered]@{namespace=$ns;archive=$inputTab.archive;archive_sha256=$inputTab.sha256
                    item_count=$inputTab.items.Count;first_45_ids=$ids;stack_check='passed'
                    padding_slots=[Math]::Max(0,45-$inputTab.items.Count);icons_status='not_verified'
                    screenshot=$null;cold=$null;warm=$null}
                $report.tabs+=,$tab
                $shotName=$prefix+'-'+$report.run_id+'-'+$ns+'-first45.png'
                $shotPath=Join-Path $game $shotName
                $null=Invoke-Ctl @('screenshot',$shotPath)
                Check ($ns+': screenshot saved') (Test-Path -LiteralPath $shotPath -PathType Leaf)
                $tab.screenshot=$shotName
                $tab.cold=Measure-Scroll 'cold_first_traversal' $inputTab.items $ui
                Save-Report
                $ui=Scroll (Read-Ui) ($MaxScrollRows+5)
                Check ($ns+': returned to top before warm traversal') (Match-Grid $ui $inputTab.items 0)
                $tab.warm=Measure-Scroll 'warm_repeat_traversal' $inputTab.items $ui
                Save-Report
            }
        }
        $ui=Click-Point ($o[0]+185) ($o[1]-40)
    }
    Check 'all four target tabs opened and measured' ($seen.Count -eq 4)
    $active='held-item tooltip'
    $null=Invoke-Ctl @('key','esc','tap')
    $inventory=Invoke-Ctl @('inventory')
    $empty=@($inventory.hotbar | Where-Object { $null -eq $_.item })
    $lang=(Invoke-Ctl @('options-get','lang')).options.lang.value
    if ($empty.Count -eq 0) {
        $report.tooltip.reason='No empty hotbar slot; existing stacks were preserved.'
    } elseif ($lang -ne 'en_us') {
        $report.tooltip.reason='English locale required for the exact When on Head assertion.'
    } else {
        $testSlot=[int]$empty[0].slot
        $null=Invoke-Ctl @('select',[string]($testSlot+1))
        $target='mcwholidays:purple_ornament_wall_deco_2'
        $null=Command ('item replace entity @s weapon.mainhand with '+$target)
        Check 'tooltip target is held with exact registry ID' ((Invoke-Ctl @('inventory')).held.item -eq $target)
        $reply=Command 'homestead tooltip show'
        $lines=@($reply.chat | Where-Object { $_.StartsWith('homestead tooltip lines: ') })
        if ($lines.Count -ne 1 -or $lines[0] -notmatch '^homestead tooltip lines: Purple Ornament Wall Deco 2(?: \||$)') {
            $report.tooltip.reason='No unique complete tooltip response for the target; text is not verified.'
        } else {
            $report.tooltip.text=$lines[0]
            Check 'target tooltip has no When on Head or Waypoint Transmit Range' ($lines[0] -notmatch 'When on Head|Waypoint Transmit Range')
            $report.tooltip.status='passed'
        }
    }
    $report.automation_passed=($report.tooltip.status -eq 'passed')
    $report.status='not_verified'
} catch {
    $report.status='failed'
    # Do not persist raw exception/control output: it may contain local paths or account names.
    $report.error=@{stage=$active;type=$_.Exception.GetType().Name;message='Scenario failed; inspect failed checks and the named stage.'}
} finally {
    if ($bound) {
        if ($null -ne $testSlot) {
            try {
                $null=Invoke-Ctl @('select',[string]($testSlot+1))
                $held=(Invoke-Ctl @('inventory')).held
                if ($null -ne $held -and $held.item -eq 'mcwholidays:purple_ornament_wall_deco_2') {
                    $null=Command 'item replace entity @s weapon.mainhand with minecraft:air'
                }
            } catch { $report.cleanup_errors+='Could not clear the test item.' }
        }
        if ($null -ne $selectedBefore) {
            try { $null=Invoke-Ctl @('select',[string]$selectedBefore) } catch { $report.cleanup_errors+='Could not restore selected hotbar slot.' }
        }
        if ($screenChanged) {
            try {
                if ((Invoke-Ctl @('screen')).screen -eq 'creative') { $null=Invoke-Ctl @('key','esc','tap') }
                if ((Invoke-Ctl @('screen')).screen -ne 'playing') { throw 'Screen not restored.' }
            } catch { $report.cleanup_errors+='Could not restore the playing screen.' }
        }
    }
    if ($report.cleanup_errors.Count) { $report.status='failed'; $report.automation_passed=$false }
    $report.completed_utc=[DateTime]::UtcNow.ToString('o')
    Save-Report
    [Threading.Thread]::CurrentThread.CurrentCulture=$cultureBefore
}
# Missing visual evidence blocks the standard runner/package gate, even when socket checks pass.
throw ('CreativeTabs '+$report.status+': inspect '+$prefix+'-result.json; icon appearance is not verified.')
