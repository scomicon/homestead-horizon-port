// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//
// Presence Footsteps (rebuilt from its public description, no code or assets of the original):
// the step sound depends on the block that is stepped on. A rule of `homestead-footsteps.txt`
// replaces the vanilla step sound or adds a second one on top. The mod ships no sounds: the
// replacement events come from the player's own resource pack.
//
// * `on_play` runs inside `native_sound_play`. A step is a starting sound whose event id is
//   `minecraft:*.step` (blocks and mobs alike) played at a position near the local player.
// * The hook has no world access, so the client tick records a snapshot of the blocks around
//   the player (5 x 5 columns, three levels, every second tick) and the hook looks the stepped-on
//   block up in it. A step outside the snapshot (a mob far away) is left alone.
// * The replacement is not played from the hook (no `Game` there): it is queued and the client
//   tick plays it with `native_play_local_sound` on the next tick, at most 50 ms later.
// * Replacement events that are themselves `minecraft:*.step` are refused when the file is read,
//   so a replacement never meets the hook as a step again.

use super::{
    Game,
    homestead_sound_config::{MAX_PENDING, Pending, Snapshot, glob, state},
};
use crate::sound_engine::SoundPlayback;

const RADIUS: i32 = 2;

fn is_step(identifier: &str) -> bool {
    identifier.starts_with("minecraft:") && identifier.ends_with(".step")
}

/// `native_sound_play`: swaps or doubles the step sound of a matching block.
pub(crate) fn on_play(sound: &SoundPlayback<'_>, volume: &mut f32, pitch: &mut f32, cancel: &mut bool) {
    if *cancel || !sound.starting || sound.relative || !is_step(sound.identifier) {
        return;
    }
    let mut st = state();
    if st.footsteps.is_empty() || !st.snapshot.valid {
        return;
    }
    let [x, y, z] = sound.position;
    let (bx, bz) = (x.floor() as i32, z.floor() as i32);
    // The block the foot rests on: the one at the sound's level, else the one below.
    let top = (y - 0.01).floor() as i32;
    let Some(block) = st
        .snapshot
        .lookup(bx, top, bz)
        .or_else(|| st.snapshot.lookup(bx, top - 1, bz))
        .map(str::to_owned)
    else {
        return;
    };
    let Some(index) = st.footsteps.iter().position(|rule| glob(&rule.block, &block)) else {
        return;
    };
    let (replace, answer, answer_volume, answer_pitch) = {
        let rule = &st.footsteps[index];
        (rule.replace, rule.sound.clone(), rule.volume, rule.pitch)
    };
    if st.pending.len() >= MAX_PENDING {
        return;
    }
    st.pending.push(Pending {
        sound: answer,
        at: sound.position,
        volume: answer_volume * *volume,
        pitch: answer_pitch * *pitch,
        ambient: false,
    });
    if replace {
        *cancel = true;
        st.counters.footsteps_replaced += 1;
    } else {
        st.counters.footsteps_added += 1;
    }
}

/// Client tick: records the blocks around the player every second tick (only the one under the
/// feet while no footstep rule exists).
pub(crate) fn refresh(game: &mut Game<'_>) {
    let (due, with_rules) = {
        let st = state();
        (st.ticks % 2 == 0, !st.footsteps.is_empty())
    };
    if !due {
        return;
    }
    let Some(player) = game.native_player() else { return };
    let (px, py, pz) = (player.x.floor() as i32, (player.y - 0.01).floor() as i32, player.z.floor() as i32);
    let radius = if with_rules { RADIUS } else { 0 };
    let mut blocks = Vec::new();
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            for dy in -1..=1 {
                let pos = [px + dx, py + dy, pz + dz];
                blocks.push((pos, game.native_block(pos)));
            }
        }
    }
    let snapshot = Snapshot { valid: true, blocks };
    let under = snapshot
        .lookup(px, py, pz)
        .or_else(|| snapshot.lookup(px, py - 1, pz))
        .unwrap_or("minecraft:air")
        .to_owned();
    let mut st = state();
    st.snapshot = snapshot;
    st.under = under;
}
