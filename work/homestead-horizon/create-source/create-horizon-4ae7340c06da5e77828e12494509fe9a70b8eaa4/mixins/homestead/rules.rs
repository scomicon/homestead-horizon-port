// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Preferred Gamerules with the Homestead pack's config. The original changes
// the values the Create New World screen starts with. This version has no hook
// for that screen's options, so the pack's values are applied to a world that
// has just been created (game time 0), and only to a rule still at its
// default: a value picked in the Game Rules screen is kept.
//
// `doFireTick=false` is `fire_spread_radius_around_player` 0 here, and
// `mobExplosionDropDecay=false` is `mob_explosion_drop_decay`, the rule the Mob
// Explosion Griefing Gamerule mod adds and this version already has. The mods'
// own rules (`secureFarmlandAndCrops`, `companion:*`) are not registered here.
use super::Game;
use horizon_client::gamerules::{self, Value};

const PREFERRED: [(&str, Value); 2] = [
    ("fire_spread_radius_around_player", Value::Int(0)),
    ("mob_explosion_drop_decay", Value::Bool(false)),
];

pub(super) fn new_world(game: &mut Game<'_>) {
    if game.world.game_time != 0 {
        return;
    }
    for (name, preferred) in PREFERRED {
        let Some(rule) = gamerules::rule(name) else {
            continue;
        };
        if game.ui.rules.get(name) == Some(rule.default) && rule.default != preferred {
            game.set_game_rule(rule, preferred);
        }
    }
}