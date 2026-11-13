// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight (MIT) `NourishmentEffect`: while the effect lasts, the player's
// exhaustion is reset every tick, so hunger does not drain, unless the player is
// healing from saturation (natural regeneration on, hurt, saturation above 0), where
// exhaustion is left alone so the saturation can be spent on healing.

use super::Game;

const EFFECT: &str = "farmersdelight:nourishment";

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.potions.effects.list.iter().any(|effect| effect.id == EFFECT) {
        return;
    }
    let regeneration = game.ui.rules.bool("natural_health_regeneration");
    let Some(player) = game.player.as_mut() else {
        return;
    };
    let healing = regeneration
        && player.health.health() < 20.0
        && player.health.food.saturation > 0.0;
    if !healing {
        player.health.food.exhaustion = 0.0;
    }
}