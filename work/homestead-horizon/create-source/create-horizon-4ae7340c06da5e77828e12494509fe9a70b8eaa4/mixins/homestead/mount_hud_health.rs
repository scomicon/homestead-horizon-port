// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_mount_hud_health(
    game: &mut crate::game::Game<'_>,
    inputs: &horizon_client::hud_layout::HealthInputs,
    wi: i32,
    hi: i32,
    draws: &mut Option<Vec<horizon_client::hud_layout::GuiDraw>>,
) {
    crate::game::homestead_mount_hud::health(game, inputs, wi, hi, draws);
}
