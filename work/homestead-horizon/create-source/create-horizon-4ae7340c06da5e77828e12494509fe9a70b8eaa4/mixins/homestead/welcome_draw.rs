// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_welcome_draw(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::mod_api::NativeScreenView<'_>,
) {
    crate::game::homestead_welcome::draw(game, batch, screen);
}
