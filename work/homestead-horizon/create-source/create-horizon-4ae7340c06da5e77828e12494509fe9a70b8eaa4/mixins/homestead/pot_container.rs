// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_pot_container(block: &str, container: &mut Option<crate::containers::ModContainer>) {
    crate::game::homestead_cooking_pot::container(block, container);
}