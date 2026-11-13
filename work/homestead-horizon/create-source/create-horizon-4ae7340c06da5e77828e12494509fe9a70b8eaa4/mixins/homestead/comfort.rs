// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight `ComfortEffect` (1.20, deprecated there in favour of Nourishment): every
// 80 ticks of remaining duration (`isDurationEffectTick`) the player is healed by 1 HP when hurt,
// unless Regeneration is active or the player still has saturation. The amplifier is ignored.
// No food of the content mod grants it; it is given by commands, brewing-less sources and datapacks.

use super::Game;

const EFFECT: &str = "farmersdelight:comfort";
const REGENERATION: &str = "minecraft:regeneration";

pub(super) fn tick(game: &mut Game<'_>) {
    let Some(duration) = game
        .potions
        .effects
        .list
        .iter()
        .find(|effect| effect.id == EFFECT)
        .map(|effect| i64::from(effect.duration))
    else {
        return;
    };
    if duration % 80 != 0 || game.potions.effects.list.iter().any(|effect| effect.id == REGENERATION) {
        return;
    }
    let Some(player) = game.player.as_mut() else {
        return;
    };
    if player.health.food.saturation > 0.0 {
        return;
    }
    let health = player.health.health();
    if health < 20.0 {
        let _ = player.set_health((health + 1.0).min(20.0), false);
    }
}
