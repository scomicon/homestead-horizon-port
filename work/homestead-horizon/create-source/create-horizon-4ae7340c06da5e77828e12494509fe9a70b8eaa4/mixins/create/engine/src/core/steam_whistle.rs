// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//! Pure Steam Whistle rules after WhistleBlock, WhistleExtenderBlock and
//! WhistleBlockEntity (Create 6.0.10, revision ac0c444d): attachment, size,
//! extender stacking, pitch, pitch id and sound parameters.
use crate::placement::Direction;

/// Longest scan of `updatePitch` (`newPitch` steps of 2 up to 24).
pub const MAX_PITCH_SCAN: i32 = 24;
/// Pitch ids of different sizes never collide (`pitch + 100 * ordinal`).
pub const SIZE_ID_STRIDE: i32 = 100;
/// How far above the whistle `incrementSize` searches.
pub const EXTENSION_SEARCH: i32 = 6;
/// Pitch added to the playback of a small whistle's new sound.
pub const SMALL_PITCH_BONUS: f32 = 0.75;
/// Chiff volume factor.
pub const CHIFF_VOLUME: f32 = 0.175;
/// Distance at which the whistle becomes inaudible.
pub const AUDIBLE_DISTANCE: f64 = 64.0;

/// `WhistleBlock.WhistleSize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    Small,
    Medium,
    Large,
}

impl Size {
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn parse(v: &str) -> Option<Self> {
        match v {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }

    /// `getRotatedBlockState` (wrench): cycles the size.
    pub fn cycled(self) -> Self {
        match self {
            Self::Small => Self::Medium,
            Self::Medium => Self::Large,
            Self::Large => Self::Small,
        }
    }
}

/// `WhistleExtenderBlock.WhistleExtenderShape`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtenderShape {
    Single,
    Double,
    DoubleConnected,
}

impl ExtenderShape {
    pub fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Double => "double",
            Self::DoubleConnected => "double_connected",
        }
    }

    pub fn parse(v: &str) -> Option<Self> {
        match v {
            "single" => Some(Self::Single),
            "double" => Some(Self::Double),
            "double_connected" => Some(Self::DoubleConnected),
            _ => None,
        }
    }
}

/// Shapes of a stack of `count` extenders, bottom up. Only the top one can be
/// a `Single` (a single cannot carry another); every extender below another
/// one is `DoubleConnected` (no rim between them), and a double top keeps the
/// rim. Port rule: the original's per-block assignment was not visible, this
/// is the one consistent with `extender_can_survive` and the rim models.
pub fn stack_shapes(count: usize, top_single: bool) -> Vec<ExtenderShape> {
    (0..count)
        .map(|i| {
            if i + 1 < count {
                ExtenderShape::DoubleConnected
            } else if top_single {
                ExtenderShape::Single
            } else {
                ExtenderShape::Double
            }
        })
        .collect()
}

/// Replay period (ticks) of the sustained note when it is
/// replayed as one-shot sounds: the sample lasts `sample_ticks` at pitch 1 and
/// `1 / rate` times longer at a lower rate (vanilla clamps the rate to
/// 0.5..=2); `overlap` ticks are cut so the next note starts before the last
/// ends. Never below 5.
pub fn replay_period(sample_ticks: i32, rate: f32, overlap: i32) -> i32 {
    let rate = rate.clamp(0.5, 2.0);
    (((sample_ticks as f32) / rate).floor() as i32 - overlap).max(5)
}

/// `WhistleBlock.getAttachedDirection`.
pub fn attached_direction(wall: bool, facing: Direction) -> Direction {
    if wall { facing } else { Direction::Down }
}

/// `canSurvive`: a tank must be on the attached side.
pub fn can_survive(wall: bool, facing: Direction, is_tank_at: impl Fn(Direction) -> bool) -> bool {
    is_tank_at(attached_direction(wall, facing))
}

