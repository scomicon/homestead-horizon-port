// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Shulker Drops Two 3.5 (Serilum), default behaviour from its public README: a shulker that
// would drop a shell (the vanilla loot table decides, 50 % with looting) drops two instead.
// The "always drop" and the amount options are not ported. The jar was not read.

use crate::items::ItemStack;
use horizon_client::mod_api::NativeLootDrops;

const SHELL: &str = "minecraft:shulker_shell";
const AMOUNT: u32 = 2;

/// `loot_drops` HEAD: raise a rolled shell stack of a dying shulker to two.
pub(super) fn drops(context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let Some((_, kind)) = context.entity else {
        return;
    };
    if kind != "minecraft:shulker" {
        return;
    }
    for stack in drops.iter_mut() {
        if stack.item.as_ref() == SHELL && stack.count > 0 && (stack.count as u32) < AMOUNT {
            stack.count = AMOUNT as _;
        }
    }
}
