// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_pot_ticks(block: &str, ticks: &mut bool) {
    crate::game::homestead_cooking_pot::ticks(block, ticks);
}
