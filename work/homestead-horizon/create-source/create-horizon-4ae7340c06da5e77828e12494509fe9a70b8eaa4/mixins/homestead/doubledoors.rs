// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Double Doors 7.2 (Serilum) with its documented defaults: enableDoors on, fence gates and
// trapdoors off, recursive opening not ported. Clicking one half of a double door opens or
// closes the other half too; sneaking disables it. Written from the mod's public description
// (the jar is all rights reserved and was not read); see docs/logic-a.md for the differences.

use super::Game;

/// `DoorBlock` family that a hand can open: every `*_door` but the iron one (iron doors are
/// redstone only; the mod opens them by button, which this port does not do).
fn hand_door(block: &str) -> bool {
    block.ends_with("_door") && block != "minecraft:iron_door"
}

/// Forward vector (x, z) of a door's `facing` (the side its front looks at).
fn forward(facing: &str) -> Option<[i32; 2]> {
    Some(match facing {
        "north" => [0, -1],
        "south" => [0, 1],
        "east" => [1, 0],
        "west" => [-1, 0],
        _ => return None,
    })
}

fn is_open(game: &Game<'_>, pos: [i32; 3]) -> Option<bool> {
    Some(game.native_block_property(pos, "open")? == "true")
}

/// The lower half of the partner of the door whose lower half is at `lower`: the door of the
/// same block, same facing, opposite hinge, standing on the side away from the hinge.
fn partner(game: &Game<'_>, lower: [i32; 3], block: &str) -> Option<[i32; 3]> {
    let facing = game.native_block_property(lower, "facing")?;
    let hinge = game.native_block_property(lower, "hinge")?;
    let [fx, fz] = forward(&facing)?;
    // The viewer's right when looking at the front: (fz, -fx).
    let (rx, rz) = (fz, -fx);
    let (dx, dz) = if hinge == "left" { (rx, rz) } else { (-rx, -rz) };
    let other = [lower[0] + dx, lower[1], lower[2] + dz];
    if game.native_block(other) != block
        || game.native_block_property(other, "half").as_deref() != Some("lower")
        || game.native_block_property(other, "facing").as_deref() != Some(facing.as_str())
    {
        return None;
    }
    let other_hinge = game.native_block_property(other, "hinge")?;
    (other_hinge != hinge).then_some(other)
}

/// Runs before the door's own use (`right_click_block` HEAD): the clicked door will flip, so the
/// partner is set to the opposite of the clicked door's current state. Never cancels the click.
pub(super) fn click(game: &mut Game<'_>, pos: [i32; 3]) {
    if !game.native_authoritative() || game.native_sneaking() {
        return;
    }
    let block = game.native_block(pos);
    if !hand_door(&block) {
        return;
    }
    let lower = match game.native_block_property(pos, "half").as_deref() {
        Some("upper") => [pos[0], pos[1] - 1, pos[2]],
        Some("lower") => pos,
        _ => return,
    };
    if game.native_block(lower) != block {
        return;
    }
    let Some(open) = is_open(game, lower) else {
        return;
    };
    let Some(other) = partner(game, lower, &block) else {
        return;
    };
    let value = if open { "false" } else { "true" };
    // Only a partner that is not already in the state the click is about to give the clicked door.
    if is_open(game, other) == Some(!open) {
        return;
    }
    for half in [other, [other[0], other[1] + 1, other[2]]] {
        if game.native_block(half) == block {
            game.native_set_block_property(half, "open", value);
        }
    }
}
