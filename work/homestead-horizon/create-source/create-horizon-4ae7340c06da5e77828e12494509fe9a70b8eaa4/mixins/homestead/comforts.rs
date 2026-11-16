// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Comforts (sleeping bags and hammocks), rebuilt from the mod's public description only (Comforts is
// copyleft, its source was not read). What the port does:
//
// * the content mod built by tools/comforts_content.py declares 16 sleeping bags and 16 hammocks, blocks on the
//   vanilla bed template (`comforts:sleeping_bag_<colour>`, `comforts:hammock_<colour>`) with items that place
//   them; the bed template gives them the bed behaviour (two blocks, head and foot, right click to sleep),
// * sleeping in one never sets the respawn point: the engine's `player_spawn_point_set` hook (cause "bed") is
//   cancelled for these blocks, so a bag or hammock carried along is a place to pass the night, not a home.
//
// Needs engine patch 20-sleep-host.ps1: without it the hook never fires for the single-player host and a bag
// would set the respawn point like a bed, and the engine would not take a bag for a bed at all (`is_bed` by
// name). Not ported: the day-time nap, the cold/heat effects, the hammock's tie-down rules, the rope and the
// recipes of the original (the recipes here are placeholders).

use super::Game;

const BAG: &str = "comforts:sleeping_bag_";
const HAMMOCK: &str = "comforts:hammock_";

/// Whether the block at `pos` is one of the portable beds.
fn is_portable_bed(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let block = game.native_block(pos);
    block.starts_with(BAG) || block.starts_with(HAMMOCK)
}

/// `player_spawn_point_set`: a portable bed keeps the old respawn point.
pub(super) fn spawn_point_set(game: &Game<'_>, pos: [i32; 3], cause: &str, cancel: &mut bool) {
    if cause == "bed" && is_portable_bed(game, pos) {
        *cancel = true;
    }
}
