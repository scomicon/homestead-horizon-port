// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//
// Client sound mods, shared part (stage 26): Extreme Sound Muffler, AmbientSounds and Presence
// Footsteps rebuilt from their public descriptions (no code or assets of the originals).
//
// * The sound hook (`native_sound_play`) is a free function: it gets no `Game`, so everything
//   it needs lives in one `State` behind a mutex that the client tick fills and refreshes.
// * Everything that needs the game (the player, the biome, the clock, playing a sound) runs in
//   `tick`, injected into `native_client_tick_post` (client window only, never a server thread).
// * The lock is never held across a call into `Game`: playing a sound can come straight back
//   into the sound hook, which locks the same state.
// * Configuration is three plain text files in the game's `config` folder, created with
//   commented examples on first start and re-read when their modification time changes:
//   `homestead-muffler.txt`, `homestead-footsteps.txt`, `homestead-ambient.txt`.
// * `homestead-sound-status.txt` (same folder) is rewritten twice a second with the counters,
//   for tests and for players who want to see what the mods do.

use super::Game;
use crate::sound_engine::SoundPlayback;
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock},
    time::SystemTime,
};

pub(crate) const MUFFLER_FILE: &str = "homestead-muffler.txt";
pub(crate) const FOOTSTEPS_FILE: &str = "homestead-footsteps.txt";
pub(crate) const AMBIENT_FILE: &str = "homestead-ambient.txt";
pub(crate) const STATUS_FILE: &str = "homestead-sound-status.txt";
const FILES: [&str; 3] = [MUFFLER_FILE, FOOTSTEPS_FILE, AMBIENT_FILE];
const RELOAD_EVERY: u64 = 40;
const STATUS_EVERY: u64 = 10;
pub(crate) const MAX_PENDING: usize = 16;

const MUFFLER_DEFAULT: &str = r#"# Homestead sound muffler (the idea of Extreme Sound Muffler, rebuilt).
# One rule per line:   <sound id or pattern>  <percent>
#   percent 0 mutes the sound, 50 halves its volume, 150 makes it louder.
#   '*' matches any text:  minecraft:block.*.step   or   *.ambient
#   The first matching line wins. The file is re-read when it changes.
# Special lines:
#   @enabled true|false   switches the whole list (F9 toggles it in game)
#   @quiet <percent>      volume of every sound that no line matches while quiet mode is on (F10)
@enabled true
@quiet 30
# minecraft:entity.generic.explode 30
# minecraft:block.*.step 60
# minecraft:entity.experience_orb.pickup 0
"#;

const FOOTSTEPS_DEFAULT: &str = r#"# Homestead footsteps (the idea of Presence Footsteps, rebuilt).
# One rule per line:   <block id or pattern>  <add|replace>  <sound event>  [volume]  [pitch]
#   When something takes a step on a block that matches (the block under it), the vanilla step
#   sound is replaced or a second sound is added on top. The sound events come from YOUR resource
#   pack (this mod ships none); volume and pitch multiply the original step's.
#   '*' matches any text:  minecraft:*_planks
#   The first matching line wins. The file is re-read when it changes.
#   A sound event that is itself a vanilla *.step event is refused (it would loop).
# minecraft:*_planks  add  mypack:footsteps.wood_hollow  0.6  1.0
# minecraft:gravel    replace  mypack:footsteps.gravel_crunch  1.0  1.0
"#;

const AMBIENT_DEFAULT: &str = r#"# Homestead ambience (the idea of AmbientSounds, rebuilt).
# One rule per line:
#   <name>  <conditions>  <sound event>  <interval ticks>  <chance 0-1>  <volume>  [pitch]
# Conditions are joined with '+' (all must hold): day night rain thunder clear cave surface
# underwater overworld nether end biome:<pattern>   ('-' = always). 'cave' means below y 50.
# Every <interval> ticks (20 = one second) the rule rolls <chance> and, when it hits, plays the
# sound near the player. The sound events come from YOUR resource pack (the original mod's sounds
# are not redistributable); homestead_sound:tone.low|mid|high are three placeholder tones.
# Switched off until you set @enabled true.
@enabled false
# crickets  night+clear+surface  mypack:ambient.crickets  400  0.5  0.6
# drip      cave                 mypack:ambient.drip      300  0.4  0.5
"#;

