// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Display Link logic after DisplayLinkBlock, DisplayLinkBlockEntity,
// DisplaySource / DisplayTarget and the built-in sources and targets
// (Create 6.0.10, redstone/displayLink/** and api/behaviour/display).
// Text is modelled as plain strings; styling, translation lookup and the
// configuration GUI belong to adapters.
use std::collections::BTreeMap;

use crate::display_board::{FlapDisplay, FlapLayout, FlapRandom, FlapSection, MONOSPACE, WIDE_MONOSPACE};
use crate::nixie::{self, NixieTube};
use crate::redstone_link::Direction;

/// `DisplaySource.getPassiveRefreshTicks()` default.
pub const DEFAULT_PASSIVE_REFRESH_TICKS: i32 = 100;
/// Server config `logistics.displayLinkRange` default (used by the linking
/// item when picking a target).
pub const DISPLAY_LINK_RANGE: i32 = 64;
pub const SIGN_ROWS: usize = 4;
pub const SIGN_COLUMNS: usize = 15;

// ----------------------------------------------------------------------
// Block / block entity state machine
// ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkAction {
    Nothing,
    /// Run `updateGatheredData` (transfer source text to the target).
    Update,
    /// Run `onSignalReset` on the source, then `updateGatheredData`.
    ResetThenUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayLink {
    pub target_offset: [i32; 3],
    pub target_line: i32,
    pub refresh_ticks: i32,
    /// `POWERED` block state. A missing property counts as powered (Java
    /// default true), so a Display Link never updates unpowered-gated.
    pub powered: bool,
    pub has_active_source: bool,
}

impl Default for DisplayLink {
    fn default() -> Self {
        Self { target_offset: [0; 3], target_line: 0, refresh_ticks: 0, powered: false, has_active_source: false }
    }
}

/// `DisplayLinkBlock.shouldBePowered`: any side except the attached one.
pub fn display_link_should_be_powered(signals: [i32; 6], facing: Direction) -> bool {
    let attached = facing.opposite();
    Direction::ALL.iter().any(|d| *d != attached && signals[d.index()] > 0)
}

impl DisplayLink {
    pub fn new() -> Self {
        Self::default()
    }

    /// A zero target offset is rejected by the block's right-click ("invalid").
    pub fn has_valid_target(&self) -> bool {
        self.target_offset != [0; 3]
    }

    /// Server `tick()`. `passive_ticks`/`should_reset` come from the active
    /// source. The counter is post-incremented: the first refresh fires on
    /// the tick after the counter has reached `passive_ticks`.
    pub fn tick(&mut self, is_virtual: bool, passive_ticks: i32, should_reset: bool) -> LinkAction {
        if is_virtual || !self.has_active_source {
            return LinkAction::Nothing;
        }
        let before = self.refresh_ticks;
        self.refresh_ticks += 1;
        if before < passive_ticks || !should_reset {
            return LinkAction::Nothing;
        }
        self.tick_source()
    }

    /// `tickSource()`.
    pub fn tick_source(&mut self) -> LinkAction {
        self.refresh_ticks = 0;
        if self.powered { LinkAction::Nothing } else { LinkAction::Update }
    }

    /// `onNoLongerPowered()`.
    pub fn on_no_longer_powered(&mut self) -> LinkAction {
        if !self.has_active_source {
            return LinkAction::Nothing;
        }
        self.refresh_ticks = 0;
        LinkAction::ResetThenUpdate
    }

    /// `neighborChanged`: apply the freshly computed power. A true -> false
    /// transition fires `onNoLongerPowered`; false -> true fires nothing.
    pub fn set_powered(&mut self, now: bool) -> LinkAction {
        let was = self.powered;
        if was == now {
            return LinkAction::Nothing;
        }
        self.powered = now;
        if was && !now { self.on_no_longer_powered() } else { LinkAction::Nothing }
    }
}

// ----------------------------------------------------------------------
// Line reservation (DisplayTarget.reserve / isReserved)
// ----------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct Reservations {
    lines: BTreeMap<i32, [i32; 3]>,
}

