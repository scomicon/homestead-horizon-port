// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_storage_ticks(block: &str, ticks: &mut bool) {
    crate::game::homestead_storage::ticks(block, ticks);
}