// ---------------------------------------------------------------- rules and state

pub(crate) struct MufflerRule {
    pub pattern: String,
    pub factor: f32,
}

pub(crate) struct StepRule {
    pub block: String,
    pub replace: bool,
    pub sound: String,
    pub volume: f32,
    pub pitch: f32,
}

pub(crate) struct AmbientRule {
    pub name: String,
    pub conditions: Vec<String>,
    pub sound: String,
    pub interval: u32,
    pub chance: f32,
    pub volume: f32,
    pub pitch: f32,
    pub next: u64,
}

pub(crate) struct Pending {
    pub sound: String,
    pub at: [f64; 3],
    pub volume: f32,
    pub pitch: f32,
    pub ambient: bool,
}

#[derive(Default, Clone)]
pub(crate) struct Snapshot {
    pub valid: bool,
    pub blocks: Vec<([i32; 3], String)>,
}

impl Snapshot {
    /// The block recorded at a position, when it is not air.
    pub(crate) fn lookup(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        self.blocks
            .iter()
            .find(|(pos, _)| *pos == [x, y, z])
            .map(|(_, block)| block.as_str())
            .filter(|block| *block != "minecraft:air" && !block.ends_with("_air"))
    }
}

#[derive(Default)]
pub(crate) struct Counters {
    pub muffler_scaled: u64,
    pub muffler_cancelled: u64,
    pub footsteps_replaced: u64,
    pub footsteps_added: u64,
    pub footsteps_played: u64,
    pub ambient_queued: u64,
    pub ambient_played: u64,
    pub play_failed: u64,
    pub config_errors: u64,
    pub reloads: u64,
}

#[derive(Default)]
pub(crate) struct State {
    pub inited: bool,
    pub ticks: u64,
    /// Sounds the library handed to the callback and the identifier of the last one (diagnostics).
    pub hook_seen: u64,
    pub last_sound: String,
    pub dir: Option<PathBuf>,
    pub mtimes: [Option<SystemTime>; 3],
    pub muffler_enabled: bool,
    pub quiet: bool,
    pub quiet_percent: f32,
    pub muffler_rules: Vec<MufflerRule>,
    pub footsteps: Vec<StepRule>,
    pub ambient_enabled: bool,
    pub ambient_rules: Vec<AmbientRule>,
    pub snapshot: Snapshot,
    pub under: String,
    pub pending: Vec<Pending>,
    pub key_down: [bool; 2],
    pub rng: u64,
    pub counters: Counters,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

/// The shared state. Never keep the guard while calling into `Game`.
pub(crate) fn state() -> MutexGuard<'static, State> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------- text helpers

/// `*` matches any run of characters (also none); everything else must match exactly.
pub(crate) fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let last = parts.len() - 1;
    let mut rest = text;
    for (index, part) in parts.iter().enumerate() {
        if index == 0 {
            match rest.strip_prefix(part) {
                Some(after) => rest = after,
                None => return false,
            }
        } else if index == last {
            return rest.ends_with(part);
        } else if !part.is_empty() {
            match rest.find(part) {
                Some(at) => rest = &rest[at + part.len()..],
                None => return false,
            }
        }
    }
    true
}

fn content(raw: &str) -> &str {
    raw.trim_start_matches('\u{feff}').split('#').next().unwrap_or("").trim()
}

fn number(word: &str) -> Option<f32> {
    word.parse::<f32>().ok().filter(|value| value.is_finite() && *value >= 0.0)
}

fn switch(word: &str) -> Option<bool> {
    match word {
        "true" | "on" | "yes" => Some(true),
        "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

fn valid_event(sound: &str) -> bool {
    !sound.is_empty()
        && sound.len() <= 128
        && sound.contains(':')
        && sound.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, ':' | '.' | '_' | '-' | '/'))
}

// ---------------------------------------------------------------- parsing

