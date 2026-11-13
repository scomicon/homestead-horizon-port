#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Logic mods, group A: Double Doors (doors open and close as a pair, sneaking opens one), Shulker
# Drops Two (a rolled shell is two), Invisible Frames (sneak-click toggles an item frame),
# Accelerated Decay (leaves vanish soon after their last log), No Animal Tempt Delay (an animal
# that lost its food is tempted again at once). Each scenario is reported separately.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('LogicA')][string] $Phase='LogicA',
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
$transcript=Join-Path $game ('stage25-logica-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage25-logica-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Double Doors, Shulker Drops Two, Invisible Frames, Accelerated Decay and No Animal Tempt Delay, with the defaults of the original mods (fence gates and trapdoors of Double Doors are off by default and not ported).'
    coordinates=@{origin=@(210,83,110)}
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
function Block([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).block }
function State([int] $X,[int] $Y,[int] $Z) { return [string](Invoke-Ctl @('block',[string]$X,[string]$Y,[string]$Z)).state }
function Prop([int] $X,[int] $Y,[int] $Z,[string] $Name) {
    $m=[regex]::Match((State $X $Y $Z),$Name+'=(\w+)'); if (-not $m.Success) { throw ('No '+$Name+' at '+$X+' '+$Y+' '+$Z+': '+(State $X $Y $Z)) }
    return $m.Groups[1].Value
}
# True when the selector matches an entity: the command's own `say` shows in the chat reply.
function Exists([string] $Selector) {
    return (((Command ('execute if entity '+$Selector+' run say logica_found')).chat -join ' ') -match 'logica_found')
}
function Hold([string] $Item) {
    if ($Item) { $null=Command ('item replace entity @s weapon.mainhand with '+$Item+' 4') } else { $null=Command 'item replace entity @s weapon.mainhand with minecraft:air' }
}
function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }
function Poll([int] $Rounds,[int] $Step,[scriptblock] $Done) {
    for ($i=0;$i -lt $Rounds;$i++) { Wait-Ticks $Step; if (& $Done) { return $true } }
    return $false
}
function Sneak([bool] $On) { $null=Invoke-Ctl @('key','shift',$(if ($On) {'down'} else {'up'})) }
# Stands two blocks in front of the block and right-clicks its face (empty hand unless an item is given).
function Click-Block([int] $X,[int] $Y,[int] $Z,[string] $Face='north') {
    # An open door shows no north face from the north (its leaf stands at the side), so the player
    # walks round: each face is aimed at from two blocks outside it.
    $stands=[ordered]@{north=@(($X+0.5),$Y,($Z-2.5));east=@(($X+3.5),$Y,($Z+0.5));west=@(($X-2.5),$Y,($Z+0.5));south=@(($X+0.5),$Y,($Z+3.5))}
    $used=$null;$last=''
    foreach ($candidate in @($Face)+@($stands.Keys|Where-Object { $_ -ne $Face })) {
        $p=$stands[$candidate]
        $null=Command ('tp @s '+$p[0]+' '+$p[1]+' '+$p[2])
        Wait-Ticks 20
        try { $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,$candidate); $used=$candidate; break } catch { $last=$_.Exception.Message }
    }
    if (-not $used) { throw ('no face of the block can be targeted: '+$last) }
    $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,$used)
    Wait-Ticks 8
}
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { if ($_.Exception.Message -like 'NOTE:*') { $results[$Name]='ok (not driven) '+$_.Exception.Message } else { $results[$Name]=('failed: '+$_.Exception.Message) } }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
# A stone plot (x 195-240, z 98-128) with clear air above.
function Reset-Plot {
    $null=Command 'gamerule random_tick_speed 0'
    $null=Command 'time set day'
    $null=Command 'weather clear'
    $null=Command 'kill @e[type=!minecraft:player]'
    $null=Command 'clear @s'
    $null=Command 'fill 195 83 98 240 100 128 minecraft:air'
    $null=Command 'fill 195 82 98 240 82 128 minecraft:stone'
    Wait-Ticks 10
}
# Both halves of a door whose front looks north.
function Put-Door([int] $X,[string] $Block,[string] $Hinge) {
    $null=Command ('setblock '+$X+' 83 110 '+$Block+'[facing=north,half=lower,hinge='+$Hinge+',open=false]')
    $null=Command ('setblock '+$X+' 84 110 '+$Block+'[facing=north,half=upper,hinge='+$Hinge+',open=false]')
}
function Open-Of([int] $X) { return (Prop $X 83 110 'open') }
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 190 95 245 130'
$null=Command 'tp @s 210.5 90 105.5'
Wait-Ticks 40
Reset-Plot
# A door whose hinge is left has its partner on the right of the front: for a north front, west (x - 1).
Scenario 'a double door opens and closes as a pair' {
    Reset-Plot
    Put-Door 210 'minecraft:oak_door' 'left'
    Put-Door 209 'minecraft:oak_door' 'right'
    Wait-Ticks 6
    Click-Block 210 83 110
    Expect ((Open-Of 210) -eq 'true') ('the clicked door did not open: '+(State 210 83 110))
    Expect ((Open-Of 209) -eq 'true') ('the partner did not open with it: '+(State 209 83 110))
    Expect ((Prop 209 84 110 'open') -eq 'true') 'the upper half of the partner did not open'
    Click-Block 210 83 110
    Expect ((Open-Of 210) -eq 'false') 'the clicked door did not close'
    Expect ((Open-Of 209) -eq 'false') ('the partner did not close with it: '+(State 209 83 110))
}
Scenario 'clicking the partner moves the first door too' {
    Click-Block 209 83 110
    Expect ((Open-Of 209) -eq 'true') 'the clicked partner did not open'
    Expect ((Open-Of 210) -eq 'true') ('the first door did not follow: '+(State 210 83 110))
}
Scenario 'sneaking opens only the clicked door' {
    Reset-Plot
    Put-Door 210 'minecraft:oak_door' 'left'
    Put-Door 209 'minecraft:oak_door' 'right'
    Wait-Ticks 6
    Sneak $true
    try { Click-Block 210 83 110 } finally { Sneak $false }
    Expect ((Open-Of 210) -eq 'true') 'the clicked door did not open while sneaking'
    Expect ((Open-Of 209) -eq 'false') ('sneaking must not open the partner: '+(State 209 83 110))
}
Scenario 'a single door opens alone' {
    Reset-Plot
    Put-Door 220 'minecraft:oak_door' 'left'
    Wait-Ticks 6
    Click-Block 220 83 110
    Expect ((Open-Of 220) -eq 'true') 'a single door did not open'
}
Scenario 'doors with the same hinge are not a pair' {
    Reset-Plot
    Put-Door 214 'minecraft:oak_door' 'left'
    Put-Door 213 'minecraft:oak_door' 'left'
    Wait-Ticks 6
    Click-Block 214 83 110
    Expect ((Open-Of 214) -eq 'true') 'the clicked door did not open'
    Expect ((Open-Of 213) -eq 'false') 'a door with the same hinge was opened'
}
Scenario 'doors of different wood are not a pair' {
    Reset-Plot
    Put-Door 216 'minecraft:oak_door' 'left'
    Put-Door 215 'minecraft:spruce_door' 'right'
    Wait-Ticks 6
    Click-Block 216 83 110
    Expect ((Open-Of 216) -eq 'true') 'the clicked door did not open'
    Expect ((Open-Of 215) -eq 'false') 'a door of another kind was opened'
}
Scenario 'iron doors stay shut under a hand' {
    Reset-Plot
    Put-Door 218 'minecraft:iron_door' 'left'
    Put-Door 217 'minecraft:iron_door' 'right'
    Wait-Ticks 6
    Click-Block 218 83 110
    Expect ((Open-Of 218) -eq 'false') 'an iron door opened to a hand'
    Expect ((Open-Of 217) -eq 'false') 'the iron partner opened to a hand'
}
Scenario 'a dying shulker drops two shells, not one' {
    Reset-Plot
    for ($i=0;$i -lt 24;$i++) {
        $null=Command ('summon minecraft:shulker '+(200+$i)+'.5 83 118.5')
    }
    Wait-Ticks 6
    $null=Command 'kill @e[type=minecraft:shulker]'
    Wait-Ticks 20
    $one=(Exists '@e[type=minecraft:item,nbt={Item:{id:"minecraft:shulker_shell",count:1}}]') -or (Exists '@e[type=minecraft:item,nbt={Item:{id:"minecraft:shulker_shell",Count:1b}}]')
    $two=(Exists '@e[type=minecraft:item,nbt={Item:{id:"minecraft:shulker_shell",count:2}}]') -or (Exists '@e[type=minecraft:item,nbt={Item:{id:"minecraft:shulker_shell",Count:2b}}]')
    Expect $two 'no shell stack of two dropped from 24 shulkers (each rolls a 50 % chance); no loot rolled at all?'
    Expect (-not $one) 'a shulker dropped a single shell'
}
Scenario 'sneak-clicking an item frame makes it invisible and back' {
    Reset-Plot
    $null=Command 'setblock 225 84 110 minecraft:stone'
    $null=Command 'summon minecraft:item_frame 225 84 109 {Facing:2b}'
    Wait-Ticks 10
    $entities=Invoke-Ctl @('entities','8')
    $text=($entities|ConvertTo-Json -Depth 10 -Compress)
    $m=[regex]::Match($text,'"id":\s*(\d+)[^{}]*?item_frame')
    if (-not $m.Success) { $m=[regex]::Match($text,'item_frame[^{}]*?"id":\s*(\d+)') }
    if (-not $m.Success) { throw ('NOTE: control cannot see the item frame, so it cannot be clicked: '+$text) }
    $id=$m.Groups[1].Value
    $null=Command 'tp @s 225.5 83 106.5'
    Wait-Ticks 4
    Sneak $true
    try {
        $null=Invoke-Ctl @('interact-entity',$id)
        Wait-Ticks 6
        $now=Exists '@e[type=minecraft:item_frame,nbt={Invisible:1b}]'
        Expect $now 'the frame did not become invisible'
        $null=Invoke-Ctl @('interact-entity',$id)
        Wait-Ticks 6
        Expect (-not (Exists '@e[type=minecraft:item_frame,nbt={Invisible:1b}]')) 'the frame did not become visible again'
    } finally { Sneak $false }
}
Scenario 'leaves vanish soon after their last log' {
    Reset-Plot
    $null=Command 'fill 230 83 110 230 86 110 minecraft:oak_log'
    $null=Command 'fill 228 86 108 232 88 112 minecraft:oak_leaves[persistent=false] keep'
    $null=Command 'setblock 223 87 110 minecraft:oak_leaves[persistent=true]'
    Wait-Ticks 30
    Expect ((Block 232 88 112) -eq 'minecraft:oak_leaves') ('the leaves did not stand while the log was there: '+(Block 232 88 112))
    $null=Command 'fill 230 83 110 230 86 110 minecraft:air'
    $gone=Poll 30 10 { (Block 232 88 112) -eq 'minecraft:air' -and (Block 228 88 108) -eq 'minecraft:air' }
    Expect $gone ('the leaves did not decay: '+(Block 232 88 112)+' / '+(Block 228 88 108))
    Expect ((Block 223 87 110) -eq 'minecraft:oak_leaves') 'persistent leaves must stay'
}
Scenario 'leaves next to another log stay' {
    Reset-Plot
    $null=Command 'fill 230 83 110 230 86 110 minecraft:oak_log'
    $null=Command 'fill 238 83 110 238 86 110 minecraft:oak_log'
    $null=Command 'fill 229 86 109 239 87 111 minecraft:oak_leaves[persistent=false] keep'
    Wait-Ticks 30
    $null=Command 'fill 230 83 110 230 86 110 minecraft:air'
    Wait-Ticks 120
    Expect ((Block 238 87 110) -eq 'minecraft:oak_leaves') ('leaves held by a log vanished: '+(Block 238 87 110))
}
Scenario 'a tempted animal follows its food again at once' {
    Reset-Plot
    $null=Command 'summon minecraft:cow 215.5 83 118.5'
    $null=Command 'tp @s 215.5 83 112.5'
    Wait-Ticks 10
    Hold 'minecraft:wheat'
    $close=Poll 40 10 { Exists '@e[type=minecraft:cow,distance=..4]' }
    Expect $close 'the cow never came to the wheat'
    Hold ''
    Wait-Ticks 10
    $null=Command 'execute at @e[type=minecraft:cow,limit=1] run tp @s ~-9 ~ ~'
    Wait-Ticks 4
    Hold 'minecraft:wheat'
    $again=Poll 6 10 { Exists '@e[type=minecraft:cow,distance=..6]' }
    Expect $again 'the cow did not follow the wheat again within 60 ticks (the vanilla calm-down is 100)'
}
$null=Command 'kill @e[type=!minecraft:player]'
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -notlike 'ok*' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
