// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Client sound hook (native_sound_play, a free function of the client library crate with no
// Game): hands every sound to the callback the game crate installed in homestead_sound_bridge.
fn homestead_sound_play(
    sound: &crate::sound_engine::SoundPlayback<'_>,
    volume: &mut f32,
    pitch: &mut f32,
    cancel: &mut bool,
) {
    homestead_sound_bridge::call(sound, volume, pitch, cancel);
}