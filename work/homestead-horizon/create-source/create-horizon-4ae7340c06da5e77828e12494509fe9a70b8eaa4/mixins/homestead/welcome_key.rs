// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_welcome_key(
    game: &crate::game::Game<'_>,
    screen: &crate::mod_api::NativeScreenView<'_>,
    key: &str,
    repeat: bool,
    canceled: &mut bool,
) {
    crate::game::homestead_welcome::key(game, screen, key, repeat, canceled);
}
