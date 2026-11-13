// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_graves_container(block: &str, container: &mut Option<crate::containers::ModContainer>) {
    crate::game::homestead_graves::container(block, container);
}
