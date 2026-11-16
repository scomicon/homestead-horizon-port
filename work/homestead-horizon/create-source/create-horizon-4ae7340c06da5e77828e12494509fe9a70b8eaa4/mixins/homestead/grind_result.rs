// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Grind Enchantments: the grindstone's result for an enchanted item beside a plain book is the enchanted book
// of `homestead_grind::plan`; anything else is vanilla's `grindstone_result`.

fn homestead_grind_result(
    e: &Enchantments,
    input: Option<&ItemStack>,
    additional: Option<&ItemStack>,
) -> core::ops::ControlFlow<Option<ItemStack>> {
    match crate::stations::homestead_grind::plan(e, input, additional) {
        Some(plan) => core::ops::ControlFlow::Break(Some(plan.result)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
