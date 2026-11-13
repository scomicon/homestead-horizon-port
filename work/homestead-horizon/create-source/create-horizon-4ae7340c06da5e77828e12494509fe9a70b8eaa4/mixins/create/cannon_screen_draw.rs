// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_screen_draw(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::create_port_cannon::screen_draw(game, block, data, batch, origin);
    crate::game::create_port_schematic_table::screen_draw(game, block, data, batch, origin);
}
