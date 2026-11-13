// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Display Board (flap display) model after FlapDisplayBlockEntity,
// FlapDisplayLayout and FlapDisplaySection (Create 6.0.10,
// content/trains/display). Rendering, sound playback and rich text parsing
// belong to adapters; this module holds the text/flap state machines and the
// size rules.

/// Width of one monospace flap, in section-width units (`MONOSPACE`).
pub const MONOSPACE: f32 = 7.0;
/// `WIDE_MONOSPACE`.
pub const WIDE_MONOSPACE: f32 = 9.0;
pub const MAX_BOARD_WIDTH: i32 = 32;
/// Instant mode threshold: `|speed| > 128`.
pub const INSTANT_SPEED: f32 = 128.0;
pub const LAZY_TICK_RATE: i32 = 10;

/// Source of `RandomSource.nextInt`. Production code passes the level
/// random; tests use `JavaRandom` for reproducible sequences.
pub trait FlapRandom {
    fn next_int(&mut self, bound: i32) -> i32;
}

/// `java.util.Random` (48-bit LCG), for deterministic flap tests.
#[derive(Clone, Debug)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULT: u64 = 0x5DEECE66D;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self { seed: (seed as u64 ^ Self::MULT) & Self::MASK }
    }

    pub fn next_bits(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(Self::MULT).wrapping_add(0xB)) & Self::MASK;
        (self.seed >> (48 - bits)) as i64 as i32
    }
}

impl FlapRandom for JavaRandom {
    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.next_bits(31);
        let m = bound - 1;
        if bound & m == 0 {
            return ((bound as i64 * r as i64) >> 31) as i32;
        }
        let mut u = r;
        loop {
            r = u % bound;
            if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                return r;
            }
            u = self.next_bits(31);
        }
    }
}

/// Split a `flap_display.cycles.<key>` language string on `;`.
pub fn split_cycle(raw: &str) -> Vec<String> {
    raw.split(';').map(str::to_string).collect()
}

/// One section of a line (`FlapDisplaySection`).
#[derive(Clone, Debug, PartialEq)]
pub struct FlapSection {
    pub size: f32,
    pub cycle: String,
    pub single_flap: bool,
    pub has_gap: bool,
    pub right_aligned: bool,
    pub wide_flaps: bool,
    pub send_transition: bool,
    pub component: String,
    cycling_options: Option<Vec<String>>,
    spinning: Vec<bool>,
    spinning_ticks: i32,
    text: Vec<String>,
}

impl FlapSection {
    pub fn new(width: f32, cycle: &str, single_flap: bool, has_gap: bool) -> Self {
        let count = if single_flap { 1 } else { ((width / MONOSPACE) as i32).max(0) as usize };
        Self {
            size: width,
            cycle: cycle.to_string(),
            single_flap,
            has_gap,
            right_aligned: false,
            wide_flaps: false,
            send_transition: false,
            component: String::new(),
            cycling_options: None,
            spinning: vec![false; count],
            spinning_ticks: 0,
            text: vec![" ".to_string(); count],
        }
    }

    pub fn right_aligned(mut self) -> Self {
        self.right_aligned = true;
        self
    }

    pub fn wide_flaps(mut self) -> Self {
        self.wide_flaps = true;
        self
    }

    /// Install the character cycle (`getFlapCycle(cycle)`) after a layout load.
    pub fn set_cycling_options(&mut self, options: Vec<String>) {
        self.cycling_options = Some(options);
    }

    pub fn flap_count(&self) -> usize {
        self.spinning.len()
    }

    pub fn text(&self) -> String {
        self.text.concat()
    }

    pub fn flaps(&self) -> &[String] {
        &self.text
    }

    pub fn is_spinning(&self, flap: usize) -> bool {
        self.spinning.get(flap).copied().unwrap_or(false)
    }

    pub fn spinning_ticks(&self) -> i32 {
        self.spinning_ticks
    }

    /// `setText`: store and request a transition on the next refresh.
    pub fn set_text(&mut self, component: &str) {
        self.component = component.to_string();
        self.send_transition = true;
    }