/// `(wall, facing)` from `getStateForPlacement`; `clicked_face` is the face
/// of the target block that was clicked, `placer_horizontal` the horizontal direction the
/// adapter derives from the player (the source text did not show whether the
/// original flips it; no flip is applied here). Returns `None` when the state cannot survive.
pub fn placement_state(
    clicked_face: Direction,
    placer_horizontal: Direction,
    is_tank_at: impl Fn(Direction) -> bool,
) -> Option<(bool, Direction)> {
    let (wall, facing) = if clicked_face.axis() == crate::kinetics::Axis::Y {
        (false, placer_horizontal)
    } else {
        (true, clicked_face)
    };
    can_survive(wall, facing, is_tank_at).then_some((wall, facing))
}

/// `WhistleExtenderBlock.canSurvive`: the support below must be a whistle or a
/// non-single extender. A single extender cannot carry another.
pub fn extender_can_survive(below: Below) -> bool {
    match below {
        Below::Whistle => true,
        Below::Extender(shape) => shape != ExtenderShape::Single,
        Below::Other => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Below {
    Whistle,
    Extender(ExtenderShape),
    Other,
}

/// `updatePitch`. `above` yields the blocks above the whistle bottom-up:
/// `Some(shape)` for extenders, `None` for anything else.
pub fn pitch_of_stack(above: impl IntoIterator<Item = Option<ExtenderShape>>) -> i32 {
    let mut iter = above.into_iter();
    let mut pitch = 0;
    while pitch <= MAX_PITCH_SCAN {
        match iter.next().flatten() {
            None => break,
            Some(ExtenderShape::Single) => {
                pitch += 1;
                break;
            }
            Some(_) => {}
        }
        pitch += 2;
    }
    pitch
}

/// `getPitchId`.
pub fn pitch_id(pitch: i32, size: Size) -> i32 {
    pitch + SIZE_ID_STRIDE * size.ordinal()
}

/// What `incrementSize` finds in one of the six slots above a whistle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Extender(ExtenderShape),
    Replaceable,
    Solid,
}

/// The change `incrementSize` makes (right-click with a Steam Whistle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Increment {
    /// Turn the single extender at height `i` into a double one.
    MakeDouble { height: i32, sound_exponent: i32 },
    /// Place a new single extender at height `i`.
    Place { height: i32, sound_exponent: i32 },
}

/// `incrementSize`: heights are 1-based above the whistle. The sound pitch is
/// `2^(sound_exponent / 12)` (the original passes the negative exponent).
pub fn increment(above: &[Slot]) -> Option<Increment> {
    for (idx, slot) in above.iter().take(EXTENSION_SEARCH as usize).enumerate() {
        let i = idx as i32 + 1;
        match slot {
            Slot::Extender(ExtenderShape::Single) => {
                return Some(Increment::MakeDouble { height: i, sound_exponent: -(i * 2) });
            }
            Slot::Extender(_) => {}
            Slot::Solid => return None,
            Slot::Replaceable => {
                return Some(Increment::Place { height: i, sound_exponent: -(i * 2 - 1) });
            }
        }
    }
    None
}

/// Playback pitch of the sustained note: `2^(-pitch / 12)`.
pub fn sound_pitch(pitch: i32) -> f32 {
    2.0_f32.powf(-(pitch as f32) / 12.0)
}

/// Pitch used when a new sound instance starts.
pub fn start_pitch(pitch: i32, size: Size) -> f32 {
    sound_pitch(pitch) + if size == Size::Small { SMALL_PITCH_BONUS } else { 0.0 }
}

/// Volume scale by camera distance for the initial chiff sound.
pub fn chiff_volume(distance: f64) -> f32 {
    let falloff = ((AUDIBLE_DISTANCE - distance) / AUDIBLE_DISTANCE).clamp(0.0, 1.0) as f32;
    falloff * CHIFF_VOLUME
}

/// `WhistleBlockEntity.tick` client condition.
pub fn is_sounding(powered_state: bool, boiler_active: bool, hot: bool, virtual_whistle: bool) -> bool {
    (powered_state && boiler_active && hot) || virtual_whistle
}

