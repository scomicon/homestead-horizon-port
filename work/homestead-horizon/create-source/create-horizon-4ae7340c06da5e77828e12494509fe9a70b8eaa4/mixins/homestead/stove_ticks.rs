// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_stove_ticks(block: &str, ticks: &mut bool) {
    crate::game::homestead_stove::ticks(block, ticks);
}
