// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_travel_titles_layer(
    game: &mut crate::game::Game<'_>,
    layer: &str,
    batch: &mut horizon_client::hud::HudBatch,
) {
    crate::game::homestead_travel_titles::draw(&*game, layer, batch);
}
