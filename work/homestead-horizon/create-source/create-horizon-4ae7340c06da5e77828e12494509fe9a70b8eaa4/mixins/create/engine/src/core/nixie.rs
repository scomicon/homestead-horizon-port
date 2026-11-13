// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Nixie Tube model after NixieTubeBlock / NixieTubeBlockEntity
// (Create 6.0.10, redstone/nixieTube). Rendering, ComputerCraft signals and
// rich-text (DynamicComponent) parsing are out of scope; the text model works
// on already-flattened strings.
use crate::redstone_link::{Direction, MAX_SIGNAL};

/// Characters shown by one tube.
pub const CHARS_PER_TUBE: usize = 2;

/// `NixieTubeBlock.FACE` (`DoubleFaceAttachedBlock.DoubleAttachFace`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Floor,
    Ceiling,
    Wall,
    WallReversed,
}

/// `getLeftNixieDirection`: opposite of FACING, UP for WALL, DOWN for
/// WALL_REVERSED.
pub fn left_direction(facing: Direction, face: Face) -> Direction {
    match face {
        Face::Wall => Direction::Up,
        Face::WallReversed => Direction::Down,
        _ => facing.opposite(),
    }
}

pub fn right_direction(facing: Direction, face: Face) -> Direction {
    left_direction(facing, face).opposite()
}

/// Vanilla `DyeColor` order. The orange tube is the undyed block.
pub const DYE_NAMES: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple",
    "blue", "brown", "green", "red", "black",
];

/// Registry name of the tube block for a dye (`withColor`): ORANGE maps to
/// the plain `nixie_tube`, every other dye to `<dye>_nixie_tube`.
pub fn tube_block_name(dye: u8) -> String {
    let name = DYE_NAMES[(dye as usize).min(15)];
    if name == "orange" { "nixie_tube".to_string() } else { format!("{name}_nixie_tube") }
}

/// `NixieTubeBlock.getPower`: max of all six `getSignal(d)` and the UP-side
/// signals of every face except the attached one.
pub fn tube_power(direct: [i32; 6], up: [i32; 6], attached: Direction) -> i32 {
    let mut p = 0;
    for d in Direction::ALL {
        p = p.max(direct[d.index()]);
        if d != attached {
            p = p.max(up[d.index()]);
        }
    }
    p.clamp(0, MAX_SIGNAL)
}

