// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_storage_container(block: &str, container: &mut Option<crate::containers::ModContainer>) {
    crate::game::homestead_storage::container(block, container);
}
