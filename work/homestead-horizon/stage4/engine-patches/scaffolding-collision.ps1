# Local engine patch applied to the staged engine after `horizon-mixins apply`.
# Content-mod scaffolding (vanilla scaffolding template: properties distance + bottom)
# gets the vanilla scaffolding collision rules; without it the server stops with
# "unsupported block collision state" when such a block is queried.
# Idempotent: refuses to run if the anchor is missing and skips if already patched.
param([Parameter(Mandatory)][string] $Stage)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
function Patch([string] $Relative, [string] $Anchor, [string] $Replacement, [string] $Marker) {
    $path = Join-Path $Stage $Relative
    $text = [IO.File]::ReadAllText($path)
    if ($text.Contains($Marker)) { return }
    if (-not $text.Contains($Anchor)) { throw "Engine patch anchor missing in $Relative" }
    [IO.File]::WriteAllText($path, $text.Replace($Anchor, $Replacement), $utf8)
}
Patch 'crates/horizon-world/src/dynamic_support.rs' `
    '            "minecraft:scaffolding" => Kind::Scaffolding,' `
    ('            "minecraft:scaffolding" => Kind::Scaffolding,' + "`n" +
     '            // Content-mod scaffolding (vanilla template) keeps the vanilla rule.' + "`n" +
     '            name if name.ends_with("_scaffolding")' + "`n" +
     '                && state.properties.contains_key("distance")' + "`n" +
     '                && state.properties.contains_key("bottom") =>' + "`n" +
     '            {' + "`n" + '                Kind::Scaffolding' + "`n" + '            }') `
    'Content-mod scaffolding (vanilla template) keeps'
Patch 'crates/horizon-world/src/block_collision.rs' `
    '        if state.name != "minecraft:scaffolding" {' `
    ('        let content_scaffolding = state.name.ends_with("_scaffolding")' + "`n" +
     '            && state.properties.contains_key("distance")' + "`n" +
     '            && state.properties.contains_key("bottom");' + "`n" +
     '        if state.name != "minecraft:scaffolding" && !content_scaffolding {') `
    'let content_scaffolding'