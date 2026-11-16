// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_ctip_color(
    game: &crate::game::Game<'_>,
    stack: Option<&horizon_client::items::ItemStack>,
    background: &mut u32,
    border_top: &mut u32,
    border_bottom: &mut u32,
) {
    crate::game::homestead_client_tooltips::color(
        game,
        stack,
        background,
        border_top,
        border_bottom,
    );
}
