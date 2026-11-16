// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Welcome Screen, rebuilt from the mod's public description (no code or assets copied; see
// docs/client-hud.md): a screen with a few lines of text that the player sees once, the first
// time they are in a world with this install. It is a mod screen (`native_mod_screen_open`):
// the world keeps running under it, the mouse is free and Escape closes it.
//
// Client window only. The client tick (`native_client_tick_post`) opens it: after the player has
// been playing for five seconds, and only when the install has not shown it yet: the marker
// `horizon-mods/.horizon-mod-data/homestead/welcome-shown` (`native_setting`) is written when
// it opens, and the tick looks for the marker every five seconds, not every tick. The screen is
// drawn in `native_screen_render_post` (the hook is read-only), so the click and the Enter or
// Space key only raise a flag and the next tick closes the screen. The text is a default, or the
// lines of `horizon-mods/.horizon-mod-data/homestead/welcome.txt` when that file exists: its
// first line is the title.

use super::Game;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

/// The id this screen is opened under (`native_mod_screen()` answers it while it is up).
const SCREEN: &str = "homestead_welcome";
/// The mod folder (`native_data_dir`) and the marker file in it.
const MOD: &str = "homestead";
const MARKER: &str = "welcome-shown";
/// Ticks of playing before the screen opens, and between two looks for the marker.
const DELAY: u32 = 100;

const DEFAULT: [&str; 5] = [
    "Welcome to Homestead",
    "This is an experimental Rust port of the Homestead modpack",
    "for the Horizon client: Create, Farmer's Delight and a set of small mods.",
    "Many parts of the original pack are not here yet.",
    "See the README for what works and what does not.",
];

static CLOSE: AtomicBool = AtomicBool::new(false);
static OPENED: AtomicBool = AtomicBool::new(false);
static PLAYING: AtomicU32 = AtomicU32::new(0);
static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn lines() -> std::sync::MutexGuard<'static, Vec<String>> {
    LINES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The text of the screen: the custom file's lines (at most ten, 64 characters each) when it has
/// any non-empty line, else the default. The first line is the title.
pub(super) fn parse_text(custom: Option<&str>) -> Vec<String> {
    let from_file: Vec<String> = custom
        .map(|text| {
            text.lines()
                .map(|line| line.trim_end().chars().take(64).collect::<String>())
                .take(10)
                .collect()
        })
        .unwrap_or_default();
    if from_file.iter().any(|line| !line.trim().is_empty()) {
        from_file
    } else {
        DEFAULT.iter().map(|line| (*line).to_owned()).collect()
    }
}

/// Whether the tick should look at the marker now: `playing` counts consecutive playing ticks.
pub(super) fn time_to_look(playing: u32) -> bool {
    playing >= DELAY && playing % DELAY == 0
}

fn own(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(SCREEN)
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    if CLOSE.swap(false, Ordering::SeqCst) {
        game.native_mod_screen_close(SCREEN);
    }
    if game.native_mod_screen().is_some() || OPENED.load(Ordering::SeqCst) {
        return;
    }
    if game.native_player().is_none() || !game.native_playing() {
        PLAYING.store(0, Ordering::SeqCst);
        return;
    }
    let playing = PLAYING.fetch_add(1, Ordering::SeqCst) + 1;
    if !time_to_look(playing) || game.native_setting(MOD, MARKER).is_some() {
        return;
    }
    let custom = std::fs::read_to_string(game.native_data_dir(MOD).join("welcome.txt")).ok();
    *lines() = parse_text(custom.as_deref());
    if game.native_mod_screen_open(SCREEN) {
        OPENED.store(true, Ordering::SeqCst);
        game.native_set_setting(MOD, MARKER, "1");
        log::info!("Homestead welcome screen opened");
    }
}

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !own(game, screen) {
        return;
    }
    let text = {
        let cached = lines();
        if cached.is_empty() {
            parse_text(None)
        } else {
            cached.clone()
        }
    };
    let title = text.first().cloned().unwrap_or_default();
    let body = &text[1.min(text.len())..];
    let (w, h) = (batch.width, batch.height);
    let (panel_w, panel_h) = (320.0_f32, 44.0 + body.len() as f32 * 12.0 + 22.0);
    let (x, y) = ((w - panel_w) / 2.0, (h - panel_h) / 2.0);
    game.native_fill(batch, [0.0, 0.0, w, h], 0xA000_0000);
    game.native_fill(batch, [x - 2.0, y - 2.0, panel_w + 4.0, panel_h + 4.0], 0xFF5A_3F1E);
    game.native_fill(batch, [x, y, panel_w, panel_h], 0xF01C_140C);
    let title_width = game.native_text_width(&title);
    game.native_text(batch, &title, [x + (panel_w - title_width) / 2.0, y + 12.0], 0x00FF_D34D, true);
    for (row, line) in body.iter().enumerate() {
        game.native_text(batch, line, [x + 16.0, y + 34.0 + row as f32 * 12.0], 0x00E0_E0E0, false);
    }
    let footer = "Click, Enter or Escape to continue";
    let footer_width = game.native_text_width(footer);
    game.native_text(batch, footer, [x + (panel_w - footer_width) / 2.0, y + panel_h - 16.0], 0x00A0_A0A0, false);
}

pub(super) fn click(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if own(game, screen) && button == 0 {
        CLOSE.store(true, Ordering::SeqCst);
        *canceled = true;
    }
}

pub(super) fn key(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, repeat: bool, canceled: &mut bool) {
    if repeat || !own(game, screen) {
        return;
    }
    if matches!(key, "key.keyboard.enter" | "key.keyboard.space") {
        CLOSE.store(true, Ordering::SeqCst);
        *canceled = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_text_has_a_title_and_a_body() {
        let text = parse_text(None);
        assert_eq!(text[0], "Welcome to Homestead");
        assert!(text.len() > 1);
        // An empty or blank file falls back to the default.
        assert_eq!(parse_text(Some("  \n\n")), text);
    }

    #[test]
    fn a_custom_file_replaces_the_text_and_is_trimmed() {
        let long = "x".repeat(100);
        let text = parse_text(Some(&format!("Hello\nline one   \n{long}\n")));
        assert_eq!(text[0], "Hello");
        assert_eq!(text[1], "line one");
        assert_eq!(text[2].chars().count(), 64);
        // At most ten lines.
        let many = (0..30).map(|n| format!("l{n}")).collect::<Vec<_>>().join("\n");
        assert_eq!(parse_text(Some(&many)).len(), 10);
    }

    #[test]
    fn the_marker_is_looked_for_every_five_seconds_after_the_delay() {
        assert!(!time_to_look(1));
        assert!(!time_to_look(99));
        assert!(time_to_look(100));
        assert!(!time_to_look(150));
        assert!(time_to_look(200));
    }
}
