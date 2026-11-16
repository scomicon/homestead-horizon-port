# Local engine patch applied to the staged engine after `horizon-mixins apply` (stage 27, "engine20").
#
# 1. The sleep hooks (`can_player_sleep`, `player_spawn_point_set` with cause "bed", `player_bed_enter`)
#    only fire for a LAN guest: the game guards the calls with `lan_acting_uuid`/`lan_acting_name`, which the
#    single-player host never has. Mods (Comforts and any other bed rule) therefore heard nothing in a
#    single-player world. The three calls now use the acting player as the hook points name one
#    (`native_acting_id`: the guest whose turn it is, else the host; `acting_player_name`).
# 2. A bed is recognised by the name suffix `_bed` in three places (`is_bed`, the sleeper's "bed is gone"
#    wake-up, the LAN guest's wake-up). A content-mod bed on the vanilla bed template (sleeping bag, hammock)
#    has the template's state data (`part`, `occupied`, `facing`) but not the name, so the player was woken at
#    once and stand-up/spawn logic ignored it. `is_bed` now also accepts a block whose state has those three
#    properties, and the two wake-up checks go through `is_bed`.
#
# Idempotent: every edit has a marker; the script refuses to run if an anchor is missing (the engine changed).
param([Parameter(Mandatory)][string] $Stage)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
function Patch([string] $Relative, [string] $Anchor, [string] $Replacement, [string] $Marker) {
    $path = Join-Path $Stage $Relative
    $text = [IO.File]::ReadAllText($path)
    if ($text.Contains($Marker)) { return }
    if (-not $text.Contains($Anchor)) { throw "Engine patch anchor missing in $Relative (stage 27 sleep patch, marker: $Marker)" }
    [IO.File]::WriteAllText($path, $text.Replace($Anchor, $Replacement), $utf8)
}
$game = 'crates/horizon-client/src/game.rs'

# --- 1a. spawn point set by a bed (try_sleep) -------------------------------------------------------
Patch $game `
    ('            if moved' + "`n" +
     '                && let (Some(player), Some(name)) = (' + "`n" +
     '                    self.lan_acting_uuid,' + "`n" +
     '                    self.lan_acting_name.clone(),' + "`n" +
     '                )' + "`n" +
     '            {' + "`n" +
     '                self.native_player_spawn_point_set(') `
    ('            if moved' + "`n" +
     '                // homestead: the hook fires for the single-player host too.' + "`n" +
     '                && let (Some(player), Some(name)) = (' + "`n" +
     '                    Some(self.native_acting_id()),' + "`n" +
     '                    Some(self.acting_player_name().to_owned()),' + "`n" +
     '                )' + "`n" +
     '            {' + "`n" +
     '                self.native_player_spawn_point_set(') `
    'homestead: the hook fires for the single-player host too.'

# --- 1b. can_player_sleep ---------------------------------------------------------------------------
Patch $game `
    ('        if let (Some(player), Some(name)) = (' + "`n" +
     '            self.lan_acting_uuid,' + "`n" +
     '            self.lan_acting_name.clone(),' + "`n" +
     '        ) {' + "`n" +
     '            self.native_can_player_sleep(player, &name, head, &mut problem);' + "`n" +
     '        }') `
    ('        {' + "`n" +
     '            // homestead: CanPlayerSleepEvent for the single-player host too.' + "`n" +
     '            let player = self.native_acting_id();' + "`n" +
     '            let name = self.acting_player_name().to_owned();' + "`n" +
     '            self.native_can_player_sleep(player, &name, head, &mut problem);' + "`n" +
     '        }') `
    'homestead: CanPlayerSleepEvent for the single-player host too.'

# --- 1c. player_bed_enter ---------------------------------------------------------------------------
Patch $game `
    ('        if let (Some(player), Some(name)) = (' + "`n" +
     '            self.lan_acting_uuid,' + "`n" +
     '            self.lan_acting_name.clone(),' + "`n" +
     '        ) {' + "`n" +
     '            let mut cancel = false;' + "`n" +
     '            self.native_player_bed_enter(player, &name, head, &mut cancel);' + "`n" +
     '            if cancel {' + "`n" +
     '                return;' + "`n" +
     '            }' + "`n" +
     '        }') `
    ('        {' + "`n" +
     '            // homestead: the bed-enter hook for the single-player host too.' + "`n" +
     '            let player = self.native_acting_id();' + "`n" +
     '            let name = self.acting_player_name().to_owned();' + "`n" +
     '            let mut cancel = false;' + "`n" +
     '            self.native_player_bed_enter(player, &name, head, &mut cancel);' + "`n" +
     '            if cancel {' + "`n" +
     '                return;' + "`n" +
     '            }' + "`n" +
     '        }') `
    'homestead: the bed-enter hook for the single-player host too.'
# --- 2a. is_bed: a bed by its state data as well as by its name -------------------------------------
Patch 'crates/horizon-client/src/game/respawn_anchor.rs' `
    ('    pub(super) fn is_bed(&self, state: BlockStateId) -> bool {' + "`n" +
     '        self.res.facts().get(state).name.ends_with("_bed")' + "`n" +
     '    }') `
    ('    pub(super) fn is_bed(&self, state: BlockStateId) -> bool {' + "`n" +
     '        // homestead: a bed is a block with a bed''s state data (part, occupied, facing) as well as a name' + "`n" +
     '        // ending in `_bed`, so content-mod beds on the bed template (sleeping bags, hammocks) are beds.' + "`n" +
     '        self.res.facts().get(state).name.ends_with("_bed")' + "`n" +
     '            || self.pack.assets.registry.state(state).is_some_and(|s| {' + "`n" +
     '                s.properties.contains_key("part")' + "`n" +
     '                    && s.properties.contains_key("occupied")' + "`n" +
     '                    && s.properties.contains_key("facing")' + "`n" +
     '            })' + "`n" +
     '    }') `
    'homestead: a bed is a block with a bed''s state data'

# --- 2b. the sleeper's own wake-up check ------------------------------------------------------------
Patch $game `
    '            let bed_gone = !self.res.facts().get(bed).name.ends_with("_bed");' `
    '            let bed_gone = !self.is_bed(bed); // homestead: by state data too' `
    'let bed_gone = !self.is_bed(bed); // homestead'

# --- 2c. a LAN guest's wake-up check ----------------------------------------------------------------
Patch 'crates/horizon-client/src/game/lan_actions.rs' `
    '            let bed = self.res.facts().get(block).name.ends_with("_bed");' `
    '            let bed = self.is_bed(block); // homestead: by state data too' `
    'let bed = self.is_bed(block); // homestead'