fn apply_muffler(st: &mut State, text: &str) {
    let mut rules = Vec::new();
    let mut quiet = 30.0f32;
    let mut enabled = true;
    let mut errors = 0u64;
    for raw in text.lines() {
        let line = content(raw);
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["@enabled", value] => match switch(value) {
                Some(on) => enabled = on,
                None => errors += 1,
            },
            ["@quiet", value] => match number(value) {
                Some(percent) => quiet = percent.min(400.0),
                None => errors += 1,
            },
            [pattern, percent] if !pattern.starts_with('@') => match number(percent) {
                Some(percent) => rules.push(MufflerRule { pattern: (*pattern).to_owned(), factor: percent.min(400.0) / 100.0 }),
                None => errors += 1,
            },
            _ => errors += 1,
        }
    }
    st.muffler_rules = rules;
    st.muffler_enabled = enabled;
    st.quiet_percent = quiet;
    st.counters.config_errors += errors;
}

fn apply_footsteps(st: &mut State, text: &str) {
    let mut rules = Vec::new();
    let mut errors = 0u64;
    for raw in text.lines() {
        let line = content(raw);
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.len() < 3 || words.len() > 5 {
            errors += 1;
            continue;
        }
        let replace = match words[1] {
            "replace" => true,
            "add" => false,
            _ => {
                errors += 1;
                continue;
            }
        };
        let sound = words[2];
        // A vanilla step event as the answer would be caught by the step hook again.
        if !valid_event(sound) || (sound.starts_with("minecraft:") && sound.ends_with(".step")) {
            errors += 1;
            continue;
        }
        let volume = words.get(3).map_or(Some(1.0), |w| number(w));
        let pitch = words.get(4).map_or(Some(1.0), |w| number(w));
        match (volume, pitch) {
            (Some(volume), Some(pitch)) if pitch > 0.0 => rules.push(StepRule {
                block: words[0].to_owned(),
                replace,
                sound: sound.to_owned(),
                volume: volume.min(4.0),
                pitch: pitch.clamp(0.5, 2.0),
            }),
            _ => errors += 1,
        }
    }
    st.footsteps = rules;
    st.counters.config_errors += errors;
}

fn apply_ambient(st: &mut State, text: &str) {
    let mut rules = Vec::new();
    let mut enabled = false;
    let mut errors = 0u64;
    let now = st.ticks;
    for raw in text.lines() {
        let line = content(raw);
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words[0] == "@enabled" {
            match words.get(1).and_then(|w| switch(w)) {
                Some(on) if words.len() == 2 => enabled = on,
                _ => errors += 1,
            }
            continue;
        }
        if words.len() < 6 || words.len() > 7 {
            errors += 1;
            continue;
        }
        let conditions: Vec<String> = words[1].split('+').map(str::to_owned).collect();
        let interval = words[3].parse::<u32>().ok().filter(|v| (20..=24000).contains(v));
        let chance = number(words[4]).filter(|v| *v <= 1.0);
        let volume = number(words[5]).map(|v| v.min(4.0));
        let pitch = words.get(6).map_or(Some(1.0), |w| number(w)).filter(|v| *v > 0.0);
        let known = conditions.iter().all(|c| super::homestead_sound_ambient::known_condition(c));
        match (interval, chance, volume, pitch) {
            (Some(interval), Some(chance), Some(volume), Some(pitch)) if known && valid_event(words[2]) => {
                rules.push(AmbientRule {
                    name: words[0].to_owned(),
                    conditions,
                    sound: words[2].to_owned(),
                    interval,
                    chance,
                    volume,
                    pitch: pitch.clamp(0.5, 2.0),
                    next: now + u64::from(interval) / 2,
                })
            }
            _ => errors += 1,
        }
    }
    st.ambient_rules = rules;
    st.ambient_enabled = enabled;
    st.counters.config_errors += errors;
}

/// Creates a missing file with its commented defaults and re-parses a file whose modification
/// time changed (every file on `force`).
fn reload_if_changed(st: &mut State, force: bool) {
    let Some(dir) = st.dir.clone() else { return };
    let defaults = [MUFFLER_DEFAULT, FOOTSTEPS_DEFAULT, AMBIENT_DEFAULT];
    for (index, name) in FILES.iter().enumerate() {
        let path = dir.join(name);
        if !path.exists() {
            let _ = fs::write(&path, defaults[index]);
        }
        let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        if !force && st.mtimes[index].is_some() && modified == st.mtimes[index] {
            continue;
        }
        st.mtimes[index] = modified;
        let Ok(text) = fs::read_to_string(&path) else {
            st.counters.config_errors += 1;
            continue;
        };
        match index {
            0 => apply_muffler(st, &text),
            1 => apply_footsteps(st, &text),
            _ => apply_ambient(st, &text),
        }
        st.counters.reloads += 1;
    }
}