    /// `refresh(transition)`.
    pub fn refresh(&mut self, transition: bool) {
        let mut new: Vec<String>;
        if self.single_flap {
            new = vec![self.component.clone()];
        } else {
            let mut s = self.component.clone();
            if self.right_aligned {
                s = s.trim().to_string();
            }
            let n = self.spinning.len();
            let mut chars: Vec<char> = s.to_uppercase().chars().take(n).collect();
            let pad = n - chars.len();
            if self.right_aligned {
                let mut padded = vec![' '; pad];
                padded.append(&mut chars);
                chars = padded;
            } else {
                chars.extend(std::iter::repeat_n(' ', pad));
            }
            new = chars.into_iter().map(|c| c.to_string()).collect();
        }
        new.truncate(self.text.len());
        for (i, ch) in new.into_iter().enumerate() {
            if transition && self.text[i] != ch {
                self.spinning[i] = true;
            }
            self.text[i] = ch;
        }
        self.spinning_ticks = 0;
        self.send_transition = false;
    }

    /// `tick(instant, random)`: advance the spin animation and return how
    /// many flaps are still spinning (drives the clacking sound volume).
    pub fn tick(&mut self, instant: bool, rng: &mut dyn FlapRandom) -> usize {
        let Some(options) = &self.cycling_options else { return 0 };
        let max = 4.max((options.len() as f32 * 1.75) as i32);
        if self.spinning_ticks > max {
            return 0;
        }
        self.spinning_ticks += 1;
        if self.spinning_ticks == 1 {
            return 0;
        }
        let len = self.spinning.len();
        let mut count = 0;
        for i in 0..len {
            let chance = (8 - self.spinning_ticks).clamp(1, 10);
            let mut cont = !instant && rng.next_int(chance * max / 4) != 0;
            cont &= max > 5 || self.spinning_ticks < 2;
            self.spinning[i] &= cont;
            if i > 0 && rng.next_int(3) > 0 {
                self.spinning[i - 1] &= cont;
            }
            if i + 1 < len && rng.next_int(3) > 0 {
                self.spinning[i + 1] &= cont;
            }
            if self.spinning_ticks > max {
                self.spinning[i] = false;
            }
            if self.spinning[i] {
                count += 1;
            }
        }
        count
    }
}

/// `FlapDisplayLayout`: a keyed list of sections for one line.
#[derive(Clone, Debug, PartialEq)]
pub struct FlapLayout {
    pub key: String,
    pub sections: Vec<FlapSection>,
}

impl FlapLayout {
    pub fn new(max_char_count: i32) -> Self {
        let mut l = Self { key: String::new(), sections: Vec::new() };
        l.load_default(max_char_count);
        l
    }

    pub fn load_default(&mut self, max_char_count: i32) {
        self.configure(
            "Default",
            vec![FlapSection::new(max_char_count.max(0) as f32 * MONOSPACE, "alphabet", false, false)],
        );
    }

    pub fn is_layout(&self, key: &str) -> bool {
        self.key == key
    }

    pub fn configure(&mut self, key: &str, sections: Vec<FlapSection>) {
        self.key = key.to_string();
        self.sections = sections;
    }
}

/// Dye colour index (vanilla order) of a board line, `None` is white.
pub type Dye = Option<u8>;

/// Board-level state of a controller (`FlapDisplayBlockEntity`).
#[derive(Clone, Debug)]
pub struct FlapDisplay {
    pub x_size: i32,
    pub y_size: i32,
    pub colour: Vec<Dye>,
    pub glowing: Vec<bool>,
    pub manual: Vec<bool>,
    lines: Option<Vec<FlapLayout>>,
}

impl Default for FlapDisplay {
    fn default() -> Self {
        Self { x_size: 1, y_size: 1, colour: vec![None; 2], glowing: vec![false; 2], manual: vec![false; 2], lines: None }
    }
}

/// `getMaxCharCount(gaps)`.
pub fn max_char_count(x_size: i32, gaps: i32) -> i32 {
    ((x_size as f32 * 16.0 - 2.0 - 4.0 * gaps as f32) / 3.5) as i32
}

