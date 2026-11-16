// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Traveler's Titles, rebuilt from the mod's public description (no code or assets copied; see
// docs/client-hud.md): a banner with the biome's name when the player walks into another biome
// and with the dimension's name (and the biome as its subtitle) when they change dimension.
//
// Client window only. The client tick (`native_client_tick_post`, 20 times a second) reads the
// biome under the player (`native_biome`, from the chunks the server sent), keeps a small state
// and a banner timer; the banner is painted after the `minecraft:title` HUD layer, so it hides
// with F1 like vanilla's titles do. A new biome has to hold for a second before it counts and
// two banners are at least two seconds apart, so a border does not flicker. Names come from the
// language files (`biome.<namespace>.<path>`) and fall back to the id in words.

use super::Game;
use horizon_client::{
    hud::HudBatch,
    hud_layout::{GuiDraw, PoseOp},
    text::Text,
};
use std::sync::Mutex;

const FADE_IN: u32 = 10;
const HOLD: u32 = 50;
const FADE_OUT: u32 = 20;
/// Ticks a banner lives.
const TOTAL: u32 = FADE_IN + HOLD + FADE_OUT;
/// Ticks a new biome must stay under the player before it counts.
const SETTLE: u32 = 20;
/// Ticks between two banners.
const QUIET: u32 = 40;
const LAYER: &str = "minecraft:title";

struct State {
    dimension: Option<String>,
    biome: Option<String>,
    candidate: Option<String>,
    stable: u32,
    quiet: u32,
    age: u32,
    active: bool,
    title: String,
    subtitle: String,
}

impl State {
    const fn new() -> Self {
        Self {
            dimension: None,
            biome: None,
            candidate: None,
            stable: 0,
            quiet: 0,
            age: 0,
            active: false,
            title: String::new(),
            subtitle: String::new(),
        }
    }
}

static STATE: Mutex<State> = Mutex::new(State::new());

fn lock() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `minecraft:dark_forest` as words: `Dark Forest`; `the_nether` becomes `The Nether`.
pub(super) fn pretty(id: &str) -> String {
    let path = id.rsplit(':').next().unwrap_or(id);
    path.split(|c: char| c == '_' || c == '/')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The language key of a biome id: `minecraft:plains` is `biome.minecraft.plains`.
pub(super) fn biome_key(id: &str) -> String {
    format!("biome.{}", id.replacen(':', ".", 1))
}

fn biome_name(game: &Game<'_>, id: &str) -> String {
    game.translate(&biome_key(id), &pretty(id))
}

fn start(state: &mut State, title: String, subtitle: String) {
    log::info!("Homestead titles: {title} / {subtitle}");
    state.title = title;
    state.subtitle = subtitle;
    state.age = 0;
    state.active = true;
    state.quiet = 0;
}

/// What a tick decides, apart from the game: `None` keeps things as they are.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Change {
    /// First reading after joining or after a dimension change (`first` when it is the first of
    /// the session).
    Dimension { first: bool },
    /// The biome changed and held.
    Biome,
}

/// The state machine of a tick, without the game and the banner: feeds the reading `dimension`
/// and `biome`, returns what to announce. `quiet` counts ticks since the last banner.
fn step(
    dimension: &str,
    biome: &str,
    state_dimension: &mut Option<String>,
    state_biome: &mut Option<String>,
    candidate: &mut Option<String>,
    stable: &mut u32,
    quiet: u32,
) -> Option<Change> {
    let first = state_dimension.is_none();
    let dimension_changed = state_dimension.as_deref().is_some_and(|d| d != dimension);
    if first || dimension_changed {
        *state_dimension = Some(dimension.to_owned());
        *state_biome = Some(biome.to_owned());
        *candidate = None;
        *stable = 0;
        return Some(Change::Dimension { first });
    }
    if state_biome.as_deref() == Some(biome) {
        *candidate = None;
        *stable = 0;
        return None;
    }
    if candidate.as_deref() == Some(biome) {
        *stable += 1;
    } else {
        *candidate = Some(biome.to_owned());
        *stable = 1;
    }
    if *stable >= SETTLE && quiet >= QUIET {
        *state_biome = Some(biome.to_owned());
        *candidate = None;
        *stable = 0;
        return Some(Change::Biome);
    }
    None
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let mut state = lock();
    let Some(player) = game.native_player() else {
        // Title screen, loading or between dimensions: the next world starts fresh.
        *state = State::new();
        return;
    };
    state.quiet = state.quiet.saturating_add(1);
    if state.active {
        state.age += 1;
        if state.age >= TOTAL {
            state.active = false;
        }
    }
    let dimension = player.dimension.to_string();
    let pos = [
        player.x.floor() as i32,
        player.y.floor() as i32,
        player.z.floor() as i32,
    ];
    let Some(biome) = game.native_biome(pos) else {
        return;
    };
    let quiet = state.quiet;
    let state = &mut *state;
    let change = step(
        &dimension,
        &biome,
        &mut state.dimension,
        &mut state.biome,
        &mut state.candidate,
        &mut state.stable,
        quiet,
    );
    match change {
        Some(Change::Dimension { first: true }) | Some(Change::Biome) => {
            start(state, biome_name(game, &biome), String::new());
        }
        Some(Change::Dimension { first: false }) => {
            start(state, pretty(&dimension), biome_name(game, &biome));
        }
        None => {}
    }
}

