#requires -Version 7.0
# Copyright (c) 2026 create-horizon contributors / local Homestead integration.
# SPDX-License-Identifier: MIT
# Runs only a copied runner and an intentional-failure startup stub. No game or build.
[CmdletBinding()]
param(
    [string] $Runner=(Join-Path $PSScriptRoot '../Run-Acceptance.ps1'),
    [ValidateRange(5,60)][int] $TimeoutSeconds=30
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$utf8=[Text.UTF8Encoding]::new($false)
$runnerPath=(Resolve-Path -LiteralPath $Runner).Path
$pwsh=(Get-Command pwsh -ErrorAction Stop).Source
$marker='HomesteadHarness intentional startup error'
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$tempPrefix=$tempRoot.TrimEnd([IO.Path]::DirectorySeparatorChar,[IO.Path]::AltDirectorySeparatorChar)+[IO.Path]::DirectorySeparatorChar
$fixture=[IO.Path]::GetFullPath((Join-Path $tempRoot ('homestead-harness-'+[guid]::NewGuid().ToString('N'))))
if (-not $fixture.StartsWith($tempPrefix,[StringComparison]::OrdinalIgnoreCase)) { throw 'Fixture escaped the temporary directory.' }
if (Test-Path -LiteralPath $fixture) { throw 'The unique fixture already exists.' }
New-Item -ItemType Directory -Path $fixture | Out-Null

function Protect-Text([string] $Text) {
    return [regex]::Replace($Text,'(?i)[a-z]:[\\/]+users[\\/]+[^\\/\s"]+','<user-profile>')
}
function Assert([bool] $Condition,[string] $Message) {
    if (-not $Condition) { throw $Message }
}
function Save-Json([string] $Path,$Value) {
    [IO.File]::WriteAllText($Path,(Protect-Text ($Value|ConvertTo-Json -Depth 20))+[Environment]::NewLine,$utf8)
}
$report=[ordered]@{
    passed=$false
    scope='Copied acceptance runner with an intentional-failure Start-Test stub; no game or build.'
    fixture_name=[IO.Path]::GetFileName($fixture)
    fixture_retained=$true
    runner_sha256=(Get-FileHash -LiteralPath $runnerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    cases=@()
}
$reportPath=Join-Path $fixture 'harness-result.json'
Save-Json $reportPath $report

# This stub records what Start-Test would inherit, including the receipt already
# on disk at its entry. It cannot start a client and always throws the same marker.
$stub=@'
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$environment=[ordered]@{}
$variables=[Environment]::GetEnvironmentVariables('Process')
foreach ($name in 'HOMESTEAD_AUDIO','HOMESTEAD_WINDOWED','HOMESTEAD_CLIENT_DEFAULTS') {
    $environment[$name]=@{
        present=$variables.Contains($name)
        value=[Environment]::GetEnvironmentVariable($name,'Process')
    }
}
$receiptName=[Environment]::GetEnvironmentVariable('HOMESTEAD_HARNESS_RECEIPT','Process')
if ($receiptName -notmatch '^run-[a-z]+-result\.json$') { throw 'Unexpected fixture receipt name.' }
$snapshot=@{
    environment=$environment
    receipt_at_start=(Get-Content -LiteralPath (Join-Path $PSScriptRoot $receiptName) -Raw|ConvertFrom-Json -AsHashtable)
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'startup-snapshot.json'),($snapshot|ConvertTo-Json -Depth 12),[Text.UTF8Encoding]::new($false))
throw 'HomesteadHarness intentional startup error'
'@
$cases=@(
    @{name='clients';flag='ClientS';receipt='run-clients-result.json';audio=$true;window=$true;defaults=$false},
    @{name='clienth';flag='ClientH';receipt='run-clienth-result.json';audio=$false;window=$true;defaults=$false},
    @{name='clientt';flag='ClientT';receipt='run-clientt-result.json';audio=$false;window=$true;defaults=$false},
    @{name='motor';flag='Motor';receipt='run-motor-result.json';audio=$false;window=$true;defaults=$false},
    @{name='clientdefaults';flag='ClientDefaults';receipt='run-clientdefaults-result.json';audio=$false;window=$true;defaults=$true},
    @{name='flow';flag=$null;receipt='run-flow-result.json';audio=$false;window=$false;defaults=$false}
)
foreach ($case in $cases) {
    $result=[ordered]@{name=$case.name;passed=$false;exit_code=$null;checks=@()}
    $caseDir=Join-Path $fixture $case.name
    New-Item -ItemType Directory -Path $caseDir | Out-Null
    Copy-Item -LiteralPath $runnerPath -Destination (Join-Path $caseDir 'Run-Acceptance.ps1')
    [IO.File]::WriteAllText((Join-Path $caseDir 'Start-Test.ps1'),$stub,$utf8)
    $receiptPath=Join-Path $caseDir $case.receipt
    Save-Json $receiptPath @{
        passed=$true;process_id=999999;phases=@('OldPass');binary_sha256=('0'*64)
        shutdown=@{requested=$true;process_exited=$true}
    }
    $process=$null
    try {
        $info=[Diagnostics.ProcessStartInfo]::new()
        $info.FileName=$pwsh
        $info.WorkingDirectory=$caseDir
        $info.UseShellExecute=$false
        $info.CreateNoWindow=$true
        $info.RedirectStandardOutput=$true
        $info.RedirectStandardError=$true
        foreach ($word in @('-NoLogo','-NoProfile','-NonInteractive','-File',(Join-Path $caseDir 'Run-Acceptance.ps1'))) { $info.ArgumentList.Add($word) }
        if ($case.flag) { $info.ArgumentList.Add('-'+$case.flag) }
        foreach ($name in 'HOMESTEAD_AUDIO','HOMESTEAD_WINDOWED','HOMESTEAD_CLIENT_DEFAULTS') { $info.Environment[$name]='inherited-conflict' }
        $info.Environment['HOMESTEAD_HARNESS_RECEIPT']=$case.receipt
        $process=[Diagnostics.Process]::Start($info)
        $outTask=$process.StandardOutput.ReadToEndAsync()
        $errTask=$process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutSeconds*1000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw 'The copied runner timed out before the intentional startup failure completed.'
        }
        $stdout=$outTask.GetAwaiter().GetResult()
        $stderr=$errTask.GetAwaiter().GetResult()
        [IO.File]::WriteAllText((Join-Path $caseDir 'stdout.log'),(Protect-Text $stdout),$utf8)
        [IO.File]::WriteAllText((Join-Path $caseDir 'stderr.log'),(Protect-Text $stderr),$utf8)
        $result.exit_code=$process.ExitCode
        Assert ($process.ExitCode -eq 1) 'The copied runner must exit 1 for the intentional startup failure.'
        $result.checks+='The child runner exited 1.'

        $snapshot=Get-Content -LiteralPath (Join-Path $caseDir 'startup-snapshot.json') -Raw|ConvertFrom-Json -AsHashtable
        $expected=@{HOMESTEAD_AUDIO=$case.audio;HOMESTEAD_WINDOWED=$case.window;HOMESTEAD_CLIENT_DEFAULTS=$case.defaults}
        foreach ($name in $expected.Keys) {
            $actual=$snapshot.environment[$name]
            if ($expected[$name]) {
                Assert ($actual.present -eq $true -and $actual.value -ceq '1') ('Startup did not inherit '+$name+'=1.')
            } else {
                Assert ($actual.present -eq $false -and $null -eq $actual.value) ('Startup inherited '+$name+' instead of removing it.')
            }
        }
        $result.checks+='Startup inherited the required audio, window and defaults environment; other values were removed.'
        $atStart=$snapshot.receipt_at_start
        Assert ($atStart.passed -is [bool] -and -not $atStart.passed) 'The old PASS was not invalidated before startup.'
        Assert ($atStart.process_id -eq 0 -and @($atStart.phases).Count -eq 0) 'The initial receipt retained an old process or completed phase.'
        $result.checks+='The old PASS was already false with process 0 and no phases when the startup stub entered.'

        $receipt=Get-Content -LiteralPath $receiptPath -Raw|ConvertFrom-Json -AsHashtable
        Assert ($receipt.passed -is [bool] -and -not $receipt.passed) 'The final receipt falsely reports PASS.'
        Assert ($receipt.process_id -eq 0) 'A failed startup must retain process_id 0.'
        Assert (@($receipt.phases).Count -eq 0) 'A failed startup must not complete any phases.'
        Assert ($null -eq $receipt.binary_sha256) 'A failed startup must not retain a stale binary hash.'
        Assert ($receipt.error -ceq $marker) 'The final receipt lost the original intentional startup error.'
        Assert (-not $receipt.shutdown.ContainsKey('error')) 'Shutdown reported an error with no process to close.'
        Assert ($receipt.shutdown.requested -is [bool] -and -not $receipt.shutdown.requested) 'Shutdown requested quit for a nonexistent process.'
        Assert ($receipt.shutdown.process_exited -is [bool] -and $receipt.shutdown.process_exited) 'Shutdown did not handle the absent process.'
        Assert (($stdout+$stderr) -notmatch '(?i)null-valued expression|null-reference|NullReferenceException|You cannot call a method on a null') 'The child emitted a null-reference failure.'
        $result.checks+='The final receipt is false/0/empty, preserves the original error and handles shutdown without a process.'
        $result.passed=$true
        Write-Output ('PASS '+$case.name)
    } catch {
        $result.error=Protect-Text $_.Exception.Message
        Write-Output ('FAIL '+$case.name+': '+$result.error)
    } finally {
        if ($null -ne $process) { $process.Dispose() }
        $report.cases+=@($result)
        Save-Json $reportPath $report
    }
}
$report.passed=@($report.cases|Where-Object {-not $_.passed}).Count -eq 0 -and $report.cases.Count -eq $cases.Count
Save-Json $reportPath $report
# Retain this unique fixture for diagnosis. No recursive deletion is performed.
Write-Output ('Harness fixture retained: <temp>/'+$report.fixture_name)
if (-not $report.passed) { throw 'Acceptance harness self-test failed; inspect the retained fixture.' }
Write-Output 'Acceptance harness self-test passed: 6/6 cases; no game or build was started.'
