// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Peaceful Hunger, rebuilt from the mod's public description (hunger works in Peaceful as it does in the other
// difficulties). The work is one engine switch, `horizon_client::hunger::PEACEFUL_HUNGER`, added by engine patch
// 21-peaceful-hunger.ps1: while it is true the Peaceful regeneration no longer refills the food and saturation
// bars (the health regeneration stays). This module sets it once from `config/homestead-hunger.txt`:
//
//     peaceful_hunger=false     turns the mod off (the default is on, as in the Homestead pack)

use super::Game;
use std::sync::{Once, atomic::Ordering};

static INIT: Once = Once::new();

/// First server tick: read the config and set the engine switch.
pub(super) fn tick(_game: &mut Game<'_>) {
    INIT.call_once(|| {
        let off = std::fs::read_to_string("config/homestead-hunger.txt").is_ok_and(|text| {
            text.lines()
                .any(|line| line.trim().eq_ignore_ascii_case("peaceful_hunger=false"))
        });
        horizon_client::hunger::PEACEFUL_HUNGER.store(!off, Ordering::Relaxed);
    });
}