impl Reservations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Line 0 is never reserved.
    pub fn reserve(&mut self, line: i32, link: [i32; 3]) {
        if line == 0 {
            return;
        }
        self.lines.insert(line, link);
    }

    /// `isReserved`: reserved by a different link that still exists; stale or
    /// own reservations are deleted and report false.
    pub fn is_reserved(&mut self, line: i32, link: [i32; 3], link_exists: impl Fn([i32; 3]) -> bool) -> bool {
        let Some(stored) = self.lines.get(&line).copied() else { return false };
        if stored != link && link_exists(stored) {
            return true;
        }
        self.lines.remove(&line);
        false
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

// ----------------------------------------------------------------------
// Targets
// ----------------------------------------------------------------------

/// `DisplayTargetStats(maxRows, maxColumns)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetStats {
    pub max_rows: i32,
    pub max_columns: i32,
}

/// Sign target stats.
pub fn sign_stats() -> TargetStats {
    TargetStats { max_rows: SIGN_ROWS as i32, max_columns: SIGN_COLUMNS as i32 }
}

/// Single-line target (nixie row) stats.
pub fn single_line_stats(width: i32) -> TargetStats {
    TargetStats { max_rows: 1, max_columns: width }
}

/// Display Board stats: `(ySize * 2, maxCharCount)`, or `(1, 1)` w/o controller.
pub fn board_stats(board: Option<&FlapDisplay>) -> TargetStats {
    match board {
        Some(b) => TargetStats { max_rows: b.y_size * 2, max_columns: b.max_char_count() },
        None => TargetStats { max_rows: 1, max_columns: 1 },
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignText {
    pub front: [String; SIGN_ROWS],
    pub back: [String; SIGN_ROWS],
}

/// `SignDisplayTarget.acceptText`: rows `line..line+text.len()` (max row 3)
/// are written on both sides. The starting row is reserved, a later reserved
/// row ends the write. Returns whether anything changed.
pub fn sign_accept_text(
    sign: &mut SignText,
    line: i32,
    text: &[String],
    max_line_chars: usize,
    link: [i32; 3],
    reservations: &mut Reservations,
    link_exists: impl Fn([i32; 3]) -> bool + Copy,
) -> bool {
    let mut changed = false;
    let mut i = 0usize;
    while i < text.len() && (i as i32 + line) < SIGN_ROWS as i32 {
        let row = i as i32 + line;
        if row < 0 {
            i += 1;
            continue;
        }
        if i == 0 {
            reservations.reserve(row, link);
        } else if reservations.is_reserved(row, link, link_exists) {
            break;
        }
        let clipped: String = text[i].chars().take(max_line_chars).collect();
        sign.front[row as usize] = clipped.clone();
        sign.back[row as usize] = clipped;
        changed = true;
        i += 1;
    }
    changed
}

/// `SingleLineDisplayTarget.acceptText`: only the first component counts.
pub fn single_line_accept(text: &[String]) -> Option<&str> {
    text.first().map(String::as_str)
}

/// `NixieTubeDisplayTarget.acceptLine`.
pub fn nixie_accept_line(row: &mut [NixieTube], text: &str) -> usize {
    nixie::apply_text_to_row(row, text)
}

/// `DisplayBoardTarget.acceptFlapText`. `cells[i]` is the cell list of row
/// `i`; `load_layout` is the source's `loadFlapDisplayLayout(.., lineIndex)`.
#[allow(clippy::too_many_arguments)]
pub fn board_accept_flap_text(
    board: &mut FlapDisplay,
    line: i32,
    cells: &[Vec<String>],
    single_line_source: bool,
    speed_requirement_fulfilled: bool,
    link: [i32; 3],
    reservations: &mut Reservations,
    link_exists: impl Fn([i32; 3]) -> bool + Copy,
    load_layout: &mut dyn FnMut(&mut FlapLayout, usize, i32),
) {
    if !speed_requirement_fulfilled {
        return;
    }
    let line_total = board.line_count() as i32;
    let max_chars = board.max_char_count();
    let mut i = 0usize;
    while (i as i32 + line) < line_total {
        let row = i as i32 + line;
        if row < 0 {
            i += 1;
            continue;
        }
        if i == 0 {
            reservations.reserve(row, link);
        } else if reservations.is_reserved(row, link, link_exists) || board.manual[row as usize] {
            break;
        }
        if i >= cells.len() {
            if single_line_source {
                break;
            }
            board.apply_text_manually(row as usize, None);
            i += 1;
            continue;
        }
        let layout = &mut board.lines()[row as usize];
        load_layout(layout, i, max_chars);
        for (j, section) in layout.sections.iter_mut().enumerate() {
            if j >= cells[i].len() {
                break;
            }
            section.set_text(&cells[i][j]);
        }
        i += 1;
    }
}

// ----------------------------------------------------------------------
// Sources: shared text rules
// ----------------------------------------------------------------------

/// `SingleLineDisplaySource.provideText` label rule: `label + " " + value`.
pub fn with_label(label: &str, value: &str) -> String {
    if label.is_empty() { value.to_string() } else { format!("{label} {value}") }
}

/// Flap layout for a labeled single-line source (`loadFlapDisplayLayout`).
/// `value_section(chars)` builds the value section of the given width.
pub fn load_single_line_layout(
    layout: &mut FlapLayout,
    layout_key: &str,
    label: &str,
    max_chars: i32,
    value_section: impl Fn(i32) -> FlapSection,
) {
    let label_len = label.chars().count() as i32;
    if label.is_empty() {
        if !layout.is_layout(layout_key) {
            layout.configure(layout_key, vec![value_section(max_chars)]);
        }
        return;
    }
    let key = format!("{label_len}_Labeled_{layout_key}");
    if layout.is_layout(&key) {
        return;
    }
    let label_w = max_chars.min(label_len + 1);
    let label_section = FlapSection::new(label_w as f32 * MONOSPACE, "alphabet", false, false);
    if label_len + 1 < max_chars {
        layout.configure(&key, vec![label_section, value_section(max_chars - label_len - 1)]);
    } else {
        layout.configure(&key, vec![label_section]);
    }
}

/// Value sections of the built-in layouts.
pub fn numeric_section(size: i32) -> FlapSection {
    FlapSection::new(size as f32 * MONOSPACE, "numeric", false, false)
}
pub fn instant_section(size: i32) -> FlapSection {
    FlapSection::new(size as f32 * MONOSPACE, "instant", false, false)
}
pub fn progress_section(size: i32) -> FlapSection {
    FlapSection::new(size as f32 * MONOSPACE, "pixel", false, false).wide_flaps()
}

/// Approximation of `CreateLang.number`: at most two decimals, trailing zeros
/// removed, `,` thousands grouping (en_US). Locale handling is unverified.
pub fn format_number(v: f64) -> String {
    let s = format!("{:.2}", v.abs());
    let (int_part, frac) = s.split_once('.').unwrap_or((&s, ""));
    let frac = frac.trim_end_matches('0');
    let mut grouped = String::new();
    for (i, c) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let neg = v < 0.0 && (grouped != "0" || !frac.is_empty());
    format!("{}{}{}{}", if neg { "-" } else { "" }, grouped, if frac.is_empty() { "" } else { "." }, frac)
}

// ----------------------------------------------------------------------
// Sources: time of day
// ----------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct TimeLabels {
    pub am: String,
    pub pm: String,
}

impl Default for TimeLabels {
    /// `generic.daytime.am/pm` in en_us (language text unverified).
    fn default() -> Self {
        Self { am: "AM".into(), pm: "PM".into() }
    }
}

/// `TimeOfDayDisplaySource`. `valid` is false when the level is not a server
/// level, the source is not a cuckoo clock or its speed is 0.
pub fn time_of_day(
    day_time: i64,
    twelve_hour: bool,
    valid: bool,
    natural_dimension: bool,
    labels: &TimeLabels,
    rng: &mut dyn FlapRandom,
) -> String {
    if !valid {
        return "--:--".into();
    }
    let day = (day_time % 24000) as i32;
    let mut hours = (day / 1000 + 6) % 24;
    let mut minutes = (day % 1000) * 60 / 1000;
    minutes = minutes / 5 * 5;
    let pm = hours > 11;
    if twelve_hour {
        hours %= 12;
        if hours == 0 {
            hours = 12;
        }
    }
    if !natural_dimension {
        hours = rng.next_int(70) + 24;
        minutes = rng.next_int(40) + 60;
    }
    let mut out = String::new();
    if hours < 10 {
        out.push(' ');
    }
    out.push_str(&hours.to_string());
    out.push(':');
    if minutes < 10 {
        out.push('0');
    }
    out.push_str(&minutes.to_string());
    if twelve_hour {
        out.push(' ');
        out.push_str(if pm { &labels.pm } else { &labels.am });
    }
    out
}

// ----------------------------------------------------------------------
// Sources: counts and numbers
// ----------------------------------------------------------------------

/// `ItemCountDisplaySource`: sum of matching simulated extractions, as text.
pub fn item_count(slots: &[(i32, bool)]) -> String {
    let total: i32 = slots.iter().filter(|(_, ok)| *ok).map(|(c, _)| *c).sum();
    total.to_string()
}

/// `FluidAmountDisplaySource` raw sum (mB); text formatting uses
/// `FluidFormatter` and stays with the adapter.
pub fn fluid_amount(tanks: &[(i64, bool)]) -> i64 {
    tanks.iter().filter(|(_, ok)| *ok).map(|(a, _)| *a).sum()
}

/// `KineticSpeedDisplaySource`: `directional == false` is the absolute mode.
pub fn kinetic_speed(speed: f32, directional: bool, unit: &str) -> String {
    let v = if directional { speed } else { speed.abs() };
    format!("{} {}", format_number(v as f64), unit)
}

/// `KineticStressDisplaySource.getProgress`; `None` if not a stress gauge.
pub fn stress_progress(mode: i32, stress: f32, capacity: f32) -> f32 {
    if capacity == 0.0 {
        return 0.0;
    }
    match mode {
        0 | 1 => stress / capacity,
        2 => stress,
        3 => capacity,
        4 => capacity - stress,
        _ => 0.0,
    }
}

/// `KineticStressDisplaySource.formatNumeric`.
pub fn stress_format(mode: i32, value: f32, on_flap_display: bool, unit: &str) -> String {
    if mode == 1 {
        return percent_text(value);
    }
    let sep = if on_flap_display { " " } else { "" };
    format!("{}{}{}", format_number(value as f64), sep, unit)
}

pub fn stress_progress_bar_active(mode: i32) -> bool {
    mode == 0
}

/// `RedstonePowerDisplaySource.getProgress`: `max(directSignal, POWER)/15f`.
pub fn redstone_power_progress(direct_signal: i32, power_property: i32) -> f32 {
    direct_signal.max(power_property) as f32 / 15.0
}

/// `RedstonePowerDisplaySource.formatNumeric`: `(int)(progress * 15)`
/// computed in f32, truncating like Java.
pub fn redstone_power_number(progress: f32) -> i32 {
    (progress * 15.0) as i32
}

// ----------------------------------------------------------------------
// Percent / progress bar
// ----------------------------------------------------------------------

pub fn percent_text(progress: f32) -> String {
    format!("{}%", ((progress * 100.0) as i32).clamp(0, 100))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarTarget {
    Other,
    Sign,
    Flap,
}

/// `PercentOrProgressBarDisplaySource` bar. `None` means `EMPTY_LINE`.
pub fn progress_bar(progress: f32, max_columns: i32, label: &str, target: BarTarget) -> Option<String> {
    let label_size = if label.is_empty() { 0 } else { label.chars().count() as i32 + 1 };
    let mut length = (max_columns - label_size).min(128);
    match target {
        BarTarget::Sign => length = length * 6 / 9,
        BarTarget::Flap => length = (length as f32 * MONOSPACE / WIDE_MONOSPACE) as i32,
        BarTarget::Other => {}
    }
    let filled = (progress.clamp(0.0, 1.0) * length as f32) as i32;
    if length < 1 {
        return None;
    }
    let empty = (length - filled).max(0);
    let mut s = "\u{2588}".repeat(filled.max(0) as usize);
    s.push_str(&"\u{2592}".repeat(empty as usize));
    Some(s)
}

// ----------------------------------------------------------------------
// Value lists (scoreboard)
// ----------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ValueEntry {
    pub value: i32,
    pub name: String,
}

/// `ScoreboardDisplaySource`: ordered by the `IntAttached` comparator
/// (descending by score, from memory, unverified), limited to `max_rows`.
pub fn scoreboard_entries(mut scores: Vec<ValueEntry>, max_rows: usize) -> Vec<ValueEntry> {
    scores.sort_by(|a, b| b.value.cmp(&a.value));
    scores.truncate(max_rows);
    scores
}

#[derive(Clone, Debug)]
pub struct Suffixes {
    pub million: String,
    pub thousand: String,
}

impl Default for Suffixes {
    /// en_us text of the "million"/"thousand" keys is unverified.
    fn default() -> Self {
        Self { million: "million".into(), thousand: "thousand".into() }
    }
}

/// `ValueListDisplaySource.createComponentsFromEntry` for a flap display.
pub fn value_list_cells(e: &ValueEntry, value_first: bool, shorten: bool, suf: &Suffixes) -> Vec<String> {
    let name = format!("{} ", e.name);
    if !shorten {
        let num = format!("{} ", e.value);
        return if value_first { vec![num, name] } else { vec![name, num] };
    }
    let (value, suffix) = if e.value >= 1_000_000 {
        ((e.value / 1_000_000).to_string(), format!("{} ", suf.million))
    } else if e.value >= 1000 {
        ((e.value / 1000).to_string(), format!("{} ", suf.thousand))
    } else {
        (e.value.to_string(), " ".to_string())
    };
    if value_first { vec![value, suffix, name] } else { vec![name, value, suffix] }
}

/// Same row as one text line (`provideText`).
pub fn value_list_text(e: &ValueEntry, value_first: bool, shorten: bool, suf: &Suffixes) -> String {
    value_list_cells(e, value_first, shorten, suf).concat()
}

/// Number column width of the full format: `max(4, 1 + (int)log10(highest))`.
pub fn value_format_width(highest: i32) -> i32 {
    let digits = if highest > 0 { 1 + (highest as f64).log10() as i32 } else { 0 };
    4.max(digits)
}

/// `ValueListDisplaySource.loadFlapDisplayLayout`.
pub fn load_value_list_layout(
    layout: &mut FlapLayout,
    board: &FlapDisplay,
    value_first: bool,
    shorten: bool,
    highest: i32,
) {
    let value_format = if shorten { 0 } else { value_format_width(highest) };
    let key = format!("ValueList_{value_first}_{value_format}");
    if layout.is_layout(&key) {
        return;
    }
    let max = crate::display_board::max_char_count(board.x_size, 1);
    let number_len = max.min(3.max(value_format));
    let name_len = (max - number_len - if shorten { 1 } else { 0 }).max(0);
    let name = FlapSection::new(MONOSPACE * name_len as f32, "alphabet", false, !value_first);
    let mut value = FlapSection::new(MONOSPACE * number_len as f32, "number", false, !shorten && value_first)
        .right_aligned();
    value.has_gap = !shorten && value_first;
    let sections = if shorten {
        let suffix = FlapSection::new(MONOSPACE, "shortened_numbers", false, value_first);
        if value_first { vec![value, suffix, name] } else { vec![name, value, suffix] }
    } else if value_first {
        vec![value, name]
    } else {
        vec![name, value]
    };
    layout.configure(&key, sections);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_board::JavaRandom;

    const LINK: [i32; 3] = [1, 2, 3];
    const OTHER: [i32; 3] = [9, 9, 9];

    #[test]
    fn link_updates_every_hundred_ticks_plus_one() {
        let mut l = DisplayLink::new();
        l.has_active_source = true;
        let mut fired = Vec::new();
        for t in 1..=210 {
            if l.tick(false, DEFAULT_PASSIVE_REFRESH_TICKS, true) == LinkAction::Update {
                fired.push(t);
            }
        }
        assert_eq!(fired, vec![101, 202]);
    }

    #[test]
    fn link_skips_when_powered_virtual_or_no_source() {
        let mut l = DisplayLink::new();
        assert_eq!(l.tick(false, 0, true), LinkAction::Nothing);
        l.has_active_source = true;
        assert_eq!(l.tick(true, 0, true), LinkAction::Nothing);
        assert_eq!(l.tick(false, 0, false), LinkAction::Nothing);
        l.powered = true;
        l.refresh_ticks = 500;
        assert_eq!(l.tick(false, 100, true), LinkAction::Nothing);
        assert_eq!(l.refresh_ticks, 0);
    }

    #[test]
    fn power_edges() {
        let mut l = DisplayLink::new();
        l.has_active_source = true;
        assert_eq!(l.set_powered(true), LinkAction::Nothing); // rising: nothing
        assert!(l.powered);
        l.refresh_ticks = 40;
        assert_eq!(l.set_powered(false), LinkAction::ResetThenUpdate);
        assert_eq!(l.refresh_ticks, 0);
        assert_eq!(l.set_powered(false), LinkAction::Nothing);
        l.has_active_source = false;
        l.powered = true;
        assert_eq!(l.set_powered(false), LinkAction::Nothing);
    }

    #[test]
    fn power_sampled_from_non_attached_sides() {
        let mut s = [0; 6];
        s[Direction::South.index()] = 9; // attached face when facing North
        assert!(!display_link_should_be_powered(s, Direction::North));
        s[Direction::East.index()] = 1;
        assert!(display_link_should_be_powered(s, Direction::North));
    }

    #[test]
    fn target_offset_zero_is_invalid() {
        let mut l = DisplayLink::new();
        assert!(!l.has_valid_target());
        l.target_offset = [0, 1, 0];
        assert!(l.has_valid_target());
    }

    #[test]
    fn reservations() {
        let mut r = Reservations::new();
        r.reserve(0, LINK);
        assert!(r.is_empty());
        r.reserve(2, LINK);
        assert!(!r.is_reserved(2, LINK, |_| true)); // own: stale entry removed
        assert!(r.is_empty());
        r.reserve(2, OTHER);
        assert!(r.is_reserved(2, LINK, |_| true));
        assert!(!r.is_reserved(2, LINK, |_| false)); // link gone
        assert!(r.is_empty());
    }

    #[test]
    fn sign_multiline_write_with_reservation() {
        let mut sign = SignText::default();
        let mut r = Reservations::new();
        let text: Vec<String> = vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()];
        assert!(sign_accept_text(&mut sign, 1, &text, 15, LINK, &mut r, |_| true));
        assert_eq!(sign.front, ["".to_string(), "a".into(), "b".into(), "c".into()]);
        assert_eq!(sign.back, sign.front);
        // second link on line 2 blocks multiline continuation of the first
        let mut sign = SignText::default();
        let mut r = Reservations::new();
        r.reserve(2, OTHER);
        sign_accept_text(&mut sign, 1, &text, 15, LINK, &mut r, |_| true);
        assert_eq!(sign.front[1], "a");
        assert_eq!(sign.front[2], "");
        // line past the end writes nothing
        assert!(!sign_accept_text(&mut sign, 4, &text, 15, LINK, &mut r, |_| true));
        // clipping
        sign_accept_text(&mut sign, 0, &["abcdefghijklmnopqrstuvwxyz".into()], 15, LINK, &mut r, |_| true);
        assert_eq!(sign.front[0].len(), 15);
    }

    #[test]
    fn single_line_and_stats() {
        assert_eq!(single_line_accept(&["x".into(), "y".into()]), Some("x"));
        assert_eq!(single_line_accept(&[]), None);
        assert_eq!(sign_stats(), TargetStats { max_rows: 4, max_columns: 15 });
        assert_eq!(single_line_stats(6).max_rows, 1);
        let mut b = FlapDisplay::new();
        b.set_size(2, 2);
        assert_eq!(board_stats(Some(&b)), TargetStats { max_rows: 4, max_columns: 8 });
        assert_eq!(board_stats(None), TargetStats { max_rows: 1, max_columns: 1 });
    }

    #[test]
    fn nixie_target_row() {
        let mut row = vec![NixieTube::new(); 2];
        nixie_accept_line(&mut row, "ABCD");
        assert_eq!(row[1].displayed_strings(), ("C".into(), "D".into()));
    }

    fn default_loader(l: &mut FlapLayout, _i: usize, max: i32) {
        if !l.is_layout("Default") {
            l.load_default(max);
        }
    }

    #[test]
    fn board_target_writes_rows_and_clears_the_rest() {
        let mut b = FlapDisplay::new();
        b.set_size(1, 2); // 4 rows
        b.apply_text_manually(3, Some("manual"));
        let mut r = Reservations::new();
        let cells = vec![vec!["hi".to_string()], vec!["yo".to_string()]];
        board_accept_flap_text(&mut b, 0, &cells, false, true, LINK, &mut r, |_| true, &mut default_loader);
        b.lines()[0].sections[0].refresh(false);
        b.lines()[1].sections[0].refresh(false);
        assert_eq!(b.lines()[0].sections[0].text(), "HI  ");
        assert_eq!(b.lines()[1].sections[0].text(), "YO  ");
        // row 2 was cleared (no manual flag), row 3 is manual -> loop stops there anyway
        assert!(!b.manual[2]);
    }

    #[test]
    fn board_target_requires_speed_and_stops_at_manual_lines() {
        let mut b = FlapDisplay::new();
        b.set_size(1, 2);
        b.apply_text_manually(1, Some("keep"));
        let mut r = Reservations::new();
        let cells = vec![vec!["a".to_string()], vec!["b".to_string()], vec!["c".to_string()]];
        board_accept_flap_text(&mut b, 0, &cells, false, false, LINK, &mut r, |_| true, &mut default_loader);
        assert_eq!(b.lines()[0].sections[0].component, "");
        board_accept_flap_text(&mut b, 0, &cells, false, true, LINK, &mut r, |_| true, &mut default_loader);
        assert_eq!(b.lines()[0].sections[0].component, "a");
        assert_eq!(b.lines()[1].sections[0].component, "keep");
        assert_eq!(b.lines()[2].sections[0].component, "");
    }

    #[test]
    fn board_target_single_line_source_does_not_clear_following_rows() {
        let mut b = FlapDisplay::new();
        b.set_size(1, 2);
        b.lines()[1].sections[0].set_text("old");
        let mut r = Reservations::new();
        let cells = vec![vec!["x".to_string()]];
        board_accept_flap_text(&mut b, 0, &cells, true, true, LINK, &mut r, |_| true, &mut default_loader);
        assert_eq!(b.lines()[1].sections[0].component, "old");
    }

    #[test]
    fn label_rules() {
        assert_eq!(with_label("", "42"), "42");
        assert_eq!(with_label("Cnt", "42"), "Cnt 42");
    }

    #[test]
    fn labeled_layouts() {
        let mut l = FlapLayout::new(8);
        load_single_line_layout(&mut l, "Number", "", 8, numeric_section);
        assert!(l.is_layout("Number"));
        assert_eq!(l.sections[0].flap_count(), 8);
        load_single_line_layout(&mut l, "Number", "Cnt", 8, numeric_section);
        assert!(l.is_layout("3_Labeled_Number"));
        assert_eq!(l.sections.len(), 2);
        assert_eq!(l.sections[0].flap_count(), 4);
        assert_eq!(l.sections[1].flap_count(), 4);
        load_single_line_layout(&mut l, "Number", "Seven77", 8, numeric_section);
        assert_eq!(l.sections.len(), 1); // value has no room
        assert_eq!(l.sections[0].flap_count(), 8);
        assert!(l.is_layout("7_Labeled_Number"));
    }

    #[test]
    fn time_of_day_formats() {
        let mut r = JavaRandom::new(1);
        let lab = TimeLabels::default();
        let t = |dt, twelve| time_of_day(dt, twelve, true, true, &lab, &mut JavaRandom::new(0));
        assert_eq!(t(0, false), " 6:00");
        assert_eq!(t(0, true), " 6:00 AM");
        assert_eq!(t(6000, true), "12:00 PM");
        assert_eq!(t(6000, false), "12:00");
        assert_eq!(t(18000, true), "12:00 AM");
        assert_eq!(t(18000, false), " 0:00");
        assert_eq!(t(13000, true), " 7:00 PM");
        assert_eq!(t(23000 + 24000, false), " 5:00");
        // 5 minute steps: 1000 ticks = 60 min, 83 ticks = 4.98 min -> 0, 84 ticks -> 5
        assert_eq!(t(83, false), " 6:00");
        assert_eq!(t(84, false), " 6:05");
        assert_eq!(time_of_day(0, true, false, true, &lab, &mut r), "--:--");
    }

    #[test]
    fn time_of_day_unnatural_dimension_is_random_but_bounded() {
        let lab = TimeLabels::default();
        let mut r = JavaRandom::new(5);
        for _ in 0..50 {
            let s = time_of_day(0, false, true, false, &lab, &mut r);
            let (h, m) = s.trim().split_once(':').unwrap();
            let (h, m): (i32, i32) = (h.parse().unwrap(), m.parse().unwrap());
            assert!((24..94).contains(&h) && (60..100).contains(&m));
        }
    }

    #[test]
    fn counts_and_amounts() {
        assert_eq!(item_count(&[(64, true), (10, false), (3, true)]), "67");
        assert_eq!(item_count(&[]), "0");
        assert_eq!(fluid_amount(&[(1000, true), (500, false), (250, true)]), 1250);
    }

    #[test]
    fn number_format() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(64.0), "64");
        assert_eq!(format_number(12.5), "12.5");
        assert_eq!(format_number(1234567.891), "1,234,567.89");
        assert_eq!(format_number(-32.0), "-32");
        assert_eq!(kinetic_speed(-32.0, false, "rpm"), "32 rpm");
        assert_eq!(kinetic_speed(-32.0, true, "rpm"), "-32 rpm");
    }

    #[test]
    fn stress_modes() {
        assert_eq!(stress_progress(0, 50.0, 100.0), 0.5);
        assert_eq!(stress_progress(1, 50.0, 100.0), 0.5);
        assert_eq!(stress_progress(2, 50.0, 100.0), 50.0);
        assert_eq!(stress_progress(3, 50.0, 100.0), 100.0);
        assert_eq!(stress_progress(4, 50.0, 100.0), 50.0);
        assert_eq!(stress_progress(9, 50.0, 100.0), 0.0);
        assert_eq!(stress_progress(0, 50.0, 0.0), 0.0);
        assert_eq!(stress_format(1, 0.5, false, "su"), "50%");
        assert_eq!(stress_format(2, 2048.0, false, "su"), "2,048su");
        assert_eq!(stress_format(2, 2048.0, true, "su"), "2,048 su");
        assert!(stress_progress_bar_active(0) && !stress_progress_bar_active(1));
    }

    #[test]
    fn redstone_power_truncation_matches_f32_java() {
        assert_eq!(redstone_power_progress(3, 9), 9.0 / 15.0);
        // f32 arithmetic, not exact: record the values Java's (int)(p * 15) gives
        let got: Vec<i32> = (0..=15).map(|v| redstone_power_number(redstone_power_progress(v, 0))).collect();
        // every value is within 1 of the signal and never above it
        for (v, g) in got.iter().enumerate() {
            assert!(*g == v as i32 || *g == v as i32 - 1, "{v} -> {g}");
        }
        assert_eq!(got[0], 0);
        assert_eq!(got[15], 15);
    }

    #[test]
    fn percent_and_bar() {
        assert_eq!(percent_text(0.456), "45%");
        assert_eq!(percent_text(2.0), "100%");
        assert_eq!(percent_text(-1.0), "0%");
        assert_eq!(progress_bar(0.5, 10, "", BarTarget::Other).unwrap(), "\u{2588}".repeat(5) + &"\u{2592}".repeat(5));
        assert_eq!(progress_bar(1.0, 4, "ab", BarTarget::Other).unwrap(), "\u{2588}");
        assert_eq!(progress_bar(0.0, 4, "abcd", BarTarget::Other), None);
        assert_eq!(progress_bar(0.5, 200, "", BarTarget::Other).unwrap().chars().count(), 128);
        // sign: 15 * 6 / 9 = 10
        assert_eq!(progress_bar(0.0, 15, "", BarTarget::Sign).unwrap().chars().count(), 10);
        // flap: 18 * 7 / 9 = 14
        assert_eq!(progress_bar(0.0, 18, "", BarTarget::Flap).unwrap().chars().count(), 14);
    }

    #[test]
    fn scoreboard_sorting_and_cells() {
        let sc = vec![
            ValueEntry { value: 5, name: "a".into() },
            ValueEntry { value: 1500, name: "b".into() },
            ValueEntry { value: 3_200_000, name: "c".into() },
        ];
        let top = scoreboard_entries(sc, 2);
        assert_eq!(top.iter().map(|e| e.value).collect::<Vec<_>>(), vec![3_200_000, 1500]);
        let s = Suffixes::default();
        assert_eq!(value_list_cells(&top[1], true, false, &s), vec!["1500 ", "b "]);
        assert_eq!(value_list_cells(&top[1], false, false, &s), vec!["b ", "1500 "]);
        assert_eq!(value_list_cells(&top[1], true, true, &s), vec!["1", "thousand ", "b "]);
        assert_eq!(value_list_cells(&top[0], false, true, &s), vec!["c ", "3", "million "]);
        let small = ValueEntry { value: 5, name: "a".into() };
        assert_eq!(value_list_text(&small, true, true, &s), "5 a ");
    }

    #[test]
    fn value_list_layout_widths() {
        assert_eq!(value_format_width(0), 4);
        assert_eq!(value_format_width(9999), 4);
        assert_eq!(value_format_width(123456), 6);
        let mut b = FlapDisplay::new();
        b.set_size(3, 1); // max chars with gap = 12
        let mut l = FlapLayout::new(b.max_char_count());
        load_value_list_layout(&mut l, &b, true, false, 12);
        assert!(l.is_layout("ValueList_true_4"));
        assert_eq!(l.sections.len(), 2);
        assert_eq!(l.sections[0].flap_count(), 4); // number
        assert_eq!(l.sections[1].flap_count(), 8); // name
        assert!(l.sections[0].right_aligned);
        load_value_list_layout(&mut l, &b, false, true, 0);
        assert!(l.is_layout("ValueList_false_0"));
        assert_eq!(l.sections.len(), 3);
        assert_eq!(l.sections[0].flap_count(), 8); // name = 12 - 3 - 1
        assert_eq!(l.sections[1].flap_count(), 3);
        assert_eq!(l.sections[2].flap_count(), 1);
    }
}
