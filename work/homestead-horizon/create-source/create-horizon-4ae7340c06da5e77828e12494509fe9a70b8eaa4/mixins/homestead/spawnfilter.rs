// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// SpawnFilter 1.1.0 (CC0, Fabric, server side) with the Homestead pack's `config/spawnfilter.toml`:
// a player must not spawn on a blocked block (water, lava, magma, fire, wool, fluids, leaves) or in a
// blocked biome (ocean and river variants). When their spawn is blocked the mod moves them to the
// nearest safe column found by a spiral search (up to 1024 blocks); when none is found it puts a stone
// platform under them.
//
// Ported here for the two moments a native mod can see a spawn, both server side:
//
// * `player_respawned` (death respawn at the world spawn or a bed): the player has just been placed;
// * `player_logged_in` for a player who has never been checked (a marker in their persistent data) in a
//   young world (first 12000 ticks), i.e. a new world's first spawn. Existing worlds are left alone.
//
// Not ported (no API): the choice of the spawn point itself before the player is placed (the original
// changes where vanilla places them; here they are moved after being placed, so a client can see one
// frame at the first position), the biome tags beyond a fixed list (the engine has no biome tag query,
// so `#minecraft:is_ocean`, `is_deep_ocean` and `is_river` are spelled out), and block tag `#c:fluids`
// only matches where the server's tag table has it. The config file is not read: the values below are
// the pack's file as shipped.

use super::{native_world_edit::NativeHeightmap, Game};
use horizon_world::{
    level::flags,
    nbt::{Compound, Tag},
};

type Pos = [i32; 3];

/// `blocked_blocks`.
const BLOCKED_BLOCKS: [&str; 3] = ["minecraft:water", "minecraft:lava", "minecraft:magma_block"];
/// `blocked_tags`, without the `#`.
const BLOCKED_TAGS: [&str; 4] = ["minecraft:fire", "minecraft:wool", "c:fluids", "minecraft:leaves"];
/// `blocked_biomes`.
const BLOCKED_BIOMES: [&str; 0] = [];
/// `blocked_biome_tags`: the members of `#minecraft:is_ocean`, `#minecraft:is_deep_ocean`,
/// `#minecraft:is_river`.
const BLOCKED_BIOME_TAG_MEMBERS: [&str; 11] = [
    "minecraft:ocean",
    "minecraft:deep_ocean",
    "minecraft:frozen_ocean",
    "minecraft:deep_frozen_ocean",
    "minecraft:cold_ocean",
    "minecraft:deep_cold_ocean",
    "minecraft:lukewarm_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:warm_ocean",
    "minecraft:river",
    "minecraft:frozen_river",
];
/// `max_search_radius`.
const MAX_SEARCH_RADIUS: i32 = 1024;
/// Spacing of the spiral's sample columns (the original steps by chunk-sized strides too).
const STEP: i32 = 8;
/// `build_safety_platform` and `safety_platform_block`.
const BUILD_PLATFORM: bool = true;
const PLATFORM_BLOCK: &str = "minecraft:stone";
/// A first spawn is only looked at while the world is this young (ticks).
const YOUNG_WORLD: i64 = 12_000;
/// The persistent-data key that marks a player as checked.
const MARKER: &str = "homestead_spawnfilter";