/// Size of a controller board. `same_to_right(i)` tells whether the block `i`
/// steps along the row (away from the controller) is the same board state;
/// `below_has_down(i)` whether the block `i` below (`i` from 0) continues the
/// stack (`DOWN` set). Both stop at the first false.
pub fn controller_size(same_to_right: impl Fn(i32) -> bool, below_has_down: impl Fn(i32) -> bool) -> (i32, i32) {
    let mut x = 1;
    for off in 1..MAX_BOARD_WIDTH {
        if !same_to_right(off) {
            break;
        }
        x += 1;
    }
    let mut y = 1;
    for off in 0..32 {
        if !below_has_down(off) {
            break;
        }
        y += 1;
    }
    (x, y)
}

/// `shouldBeController`: top of its column, nothing of the same state on the
/// left.
pub fn should_be_controller(up_set: bool, left_is_same_state: bool) -> bool {
    !up_set && !left_is_same_state
}

impl FlapDisplay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line_count(&self) -> usize {
        (self.y_size * 2) as usize
    }

    pub fn max_char_count(&self) -> i32 {
        max_char_count(self.x_size, 0)
    }

    /// `updateControllerStatus` resize: arrays grow/shrink, manual lines reset
    /// and layouts are rebuilt lazily. Returns true if anything changed.
    pub fn set_size(&mut self, x: i32, y: i32) -> bool {
        if (x, y) == (self.x_size, self.y_size) {
            return false;
        }
        self.x_size = x;
        self.y_size = y;
        let n = self.line_count();
        self.colour.resize(n, None);
        self.glowing.resize(n, false);
        self.manual = vec![false; n];
        self.lines = None;
        true
    }

    /// `getLines` (lazy `initDefaultSections`).
    pub fn lines(&mut self) -> &mut Vec<FlapLayout> {
        let n = self.line_count();
        let chars = self.max_char_count();
        self.lines.get_or_insert_with(|| (0..n).map(|_| FlapLayout::new(chars)).collect())
    }

    /// `applyTextManually` for a non-virtual board; `None` clears the line.
    pub fn apply_text_manually(&mut self, line: usize, text: Option<&str>) {
        if line >= self.line_count() {
            return;
        }
        let chars = self.max_char_count();
        let layout = &mut self.lines()[line];
        if !layout.is_layout("Default") {
            layout.load_default(chars);
        }
        match text {
            None => {
                layout.sections[0].set_text("");
                self.manual[line] = false;
            }
            Some(t) => {
                self.manual[line] = true;
                self.lines()[line].sections[0].set_text(t);
            }
        }
    }

    pub fn set_colour(&mut self, line: usize, dye: Dye) {
        self.colour[line] = dye.filter(|d| *d != 0);
    }

    pub fn set_glowing(&mut self, line: usize) {
        self.glowing[line] = true;
    }

    /// `getLineColor` with the dye -> RGB table supplied by the adapter.
    pub fn line_color(&self, line: usize, rgb_of: impl Fn(u8) -> u32) -> u32 {
        match self.colour[line] {
            None => 0xFFD3C6BA,
            Some(d) => rgb_of(d) | 0xFF000000,
        }
    }

    /// `getLineIndexAt` (the upper bound equals the line count, as in Java).
    pub fn line_index_at(&self, world_y: f64, y_coord: i32) -> usize {
        let v = (2.0 * (world_y - y_coord as f64 + 1.0)).floor();
        v.clamp(0.0, self.line_count() as f64) as usize
    }

    /// Tick all sections; returns the active flap count.
    pub fn tick_sections(&mut self, speed: f32, rng: &mut dyn FlapRandom) -> usize {
        let instant = speed.abs() > INSTANT_SPEED;
        let mut active = 0;
        for layout in self.lines() {
            for s in &mut layout.sections {
                active += s.tick(instant, rng);
            }
        }
        active
    }
}

