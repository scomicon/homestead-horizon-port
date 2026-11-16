# Local engine patch applied to the staged engine after `horizon-mixins apply` (stage 22, privacy remap).
#
# `crash::stack_lines` embedded `env!("CARGO_MANIFEST_DIR")` into `.rdata` to strip the build machine's
# workspace prefix from backtrace paths. `--remap-path-prefix` does not remap `env!("CARGO_MANIFEST_DIR")`,
# so the Windows user profile path leaked into `horizon-client.exe` and `horizon-ctl.exe`.
# Strip `/crates/` (after `/registry/src/`) instead so both remapped (`/build/...`) and unremapped
# backtrace paths format identically without embedding `CARGO_MANIFEST_DIR`.
#
# Idempotent: every edit has a marker; refuses to run if an anchor is missing.
param([Parameter(Mandatory)][string] $Stage)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
function Patch([string] $Relative, [string] $Anchor, [string] $Replacement, [string] $Marker) {
    $path = Join-Path $Stage $Relative
    $text = [IO.File]::ReadAllText($path)
    if ($text.Contains($Marker)) { return }
    if (-not $text.Contains($Anchor)) { throw "Engine patch anchor missing in $Relative (stage 22 crash path patch, marker: $Marker)" }
    [IO.File]::WriteAllText($path, $text.Replace($Anchor, $Replacement), $utf8)
}

$crash = 'crates/horizon-client-render/src/crash.rs'

Patch $crash `
    ('    let manifest = env!("CARGO_MANIFEST_DIR").replace(''\\'', "/");' + "`n" +
     '    let root = manifest' + "`n" +
     '        .strip_suffix("crates/horizon-client-render")' + "`n" +
     '        .unwrap_or(&manifest);') `
    ('    // homestead: avoid embedding build-machine CARGO_MANIFEST_DIR into binaries (--remap-path-prefix builds).' + "`n" +
     '    let root = "";') `
    'homestead: avoid embedding build-machine CARGO_MANIFEST_DIR'

Patch $crash `
    ('fn short_path<''a>(path: &''a str, root: &str) -> &''a str {' + "`n" +
     '    if let Some(rest) = path.strip_prefix(root) {' + "`n" +
     '        return rest;' + "`n" +
     '    }' + "`n" +
     '    if let Some(i) = path.find("/registry/src/") {' + "`n" +
     '        let rest = &path[i + "/registry/src/".len()..];' + "`n" +
     '        if let Some((_index, krate)) = rest.split_once(''/'') {' + "`n" +
     '            return krate;' + "`n" +
     '        }' + "`n" +
     '    }' + "`n" +
     '    path' + "`n" +
     '}') `
    ('fn short_path<''a>(path: &''a str, _root: &str) -> &''a str {' + "`n" +
     '    // homestead: strip workspace prefix without CARGO_MANIFEST_DIR.' + "`n" +
     '    if let Some(i) = path.find("/registry/src/") {' + "`n" +
     '        let rest = &path[i + "/registry/src/".len()..];' + "`n" +
     '        if let Some((_index, krate)) = rest.split_once(''/'') {' + "`n" +
     '            return krate;' + "`n" +
     '        }' + "`n" +
     '    }' + "`n" +
     '    if let Some(i) = path.find("/crates/") {' + "`n" +
     '        return &path[i + 1..];' + "`n" +
     '    }' + "`n" +
     '    path' + "`n" +
     '}') `
    'homestead: strip workspace prefix without CARGO_MANIFEST_DIR.'