/// Block-entity state of a whistle between ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Whistle {
    pub size: Size,
    pub pitch: i32,
}

impl Whistle {
    pub fn new(size: Size) -> Self {
        Self { size, pitch: 0 }
    }

    pub fn id(&self) -> i32 {
        pitch_id(self.pitch, self.size)
    }

    /// Recompute the pitch; returns true when it changed.
    pub fn update_pitch(&mut self, above: impl IntoIterator<Item = Option<ExtenderShape>>) -> bool {
        let new = pitch_of_stack(above);
        let changed = new != self.pitch;
        self.pitch = new;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ExtenderShape::*;

    #[test]
    fn bare_whistle_has_pitch_zero() {
        assert_eq!(pitch_of_stack([None]), 0);
        assert_eq!(pitch_of_stack([]), 0);
    }

    #[test]
    fn each_extender_raises_two_and_a_single_tops_with_one() {
        assert_eq!(pitch_of_stack([Some(Single), None]), 1);
        assert_eq!(pitch_of_stack([Some(Double), Some(Single)]), 3);
        assert_eq!(pitch_of_stack([Some(Double), Some(DoubleConnected), Some(Single)]), 5);
        assert_eq!(pitch_of_stack([Some(Double), Some(Double), None]), 4);
    }

    #[test]
    fn the_scan_is_capped() {
        // Thirteen doubles in a row run the loop to 26.
        let tall = std::iter::repeat_n(Some(DoubleConnected), 20);
        assert_eq!(pitch_of_stack(tall), 26);
    }

    #[test]
    fn pitch_ids_are_unique_per_size() {
        assert_eq!(pitch_id(3, Size::Small), 3);
        assert_eq!(pitch_id(3, Size::Medium), 103);
        assert_eq!(pitch_id(3, Size::Large), 203);
        let mut w = Whistle::new(Size::Large);
        assert!(w.update_pitch([Some(Double), Some(Single)]));
        assert_eq!(w.id(), 203);
        assert!(!w.update_pitch([Some(Double), Some(Single)]));
    }

    #[test]
    fn sound_pitch_is_one_semitone_per_unit() {
        assert!((sound_pitch(0) - 1.0).abs() < 1e-6);
        assert!((sound_pitch(12) - 0.5).abs() < 1e-6);
        assert!((sound_pitch(1) - 0.943_874).abs() < 1e-5);
        assert!((start_pitch(0, Size::Small) - 1.75).abs() < 1e-6);
        assert!((start_pitch(0, Size::Large) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn chiff_volume_falls_off_over_64_blocks() {
        assert!((chiff_volume(0.0) - 0.175).abs() < 1e-6);
        assert!((chiff_volume(32.0) - 0.0875).abs() < 1e-6);
        assert_eq!(chiff_volume(64.0), 0.0);
        assert_eq!(chiff_volume(500.0), 0.0);
    }

    #[test]
    fn attachment_and_placement() {
        assert_eq!(attached_direction(false, Direction::North), Direction::Down);
        assert_eq!(attached_direction(true, Direction::North), Direction::North);
        // On top of a tank.
        let on_top = placement_state(Direction::Up, Direction::South, |d| d == Direction::Down);
        assert_eq!(on_top, Some((false, Direction::South)));
        // Side of a tank to the west: clicked face is West -> facing West.
        let side = placement_state(Direction::West, Direction::South, |d| d == Direction::West);
        assert_eq!(side, Some((true, Direction::West)));
        // Not against a tank.
        assert_eq!(placement_state(Direction::Up, Direction::South, |_| false), None);
        // A floor-mounted whistle needs the tank below only.
        assert!(!can_survive(false, Direction::East, |d| d == Direction::East));
    }

    #[test]
    fn extenders_stand_on_whistles_and_non_single_extenders() {
        assert!(extender_can_survive(Below::Whistle));
        assert!(extender_can_survive(Below::Extender(Double)));
        assert!(extender_can_survive(Below::Extender(DoubleConnected)));
        assert!(!extender_can_survive(Below::Extender(Single)));
        assert!(!extender_can_survive(Below::Other));
    }

    #[test]
    fn increment_walks_up_the_stack() {
        use Slot::*;
        assert_eq!(
            increment(&[Replaceable]),
            Some(Increment::Place { height: 1, sound_exponent: -1 })
        );
        assert_eq!(
            increment(&[Extender(Single)]),
            Some(Increment::MakeDouble { height: 1, sound_exponent: -2 })
        );
        assert_eq!(
            increment(&[Extender(Double), Extender(Single)]),
            Some(Increment::MakeDouble { height: 2, sound_exponent: -4 })
        );
        assert_eq!(
            increment(&[Extender(Double), Extender(DoubleConnected), Replaceable]),
            Some(Increment::Place { height: 3, sound_exponent: -5 })
        );
        assert_eq!(increment(&[Solid]), None);
        assert_eq!(increment(&[Extender(Double), Solid]), None);
        // Only six slots are examined.
        let full = [Extender(Double); 6];
        assert_eq!(increment(&full), None);
        assert_eq!(increment(&[]), None);
    }

    #[test]
    fn stack_shapes_keep_a_single_only_on_top() {
        assert!(stack_shapes(0, true).is_empty());
        assert_eq!(stack_shapes(1, true), [Single]);
        assert_eq!(stack_shapes(1, false), [Double]);
        assert_eq!(stack_shapes(3, true), [DoubleConnected, DoubleConnected, Single]);
        assert_eq!(stack_shapes(2, false), [DoubleConnected, Double]);
        // Every stack is one the survival rule accepts.
        let stack = stack_shapes(4, true);
        for pair in stack.windows(2) {
            assert!(extender_can_survive(Below::Extender(pair[0])));
        }
        // And the pitch scan reads it like the hand-built stacks above.
        let above: Vec<_> = stack.iter().map(|s| Some(*s)).collect();
        assert_eq!(pitch_of_stack(above), 7);
    }

    #[test]
    fn extender_shape_names_round_trip() {
        for s in [Single, Double, DoubleConnected] {
            assert_eq!(ExtenderShape::parse(s.name()), Some(s));
        }
        assert_eq!(ExtenderShape::parse("x"), None);
    }

    #[test]
    fn replay_period_follows_the_playback_rate() {
        assert_eq!(replay_period(60, 1.0, 2), 58);
        assert_eq!(replay_period(60, 0.5, 2), 118);
        // Rates outside the vanilla clamp are clamped.
        assert_eq!(replay_period(60, 0.2, 2), 118);
        assert_eq!(replay_period(60, 4.0, 2), 28);
        assert_eq!(replay_period(4, 2.0, 2), 5);
    }

    #[test]
    fn sounding_condition_and_size_cycle() {
        assert!(is_sounding(true, true, true, false));
        assert!(!is_sounding(true, true, false, false));
        assert!(!is_sounding(false, true, true, false));
        assert!(is_sounding(false, false, false, true));
        assert_eq!(Size::Small.cycled(), Size::Medium);
        assert_eq!(Size::Large.cycled(), Size::Small);
        assert_eq!(Size::parse("large"), Some(Size::Large));
        assert_eq!(Size::Medium.name(), "medium");
    }

    #[test]
    fn a_full_stack_produces_twelve_distinct_pitch_ids() {
        // Pitches 0..=11 for one size need stacks that give every value.
        let mut ids = Vec::new();
        for stack in 0..6 {
            for top in [None, Some(Single)] {
                let mut s: Vec<Option<ExtenderShape>> = vec![Some(Double); stack];
                if let Some(t) = top {
                    s.push(Some(t));
                } else {
                    s.push(None);
                }
                ids.push(pitch_id(pitch_of_stack(s), Size::Medium));
            }
        }
        assert!(crate::boiler::pipe_organ(ids));
    }
}

