// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Client tick (native_client_tick_post: client window only): configuration, keys, snapshot,
// ambience, queued sounds.
fn homestead_sound_tick(game: &mut crate::game::Game<'_>) {
    crate::game::homestead_sound_config::tick(game);
}
