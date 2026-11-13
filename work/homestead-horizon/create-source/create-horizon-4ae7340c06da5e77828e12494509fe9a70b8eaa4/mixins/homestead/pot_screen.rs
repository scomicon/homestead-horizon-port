// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_pot_screen(block: &str, screen: &mut Option<crate::mod_api::NativeScreen>) {
    crate::game::homestead_cooking_pot::screen(block, screen);
}