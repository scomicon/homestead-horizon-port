# Client defaults for a new Homestead instance (ASCII only: Windows PowerShell 5 reads this file).
#
# Horizon ships built-in equivalents of several client mods of the Homestead pack, but every one of
# them is OFF by default. This script turns the ones we mirror ON, once per instance, and never
# overwrites a choice the player already made.
#
#   horizon-extras.txt (next to options.txt, "key:value" per line, see Horizon docs/extras.md)
#     dynamicLights:true      LambDynamicLights
#     mouseTweaks:true        Mouse Tweaks (inventory click-drag controls)
#     mouseTweaksWheel:true   Mouse Tweaks (wheel moves stacks)
#     threeDSkinLayers:true   3d-Skin-Layers
#
# Rules:
#   * A key that is already in the file keeps its value (even false): the game rewrites the whole
#     file when the player changes any extra, so an existing key is treated as the player's choice.
#   * The marker file .homestead-client-defaults records that the defaults were applied, so a key
#     the player deletes later is not added again.
#   * Nothing else in the instance is touched (options.txt is never written here).
#
# Usage (dot-source, then call):
#   . .\Apply-ClientDefaults.ps1
#   Set-HomesteadClientDefaults -GameDir 'D:\Games\x\private\instance'

function Get-HomesteadClientDefaults {
    return [ordered]@{
        'dynamicLights'    = 'true'
        'mouseTweaks'      = 'true'
        'mouseTweaksWheel' = 'true'
        'threeDSkinLayers' = 'true'
    }
}

function Set-HomesteadClientDefaults {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string] $GameDir,
        [switch] $Force   # ignore the marker (used by tests)
    )
    $marker = Join-Path $GameDir '.homestead-client-defaults'
    $file = Join-Path $GameDir 'horizon-extras.txt'
    if ((Test-Path -LiteralPath $marker) -and -not $Force) { return @() }
    New-Item -ItemType Directory -Force -Path $GameDir | Out-Null

    $text = ''
    if (Test-Path -LiteralPath $file) { $text = [IO.File]::ReadAllText($file) }
    $present = @{}
    foreach ($line in ($text -split "`r?`n")) {
        $i = $line.IndexOf(':')
        if ($i -gt 0) { $present[$line.Substring(0, $i).Trim()] = $true }
    }
    $added = @()
    $append = ''
    foreach ($entry in (Get-HomesteadClientDefaults).GetEnumerator()) {
        if (-not $present.ContainsKey($entry.Key)) {
            $append += ($entry.Key + ':' + $entry.Value + "`n")
            $added += $entry.Key
        }
    }
    if ($append) {
        # Keep the file's own line ending: add a newline first if the last line has none.
        if ($text -and -not $text.EndsWith("`n")) { $text += "`n" }
        [IO.File]::WriteAllText($file, ($text + $append), (New-Object Text.UTF8Encoding($false)))
    }
    [IO.File]::WriteAllText($marker, ((Get-Date).ToString('o') + "`n"), (New-Object Text.UTF8Encoding($false)))
    return $added
}
