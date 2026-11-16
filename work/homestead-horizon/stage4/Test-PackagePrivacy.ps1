#requires -Version 7.0
# Scans a package directory or an archive for private data: Windows user name in a profile path, the git e-mail, the workspace root.
# Case-sensitive on purpose (a bare word such as "admin" is not a leak). Archives (.zip/.hmod/.hnmod/.jar) are scanned entry by entry.
# Exit: throws when anything is found.
param([Parameter(Mandatory)][string] $Path)
$ErrorActionPreference = 'Stop'
$Path = [IO.Path]::GetFullPath($Path)
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$gitExe = Join-Path $workspaceRoot 'work/mingit/cmd/git.exe'
Add-Type -AssemblyName System.IO.Compression
$needles = [System.Collections.Generic.List[string]]::new()
foreach ($u in @($env:USERNAME, [Environment]::UserName) | Where-Object { $_ } | Select-Object -Unique) {
    foreach ($sep in '\', '/') { $needles.Add("${sep}Users${sep}$u"); $needles.Add("${sep}home${sep}$u") }
}
$mail = (& $gitExe -C $workspaceRoot config user.email 2>$null)
if ($mail -and $mail -notmatch 'noreply') { $needles.Add($mail.Trim()) }
$needles.Add($workspaceRoot); $needles.Add($workspaceRoot.Replace('\', '/'))
$latin1 = [Text.Encoding]::Latin1
function Test-Bytes([byte[]] $Bytes, [string] $Where, [System.Collections.Generic.List[string]] $Found) {
    $s = $latin1.GetString($Bytes)
    foreach ($n in $needles) { if ($s.IndexOf($n, [StringComparison]::Ordinal) -ge 0) { $Found.Add("$Where  <-  $n") } }
    $u = [Text.Encoding]::Unicode.GetString($Bytes)
    foreach ($n in $needles) { if ($u.IndexOf($n, [StringComparison]::Ordinal) -ge 0) { $Found.Add("$Where (UTF-16)  <-  $n") } }
}
function Test-File([string] $Path, [string] $Where, [System.Collections.Generic.List[string]] $Found, [int] $Depth) {
    if ($Path -match '\.(zip|hmod|hnmod)$' -and $Depth -lt 3) {
        $zip = [IO.Compression.ZipFile]::OpenRead($Path)
        try {
            foreach ($e in $zip.Entries) {
                if ($e.Length -eq 0) { continue }
                $ms = [IO.MemoryStream]::new(); $st = $e.Open(); $st.CopyTo($ms); $st.Dispose()
                $bytes = $ms.ToArray()
                if ($e.FullName -match '\.(zip|jar|hmod|hnmod)$') {
                    $tmp = [IO.Path]::GetTempFileName(); try { [IO.File]::WriteAllBytes($tmp, $bytes); $tmpZip = $tmp + '.zip'; Move-Item $tmp $tmpZip; Test-File $tmpZip "$Where!$($e.FullName)" $Found ($Depth + 1) } finally { Remove-Item $tmp, $tmpZip -ErrorAction SilentlyContinue }
                } else { Test-Bytes $bytes "$Where!$($e.FullName)" $Found }
            }
        } finally { $zip.Dispose() }
    } else {
        Test-Bytes ([IO.File]::ReadAllBytes($Path)) $Where $Found
    }
}
$found = [System.Collections.Generic.List[string]]::new()
if (Test-Path -LiteralPath $Path -PathType Leaf) { $files = @(Get-Item -LiteralPath $Path); $base = Split-Path $Path } else { $files = Get-ChildItem -LiteralPath $Path -Recurse -File; $base = $Path }
foreach ($f in $files) {
    Test-File $f.FullName ($f.FullName.Substring($base.Length).TrimStart([char]92)) $found 0
}
if ($found.Count -gt 0) {
    $found | Select-Object -Unique | ForEach-Object { Write-Warning "Private data: $_" }
    throw "Privacy scan failed: $(@($found | Select-Object -Unique).Count) finding(s) in $Path. Executables carrying build paths need a rebuild with --remap-path-prefix (work/rust-env.ps1)."
}
Write-Output 'Privacy scan: 0 findings.'
