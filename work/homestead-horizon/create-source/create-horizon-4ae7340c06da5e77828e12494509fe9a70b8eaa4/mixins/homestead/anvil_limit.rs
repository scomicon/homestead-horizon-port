// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Too Cheap! 1.4 with the Homestead pack's config: `maxLevelCost` is
// 2147483647, so AnvilMenu's "Too Expensive!" limit of 40 levels never applies.
// The creative answer of `anvil_result` is the only thing the limit depends on,
// so a survival call is answered with the creative one. Every other option of
// the config is vanilla (the cost-rewrite flags are off).

fn homestead_anvil_limit(
    enchantments: &Enchantments,
    items: &Items,
    input: Option<&ItemStack>,
    addition: Option<&ItemStack>,
    name: Option<&str>,
    hover_name: &str,
    creative: bool,
) -> core::ops::ControlFlow<AnvilResult> {
    if creative {
        return core::ops::ControlFlow::Continue(());
    }
    core::ops::ControlFlow::Break(anvil_result(
        enchantments,
        items,
        input,
        addition,
        name,
        hover_name,
        true,
    ))
}