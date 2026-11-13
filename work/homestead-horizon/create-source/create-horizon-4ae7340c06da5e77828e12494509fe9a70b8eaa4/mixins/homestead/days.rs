// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Better Days 3.3.4.5 (Serilum, LGPL-3.0-or-later) with the Homestead pack's
// betterdays-common.toml: speedMethod MINUTES, a day of 25 minutes and a night of
// 15 minutes, day from 23500 to 12500, night from 12500 to 23500.

use super::Game;

const DAY_START: i64 = 23500;
const NIGHT_START: i64 = 12500;
const DAY_MINUTES: f32 = 25.0;
const NIGHT_MINUTES: f32 = 15.0;

/// Clock ticks per game tick for the given day time.
fn rate(day_time: i64) -> f32 {
    let t = day_time.rem_euclid(24000);
    let night = t >= NIGHT_START && t < DAY_START;
    if night {
        (DAY_START - NIGHT_START) as f32 / (NIGHT_MINUTES * 1200.0)
    } else {
        (24000 - (DAY_START - NIGHT_START)) as f32 / (DAY_MINUTES * 1200.0)
    }
}

pub(super) fn before_clock(game: &mut Game<'_>) {
    game.ui.clock.rate = rate(game.world.day_time);
}