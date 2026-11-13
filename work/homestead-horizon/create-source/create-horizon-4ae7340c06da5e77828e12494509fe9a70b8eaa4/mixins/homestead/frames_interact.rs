// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_frames_interact(
    game: &mut crate::game::Game<'_>,
    entity: i32,
    kind: &str,
    claimed: &mut bool,
) {
    if !*claimed && crate::game::homestead_frames::interact(game, entity, kind) {
        *claimed = true;
    }
}