/// `charOrEmpty`: a space when past the end.
fn char_or_empty(text: &str, index: usize) -> String {
    text.chars().nth(index).map_or_else(|| " ".to_string(), |c| c.to_string())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NixieTube {
    pub redstone_strength: i32,
    /// Flattened custom text; `Some` takes priority over redstone.
    pub custom_text: Option<String>,
    /// Position in the row; selects characters `2*i` and `2*i + 1`.
    pub nixie_index: usize,
}

impl NixieTube {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reacts_to_redstone(&self) -> bool {
        self.custom_text.is_none()
    }

    /// `displayCustomText`. Returns false if the same text is already set.
    pub fn display_custom_text(&mut self, text: &str, position_in_row: usize) -> bool {
        if self.custom_text.as_deref() == Some(text) {
            return false;
        }
        self.custom_text = Some(text.to_string());
        self.nixie_index = position_in_row;
        true
    }

    /// `clearCustomText`.
    pub fn clear_custom_text(&mut self) {
        self.nixie_index = 0;
        self.custom_text = None;
    }

    /// `updateRedstoneStrength`: clears custom text first.
    pub fn update_redstone_strength(&mut self, strength: i32) {
        self.clear_custom_text();
        self.redstone_strength = strength;
    }

    /// `updateDisplayedRedstoneValue(force)`: tubes showing custom text keep
    /// it unless forced. Returns true if the strength was applied.
    pub fn update_displayed_redstone_value(&mut self, power: i32, force: bool) -> bool {
        if self.reacts_to_redstone() || force {
            self.update_redstone_strength(power);
            true
        } else {
            false
        }
    }

    /// `updateDisplayedStrings`: the two characters on this tube.
    pub fn displayed_strings(&self) -> (String, String) {
        match &self.custom_text {
            Some(t) => (char_or_empty(t, self.nixie_index * 2), char_or_empty(t, self.nixie_index * 2 + 1)),
            None => {
                let first = if self.redstone_strength < 10 { "0".to_string() } else { "1".to_string() };
                let second = (self.redstone_strength.rem_euclid(10)).to_string();
                (first, second)
            }
        }
    }

    /// `getFullText`.
    pub fn full_text(&self) -> String {
        self.custom_text.clone().unwrap_or_else(|| self.redstone_strength.to_string())
    }
}

/// Apply a text to a row of tubes the way `walkNixies` + `acceptLine` do:
/// every tube receives the full string and its position.
pub fn apply_text_to_row(row: &mut [NixieTube], text: &str) -> usize {
    let mut changed = 0;
    for (i, t) in row.iter_mut().enumerate() {
        if t.display_custom_text(text, i) {
            changed += 1;
        }
    }
    changed
}

/// `NixieTubeDisplayTarget.getWidth`: two characters per tube.
pub fn row_width(tubes: usize) -> usize {
    tubes * CHARS_PER_TUBE
}

/// `NixieTubeBlock.onRemove` / walk: after a tube is removed the remaining
/// segments are refreshed from redstone.
pub fn refresh_row_from_redstone(row: &mut [NixieTube], power: i32) {
    for t in row {
        t.update_redstone_strength(power);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redstone_digits() {
        let mut t = NixieTube::new();
        for (p, expected) in [(0, ("0", "0")), (7, ("0", "7")), (9, ("0", "9")), (10, ("1", "0")), (15, ("1", "5"))] {
            t.update_redstone_strength(p);
            assert_eq!(t.displayed_strings(), (expected.0.to_string(), expected.1.to_string()));
        }
        assert_eq!(t.full_text(), "15");
    }

    #[test]
    fn custom_text_splits_two_chars_per_tube() {
        let mut row = vec![NixieTube::new(); 3];
        assert_eq!(apply_text_to_row(&mut row, "HELLO"), 3);
        let shown: Vec<_> = row.iter().map(|t| t.displayed_strings()).collect();
        assert_eq!(shown[0], ("H".into(), "E".into()));
        assert_eq!(shown[1], ("L".into(), "L".into()));
        assert_eq!(shown[2], ("O".into(), " ".into()));
        // same text again is a no-op
        assert_eq!(apply_text_to_row(&mut row, "HELLO"), 0);
        assert_eq!(row_width(3), 6);
    }

    #[test]
    fn custom_text_overrides_redstone_until_cleared() {
        let mut t = NixieTube::new();
        t.display_custom_text("AB", 0);
        assert!(!t.reacts_to_redstone());
        assert!(!t.update_displayed_redstone_value(9, false));
        assert_eq!(t.displayed_strings(), ("A".into(), "B".into()));
        assert!(t.update_displayed_redstone_value(9, true));
        assert_eq!(t.nixie_index, 0);
        assert_eq!(t.displayed_strings(), ("0".into(), "9".into()));
    }

    #[test]
    fn empty_text_blanks_the_row() {
        let mut row = vec![NixieTube::new(); 2];
        apply_text_to_row(&mut row, "");
        assert_eq!(row[1].displayed_strings(), (" ".into(), " ".into()));
        refresh_row_from_redstone(&mut row, 12);
        assert_eq!(row[0].displayed_strings(), ("1".into(), "2".into()));
    }

    #[test]
    fn row_directions() {
        assert_eq!(left_direction(Direction::North, Face::Floor), Direction::South);
        assert_eq!(right_direction(Direction::North, Face::Floor), Direction::North);
        assert_eq!(left_direction(Direction::North, Face::Wall), Direction::Up);
        assert_eq!(left_direction(Direction::East, Face::WallReversed), Direction::Down);
        assert_eq!(right_direction(Direction::East, Face::WallReversed), Direction::Up);
    }

    #[test]
    fn dye_block_names() {
        assert_eq!(tube_block_name(1), "nixie_tube");
        assert_eq!(tube_block_name(0), "white_nixie_tube");
        assert_eq!(tube_block_name(14), "red_nixie_tube");
    }

    #[test]
    fn power_probe() {
        let mut direct = [0; 6];
        let mut up = [0; 6];
        up[Direction::North.index()] = 6;
        up[Direction::South.index()] = 12;
        assert_eq!(tube_power(direct, up, Direction::South), 6);
        direct[Direction::Down.index()] = 3;
        assert_eq!(tube_power(direct, up, Direction::South), 6);
        direct[Direction::East.index()] = 14;
        assert_eq!(tube_power(direct, up, Direction::South), 14);
    }
}
