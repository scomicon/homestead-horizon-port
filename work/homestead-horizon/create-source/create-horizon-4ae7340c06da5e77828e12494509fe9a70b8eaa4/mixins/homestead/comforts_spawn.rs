// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_comforts_spawn(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    cause: &str,
    cancel: &mut bool,
) {
    crate::game::homestead_comforts::spawn_point_set(game, pos, cause, cancel);
}
