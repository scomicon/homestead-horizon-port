// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Accelerated Decay 3.0.1 (errormikey, GPL-3.0), from its public description: once the last log
// that holds up a set of leaves is gone, the leaves are removed as soon as they are ready instead
// of waiting for random ticks. The mod delays its check so the game can mark the leaves first;
// this port does the same with a short per-removal queue. No mod source was read or copied.

use super::{Game, GameLevel};
use horizon_core::position::BlockPos;
use horizon_world::{
    level::{Dimension, flags},
    registry::BlockStateId,
};
use std::sync::Mutex;

/// Ticks to wait so vanilla's distance updates have run before leaves are checked.
const WAIT: i32 = 5;
/// `LeavesBlock.DECAY_DISTANCE`: leaves at this distance have no log within reach.
const DECAY_DISTANCE: &str = "7";
/// How far around a removed log leaves can have depended on it.
const RADIUS: i32 = 7;
/// Checks per removal; each one that found leaves queues the next.
const ROUNDS: u8 = 6;
const MAX_JOBS: usize = 128;

struct Job {
    dimension: &'static str,
    pos: [i32; 3],
    wait: i32,
    rounds: u8,
}

static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());

/// `#minecraft:logs` by name: logs, woods, stems and hyphae, stripped ones included.
fn log(name: &str) -> bool {
    ["_log", "_wood", "_stem", "_hyphae"].iter().any(|end| name.ends_with(end))
}

fn leaves(name: &str) -> bool {
    name.ends_with("_leaves")
}

/// `block_changed`: a log disappeared, so leaves near it may have lost their support.
pub(super) fn changed(
    level: &GameLevel<'_>,
    dimension: Dimension,
    pos: BlockPos,
    old: BlockStateId,
    new: BlockStateId,
) {
    if !level.native_authoritative {
        return;
    }
    let (Some(was), Some(now)) = (level.registry.state(old), level.registry.state(new)) else {
        return;
    };
    if !log(&was.name) || log(&now.name) {
        return;
    }
    let dimension = match dimension {
        Dimension::Overworld => "minecraft:overworld",
        Dimension::Nether => "minecraft:the_nether",
        Dimension::End => "minecraft:the_end",
    };
    if let Ok(mut jobs) = JOBS.lock() {
        if jobs.len() < MAX_JOBS {
            jobs.push(Job { dimension, pos: [pos.x, pos.y, pos.z], wait: WAIT, rounds: ROUNDS });
        }
    }
}

/// `LeavesBlock.randomTick` for a block that is ready: `dropResources`, then `removeBlock`.
fn decay(game: &mut Game<'_>, pos: [i32; 3]) {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let drops = game.block_loot_at(pos, state, None, None);
    let Some(air) = game.pack.assets.registry.default_state("minecraft:air") else {
        return;
    };
    if game.native_set_block(game.dim.id(), pos, air, flags::UPDATE_ALL) {
        for stack in drops.into_iter().filter(|stack| stack.count > 0) {
            game.pop_resource(pos, stack);
        }
    }
}

/// Returns (leaves removed, leaves still standing that are not persistent) around `centre`.
fn sweep(game: &mut Game<'_>, centre: [i32; 3]) -> (u32, u32) {
    let (mut removed, mut standing) = (0, 0);
    for dx in -RADIUS..=RADIUS {
        for dy in -RADIUS..=RADIUS {
            for dz in -RADIUS..=RADIUS {
                let pos = [centre[0] + dx, centre[1] + dy, centre[2] + dz];
                if !leaves(&game.native_block(pos)) {
                    continue;
                }
                if game.native_block_property(pos, "persistent").as_deref() == Some("true") {
                    continue;
                }
                if game.native_block_property(pos, "distance").as_deref() == Some(DECAY_DISTANCE) {
                    decay(game, pos);
                    removed += 1;
                } else {
                    standing += 1;
                }
            }
        }
    }
    (removed, standing)
}

/// `level_tick_post` HEAD: run the checks that have waited long enough.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_authoritative() || game.dim.id() != dimension {
        return;
    }
    let due: Vec<Job> = {
        let Ok(mut jobs) = JOBS.lock() else {
            return;
        };
        let mut due = Vec::new();
        let mut keep = Vec::new();
        for mut job in jobs.drain(..) {
            if job.dimension == dimension {
                job.wait -= 1;
                if job.wait <= 0 {
                    due.push(job);
                    continue;
                }
            }
            keep.push(job);
        }
        *jobs = keep;
        due
    };
    for job in due {
        let (removed, standing) = sweep(game, job.pos);
        // Removing leaves changes distances nearby, and distances travel one block a tick: look
        // again while anything within reach is still there.
        if job.rounds > 1 && (removed > 0 || standing > 0) {
            if let Ok(mut jobs) = JOBS.lock() {
                if jobs.len() < MAX_JOBS {
                    jobs.push(Job { wait: WAIT, rounds: job.rounds - 1, ..job });
                }
            }
        }
    }
}