fn is_air(name: &str) -> bool {
    matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// Whether the block at `pos` is a blocked one: listed, or in a blocked tag.
fn blocked_block(game: &Game<'_>, pos: Pos) -> bool {
    let id = game.world.block(pos[0], pos[1], pos[2]);
    let Some(state) = game.pack.assets.registry.state(id) else {
        return false;
    };
    let name: &str = &state.name;
    BLOCKED_BLOCKS.iter().any(|blocked| *blocked == name)
        || BLOCKED_TAGS
            .iter()
            .any(|tag| game.native_server_block_in_tag(state, tag).unwrap_or(false))
}

fn blocked_biome(game: &Game<'_>, pos: Pos) -> bool {
    let Some(biome) = game.native_biome(pos) else {
        return false;
    };
    BLOCKED_BIOMES.iter().any(|blocked| *blocked == biome)
        || BLOCKED_BIOME_TAG_MEMBERS.iter().any(|member| *member == biome)
}

/// A spawn at feet position `feet` is blocked when the block they stand on, the one they stand in or the
/// biome is blocked.
fn blocked_spawn(game: &Game<'_>, feet: Pos) -> bool {
    blocked_block(game, [feet[0], feet[1] - 1, feet[2]])
        || blocked_block(game, feet)
        || blocked_biome(game, feet)
}

/// A column whose surface `height` (the first free block) is a fine place to stand: solid ground that is
/// not blocked, two free blocks above, and a biome that is not blocked.
fn safe_column(game: &Game<'_>, x: i32, height: i32, z: i32) -> bool {
    let below = [x, height - 1, z];
    let feet = [x, height, z];
    let head = [x, height + 1, z];
    !is_air(&game.native_block(below))
        && is_air(&game.native_block(feet))
        && is_air(&game.native_block(head))
        && !blocked_block(game, below)
        && !blocked_block(game, feet)
        && !blocked_biome(game, feet)
}

/// The columns of the square ring at Chebyshev distance `radius` around `(cx, cz)`, sampled every `STEP`
/// blocks and ordered nearest first.
fn ring(cx: i32, cz: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let mut d = -radius;
    while d <= radius {
        out.push((cx + d, cz - radius));
        out.push((cx + d, cz + radius));
        d += STEP;
    }
    let mut d = -radius + STEP;
    while d < radius {
        out.push((cx - radius, cz + d));
        out.push((cx + radius, cz + d));
        d += STEP;
    }
    out.sort_by_key(|(x, z)| {
        let (dx, dz) = (i64::from(x - cx), i64::from(z - cz));
        dx * dx + dz * dz
    });
    out
}

/// The nearest safe standing position around `from` (feet position), searched ring by ring over the
/// columns that are loaded (a column not loaded has no height and is skipped, so the search reaches as far as
/// the world is loaded). None when nothing safe is loaded or the height query is refused.
fn find_safe(game: &Game<'_>, dimension: &str, from: Pos) -> Option<Pos> {
    let mut radius = STEP;
    while radius <= MAX_SEARCH_RADIUS {
        let columns = ring(from[0], from[2], radius);
        let heights = game
            .native_surface_heights(dimension, &columns, NativeHeightmap::MotionBlocking)
            .ok()?;
        for ((x, z), height) in columns.into_iter().zip(heights) {
            if let Some(height) = height
                && safe_column(game, x, height, z)
            {
                return Some([x, height, z]);
            }
        }
        radius += STEP;
    }
    None
}

/// `build_safety_platform`: the block under the player and its eight neighbours, where they are air, a fluid
/// or a blocked block, become the platform block.
fn build_platform(game: &mut Game<'_>, dimension: &str, feet: Pos) {
    let Some(stone) = game.pack.assets.registry.default_state(PLATFORM_BLOCK) else {
        return;
    };
    for dx in -1..=1 {
        for dz in -1..=1 {
            let pos = [feet[0] + dx, feet[1] - 1, feet[2] + dz];
            let name = game.native_block(pos);
            if is_air(&name) || blocked_block(game, pos) {
                game.native_set_block(dimension, pos, stone, flags::UPDATE_ALL);
            }
        }
    }
}

/// Moves the server player `uuid` when their spawn is blocked.
fn check(game: &mut Game<'_>, uuid: u128) {
    let Some(player) = game.native_server_players().into_iter().find(|p| p.uuid == uuid) else {
        return;
    };
    let dimension = player.dimension.clone();
    // Only the level being ticked can be read and edited.
    if dimension != game.dim.id() {
        return;
    }
    let feet = [
        player.position[0].floor() as i32,
        player.position[1].floor() as i32,
        player.position[2].floor() as i32,
    ];
    if !blocked_spawn(game, feet) {
        return;
    }
    match find_safe(game, &dimension, feet) {
        Some([x, y, z]) => {
            let to = [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5];
            let _ = game.native_teleport_player(uuid, &dimension, to, player.yaw, player.pitch);
        }
        None if BUILD_PLATFORM => build_platform(game, &dimension, feet),
        None => {}
    }
}

pub(super) fn respawned(game: &mut Game<'_>, player: u128) {
    check(game, player);
}

fn marked(data: &Option<Tag>) -> bool {
    matches!(data, Some(Tag::Compound(map)) if map.contains_key(&MARKER.into()))
}

/// A player's first login to a young world: checked once, then marked.
pub(super) fn logged_in(game: &mut Game<'_>, player: u128) {
    if game.world.game_time as i64 >= YOUNG_WORLD {
        return;
    }
    let data = game.native_player_persistent_data(player);
    if marked(&data) {
        return;
    }
    let mut compound = match data {
        Some(Tag::Compound(map)) => map,
        _ => Compound::new(),
    };
    compound.insert(MARKER.into(), Tag::Byte(1));
    let _ = game.native_set_player_persistent_data(player, Tag::Compound(compound));
    check(game, player);
}
