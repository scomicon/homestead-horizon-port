// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//
// Extreme Sound Muffler (rebuilt from its public description, no code or assets of the original):
// a list of sounds whose volume is lowered, raised or muted, plus a quiet mode for everything else.
//
// * `on_play` runs inside `native_sound_play` (a client hook: it fires for every sound that
//   starts or updates, on the game thread, with no `Game`). The first rule of
//   `homestead-muffler.txt` whose pattern matches the sound's event id sets the volume factor;
//   0 cancels the sound. Parameters are the instance's base values, so the edit is transient.
// * F9 switches the list on and off, F10 switches quiet mode (the `@quiet` percentage for every
//   sound no rule matches). Both need the player to be playing (no screen open).
// * The game's own `native_muffle_sounds` is not used: it fades back to normal by itself over a
//   fixed number of ticks, which is not a switch.

use super::{
    Game,
    homestead_sound_config::{glob, state},
};
use crate::sound_engine::SoundPlayback;
use winit::keyboard::KeyCode;

const KEY_LIST: KeyCode = KeyCode::F9;
const KEY_QUIET: KeyCode = KeyCode::F10;

/// `native_sound_play`: scales or cancels the sound a rule names.
pub(crate) fn on_play(sound: &SoundPlayback<'_>, volume: &mut f32, _pitch: &mut f32, cancel: &mut bool) {
    if *cancel {
        return;
    }
    let mut st = state();
    let id = sound.identifier;
    let matched = if st.muffler_enabled {
        st.muffler_rules.iter().find(|rule| glob(&rule.pattern, id)).map(|rule| rule.factor)
    } else {
        None
    };
    let factor = match matched {
        Some(factor) => factor,
        None if st.quiet => st.quiet_percent / 100.0,
        None => return,
    };
    if factor <= 0.0 {
        *cancel = true;
        if sound.starting {
            st.counters.muffler_cancelled += 1;
        }
    } else if (factor - 1.0).abs() > 0.001 {
        *volume *= factor;
        if sound.starting {
            st.counters.muffler_scaled += 1;
        }
    }
}

/// F9 / F10 (client tick): toggles with a chat line, edge triggered.
pub(crate) fn keys(game: &mut Game<'_>) {
    if !game.native_playing() {
        return;
    }
    let down = [game.keys.contains(&KEY_LIST), game.keys.contains(&KEY_QUIET)];
    let message = {
        let mut st = state();
        let list_edge = down[0] && !st.key_down[0];
        let quiet_edge = down[1] && !st.key_down[1];
        st.key_down = down;
        if list_edge {
            st.muffler_enabled = !st.muffler_enabled;
            Some(format!("Sound muffler list: {}", if st.muffler_enabled { "on" } else { "off" }))
        } else if quiet_edge {
            st.quiet = !st.quiet;
            Some(format!(
                "Sound muffler quiet mode: {}",
                if st.quiet { format!("on ({}%)", st.quiet_percent) } else { "off".to_owned() }
            ))
        } else {
            None
        }
    };
    if let Some(text) = message {
        game.native_chat(&text);
    }
}
