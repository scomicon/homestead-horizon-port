// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Behaviour after Panda's Falling Tree's 0.13.2 (a player breaking a log fells the whole tree
// above it, `TreeHandler.destroyTree` on `BlockEvent.Break`), written from the observable
// behaviour and the mod's configuration names; no source of the mod is used. Default
// configuration: the generic tree type (`#minecraft:logs` stems, `#minecraft:leaves` leaves,
// persistent leaves ignored), crouch mining allowed (sneaking breaks one block), no tool required,
// tool damage and food exhaustion multiplied by the number of logs.
//
// Not ported (docs/logic-b.md): the falling tree entity, its fall/bounce animation, its sounds,
// the dynamic mining speed that slows a big trunk, the vertical tree types (cactus, bamboo: their
// support rules already drop the column), chorus and huge mushroom trees, the config files.
use super::Game;
use horizon_world::player_inventory::EquipmentSlot;
use std::{
    collections::{HashSet, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
};

type Pos = [i32; 3];

/// `GenericTreeConfig.Algorithm.maxLogAmount` (recalled default; larger trees are left standing,
/// the original's `TreeTooBigException`).
const MAX_LOGS: usize = 500;
/// Leaves taken with one tree; a safety cap, the original's radius is `maxLeavesRadius`.
const MAX_LEAVES: usize = 2000;
/// A leaf block holds on up to distance 7 from a log (`LeavesBlock.DECAY_DISTANCE`).
const LEAF_STEPS: u32 = 7;
/// Vanilla block breaking costs 0.005 exhaustion; the original multiplies it by the logs felled.
const EXHAUSTION: f32 = 0.005;

const SIDES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Set while the tree's own blocks are broken, so none of them fells a tree again.
static BUSY: AtomicBool = AtomicBool::new(false);

fn in_tag(game: &Game<'_>, pos: Pos, tag: &str) -> bool {
    let registry = game.pack.assets.registry;
    registry
        .state(game.world.block(pos[0], pos[1], pos[2]))
        .is_some_and(|state| game.native_server_block_in_tag(state, tag).unwrap_or(false))
}

fn is_log(game: &Game<'_>, pos: Pos) -> bool {
    game.world.loaded(pos[0], pos[2]) && in_tag(game, pos, "minecraft:logs")
}

/// A leaf block a tree takes with it: not placed by a player (`persistent`).
fn is_leaf(game: &Game<'_>, pos: Pos) -> bool {
    game.world.loaded(pos[0], pos[2])
        && in_tag(game, pos, "minecraft:leaves")
        && game.native_block_property(pos, "persistent").as_deref() != Some("true")
}

struct Tree {
    logs: Vec<Pos>,
    leaves: Vec<Pos>,
}

/// `GenericTreeType.gatherTreeData`: the logs joined to `origin` through any of their 26
/// neighbours, never below `origin`, then the leaves held up by them. None when it is not a tree
/// (no leaves: a log wall or a pillar) or is too big.
fn gather(game: &Game<'_>, origin: Pos) -> Option<Tree> {
    let mut logs = vec![origin];
    let mut seen: HashSet<Pos> = HashSet::from([origin]);
    let mut queue = VecDeque::from([origin]);
    while let Some(at) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if (dx, dy, dz) == (0, 0, 0) {
                        continue;
                    }
                    let next = [at[0] + dx, at[1] + dy, at[2] + dz];
                    if next[1] < origin[1] || !seen.insert(next) || !is_log(game, next) {
                        continue;
                    }
                    if logs.len() >= MAX_LOGS {
                        return None;
                    }
                    logs.push(next);
                    queue.push_back(next);
                }
            }
        }
    }
    let mut leaves: Vec<Pos> = Vec::new();
    let mut seen_leaves: HashSet<Pos> = HashSet::new();
    let mut frontier: VecDeque<(Pos, u32)> = VecDeque::new();
    for log in &logs {
        for side in SIDES {
            let next = [log[0] + side[0], log[1] + side[1], log[2] + side[2]];
            if seen_leaves.insert(next) && is_leaf(game, next) {
                leaves.push(next);
                frontier.push_back((next, 1));
            }
        }
    }
    while let Some((at, steps)) = frontier.pop_front() {
        if steps >= LEAF_STEPS || leaves.len() >= MAX_LEAVES {
            continue;
        }
        for side in SIDES {
            let next = [at[0] + side[0], at[1] + side[1], at[2] + side[2]];
            if seen_leaves.insert(next) && is_leaf(game, next) {
                leaves.push(next);
                frontier.push_back((next, steps + 1));
            }
        }
    }
    if leaves.is_empty() {
        return None;
    }
    Some(Tree { logs, leaves })
}

/// `EventHandler.onBlockBreak`: a player breaks the log at `pos` (the block is still there; the
/// game breaks it after this). The rest of its tree is broken now, with the tool in hand.
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if *canceled || BUSY.load(Ordering::Relaxed) {
        return;
    }
    // `isCrouchMiningAllowed`: sneaking breaks the one block.
    if game.native_sneaking() || !game.native_authoritative() || !is_log(game, pos) {
        return;
    }
    let Some(tree) = gather(game, pos) else {
        return;
    };
    BUSY.store(true, Ordering::Relaxed);
    let tool = game.held().cloned();
    let creative = game.creative;
    let mut extra: u32 = 0;
    for log in tree.logs.iter().filter(|log| **log != pos) {
        // In creative the blocks go without drops, as a creative player's own break does.
        if game
            .native_machine_destroy_block(*log, tool.as_ref(), creative)
            .is_some()
        {
            extra += 1;
        }
    }
    for leaf in &tree.leaves {
        let _ = game.native_machine_destroy_block(*leaf, tool.as_ref(), creative);
    }
    BUSY.store(false, Ordering::Relaxed);
    if creative || extra == 0 {
        return;
    }
    // `multiplyFoodExhaustion` and `multiplyToolDamage`: what the extra logs would have cost.
    if let Some(player) = game.player.as_mut() {
        player.cause_food_exhaustion(EXHAUSTION * extra as f32);
    }
    game.damage_held_in_slot(extra, EquipmentSlot::MainHand);
}