// ---------------------------------------------------------------- status

fn status_text(st: &State) -> String {
    let c = &st.counters;
    let mut text = format!(
        "ticks={}\nmuffler_enabled={}\nquiet={}\nmuffler_rules={}\nfootsteps_rules={}\nambient_enabled={}\nambient_rules={}\n\
         muffler_scaled={}\nmuffler_cancelled={}\nfootsteps_replaced={}\nfootsteps_added={}\nfootsteps_played={}\n\
         ambient_queued={}\nambient_played={}\nplay_failed={}\nconfig_errors={}\nreloads={}\nunder={}\n",
        st.ticks,
        st.muffler_enabled,
        st.quiet,
        st.muffler_rules.len(),
        st.footsteps.len(),
        st.ambient_enabled,
        st.ambient_rules.len(),
        c.muffler_scaled,
        c.muffler_cancelled,
        c.footsteps_replaced,
        c.footsteps_added,
        c.footsteps_played,
        c.ambient_queued,
        c.ambient_played,
        c.play_failed,
        c.config_errors,
        c.reloads,
        st.under,
    );
    text.push_str(&format!("hook_seen={}\nlast_sound={}\n", st.hook_seen, st.last_sound));
    text
}

fn write_status(dir: &PathBuf, text: &str) {
    let target = dir.join(STATUS_FILE);
    let temp = dir.join(format!("{STATUS_FILE}.tmp"));
    if fs::write(&temp, text).is_ok() {
        let _ = fs::rename(&temp, &target);
    }
}

// ---------------------------------------------------------------- tick

/// `native_client_tick_post` (client window only): configuration, keys, the footstep snapshot,
/// ambience rolls, and finally the sounds queued by the hook and by ambience.
/// The callback the client library calls for every sound: muffler first, so a muffled step is not
/// also answered by a footstep rule.
fn dispatch(sound: &SoundPlayback<'_>, volume: &mut f32, pitch: &mut f32, cancel: &mut bool) {
    {
        let mut st = state();
        st.hook_seen += 1;
        if sound.starting {
            st.last_sound = sound.identifier.to_string();
        }
    }
    super::homestead_sound_muffler::on_play(sound, volume, pitch, cancel);
    super::homestead_sound_footsteps::on_play(sound, volume, pitch, cancel);
}

pub(crate) fn tick(game: &mut Game<'_>) {
    if !game.native_window() || !game.native_on("homestead") {
        return;
    }
    crate::sound_playback::homestead_sound_bridge::install(dispatch);
    {
        let mut st = state();
        st.ticks += 1;
        if !st.inited {
            st.inited = true;
            st.rng = 0x2545_F491_4F6C_DD1D;
            st.muffler_enabled = true;
            st.quiet_percent = 30.0;
            st.dir = Game::native_config_dir().ok();
            reload_if_changed(&mut st, true);
        } else if st.ticks % RELOAD_EVERY == 0 {
            reload_if_changed(&mut st, false);
        }
    }
    if game.native_player().is_some() {
        super::homestead_sound_muffler::keys(game);
        super::homestead_sound_footsteps::refresh(game);
        super::homestead_sound_ambient::tick(game);
        flush(game);
    }
    let (dir, text) = {
        let st = state();
        if st.ticks % STATUS_EVERY != 0 {
            return;
        }
        (st.dir.clone(), status_text(&st))
    };
    if let Some(dir) = dir {
        write_status(&dir, &text);
    }
}

/// Plays what the hook and the ambience rules queued. No lock is held while the game plays a
/// sound, because that comes back through the sound hook.
fn flush(game: &mut Game<'_>) {
    let batch: Vec<Pending> = std::mem::take(&mut state().pending);
    for pending in batch {
        let volume = pending.volume.clamp(0.0, 4.0);
        let pitch = pending.pitch.clamp(0.5, 2.0);
        let played = game.native_play_local_sound(&pending.sound, pending.at, volume, pitch, 0);
        let mut st = state();
        if !played {
            st.counters.play_failed += 1;
        } else if pending.ambient {
            st.counters.ambient_played += 1;
        } else {
            st.counters.footsteps_played += 1;
        }
    }
}
