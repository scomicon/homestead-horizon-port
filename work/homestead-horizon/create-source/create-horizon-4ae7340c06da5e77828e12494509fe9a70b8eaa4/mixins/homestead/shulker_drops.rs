// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_shulker_drops(
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<crate::items::ItemStack>,
) {
    crate::game::homestead_shulker::drops(context, drops);
}