// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_graves_loot(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<crate::items::ItemStack>,
) {
    crate::game::homestead_graves::death_drops(game, context, drops);
}
