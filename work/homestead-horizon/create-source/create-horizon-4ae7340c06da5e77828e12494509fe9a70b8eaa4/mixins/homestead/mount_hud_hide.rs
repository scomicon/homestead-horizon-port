// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_mount_hud_hide(game: &crate::game::Game<'_>, layer: &str, canceled: &mut bool) {
    crate::game::homestead_mount_hud::hide(game, layer, canceled);
}