/// Sound volumes of the clacking: `(scroll_value, calcite_background)`.
pub fn flap_volumes(active_flaps: usize) -> (f32, f32) {
    let a = active_flaps as f32;
    ((a / 20.0).clamp(0.25, 1.5), (a / 40.0).clamp(0.25, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_random_matches_reference_sequence() {
        // java.util.Random(0).nextInt() == -1155484576
        let mut r = JavaRandom::new(0);
        assert_eq!(r.next_bits(32), -1155484576);
        // java.util.Random(42).nextInt(10): 0, 3, 8
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int(10), 0);
        assert_eq!(r.next_int(10), 3);
        assert_eq!(r.next_int(10), 8);
        // power-of-two bound path
        let mut r = JavaRandom::new(42);
        let v = r.next_int(16);
        assert!((0..16).contains(&v));
    }

    #[test]
    fn char_capacity_formula() {
        assert_eq!(max_char_count(1, 0), 4);
        assert_eq!(max_char_count(2, 0), 8);
        assert_eq!(max_char_count(1, 1), 2);
        assert_eq!(max_char_count(3, 1), 12);
    }

    #[test]
    fn size_scan_stops_at_first_gap_and_caps() {
        assert_eq!(controller_size(|i| i < 3, |i| i < 2), (3, 3));
        assert_eq!(controller_size(|_| true, |_| true), (32, 33));
        assert_eq!(controller_size(|_| false, |_| false), (1, 1));
        assert!(should_be_controller(false, false));
        assert!(!should_be_controller(true, false));
        assert!(!should_be_controller(false, true));
    }

    #[test]
    fn section_refresh_left_aligned() {
        let mut s = FlapSection::new(5.0 * MONOSPACE, "alphabet", false, false);
        assert_eq!(s.flap_count(), 5);
        s.set_text("ab");
        s.refresh(false);
        assert_eq!(s.text(), "AB   ");
        s.set_text("abcdefgh");
        s.refresh(false);
        assert_eq!(s.text(), "ABCDE");
    }

    #[test]
    fn section_refresh_right_aligned_trims_and_pads_left() {
        let mut s = FlapSection::new(4.0 * MONOSPACE, "number", false, false).right_aligned();
        s.set_text(" 12 ");
        s.refresh(false);
        assert_eq!(s.text(), "  12");
    }

    #[test]
    fn transition_marks_only_changed_flaps_spinning() {
        let mut s = FlapSection::new(3.0 * MONOSPACE, "alphabet", false, false);
        s.set_text("abc");
        s.refresh(false);
        s.set_text("abd");
        s.refresh(true);
        assert!(!s.is_spinning(0) && !s.is_spinning(1) && s.is_spinning(2));
        assert_eq!(s.spinning_ticks(), 0);
    }

    #[test]
    fn single_flap_holds_whole_component() {
        let mut s = FlapSection::new(10.0, "x", true, false);
        assert_eq!(s.flap_count(), 1);
        s.set_text("icon");
        s.refresh(true);
        assert_eq!(s.flaps(), &["icon".to_string()]);
        assert!(s.is_spinning(0));
    }

    #[test]
    fn tick_without_cycle_does_nothing() {
        let mut s = FlapSection::new(3.0 * MONOSPACE, "alphabet", false, false);
        let mut r = JavaRandom::new(1);
        assert_eq!(s.tick(false, &mut r), 0);
        assert_eq!(s.spinning_ticks(), 0);
    }

    #[test]
    fn spin_runs_out_and_instant_stops_immediately() {
        let cycle: Vec<String> = (0..37).map(|i| i.to_string()).collect();
        let mut s = FlapSection::new(4.0 * MONOSPACE, "alphabet", false, false);
        s.set_cycling_options(cycle.clone());
        s.set_text("abcd");
        s.refresh(true);
        assert!(s.is_spinning(0));
        let mut r = JavaRandom::new(7);
        assert_eq!(s.tick(false, &mut r), 0); // first tick only counts
        let mut guard = 0;
        while s.tick(false, &mut r) > 0 {
            guard += 1;
            assert!(guard < 200);
        }
        // after max (= 64) ticks nothing is spinning for sure
        for _ in 0..80 {
            s.tick(false, &mut r);
        }
        assert!((0..4).all(|i| !s.is_spinning(i)));

        let mut inst = FlapSection::new(4.0 * MONOSPACE, "alphabet", false, false);
        inst.set_cycling_options(cycle);
        inst.set_text("abcd");
        inst.refresh(true);
        inst.tick(true, &mut r);
        assert_eq!(inst.tick(true, &mut r), 0);
        assert!((0..4).all(|i| !inst.is_spinning(i)));
    }

    #[test]
    fn spin_max_scales_with_cycle_length() {
        // max = max(4, (int)(len * 1.75)); a 2 entry cycle gives 4.
        let mut s = FlapSection::new(2.0 * MONOSPACE, "x", false, false);
        s.set_cycling_options(vec!["a".into(), "b".into()]);
        s.set_text("ab");
        s.refresh(true);
        let mut r = JavaRandom::new(3);
        for _ in 0..5 {
            s.tick(false, &mut r);
        }
        // after ticks > max further ticks return 0 and leave it alone
        assert_eq!(s.spinning_ticks(), 5);
        assert_eq!(s.tick(false, &mut r), 0);
        assert_eq!(s.spinning_ticks(), 5);
    }

    #[test]
    fn layout_default() {
        let l = FlapLayout::new(8);
        assert!(l.is_layout("Default"));
        assert_eq!(l.sections.len(), 1);
        assert_eq!(l.sections[0].flap_count(), 8);
        assert_eq!(l.sections[0].cycle, "alphabet");
    }

    #[test]
    fn board_lines_and_manual_text() {
        let mut b = FlapDisplay::new();
        assert_eq!(b.line_count(), 2);
        assert!(b.set_size(2, 3));
        assert!(!b.set_size(2, 3));
        assert_eq!(b.line_count(), 6);
        assert_eq!(b.colour.len(), 6);
        assert_eq!(b.max_char_count(), 8);
        b.apply_text_manually(1, Some("hello"));
        assert!(b.manual[1]);
        b.lines()[1].sections[0].refresh(false);
        assert_eq!(b.lines()[1].sections[0].text(), "HELLO   ");
        b.apply_text_manually(1, None);
        assert!(!b.manual[1]);
        b.apply_text_manually(99, Some("x")); // ignored
        // resizing resets manual flags and layouts
        b.apply_text_manually(0, Some("a"));
        b.set_size(1, 1);
        assert!(b.manual.iter().all(|m| !m));
        assert_eq!(b.lines().len(), 2);
    }

    #[test]
    fn manual_text_restores_default_layout() {
        let mut b = FlapDisplay::new();
        b.lines()[0].configure("Other", vec![FlapSection::new(7.0, "a", false, false)]);
        b.apply_text_manually(0, Some("x"));
        assert!(b.lines()[0].is_layout("Default"));
    }

    #[test]
    fn colours_and_glow() {
        let mut b = FlapDisplay::new();
        assert_eq!(b.line_color(0, |_| 0x112233), 0xFFD3C6BA);
        b.set_colour(0, Some(14));
        assert_eq!(b.line_color(0, |_| 0x112233), 0xFF112233);
        b.set_colour(0, Some(0)); // white is stored as default
        assert_eq!(b.colour[0], None);
        b.set_glowing(1);
        assert!(b.glowing[1] && !b.glowing[0]);
    }

    #[test]
    fn line_index_and_volumes() {
        let b = FlapDisplay::new();
        assert_eq!(b.line_index_at(64.2, 64), 2); // 2 * 1.2 = 2.4 -> 2 == line_count (Java quirk)
        assert_eq!(b.line_index_at(63.0, 64), 0);
        assert_eq!(flap_volumes(0), (0.25, 0.25));
        assert_eq!(flap_volumes(20), (1.0, 0.5));
        assert_eq!(flap_volumes(1000), (1.5, 1.0));
    }

    #[test]
    fn tick_sections_reports_active_flaps_and_instant_threshold() {
        let mut b = FlapDisplay::new();
        let cycle: Vec<String> = (0..37).map(|i| i.to_string()).collect();
        for l in b.lines().iter_mut() {
            for s in &mut l.sections {
                s.set_cycling_options(cycle.clone());
                s.set_text("abcd");
                s.refresh(true);
            }
        }
        let mut r = JavaRandom::new(11);
        b.tick_sections(10.0, &mut r);
        let _ = b.tick_sections(10.0, &mut r);
        // 129 rpm -> instant: nothing keeps spinning
        assert_eq!(b.tick_sections(129.0, &mut r), 0);
    }

    #[test]
    fn cycle_split() {
        assert_eq!(split_cycle(" ;A;B"), vec![" ", "A", "B"]);
    }
}