/// The opacity (0..1) of a banner `age` ticks old.
pub(super) fn alpha(age: u32) -> f32 {
    if age < FADE_IN {
        age as f32 / FADE_IN as f32
    } else if age < FADE_IN + HOLD {
        1.0
    } else {
        ((TOTAL.saturating_sub(age)) as f32 / FADE_OUT as f32).clamp(0.0, 1.0)
    }
}

pub(super) fn draw(game: &Game<'_>, layer: &str, batch: &mut HudBatch) {
    if layer != LAYER || !game.native_window() {
        return;
    }
    let state = lock();
    if !state.active {
        return;
    }
    let a = (alpha(state.age) * 255.0) as u32;
    if a < 8 {
        return;
    }
    let (w, h) = (batch.width, batch.height);
    let top = h * 0.2;
    let title_width = game.native_text_width(&state.title);
    let mut draws = vec![
        GuiDraw::Push(vec![PoseOp::Translate(w / 2.0, top), PoseOp::Scale(2.5, 2.5)]),
        GuiDraw::Text {
            text: Text::literal(&state.title),
            x: -(title_width / 2.0) as i32,
            y: 0,
            color: (a << 24) | 0x00FF_D34D,
            shadow: true,
        },
        GuiDraw::Pop,
    ];
    if !state.subtitle.is_empty() {
        let subtitle_width = game.native_text_width(&state.subtitle);
        draws.push(GuiDraw::Push(vec![
            PoseOp::Translate(w / 2.0, top + 28.0),
            PoseOp::Scale(1.25, 1.25),
        ]));
        draws.push(GuiDraw::Text {
            text: Text::literal(&state.subtitle),
            x: -(subtitle_width / 2.0) as i32,
            y: 0,
            color: (a << 24) | 0x00E8_E8E8,
            shadow: true,
        });
        draws.push(GuiDraw::Pop);
    }
    game.native_gui_paint(batch, &draws);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_fall_back_to_words() {
        assert_eq!(pretty("minecraft:dark_forest"), "Dark Forest");
        assert_eq!(pretty("minecraft:the_nether"), "The Nether");
        assert_eq!(pretty("mymod:caves/lush"), "Caves Lush");
        assert_eq!(pretty("overworld"), "Overworld");
        assert_eq!(biome_key("minecraft:plains"), "biome.minecraft.plains");
    }

    #[test]
    fn alpha_fades_in_holds_and_fades_out() {
        assert_eq!(alpha(0), 0.0);
        assert!((alpha(5) - 0.5).abs() < 1e-6);
        assert_eq!(alpha(30), 1.0);
        assert!((alpha(70) - 0.5).abs() < 1e-6);
        assert_eq!(alpha(TOTAL), 0.0);
    }

    struct Run {
        dimension: Option<String>,
        biome: Option<String>,
        candidate: Option<String>,
        stable: u32,
    }

    fn feed(run: &mut Run, dimension: &str, biome: &str, quiet: u32) -> Option<Change> {
        step(
            dimension,
            biome,
            &mut run.dimension,
            &mut run.biome,
            &mut run.candidate,
            &mut run.stable,
            quiet,
        )
    }

    #[test]
    fn a_new_biome_has_to_hold_and_banners_keep_apart() {
        let mut run = Run { dimension: None, biome: None, candidate: None, stable: 0 };
        assert_eq!(
            feed(&mut run, "minecraft:overworld", "minecraft:plains", 0),
            Some(Change::Dimension { first: true })
        );
        assert_eq!(feed(&mut run, "minecraft:overworld", "minecraft:plains", 1), None);
        // A flicker onto another biome and back does not count.
        for _ in 0..5 {
            assert_eq!(feed(&mut run, "minecraft:overworld", "minecraft:forest", 100), None);
        }
        assert_eq!(feed(&mut run, "minecraft:overworld", "minecraft:plains", 100), None);
        // Holding for SETTLE ticks counts, but not while the last banner is too fresh.
        let mut last = None;
        for tick in 0..SETTLE {
            last = feed(&mut run, "minecraft:overworld", "minecraft:forest", 5 + tick);
        }
        assert_eq!(last, None);
        assert_eq!(feed(&mut run, "minecraft:overworld", "minecraft:forest", QUIET), Some(Change::Biome));
        // The dimension change announces at once, whatever the quiet time.
        assert_eq!(
            feed(&mut run, "minecraft:the_nether", "minecraft:nether_wastes", 0),
            Some(Change::Dimension { first: false })
        );
    }
}
