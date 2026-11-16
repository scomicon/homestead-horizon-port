// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Bridge between the sound dispatcher in the client library crate (sound_playback.rs, which cannot
// see the game crate) and the Homestead sound modules in the game crate: the game side installs
// one callback at its first client tick, the library side calls it for every sound.
use std::sync::OnceLock;

pub type Callback = fn(&crate::sound_engine::SoundPlayback<'_>, &mut f32, &mut f32, &mut bool);

static CALLBACK: OnceLock<Callback> = OnceLock::new();

/// Installs the callback once; later calls keep the first one.
pub fn install(callback: Callback) {
    let _ = CALLBACK.set(callback);
}

/// Called by the injected head of `native_sound_play` for every sound that starts or updates.
pub fn call(
    sound: &crate::sound_engine::SoundPlayback<'_>,
    volume: &mut f32,
    pitch: &mut f32,
    cancel: &mut bool,
) {
    if let Some(callback) = CALLBACK.get() {
        callback(sound, volume, pitch, cancel);
    }
}