// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Invisible Frames 1.1.0 (Roundaround, MIT), from its README: sneaking while right-clicking an
// item frame toggles the frame's invisibility. Item frames keep the vanilla `Invisible` flag, so
// the toggle goes through the entity data the engine already reloads (`native_entity_merge_nbt`).

use super::Game;
use horizon_client::mod_api::EntityRef;
use horizon_world::nbt::{Compound, Tag};

fn frame(kind: &str) -> bool {
    matches!(kind, "minecraft:item_frame" | "minecraft:glow_item_frame")
}

/// `interact_entity`: true when the click was taken (the engine swings the arm).
pub(super) fn interact(game: &mut Game<'_>, entity: i32, kind: &str) -> bool {
    if !frame(kind) || !game.native_sneaking() || entity < 0 {
        return false;
    }
    let Some(epoch) = game.native_lifecycle.epoch() else {
        return false;
    };
    let handle = EntityRef::new(epoch, entity as u32);
    let Some(data) = game.native_entity_nbt(handle) else {
        return false;
    };
    let invisible = matches!(data.get(&"Invisible".into()), Some(Tag::Byte(1)));
    let mut patch = Compound::default();
    patch.insert("Invisible".into(), Tag::Byte(if invisible { 0 } else { 1 }));
    game.native_entity_merge_nbt(handle, patch)
}
