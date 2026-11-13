#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Farmer's Delight crops: seeds, onions and rice plant their crop, crops grow, ripe crops are harvested
# by a right click, rice needs water and bears panicles, rich soil takes a hoe and speeds growth, compost matures.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Ctl,
    [Parameter(Mandatory)][string] $Socket,
    [Parameter(Mandatory)][string] $GameDir,
    [ValidateSet('Crops')][string] $Phase='Crops',
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
$transcript=Join-Path $game ('stage24-crops-'+$Phase.ToLowerInvariant()+'-transcript.jsonl')
$reportPath=Join-Path $game ('stage24-crops-'+$Phase.ToLowerInvariant()+'-result.json')
$utf8=[Text.UTF8Encoding]::new($false)
$report=[ordered]@{
    passed=$false;phase=$Phase;world=$World;process_id=$GameProcessId;checks=@()
    scope='Farmer''s Delight crops: seed, onion and rice items plant cabbages, onions, budding tomatoes and rice (rice only in water); crops and the tomato vine grow; ripe crops are harvested by a right click and replanted; rice bears panicles; a hoe makes rich soil farmland; crops stand on it and grow faster; organic compost matures into rich soil; wild crops stand. Each scenario is reported separately.'
    coordinates=@{origin=@(170,83,66)}
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
function Age([int] $X,[int] $Y,[int] $Z) {
    $m=[regex]::Match((State $X $Y $Z),'age=(\d+)'); if (-not $m.Success) { throw ('No age at '+$X+' '+$Y+' '+$Z+': '+(State $X $Y $Z)) }
    return [int]$m.Groups[1].Value
}
function Prop([int] $X,[int] $Y,[int] $Z,[string] $Name) {
    $m=[regex]::Match((State $X $Y $Z),$Name+'=(\w+)'); if (-not $m.Success) { throw ('No '+$Name+' at '+$X+' '+$Y+' '+$Z+': '+(State $X $Y $Z)) }
    return $m.Groups[1].Value
}
function Have([string] $Id) {
    $text=((Command ('execute if entity @e[type=minecraft:item,x=178,y=84,z=72,distance=..40,nbt={Item:{id:"'+$Id+'"}}] run say crops_item_found')).chat -join ' ')
    if ($text -match 'crops_item_found') { return $true }
    $inv=(Invoke-Ctl @('inventory')).counts
    return ($inv.ContainsKey($Id) -and [int]$inv[$Id] -ge 1)
}
function Hold([string] $Item) {
    if ($Item) { $null=Command ('item replace entity @s weapon.mainhand with '+$Item+$(if ($Item -like '*_hoe') {''} else {' 4'})) } else { $null=Command 'item replace entity @s weapon.mainhand with minecraft:air' }
}
# Stands three blocks above the clicked block and clicks its top face with the held item.
function Use-On([int] $X,[int] $Y,[int] $Z,[string] $Item) {
    $null=Command ('tp @s '+($X+0.5)+' '+($Y+3)+' '+($Z+0.5))
    Wait-Ticks 4
    Hold $Item
    $null=Invoke-Ctl @('look-at','block',[string]$X,[string]$Y,[string]$Z,'up')
    $null=Invoke-Ctl @('interact',[string]$X,[string]$Y,[string]$Z,'up')
    Wait-Ticks 8
}
function Expect([bool] $Condition,[string] $Message) { if (-not $Condition) { throw $Message } }
function Poll([int] $Rounds,[int] $Step,[scriptblock] $Done) {
    for ($i=0;$i -lt $Rounds;$i++) { Wait-Ticks $Step; if (& $Done) { return $true } }
    return $false
}
$results=[ordered]@{}
function Scenario([string] $Name,[scriptblock] $Body) {
    try { & $Body; $results[$Name]='ok' } catch { $results[$Name]=('failed: '+$_.Exception.Message) }
    Write-Output ($Name+': '+$results[$Name])
    $report.checks+=($Name+': '+$results[$Name]);Save-Report
}
# A moist farm plot (water row at z=76), a one deep sand pond (x 170-176, z 66-68) and a dirt strip.
function Reset-Plot {
    $null=Command 'gamerule random_tick_speed 0'
    $null=Command 'time set day'
    $null=Command 'weather clear'
    $null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'clear @s'
    $null=Command 'fill 168 82 62 190 95 90 minecraft:air'
    $null=Command 'fill 168 82 62 190 82 90 minecraft:dirt'
    $null=Command 'fill 170 83 72 186 83 80 minecraft:farmland'
    $null=Command 'fill 170 83 76 186 83 76 minecraft:water'
    $null=Command 'fill 169 83 65 177 84 69 minecraft:stone'
    $null=Command 'fill 170 83 66 176 83 68 minecraft:sand'
    $null=Command 'fill 170 84 66 176 84 68 minecraft:water'
    Wait-Ticks 20
}
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 160 55 200 100'
$null=Command 'tp @s 175.5 90 70.5'
Wait-Ticks 40
Reset-Plot
Scenario 'crop items and blocks exist' {
    foreach ($item in 'farmersdelight:rice','farmersdelight:cabbage_seeds','farmersdelight:tomato_seeds','farmersdelight:onion','farmersdelight:rich_soil','farmersdelight:organic_compost','farmersdelight:wild_cabbages','farmersdelight:wild_rice') {
        $null=Command ('give @s '+$item+' 1')
    }
    Wait-Ticks 5
    $inv=(Invoke-Ctl @('inventory')).counts
    foreach ($item in 'farmersdelight:rice','farmersdelight:cabbage_seeds','farmersdelight:tomato_seeds','farmersdelight:onion','farmersdelight:rich_soil','farmersdelight:organic_compost','farmersdelight:wild_cabbages','farmersdelight:wild_rice') {
        Expect ($inv.ContainsKey($item)) ('no '+$item+' item: '+($inv|ConvertTo-Json -Compress))
    }
    $null=Command 'clear @s'
}
Scenario 'cabbage seeds plant cabbages on farmland' {
    Reset-Plot
    Use-On 171 83 72 'farmersdelight:cabbage_seeds'
    Expect ((Block 171 84 72) -eq 'farmersdelight:cabbages') ('found '+(Block 171 84 72))
    Expect ((Age 171 84 72) -eq 0) 'a planted cabbage must start at age 0'
}
Scenario 'an onion plants onions on farmland' {
    Use-On 172 83 72 'farmersdelight:onion'
    Expect ((Block 172 84 72) -eq 'farmersdelight:onions') ('found '+(Block 172 84 72))
}
Scenario 'tomato seeds plant a budding tomato' {
    Use-On 173 83 72 'farmersdelight:tomato_seeds'
    Expect ((Block 173 84 72) -eq 'farmersdelight:budding_tomatoes') ('found '+(Block 173 84 72))
}
Scenario 'seeds are refused on bare dirt' {
    $null=Command 'setblock 180 83 84 minecraft:dirt'
    Use-On 180 83 84 'farmersdelight:cabbage_seeds'
    Expect ((Block 180 84 84) -eq 'minecraft:air') ('a crop stood on dirt: '+(Block 180 84 84))
}
Scenario 'cabbages, onions and tomatoes grow with random ticks' {
    Reset-Plot
    foreach ($x in 171..175) {
        $null=Command "setblock $x 84 72 farmersdelight:cabbages[age=0]"
        $null=Command "setblock $x 84 73 farmersdelight:onions[age=0]"
        $null=Command "setblock $x 84 74 farmersdelight:budding_tomatoes[age=0]"
    }
    $null=Command 'gamerule random_tick_speed 300'
    $grown=Poll 10 100 {
        $cab=0;$oni=0;$vine=0
        foreach ($x in 171..175) {
            if ((Block $x 84 72) -eq 'farmersdelight:cabbages' -and (Age $x 84 72) -gt 0) { $cab++ }
            if ((Block $x 84 73) -eq 'farmersdelight:onions' -and (Age $x 84 73) -gt 0) { $oni++ }
            if ((Block $x 84 74) -eq 'farmersdelight:tomatoes') { $vine++ }
        }
        ($cab -gt 0 -and $oni -gt 0 -and $vine -gt 0)
    }
    $null=Command 'gamerule random_tick_speed 0'
    Expect $grown ('cabbages, onions and the ripe tomato bud did not all advance: '+(State 171 84 72)+' | '+(State 172 84 73)+' | '+(State 173 84 74)+' | '+(State 175 84 72)+' | '+(State 175 84 73)+' | '+(State 175 84 74)+' | farmland '+(State 171 83 72))
}
Scenario 'a ripe cabbage is harvested by a right click and keeps growing' {
    Reset-Plot
    $null=Command 'setblock 178 84 72 farmersdelight:cabbages[age=7]'
    Use-On 178 84 72 $null
    Expect ((Block 178 84 72) -eq 'farmersdelight:cabbages' -and (Age 178 84 72) -eq 0) ('the cabbage was not reset: '+(State 178 84 72))
    Expect (Have 'farmersdelight:cabbage') 'the harvest dropped no cabbage'
}
Scenario 'a ripe tomato vine is harvested by a right click' {
    $null=Command 'clear @s';$null=Command 'kill @e[type=minecraft:item]'
    $null=Command 'setblock 178 84 73 farmersdelight:tomatoes[age=3,ropelogged=false]'
    Use-On 178 84 73 $null
    Expect ((Block 178 84 73) -eq 'farmersdelight:tomatoes' -and (Age 178 84 73) -eq 0) ('the vine was not reset: '+(State 178 84 73))
    Expect (Have 'farmersdelight:tomato') 'the harvest dropped no tomato'
}
Scenario 'rice is planted only in water' {
    Reset-Plot
    Use-On 171 83 67 'farmersdelight:rice'
    Expect ((Block 171 84 67) -eq 'farmersdelight:rice') ('rice not planted in the pond: '+(Block 171 84 67))
    $null=Command 'setblock 180 83 84 minecraft:dirt'
    Use-On 180 83 84 'farmersdelight:rice'
    Expect ((Block 180 84 84) -eq 'minecraft:air') ('rice stood on dry ground: '+(Block 180 84 84))
}
Scenario 'rice grows and bears panicles' {
    Reset-Plot
    $null=Command 'setblock 173 84 67 farmersdelight:rice[age=1,supporting=false]'
    $null=Command 'gamerule random_tick_speed 300'
    $done=Poll 10 100 { (Block 173 85 67) -eq 'farmersdelight:rice_panicles' }
    $null=Command 'gamerule random_tick_speed 0'
    Expect $done ('no panicles on the grown rice; rice '+(State 173 84 67)+', above '+(Block 173 85 67))
}
Scenario 'ripe panicles are harvested by a right click' {
    Reset-Plot
    $null=Command 'setblock 175 84 67 farmersdelight:rice[age=3,supporting=false]'
    $null=Command 'setblock 175 85 67 farmersdelight:rice_panicles[age=3]'
    Use-On 175 85 67 $null
    Expect ((Block 175 85 67) -eq 'farmersdelight:rice_panicles' -and (Age 175 85 67) -eq 0) ('the panicles were not reset: '+(State 175 85 67))
    Expect (Have 'farmersdelight:rice_panicle') 'the harvest dropped no rice panicle'
}
Scenario 'a hoe makes rich soil farmland' {
    Reset-Plot
    $null=Command 'setblock 182 83 72 farmersdelight:rich_soil'
    Use-On 182 83 72 'minecraft:iron_hoe'
    Expect ((Block 182 83 72) -eq 'farmersdelight:rich_soil_farmland') ('found '+(Block 182 83 72)+' held '+((Invoke-Ctl @('inventory')).counts|ConvertTo-Json -Compress))
}
Scenario 'crops stand on rich soil farmland' {
    $null=Command 'kill @e[type=minecraft:item]'
    Use-On 182 83 72 'farmersdelight:cabbage_seeds'
    Expect ((Block 182 84 72) -eq 'farmersdelight:cabbages') ('the crop was refused: '+(Block 182 84 72))
    $null=Command 'setblock 183 83 72 farmersdelight:rich_soil_farmland[moisture=7]'
    $null=Command 'setblock 183 84 72 minecraft:wheat[age=0]'
    Wait-Ticks 20
    Expect ((Block 183 84 72) -eq 'minecraft:wheat') 'wheat did not survive on rich soil farmland'
}
Scenario 'rich soil farmland speeds up growth' {
    Reset-Plot
    foreach ($x in 171..180) {
        $null=Command "setblock $x 84 79 minecraft:wheat[age=0]"
        $null=Command "setblock $x 83 80 farmersdelight:rich_soil_farmland[moisture=7]"
        $null=Command "setblock $x 84 80 minecraft:wheat[age=0]"
    }
    $null=Command 'gamerule random_tick_speed 100'
    Wait-Ticks 300
    $null=Command 'gamerule random_tick_speed 0'
    $plain=0;$rich=0
    foreach ($x in 171..180) {
        if ((Block $x 84 79) -eq 'minecraft:wheat') { $plain+=(Age $x 84 79) }
        if ((Block $x 84 80) -eq 'minecraft:wheat') { $rich+=(Age $x 84 80) }
    }
    Expect ($plain -gt 0) ('the plain field did not grow at all: '+$plain)
    Expect ($rich -gt $plain) ('rich soil farmland must grow faster: rich '+$rich+', plain '+$plain)
}
Scenario 'organic compost matures into rich soil' {
    Reset-Plot
    $null=Command 'setblock 184 83 79 farmersdelight:organic_compost[composting=0]'
    $null=Command 'setblock 185 83 79 minecraft:podzol'
    $null=Command 'gamerule random_tick_speed 300'
    $risen=Poll 10 60 { (Block 184 83 79) -ne 'farmersdelight:organic_compost' -or (Prop 184 83 79 'composting') -ne '0' }
    Expect $risen 'the composting stage never rose'
    $null=Command 'setblock 184 83 79 farmersdelight:organic_compost[composting=7]'
    $done=Poll 15 60 { (Block 184 83 79) -eq 'farmersdelight:rich_soil' }
    $null=Command 'gamerule random_tick_speed 0'
    Expect $done ('the ripe compost did not become rich soil: '+(State 184 83 79))
}
Scenario 'wild crops stand' {
    Reset-Plot
    $null=Command 'setblock 183 82 70 minecraft:grass_block'
    $null=Command 'setblock 183 83 70 farmersdelight:wild_cabbages'
    $null=Command 'setblock 176 84 68 farmersdelight:wild_rice[half=lower]'
    Wait-Ticks 40
    Expect ((Block 183 83 70) -eq 'farmersdelight:wild_cabbages') ('wild cabbages fell: '+(Block 183 83 70))
    Expect ((Block 176 84 68) -eq 'farmersdelight:wild_rice') ('wild rice fell: '+(Block 176 84 68))
}
$failed=@($results.GetEnumerator()|Where-Object { $_.Value -ne 'ok' })
$report.measured=@{scenarios=$results.Count;failed=$failed.Count}
$report.passed=($failed.Count -eq 0)
Save-Report
Write-Output ('Scenarios: '+$results.Count+', failed: '+$failed.Count)
