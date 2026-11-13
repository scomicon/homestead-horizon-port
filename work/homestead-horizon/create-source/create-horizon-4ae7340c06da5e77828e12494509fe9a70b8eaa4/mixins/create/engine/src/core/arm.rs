// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

//! Mechanical Arm rules after `ArmBlockEntity`, `ArmInteractionPoint`,
//! `ArmInteractionPointType`, `AllArmInteractionPointTypes`, `ArmAngleTarget`,
//! `ArmInteractionPointHandler` and `ArmBlock` (Create 6.0.10).
//!
//! Everything here is pure. World access (block probing, item handlers, the
//! jukebox state) goes through [`ArmWorld`]; effects (sounds, advancements) are
//! returned as [`Event`]s for an adapter to play.

use std::collections::HashSet;

/// `mechanicalArmRange` server config default.
pub const DEFAULT_RANGE: i32 = 5;
/// Items the arm will carry are limited by the source handler, never by the
/// arm; this is `extract(slot, simulate)`'s default amount in the Java class.
pub const DEFAULT_EXTRACT_AMOUNT: u32 = 64;
/// `outputs.size() >= 10` awards `ARM_MANY_TARGETS`.
pub const MANY_TARGETS: usize = 10;
/// Speed ceiling used by `tickMovementProgress`.
pub const MAX_SPEED: f32 = 256.0;

// ---------------------------------------------------------------- directions

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub fn normal(self) -> [i32; 3] {
        match self {
            Self::Down => [0, -1, 0],
            Self::Up => [0, 1, 0],
            Self::North => [0, 0, -1],
            Self::South => [0, 0, 1],
            Self::West => [-1, 0, 0],
            Self::East => [1, 0, 0],
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    fn scaled(self, factor: f64) -> [f64; 3] {
        let n = self.normal();
        [n[0] as f64 * factor, n[1] as f64 * factor, n[2] as f64 * factor]
    }
}

// -------------------------------------------------------------------- enums

/// `ArmInteractionPoint.Mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Deposit,
    Take,
}

impl Mode {
    pub fn cycled(self) -> Self {
        match self {
            Self::Deposit => Self::Take,
            Self::Take => Self::Deposit,
        }
    }

    /// Hex colour of the selection outline / message.
    pub fn colour(self) -> u32 {
        match self {
            Self::Deposit => 0xDDC166,
            Self::Take => 0x7FCDE0,
        }
    }

    pub fn translation_key(self) -> &'static str {
        match self {
            Self::Deposit => "mechanical_arm.deposit_to",
            Self::Take => "mechanical_arm.extract_from",
        }
    }

    /// `NBTHelper.writeEnum` writes the constant name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Deposit => "DEPOSIT",
            Self::Take => "TAKE",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "DEPOSIT" => Some(Self::Deposit),
            "TAKE" => Some(Self::Take),
            _ => None,
        }
    }
}

/// `ArmBlockEntity.SelectionMode` (the "when multiple outputs available" option).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    RoundRobin,
    ForcedRoundRobin,
    PreferFirst,
}

impl SelectionMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::RoundRobin => "ROUND_ROBIN",
            Self::ForcedRoundRobin => "FORCED_ROUND_ROBIN",
            Self::PreferFirst => "PREFER_FIRST",
        }
    }

    /// Language key `create.mechanical_arm.selection_mode.<id>`.
    pub fn translation_key(self) -> &'static str {
        match self {
            Self::RoundRobin => "create.mechanical_arm.selection_mode.round_robin",
            Self::ForcedRoundRobin => "create.mechanical_arm.selection_mode.forced_round_robin",
            Self::PreferFirst => "create.mechanical_arm.selection_mode.prefer_first",
        }
    }
}

/// `ArmBlockEntity.Phase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    SearchInputs,
    MoveToInput,
    SearchOutputs,
    MoveToOutput,
    Dancing,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::SearchInputs => "SEARCH_INPUTS",
            Self::MoveToInput => "MOVE_TO_INPUT",
            Self::SearchOutputs => "SEARCH_OUTPUTS",
            Self::MoveToOutput => "MOVE_TO_OUTPUT",
            Self::Dancing => "DANCING",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "SEARCH_INPUTS" => Self::SearchInputs,
            "MOVE_TO_INPUT" => Self::MoveToInput,
            "SEARCH_OUTPUTS" => Self::SearchOutputs,
            "MOVE_TO_OUTPUT" => Self::MoveToOutput,
            "DANCING" => Self::Dancing,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------- point types

/// The sixteen `ArmInteractionPointType`s in registration order. Nothing
/// overrides `getPriority`, so the stable sort keeps this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointKind {
    Basin,
    Belt,
    BlazeBurner,
    Chute,
    Crafter,
    CrushingWheels,
    Deployer,
    Depot,
    Funnel,
    Millstone,
    Packager,
    Saw,
    Campfire,
    Composter,
    Jukebox,
    RespawnAnchor,
    /// Any ordinary inventory read through its top face (chests, barrels, vaults, hoppers).
    /// Lowest priority; added by the port, see docs/mechanical-arm.md.
    Container,
}

pub const ALL_KINDS: [PointKind; 17] = [
    PointKind::Basin,
    PointKind::Belt,
    PointKind::BlazeBurner,
    PointKind::Chute,
    PointKind::Crafter,
    PointKind::CrushingWheels,
    PointKind::Deployer,
    PointKind::Depot,
    PointKind::Funnel,
    PointKind::Millstone,
    PointKind::Packager,
    PointKind::Saw,
    PointKind::Campfire,
    PointKind::Composter,
    PointKind::Jukebox,
    PointKind::RespawnAnchor,
    PointKind::Container,
];

/// What the adapter reads from the world for `canCreatePoint`.
#[derive(Clone, Debug, Default)]
pub struct BlockProbe {
    pub basin: bool,
    pub belt: bool,
    pub belt_can_transport_objects: bool,
    pub belt_tunnel_above: bool,
    pub blaze_burner: bool,
    pub chute: bool,
    pub mechanical_crafter: bool,
    pub crushing_wheel_controller: bool,
    pub deployer: bool,
    pub depot: bool,
    pub weighted_ejector: bool,
    pub track_station: bool,
    pub funnel: bool,
    pub funnel_extracting: bool,
    pub belt_funnel_pushing: bool,
    pub millstone: bool,
    pub packager: bool,
    pub repackager: bool,
    pub saw: bool,
    pub saw_facing_up: bool,
    /// `None` when the block entity is missing (the Java cast would throw; the
    /// port treats it as "cannot create").
    pub saw_speed: Option<f32>,
    pub campfire: bool,
    pub composter: bool,
    pub jukebox: bool,
    pub respawn_anchor: bool,
    /// An ordinary inventory reachable through its top face.
    pub container: bool,
    /// Block-state facing used by crafter (horizontal), deployer and funnel.
    pub facing: Option<Direction>,
}

impl PointKind {
    pub fn register_name(self) -> &'static str {
        match self {
            Self::Basin => "basin",
            Self::Belt => "belt",
            Self::BlazeBurner => "blaze_burner",
            Self::Chute => "chute",
            Self::Crafter => "crafter",
            Self::CrushingWheels => "crushing_wheels",
            Self::Deployer => "deployer",
            Self::Depot => "depot",
            Self::Funnel => "funnel",
            Self::Millstone => "millstone",
            Self::Packager => "packager",
            Self::Saw => "saw",
            Self::Campfire => "campfire",
            Self::Composter => "composter",
            Self::Jukebox => "jukebox",
            Self::RespawnAnchor => "respawn_anchor",
            Self::Container => "container",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let id = name.strip_prefix("create:").unwrap_or(name);
        ALL_KINDS.iter().copied().find(|k| k.register_name() == id)
    }

    /// `canCreatePoint`.
    pub fn can_create(self, b: &BlockProbe) -> bool {
        match self {
            Self::Basin => b.basin,
            Self::Belt => b.belt && !b.belt_tunnel_above && b.belt_can_transport_objects,
            Self::BlazeBurner => b.blaze_burner,
            Self::Chute => b.chute,
            Self::Crafter => b.mechanical_crafter,
            Self::CrushingWheels => b.crushing_wheel_controller,
            Self::Deployer => b.deployer,
            Self::Depot => b.depot || b.weighted_ejector || b.track_station,
            Self::Funnel => b.funnel && !b.funnel_extracting && !b.belt_funnel_pushing,
            Self::Millstone => b.millstone,
            Self::Packager => b.packager || b.repackager,
            Self::Saw => b.saw && b.saw_facing_up && b.saw_speed.is_some_and(|s| s != 0.0),
            Self::Campfire => b.campfire,
            Self::Composter => b.composter,
            Self::Jukebox => b.jukebox,
            Self::RespawnAnchor => b.respawn_anchor,
            Self::Container => b.container,
        }
    }

    /// `DepositOnlyArmInteractionPoint` subclasses.
    pub fn deposit_only(self) -> bool {
        matches!(
            self,
            Self::BlazeBurner | Self::Funnel | Self::Campfire | Self::RespawnAnchor | Self::CrushingWheels
        )
    }

    /// Fixed slot count when the Java point overrides `getSlotCount`.
    pub fn fixed_slot_count(self) -> Option<usize> {
        match self {
            k if k.deposit_only() => Some(0),
            Self::Jukebox => Some(1),
            Self::Composter => Some(2),
            _ => None,
        }
    }

    /// `CrafterPoint.extract` wraps `allowExtraction()` / `forbidExtraction()`
    /// around the real extraction.
    pub fn needs_extraction_window(self) -> bool {
        self == Self::Crafter
    }

    /// Direction the claw faces when approaching the point.
    pub fn interaction_direction(self, facing: Option<Direction>) -> Direction {
        match self {
            Self::Crafter => facing.unwrap_or(Direction::South).opposite(),
            Self::Deployer => facing.unwrap_or(Direction::Up).opposite(),
            Self::Funnel => facing.map_or(Direction::Up, Direction::opposite),
            _ => Direction::Down,
        }
    }

    /// `getInteractionPositionVector` for a block at `pos`.
    pub fn target_vector(self, pos: [i32; 3], facing: Option<Direction>) -> [f64; 3] {
        let [x, y, z] = pos.map(f64::from);
        let centre = [x + 0.5, y + 0.5, z + 0.5];
        let add = |base: [f64; 3], d: [f64; 3]| [base[0] + d[0], base[1] + d[1], base[2] + d[2]];
        match self {
            Self::Belt | Self::Depot | Self::Saw => [x + 0.5, y + 14.0 / 16.0, z + 0.5],
            Self::Chute | Self::CrushingWheels | Self::Jukebox | Self::RespawnAnchor | Self::Container => {
                [x + 0.5, y + 1.0, z + 0.5]
            }
            Self::Composter => [x + 0.5, y + 13.0 / 16.0, z + 0.5],
            Self::Crafter => add(centre, self.interaction_direction(facing).scaled(0.5)),
            Self::Deployer => add(centre, self.interaction_direction(facing).scaled(0.65)),
            Self::Funnel => match facing {
                // Centre minus 0.15 along the funnel facing (zero when unknown).
                Some(f) => add(centre, f.scaled(-0.15)),
                None => centre,
            },
            _ => centre,
        }
    }
}

/// `ArmInteractionPointType.getPrimaryType`.
pub fn primary_type(b: &BlockProbe) -> Option<PointKind> {
    ALL_KINDS.iter().copied().find(|k| k.can_create(b))
}

// ------------------------------------------------------------------ points

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub kind: PointKind,
    pub pos: [i32; 3],
    pub mode: Mode,
}

/// Serialized form (`Type`, `Pos` relative to the arm, `Mode`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointRecord {
    pub kind: String,
    pub pos: [i32; 3],
    pub mode: String,
}

impl Point {
    /// `ArmInteractionPoint.create`: a new point starts in DEPOSIT mode.
    pub fn create(probe: &BlockProbe, pos: [i32; 3]) -> Option<Self> {
        primary_type(probe).map(|kind| Self { kind, pos, mode: Mode::Deposit })
    }

    /// `cycleMode`; deposit-only points ignore it.
    pub fn cycle_mode(&mut self) {
        if !self.kind.deposit_only() {
            self.mode = self.mode.cycled();
        }
    }

    pub fn serialize(&self, anchor: [i32; 3]) -> PointRecord {
        PointRecord {
            kind: format!("create:{}", self.kind.register_name()),
            pos: [self.pos[0] - anchor[0], self.pos[1] - anchor[1], self.pos[2] - anchor[2]],
            mode: self.mode.name().to_string(),
        }
    }

    /// `deserialize`: unknown type, failed `canCreatePoint` or a bad mode
    /// yields `None`. `probe` reads the world at the absolute position.
    pub fn deserialize(
        record: &PointRecord,
        anchor: [i32; 3],
        probe: impl Fn([i32; 3]) -> BlockProbe,
    ) -> Option<Self> {
        let kind = PointKind::from_name(&record.kind)?;
        let pos = [anchor[0] + record.pos[0], anchor[1] + record.pos[1], anchor[2] + record.pos[2]];
        if !kind.can_create(&probe(pos)) {
            return None;
        }
        let mode = Mode::parse(&record.mode)?;
        Some(Self { kind, pos, mode })
    }

    /// Target of the claw for the arm at `arm`.
    pub fn angle_target(&self, arm: [i32; 3], facing: Option<Direction>, ceiling: bool) -> AngleTarget {
        AngleTarget::solve(
            arm,
            self.kind.target_vector(self.pos, facing),
            self.kind.interaction_direction(facing),
            ceiling,
        )
    }
}

/// `ArmInteractionPoint.transformPos` for a structure transform, supplied as a
/// map on relative offsets.
pub fn transform_record(record: &mut PointRecord, apply: impl Fn([i32; 3]) -> [i32; 3]) {
    record.pos = apply(record.pos);
}

// ------------------------------------------------------- range and loading

/// `BlockPos.closerThan(pos, range)`: squared distance strictly below range^2.
pub fn in_range(arm: [i32; 3], point: [i32; 3], range: i32) -> bool {
    let d = [point[0] - arm[0], point[1] - arm[1], point[2] - arm[2]];
    let sq = (d[0] as i64).pow(2) + (d[1] as i64).pow(2) + (d[2] as i64).pow(2);
    sq < (range as i64).pow(2)
}

/// Chunk columns that must be loaded before points are initialized
/// (`isAreaActuallyLoaded(pos, range + 1)`): the square of that radius.
pub fn required_chunks(arm: [i32; 3], range: i32) -> Vec<(i32, i32)> {
    let r = range + 1;
    let mut out = Vec::new();
    for cx in (arm[0] - r) >> 4..=(arm[0] + r) >> 4 {
        for cz in (arm[2] - r) >> 4..=(arm[2] + r) >> 4 {
            out.push((cx, cz));
        }
    }
    out
}

// -------------------------------------------------------- selection (item)

/// Client-side selection held by the Mechanical Arm item
/// (`ArmInteractionPointHandler`).
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub points: Vec<Point>,
}

/// What a right click did, for the status message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClickOutcome {
    pub mode: Mode,
    pub kind: PointKind,
    pub added: bool,
}

/// Result of [`Selection::flush`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flushed {
    pub inputs: usize,
    pub outputs: usize,
    pub removed_out_of_range: usize,
    pub points: Vec<Point>,
}

impl Selection {
    pub fn get(&self, pos: [i32; 3]) -> Option<&Point> {
        self.points.iter().find(|p| p.pos == pos)
    }

    /// Right click on a block: find or create the point, then cycle its mode.
    /// Returns `None` if the block is not interactable (event not cancelled).
    pub fn right_click(&mut self, probe: &BlockProbe, pos: [i32; 3]) -> Option<ClickOutcome> {
        if let Some(i) = self.points.iter().position(|p| p.pos == pos) {
            self.points[i].cycle_mode();
            return Some(ClickOutcome { mode: self.points[i].mode, kind: self.points[i].kind, added: false });
        }
        let mut point = Point::create(probe, pos)?;
        point.cycle_mode();
        self.points.push(point);
        Some(ClickOutcome { mode: point.mode, kind: point.kind, added: true })
    }

    /// Left click removes the point; true cancels the attack.
    pub fn left_click(&mut self, pos: [i32; 3]) -> bool {
        match self.points.iter().position(|p| p.pos == pos) {
            Some(i) => {
                self.points.remove(i);
                true
            }
            None => false,
        }
    }

    /// `flushSettings`: drop points not `closerThan(range)` of the placed arm,
    /// keep selection order.
    pub fn flush(&mut self, arm: [i32; 3], range: i32) -> Flushed {
        let mut out = Flushed::default();
        for p in self.points.drain(..) {
            if !in_range(arm, p.pos, range) {
                out.removed_out_of_range += 1;
                continue;
            }
            if p.mode == Mode::Deposit {
                out.outputs += 1;
            } else {
                out.inputs += 1;
            }
            out.points.push(p);
        }
        out
    }
}

// ------------------------------------------------------------- arm angles

/// `ArmAngleTarget`: the four joint angles in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngleTarget {
    pub lower_arm: f32,
    pub upper_arm: f32,
    pub head: f32,
    pub base: f32,
}

pub const NO_TARGET: AngleTarget = AngleTarget { lower_arm: 135.0, upper_arm: 45.0, head: 0.0, base: 0.0 };

const LOWER: f64 = 14.0 / 16.0;
const UPPER: f64 = 15.0 / 16.0;
const ORIGIN_OFFSET: f64 = 6.0 / 16.0;
const CLAW_OFFSET: f64 = 0.5;
const MIN_REACH: f64 = 1.0 / 8.0;

fn rot_x(v: [f64; 3], deg: f64) -> [f64; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    [v[0], v[1] * c - v[2] * s, v[2] * c + v[1] * s]
}

fn rot_y(v: [f64; 3], deg: f64) -> [f64; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    [v[0] * c + v[2] * s, v[1], v[2] * c - v[0] * s]
}

impl AngleTarget {
    /// Two-link inverse kinematics from the shoulder (6/16 above or below the
    /// arm block centre) to the claw-adjusted target, plus the head angle.
    pub fn solve(arm: [i32; 3], target: [f64; 3], claw_facing: Direction, ceiling: bool) -> Self {
        let centre = [arm[0] as f64 + 0.5, arm[1] as f64 + 0.5, arm[2] as f64 + 0.5];
        let origin = [centre[0], centre[1] + if ceiling { -ORIGIN_OFFSET } else { ORIGIN_OFFSET }, centre[2]];
        let n = claw_facing.scaled(-CLAW_OFFSET);
        let claw_target = target;
        let t = [target[0] + n[0], target[1] + n[1], target[2] + n[2]];
        let mut diff = [t[0] - origin[0], t[1] - origin[1], t[2] - origin[2]];
        let horiz = (diff[0] * diff[0] + diff[2] * diff[2]).sqrt();
        let mut base = diff[0].atan2(diff[2]).to_degrees() + 180.0;
        if ceiling {
            diff[1] = -diff[1];
            base = 180.0 - base;
        }
        let alpha_offset = diff[1].atan2(horiz).to_degrees();
        let d = (diff[1] * diff[1] + horiz * horiz).sqrt().clamp(MIN_REACH, LOWER + UPPER);
        let alpha_ratio = (d * d + LOWER * LOWER - UPPER * UPPER) / (2.0 * LOWER * d);
        let mut alpha = alpha_ratio.acos().to_degrees() + alpha_offset;
        let beta_ratio = (LOWER * LOWER + UPPER * UPPER - d * d) / (2.0 * LOWER * UPPER);
        let mut beta = beta_ratio.acos().to_degrees();
        if alpha.is_nan() {
            alpha = 0.0;
        }
        if beta.is_nan() {
            beta = 0.0;
        }

        // Forward kinematics to the wrist, to aim the head at the real target.
        let mut p = rot_x([0.0, UPPER, 0.0], beta + 180.0);
        p = rot_x([p[0], p[1] + LOWER, p[2]], alpha - 90.0);
        p = rot_y(p, base);
        if ceiling {
            p = rot_x(p, 180.0);
        }
        p = [p[0] + origin[0], p[1] + origin[1], p[2] + origin[2]];
        let mut head_diff = [claw_target[0] - p[0], claw_target[1] - p[1], claw_target[2] - p[2]];
        if ceiling {
            head_diff[1] = -head_diff[1];
        }
        let horiz_head = (head_diff[0] * head_diff[0] + head_diff[2] * head_diff[2]).sqrt();
        let head = alpha + beta + 135.0 - head_diff[1].atan2(horiz_head).to_degrees();
        Self { lower_arm: alpha as f32, upper_arm: beta as f32, head: -head as f32, base: base as f32 }
    }

    /// Wrist position the solver reaches for (testing / debugging aid): the
    /// forward kinematics of the stored angles.
    pub fn wrist(&self, arm: [i32; 3], ceiling: bool) -> [f64; 3] {
        let centre = [arm[0] as f64 + 0.5, arm[1] as f64 + 0.5, arm[2] as f64 + 0.5];
        let oy = if ceiling { -ORIGIN_OFFSET } else { ORIGIN_OFFSET };
        let mut p = rot_x([0.0, UPPER, 0.0], self.upper_arm as f64 + 180.0);
        p = rot_x([p[0], p[1] + LOWER, p[2]], self.lower_arm as f64 - 90.0);
        p = rot_y(p, self.base as f64);
        if ceiling {
            p = rot_x(p, 180.0);
        }
        [p[0] + centre[0], p[1] + centre[1] + oy, p[2] + centre[2]]
    }
}

/// `AngleHelper.getShortestAngleDiff`.
pub fn shortest_angle_diff(current: f32, target: f32) -> f32 {
    let current = current % 360.0;
    let target = target % 360.0;
    (((target - current) % 360.0 + 540.0) % 360.0) - 180.0
}

/// `AngleHelper.angleLerp`.
pub fn angle_lerp(pct: f32, current: f32, target: f32) -> f32 {
    current + shortest_angle_diff(current, target) * pct
}

fn lerp(p: f32, a: f32, b: f32) -> f32 {
    a + p * (b - a)
}

/// Rendered pose (`tickMovementProgress`, client side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub base: f32,
    pub lower_arm: f32,
    pub upper_arm: f32,
    pub head: f32,
}

/// Pose at `progress` (0..=1). The first half retracts from the previous
/// target to the rest pose, the second half extends to the current one.
/// `previous_base` is the base angle the arm held before the move.
pub fn pose(
    progress: f32,
    previous_base: f32,
    previous: Option<AngleTarget>,
    current: Option<AngleTarget>,
) -> Pose {
    let target_base = current.map_or(previous_base, |t| t.base);
    let base = angle_lerp(progress, previous_base, target_base);
    let mut prev = previous.unwrap_or(NO_TARGET);
    let mut cur = current.unwrap_or(NO_TARGET);
    if progress < 0.5 {
        cur = NO_TARGET;
    } else {
        prev = NO_TARGET;
    }
    let local = if progress == 1.0 { 1.0 } else { (progress % 0.5) * 2.0 };
    Pose {
        base,
        lower_arm: lerp(local, prev.lower_arm, cur.lower_arm),
        upper_arm: lerp(local, prev.upper_arm, cur.upper_arm),
        head: angle_lerp(local, prev.head % 360.0, cur.head % 360.0),
    }
}

/// Per-tick progress step for a given kinetic speed.
pub fn progress_step(speed: f32) -> f32 {
    speed.abs().min(MAX_SPEED) / 1024.0
}

// --------------------------------------------------------------- items

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack<I> {
    pub item: I,
    pub count: u32,
}

/// Adapter boundary. `insert` returns the remainder (None when everything
/// fit); `extract` returns what was (or would be) taken.
pub trait ArmWorld<I: Clone + PartialEq> {
    /// `ArmInteractionPoint.isValid`: block still creates a point of its type.
    fn is_valid(&mut self, point: &Point) -> bool;
    fn slot_count(&mut self, point: &Point) -> usize;
    fn extract(&mut self, point: &Point, slot: usize, amount: u32, simulate: bool) -> Option<Stack<I>>;
    fn insert(&mut self, point: &Point, stack: Stack<I>, simulate: bool) -> Option<Stack<I>>;
    /// Called every tick while travelling to an input (belts lock one item).
    fn keep_alive(&mut self, _point: &Point) {}
    /// `JukeboxBlock.HAS_RECORD` of a jukebox point.
    fn jukebox_has_record(&mut self, _point: &Point) -> bool {
        false
    }
}

/// Effects for the adapter to realise.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `SoundEvents.ITEM_PICKUP`, volume 0.125, pitch `pickup_pitch(random)`.
    PickupSound,
    Award(Award),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Award {
    ArmManyTargets,
    ArmBlazeBurner,
    MechanicalArm,
    MusicalArm,
}

/// `.5f + random * .25f`.
pub fn pickup_pitch(random: f32) -> f32 {
    0.5 + random * 0.25
}

// ------------------------------------------------------------------ arm

#[derive(Clone, Debug)]
pub struct Arm<I> {
    pub inputs: Vec<Point>,
    pub outputs: Vec<Point>,
    pub held: Option<Stack<I>>,
    pub phase: Phase,
    /// -1 when none.
    pub chased_index: i32,
    pub progress: f32,
    pub last_input: i32,
    pub last_output: i32,
    pub selection_mode: SelectionMode,
    pub redstone_locked: bool,
    pub goggles: bool,
    pub events: Vec<Event>,
}

impl<I: Clone + PartialEq> Default for Arm<I> {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            held: None,
            phase: Phase::SearchInputs,
            chased_index: 0, // Java leaves the int at 0 until the first selection
            progress: 0.0,
            last_input: -1,
            last_output: -1,
            selection_mode: SelectionMode::RoundRobin,
            redstone_locked: false,
            goggles: false,
            events: Vec::new(),
        }
    }
}

fn point_slot_count<I: Clone + PartialEq>(w: &mut dyn ArmWorld<I>, p: &Point) -> usize {
    p.kind.fixed_slot_count().unwrap_or_else(|| w.slot_count(p))
}

fn point_extract<I: Clone + PartialEq>(
    w: &mut dyn ArmWorld<I>,
    p: &Point,
    slot: usize,
    amount: u32,
    simulate: bool,
) -> Option<Stack<I>> {
    if p.kind.deposit_only() {
        return None;
    }
    w.extract(p, slot, amount, simulate)
}

impl<I: Clone + PartialEq> Arm<I> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install points from the item: `DEPOSIT` goes to outputs, `TAKE` to
    /// inputs, selection order preserved (`initInteractionPoints`). Returns
    /// the awards produced (server side).
    pub fn set_points(&mut self, points: impl IntoIterator<Item = Point>) {
        self.inputs.clear();
        self.outputs.clear();
        for p in points {
            match p.mode {
                Mode::Deposit => self.outputs.push(p),
                Mode::Take => self.inputs.push(p),
            }
        }
        if self.outputs.len() >= MANY_TARGETS {
            self.events.push(Event::Award(Award::ArmManyTargets));
        }
        if self.inputs.iter().chain(&self.outputs).any(|p| p.kind == PointKind::BlazeBurner) {
            self.events.push(Event::Award(Award::ArmBlazeBurner));
        }
    }

    /// `getTargetedInteractionPoint`.
    pub fn targeted(&self) -> Option<&Point> {
        if self.chased_index < 0 {
            return None;
        }
        let i = self.chased_index as usize;
        match self.phase {
            Phase::MoveToInput => self.inputs.get(i),
            Phase::MoveToOutput => self.outputs.get(i),
            _ => None,
        }
    }

    /// `tickMovementProgress` (server part): true on the tick the target is
    /// first reached.
    pub fn tick_movement(&mut self, speed: f32) -> bool {
        let reached_before = self.progress >= 1.0;
        self.progress += progress_step(speed);
        if self.progress > 1.0 {
            self.progress = 1.0;
        }
        !reached_before && self.progress >= 1.0
    }

    /// One server tick of `ArmBlockEntity.tick`. The periodic lazy tick of the
    /// block entity (not just target arrival) should call [`Arm::lazy_tick`].
    pub fn tick(&mut self, speed: f32, world: &mut dyn ArmWorld<I>) {
        let reached = self.tick_movement(speed);
        if self.progress < 1.0 {
            if self.phase == Phase::MoveToInput {
                if let Some(p) = self.targeted().copied() {
                    world.keep_alive(&p);
                }
            }
            return;
        }
        match self.phase {
            Phase::MoveToInput => self.collect_item(world),
            Phase::MoveToOutput => self.deposit_item(world),
            Phase::SearchInputs | Phase::Dancing => self.search_for_item(world),
            Phase::SearchOutputs => {}
        }
        if reached {
            self.lazy_tick(world);
        }
    }

    /// `lazyTick` (server).
    pub fn lazy_tick(&mut self, world: &mut dyn ArmWorld<I>) {
        if self.progress < 0.5 {
            return;
        }
        match self.phase {
            Phase::SearchInputs | Phase::Dancing => self.check_for_music(world),
            Phase::SearchOutputs => self.search_for_destination(world),
            _ => {}
        }
    }

    fn check_for_music(&mut self, world: &mut dyn ArmWorld<I>) {
        let mut music = false;
        for p in self.inputs.iter().chain(self.outputs.iter()) {
            if p.kind == PointKind::Jukebox && world.jukebox_has_record(p) {
                music = true;
                break;
            }
        }
        if music != (self.phase == Phase::Dancing) {
            self.phase = if music { Phase::Dancing } else { Phase::SearchInputs };
        }
    }

    /// `redstoneUpdate`: returns true if the lock state changed.
    pub fn redstone_update(&mut self, powered: bool, world: &mut dyn ArmWorld<I>) -> bool {
        if powered == self.redstone_locked {
            return false;
        }
        self.redstone_locked = powered;
        if !powered {
            self.search_for_item(world);
        }
        true
    }

    fn select_index(&mut self, input: bool, index: usize) {
        self.phase = if input { Phase::MoveToInput } else { Phase::MoveToOutput };
        self.chased_index = index as i32;
        self.progress = 0.0;
        if input {
            self.last_input = index as i32;
        } else {
            self.last_output = index as i32;
        }
    }

    /// Start index and exclusive scan limit for a search.
    fn scan_window(&self, last: i32, len: usize) -> (usize, usize) {
        let start = if self.selection_mode == SelectionMode::PreferFirst { 0 } else { (last + 1) as usize };
        let mut range = if self.selection_mode == SelectionMode::ForcedRoundRobin {
            (last + 2) as usize
        } else {
            len
        };
        if range > len {
            range = len;
        }
        (start, range)
    }

    /// `getDistributableAmount`.
    fn distributable(&mut self, world: &mut dyn ArmWorld<I>, point: &Point, slot: usize) -> u32 {
        let Some(stack) = point_extract(world, point, slot, DEFAULT_EXTRACT_AMOUNT, true) else {
            return 0;
        };
        let count = stack.count;
        let remainder = self.simulate_insertion(world, Some(stack.clone()));
        match remainder {
            Some(r) if r.item == stack.item => count - r.count.min(count),
            _ => count,
        }
    }

    fn simulate_insertion(&mut self, world: &mut dyn ArmWorld<I>, mut stack: Option<Stack<I>>) -> Option<Stack<I>> {
        let outputs = self.outputs.clone();
        for p in &outputs {
            if let Some(s) = stack.take() {
                stack = if world.is_valid(p) { world.insert(p, s, true) } else { Some(s) };
            }
            if stack.is_none() {
                break;
            }
        }
        stack
    }

    pub fn search_for_item(&mut self, world: &mut dyn ArmWorld<I>) {
        if self.redstone_locked {
            return;
        }
        let mut found = false;
        let (start, range) = self.scan_window(self.last_input, self.inputs.len());
        'points: for i in start..range {
            let p = self.inputs[i];
            if !world.is_valid(&p) {
                continue;
            }
            for slot in 0..point_slot_count(world, &p) {
                if self.distributable(world, &p, slot) > 0 {
                    self.select_index(true, i);
                    found = true;
                    break 'points;
                }
            }
        }
        if !found && self.selection_mode == SelectionMode::RoundRobin {
            self.last_input = -1;
        }
        if self.last_input == self.inputs.len() as i32 - 1 {
            self.last_input = -1;
        }
    }

    pub fn search_for_destination(&mut self, world: &mut dyn ArmWorld<I>) {
        let held = self.held.clone();
        let mut found = false;
        let (start, range) = self.scan_window(self.last_output, self.outputs.len());
        for i in start..range {
            let p = self.outputs[i];
            if !world.is_valid(&p) {
                continue;
            }
            let Some(h) = held.clone() else { continue };
            let remainder = world.insert(&p, h, true);
            if remainder == held {
                continue; // ItemStack.matches(remainder, heldItem)
            }
            self.select_index(false, i);
            found = true;
            break;
        }
        if !found && self.selection_mode == SelectionMode::RoundRobin {
            self.last_output = -1;
        }
        if self.last_output == self.outputs.len() as i32 - 1 {
            self.last_output = -1;
        }
    }

    fn collect_item(&mut self, world: &mut dyn ArmWorld<I>) {
        if let Some(p) = self.targeted().copied() {
            if world.is_valid(&p) {
                for slot in 0..point_slot_count(world, &p) {
                    let amount = self.distributable(world, &p, slot);
                    if amount == 0 {
                        continue;
                    }
                    let previous = self.held.clone();
                    self.held = point_extract(world, &p, slot, amount, false);
                    self.phase = Phase::SearchOutputs;
                    self.progress = 0.0;
                    self.chased_index = -1;
                    if previous.as_ref().map(|s| &s.item) != self.held.as_ref().map(|s| &s.item) {
                        self.events.push(Event::PickupSound);
                    }
                    return;
                }
            }
        }
        self.phase = Phase::SearchInputs;
        self.progress = 0.0;
        self.chased_index = -1;
    }

    fn deposit_item(&mut self, world: &mut dyn ArmWorld<I>) {
        if let Some(p) = self.targeted().copied() {
            if world.is_valid(&p) {
                if let Some(h) = self.held.take() {
                    let remainder = world.insert(&p, h, false);
                    if p.kind == PointKind::Jukebox && remainder.is_none() {
                        self.events.push(Event::Award(Award::MusicalArm));
                    }
                    self.held = remainder;
                }
            }
        }
        self.phase = if self.held.is_none() { Phase::SearchInputs } else { Phase::SearchOutputs };
        self.progress = 0.0;
        self.chased_index = -1;
        self.events.push(Event::Award(Award::MechanicalArm));
    }

    /// Empty-hand use on the block: hand the held stack to the player and
    /// restart the search (`ArmBlock.onBlockEntityUse`).
    pub fn take_held(&mut self) -> Option<Stack<I>> {
        let held = self.held.take()?;
        self.phase = Phase::SearchInputs;
        Some(held)
    }

    /// Goggles use: sets the flag and reports whether it was newly set.
    pub fn apply_goggles(&mut self) -> bool {
        let changed = !self.goggles;
        self.goggles = true;
        changed
    }

    /// Dismantle / destroy: the held stack is dropped.
    pub fn destroy(&mut self) -> Option<Stack<I>> {
        self.held.take()
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

// ------------------------------------------- per-kind insertion rules

/// `FunnelPoint.insert` preconditions, in Java order.
pub fn funnel_accepts(powered: bool, has_inserter: bool, filter_accepts: bool) -> bool {
    !powered && has_inserter && filter_accepts
}

/// `CampfirePoint.insert`: needs a campfire recipe and a free cooking slot;
/// one item is consumed. Returns the remainder count.
pub fn campfire_insert(count: u32, has_recipe: bool, free_slot: bool) -> u32 {
    if count == 0 || !has_recipe || !free_slot { count } else { count - 1 }
}

/// `RespawnAnchorPoint.insert`: glowstone only, below charge 4; one item used.
/// A missing CHARGE property reads as 4 (reject).
pub fn respawn_anchor_insert(is_glowstone: bool, charge: Option<u32>, count: u32) -> u32 {
    if !is_glowstone || charge.unwrap_or(4) >= 4 || count == 0 { count } else { count - 1 }
}

/// `JukeboxPoint.insert`: playable record, no record flag (default true when
/// missing), empty block entity; one item moves.
pub fn jukebox_insert(playable: bool, has_record: Option<bool>, be_empty: bool, count: u32) -> u32 {
    if !playable || has_record.unwrap_or(true) || !be_empty || count == 0 { count } else { count - 1 }
}

/// `JukeboxPoint.extract` precondition (missing HAS_RECORD reads false).
pub fn jukebox_can_extract(has_record: Option<bool>, be_present: bool) -> bool {
    has_record.unwrap_or(false) && be_present
}

/// `BeltPoint.keepAlive`: only the first transported item is locked.
pub fn lock_first(locked: &mut [bool]) {
    if let Some(first) = locked.first_mut() {
        *first = true;
    }
}

/// Composter handler sides: insert from above, extract from below.
pub const COMPOSTER_INSERT_SIDE: Direction = Direction::Up;
pub const COMPOSTER_EXTRACT_SIDE: Direction = Direction::Down;

/// Distinct chunk-set helper kept for adapters that cache validity.
pub fn unique_positions(points: &[Point]) -> usize {
    points.iter().map(|p| p.pos).collect::<HashSet<_>>().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    type Item = &'static str;

    #[derive(Default)]
    struct Container {
        slots: Vec<Option<Stack<Item>>>,
        /// Accepts nothing when false.
        accepts: bool,
        valid: bool,
    }

    #[derive(Default)]
    struct World {
        blocks: HashMap<[i32; 3], Container>,
        record: bool,
        keep_alive_calls: u32,
    }

    impl World {
        fn chest(&mut self, pos: [i32; 3], slots: &[Option<(Item, u32)>]) {
            self.blocks.insert(
                pos,
                Container {
                    slots: slots.iter().map(|s| s.map(|(item, count)| Stack { item, count })).collect(),
                    accepts: true,
                    valid: true,
                },
            );
        }

        fn count(&self, pos: [i32; 3]) -> u32 {
            self.blocks[&pos].slots.iter().flatten().map(|s| s.count).sum()
        }
    }

    impl ArmWorld<Item> for World {
        fn is_valid(&mut self, p: &Point) -> bool {
            self.blocks.get(&p.pos).is_some_and(|c| c.valid)
        }
        fn slot_count(&mut self, p: &Point) -> usize {
            self.blocks.get(&p.pos).map_or(0, |c| c.slots.len())
        }
        fn extract(&mut self, p: &Point, slot: usize, amount: u32, simulate: bool) -> Option<Stack<Item>> {
            let c = self.blocks.get_mut(&p.pos)?;
            let s = c.slots[slot].as_mut()?;
            let n = s.count.min(amount);
            let out = Stack { item: s.item, count: n };
            if !simulate {
                s.count -= n;
                if s.count == 0 {
                    c.slots[slot] = None;
                }
            }
            Some(out)
        }
        fn insert(&mut self, p: &Point, mut stack: Stack<Item>, simulate: bool) -> Option<Stack<Item>> {
            let c = self.blocks.get_mut(&p.pos)?;
            if !c.accepts {
                return Some(stack);
            }
            // ItemHandlerHelper.insertItem: merge into matching, then empty slots.
            for pass in 0..2 {
                for slot in c.slots.iter_mut() {
                    if stack.count == 0 {
                        break;
                    }
                    match slot {
                        Some(s) if pass == 0 && s.item == stack.item => {
                            let n = (64 - s.count).min(stack.count);
                            if !simulate {
                                s.count += n;
                            }
                            stack.count -= n;
                        }
                        None if pass == 1 => {
                            let n = 64.min(stack.count);
                            if !simulate {
                                *slot = Some(Stack { item: stack.item, count: n });
                            }
                            stack.count -= n;
                        }
                        _ => {}
                    }
                }
            }
            if stack.count == 0 { None } else { Some(stack) }
        }
        fn keep_alive(&mut self, _p: &Point) {
            self.keep_alive_calls += 1;
        }
        fn jukebox_has_record(&mut self, _p: &Point) -> bool {
            self.record
        }
    }

    fn pt(kind: PointKind, pos: [i32; 3], mode: Mode) -> Point {
        Point { kind, pos, mode }
    }

    fn arm_with(inputs: &[[i32; 3]], outputs: &[[i32; 3]]) -> Arm<Item> {
        let mut arm = Arm::new();
        arm.set_points(
            inputs
                .iter()
                .map(|p| pt(PointKind::Depot, *p, Mode::Take))
                .chain(outputs.iter().map(|p| pt(PointKind::Depot, *p, Mode::Deposit))),
        );
        arm
    }

    // ---------------------------------------------------------- types

    #[test]
    fn registered_kinds_and_names_roundtrip() {
        assert_eq!(ALL_KINDS.len(), 17);
        for k in ALL_KINDS {
            assert_eq!(PointKind::from_name(&format!("create:{}", k.register_name())), Some(k));
        }
        assert_eq!(PointKind::from_name("create:harvester"), None);
        assert_eq!(PointKind::from_name("create:track_station"), None);
    }

    #[test]
    fn can_create_rules_follow_the_java_conditions() {
        let belt = BlockProbe { belt: true, belt_can_transport_objects: true, ..Default::default() };
        assert_eq!(primary_type(&belt), Some(PointKind::Belt));
        let tunnel = BlockProbe { belt_tunnel_above: true, ..belt.clone() };
        assert_eq!(primary_type(&tunnel), None);
        let no_transport = BlockProbe { belt_can_transport_objects: false, ..belt };
        assert_eq!(primary_type(&no_transport), None);

        let funnel = BlockProbe { funnel: true, ..Default::default() };
        assert_eq!(primary_type(&funnel), Some(PointKind::Funnel));
        assert_eq!(primary_type(&BlockProbe { funnel_extracting: true, ..funnel.clone() }), None);
        assert_eq!(primary_type(&BlockProbe { belt_funnel_pushing: true, ..funnel }), None);

        let saw = BlockProbe { saw: true, saw_facing_up: true, saw_speed: Some(16.0), ..Default::default() };
        assert_eq!(primary_type(&saw), Some(PointKind::Saw));
        assert_eq!(primary_type(&BlockProbe { saw_speed: Some(0.0), ..saw.clone() }), None);
        assert_eq!(primary_type(&BlockProbe { saw_speed: None, ..saw.clone() }), None);
        assert_eq!(primary_type(&BlockProbe { saw_facing_up: false, ..saw }), None);
        assert_eq!(primary_type(&BlockProbe { saw: true, saw_facing_up: true, saw_speed: Some(-8.0), ..Default::default() }), Some(PointKind::Saw));

        for (probe, kind) in [
            (BlockProbe { weighted_ejector: true, ..Default::default() }, PointKind::Depot),
            (BlockProbe { track_station: true, ..Default::default() }, PointKind::Depot),
            (BlockProbe { repackager: true, ..Default::default() }, PointKind::Packager),
            (BlockProbe { basin: true, ..Default::default() }, PointKind::Basin),
            (BlockProbe { jukebox: true, ..Default::default() }, PointKind::Jukebox),
            (BlockProbe { respawn_anchor: true, ..Default::default() }, PointKind::RespawnAnchor),
        ] {
            assert_eq!(primary_type(&probe), Some(kind));
        }
        assert_eq!(primary_type(&BlockProbe::default()), None);
    }

    #[test]
    fn registration_order_decides_overlapping_blocks() {
        let both = BlockProbe { basin: true, depot: true, ..Default::default() };
        assert_eq!(primary_type(&both), Some(PointKind::Basin));
    }

    #[test]
    fn deposit_only_and_slot_counts() {
        for k in [PointKind::BlazeBurner, PointKind::Funnel, PointKind::Campfire, PointKind::RespawnAnchor, PointKind::CrushingWheels] {
            assert!(k.deposit_only());
            assert_eq!(k.fixed_slot_count(), Some(0));
            let mut p = pt(k, [0, 0, 0], Mode::Deposit);
            p.cycle_mode();
            assert_eq!(p.mode, Mode::Deposit);
        }
        assert!(!PointKind::Jukebox.deposit_only());
        assert_eq!(PointKind::Jukebox.fixed_slot_count(), Some(1));
        assert_eq!(PointKind::Composter.fixed_slot_count(), Some(2));
        assert_eq!(PointKind::Depot.fixed_slot_count(), None);
        let mut jb = pt(PointKind::Jukebox, [0, 0, 0], Mode::Deposit);
        jb.cycle_mode();
        assert_eq!(jb.mode, Mode::Take);
        assert!(PointKind::Crafter.needs_extraction_window());
    }

    #[test]
    fn target_vectors_match_the_java_offsets() {
        let p = [10, 20, 30];
        let t = |k: PointKind, f| k.target_vector(p, f);
        assert_eq!(t(PointKind::Basin, None), [10.5, 20.5, 30.5]);
        assert_eq!(t(PointKind::Depot, None), [10.5, 20.875, 30.5]);
        assert_eq!(t(PointKind::Belt, None), [10.5, 20.875, 30.5]);
        assert_eq!(t(PointKind::Saw, None), [10.5, 20.875, 30.5]);
        assert_eq!(t(PointKind::Chute, None), [10.5, 21.0, 30.5]);
        assert_eq!(t(PointKind::Jukebox, None), [10.5, 21.0, 30.5]);
        assert_eq!(t(PointKind::Composter, None), [10.5, 20.8125, 30.5]);
        // Deployer faces `facing`; the claw comes from the opposite side.
        assert_eq!(t(PointKind::Deployer, Some(Direction::North)), [10.5, 20.5, 31.15]);
        assert_eq!(PointKind::Deployer.interaction_direction(Some(Direction::North)), Direction::South);
        assert_eq!(PointKind::Deployer.interaction_direction(None), Direction::Down);
        assert_eq!(t(PointKind::Crafter, Some(Direction::East)), [10.0, 20.5, 30.5]);
        assert_eq!(PointKind::Crafter.interaction_direction(None), Direction::North);
        let f = t(PointKind::Funnel, Some(Direction::East));
        assert!((f[0] - 10.35).abs() < 1e-9 && f[1] == 20.5);
        assert_eq!(PointKind::Funnel.interaction_direction(Some(Direction::East)), Direction::West);
        assert_eq!(PointKind::Funnel.interaction_direction(None), Direction::Up);
        assert_eq!(PointKind::Basin.interaction_direction(None), Direction::Down);
    }

    // ------------------------------------------------------ geometry

    #[test]
    fn no_target_is_the_rest_pose() {
        assert_eq!(NO_TARGET, AngleTarget { lower_arm: 135.0, upper_arm: 45.0, head: 0.0, base: 0.0 });
    }

    #[test]
    fn solver_wrist_reaches_reachable_targets_floor_and_ceiling() {
        let arm = [0, 0, 0];
        for ceiling in [false, true] {
            let sign = if ceiling { -1.0 } else { 1.0 };
            for (dx, dy, dz) in [(1.0, 0.0, 0.0), (0.0, 0.2, 1.2), (-1.0, -0.3, -0.6), (0.9, 0.5, -0.9)] {
                // Claw target (facing down) sits 0.5 above the wrist.
                let origin = [0.5, 0.5 + sign * 0.375, 0.5];
                let wrist_goal = [origin[0] + dx, origin[1] + sign * dy, origin[2] + dz];
                let target = [wrist_goal[0], wrist_goal[1] - 0.5, wrist_goal[2]];
                let a = AngleTarget::solve(arm, target, Direction::Down, ceiling);
                let w = a.wrist(arm, ceiling);
                for i in 0..3 {
                    assert!((w[i] - wrist_goal[i]).abs() < 1e-3, "ceiling={ceiling} axis {i}: {w:?} vs {wrist_goal:?}");
                }
            }
        }
    }

    #[test]
    fn base_angle_follows_atan2_plus_half_turn() {
        let target = |x: f64, z: f64| AngleTarget::solve([0, 0, 0], [x, 0.5, z], Direction::Down, false).base;
        assert!((target(0.5, 3.5) - 180.0).abs() < 1e-4); // +z
        assert!((target(3.5, 0.5) - 270.0).abs() < 1e-4); // +x
        assert!((shortest_angle_diff(target(-2.5, 0.5), 90.0)).abs() < 1e-3); // -x
        let ceil = AngleTarget::solve([0, 0, 0], [0.5, 0.5, 3.5], Direction::Down, true).base;
        assert!((ceil - 0.0).abs() < 1e-4);
    }

    #[test]
    fn unreachable_targets_clamp_to_full_extension() {
        let a = AngleTarget::solve([0, 0, 0], [0.5, 0.875, 40.5], Direction::Down, false);
        // d clamps to a + b, so the law of cosines gives a fully straight arm.
        assert!((a.upper_arm - 180.0).abs() < 1e-3, "got {}", a.upper_arm);
        assert!(a.lower_arm.abs() < 1.0 + 2.0, "arm points almost level, got {}", a.lower_arm);
        // A target at the shoulder itself clamps to the minimum reach and stays finite.
        let b = AngleTarget::solve([0, 0, 0], [0.5, 0.875 + 0.5, 0.5], Direction::Down, false);
        assert!(b.lower_arm.is_finite() && b.upper_arm.is_finite() && b.head.is_finite());
    }

    #[test]
    fn angle_helpers_take_the_short_way() {
        assert_eq!(shortest_angle_diff(350.0, 10.0), 20.0);
        assert_eq!(shortest_angle_diff(10.0, 350.0), -20.0);
        assert_eq!(angle_lerp(0.5, 350.0, 10.0), 360.0);
        assert_eq!(angle_lerp(0.0, 90.0, 270.0), 90.0);
    }

    #[test]
    fn pose_retracts_then_extends() {
        let prev = AngleTarget { lower_arm: 60.0, upper_arm: 100.0, head: 10.0, base: 90.0 };
        let cur = AngleTarget { lower_arm: 20.0, upper_arm: 80.0, head: 30.0, base: 180.0 };
        let start = pose(0.0, 90.0, Some(prev), Some(cur));
        assert_eq!((start.lower_arm, start.upper_arm, start.base), (60.0, 100.0, 90.0));
        let mid = pose(0.25, 90.0, Some(prev), Some(cur));
        // half way (local 0.5) between prev and the rest pose
        assert_eq!(mid.lower_arm, 97.5);
        assert_eq!(mid.upper_arm, 72.5);
        let end = pose(1.0, 90.0, Some(prev), Some(cur));
        assert_eq!((end.lower_arm, end.upper_arm, end.head, end.base), (20.0, 80.0, 30.0, 180.0));
        let rest = pose(0.5, 90.0, Some(prev), Some(cur));
        assert_eq!((rest.lower_arm, rest.upper_arm), (135.0, 45.0));
        // no target: stays at the previous base angle
        assert_eq!(pose(0.7, 123.0, None, None).base, 123.0);
    }

    // ----------------------------------------------------- movement

    #[test]
    fn progress_speed_is_capped_at_256_and_ignores_sign() {
        assert_eq!(progress_step(256.0), 0.25);
        assert_eq!(progress_step(-256.0), 0.25);
        assert_eq!(progress_step(512.0), 0.25);
        assert_eq!(progress_step(16.0), 0.015625);
        assert_eq!(progress_step(0.0), 0.0);
        let mut arm = Arm::<Item>::new();
        let mut reached = Vec::new();
        for _ in 0..5 {
            reached.push(arm.tick_movement(256.0));
        }
        assert_eq!(reached, [false, false, false, true, false]);
        assert_eq!(arm.progress, 1.0);
    }

    #[test]
    fn slow_arm_takes_proportionally_longer() {
        let mut arm = Arm::<Item>::new();
        let mut ticks = 0;
        while !arm.tick_movement(32.0) {
            ticks += 1;
        }
        assert_eq!(ticks + 1, 32);
    }

    // ------------------------------------------------ selection modes

    fn three_inputs() -> (Arm<Item>, World) {
        let mut w = World::default();
        for x in 0..3 {
            w.chest([x, 0, 0], &[Some(("wheat", 5))]);
        }
        w.chest([10, 0, 0], &[None, None]);
        let mut arm = arm_with(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]], &[[10, 0, 0]]);
        arm.progress = 1.0;
        (arm, w)
    }

    #[test]
    fn round_robin_cycles_inputs_and_wraps() {
        let (mut arm, mut w) = three_inputs();
        let mut order = Vec::new();
        for _ in 0..4 {
            arm.phase = Phase::SearchInputs;
            arm.search_for_item(&mut w);
            order.push(arm.chased_index);
        }
        assert_eq!(order, [0, 1, 2, 0]);
        // After picking the last input the index wraps immediately.
        arm.phase = Phase::SearchInputs;
        arm.last_input = 1;
        arm.search_for_item(&mut w);
        assert_eq!((arm.chased_index, arm.last_input), (2, -1));
    }

    #[test]
    fn prefer_first_always_starts_at_zero() {
        let (mut arm, mut w) = three_inputs();
        arm.selection_mode = SelectionMode::PreferFirst;
        for _ in 0..3 {
            arm.phase = Phase::SearchInputs;
            arm.search_for_item(&mut w);
            assert_eq!(arm.chased_index, 0);
        }
        // empty first input: falls through to the second
        w.blocks.get_mut(&[0, 0, 0]).unwrap().slots[0] = None;
        arm.phase = Phase::SearchInputs;
        arm.search_for_item(&mut w);
        assert_eq!(arm.chased_index, 1);
    }

    #[test]
    fn forced_round_robin_only_looks_at_the_next_input() {
        let (mut arm, mut w) = three_inputs();
        arm.selection_mode = SelectionMode::ForcedRoundRobin;
        w.blocks.get_mut(&[1, 0, 0]).unwrap().slots[0] = None;
        arm.search_for_item(&mut w);
        assert_eq!(arm.chased_index, 0);
        // next is input 1 which is empty: nothing found even though 2 has items
        arm.phase = Phase::SearchInputs;
        arm.search_for_item(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs);
        assert_eq!(arm.last_input, 0, "forced mode keeps its position");
        // restore input 1 and it proceeds
        w.chest([1, 0, 0], &[Some(("wheat", 5))]);
        arm.search_for_item(&mut w);
        assert_eq!(arm.chased_index, 1);
    }

    #[test]
    fn round_robin_resets_when_nothing_found() {
        let (mut arm, mut w) = three_inputs();
        arm.last_input = 1;
        for x in 0..3 {
            w.blocks.get_mut(&[x, 0, 0]).unwrap().slots[0] = None;
        }
        arm.search_for_item(&mut w);
        assert_eq!(arm.last_input, -1);
        assert_eq!(arm.phase, Phase::SearchInputs);
    }

    #[test]
    fn no_inputs_is_harmless() {
        let mut arm = Arm::<Item>::new();
        let mut w = World::default();
        arm.last_input = -1;
        arm.search_for_item(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs);
        assert_eq!(arm.last_input, -1);
    }

    #[test]
    fn invalid_inputs_are_skipped() {
        let (mut arm, mut w) = three_inputs();
        w.blocks.get_mut(&[0, 0, 0]).unwrap().valid = false;
        arm.search_for_item(&mut w);
        assert_eq!(arm.chased_index, 1);
    }

    #[test]
    fn destination_search_skips_outputs_that_accept_nothing() {
        let mut w = World::default();
        w.chest([5, 0, 0], &[Some(("stone", 64))]);
        w.chest([6, 0, 0], &[None]);
        w.chest([7, 0, 0], &[None]);
        let mut arm = arm_with(&[], &[[5, 0, 0], [6, 0, 0], [7, 0, 0]]);
        arm.held = Some(Stack { item: "wheat", count: 3 });
        arm.phase = Phase::SearchOutputs;
        arm.progress = 1.0;
        w.blocks.get_mut(&[5, 0, 0]).unwrap().accepts = false;
        arm.search_for_destination(&mut w);
        assert_eq!((arm.phase, arm.chased_index), (Phase::MoveToOutput, 1));
        assert_eq!(arm.last_output, 1);
        // second pass starts after the last used output
        arm.phase = Phase::SearchOutputs;
        arm.search_for_destination(&mut w);
        assert_eq!(arm.chased_index, 2);
        assert_eq!(arm.last_output, -1, "last output wraps to -1");
    }

    #[test]
    fn destination_accepts_partial_insertion() {
        let mut w = World::default();
        w.chest([5, 0, 0], &[Some(("wheat", 63))]);
        let mut arm = arm_with(&[], &[[5, 0, 0]]);
        arm.held = Some(Stack { item: "wheat", count: 10 });
        arm.phase = Phase::SearchOutputs;
        arm.search_for_destination(&mut w);
        assert_eq!(arm.phase, Phase::MoveToOutput);
        // full chest: remainder equals the held stack, skip
        let mut w = World::default();
        w.chest([5, 0, 0], &[Some(("wheat", 64))]);
        let mut arm = arm_with(&[], &[[5, 0, 0]]);
        arm.held = Some(Stack { item: "wheat", count: 10 });
        arm.phase = Phase::SearchOutputs;
        arm.search_for_destination(&mut w);
        assert_eq!(arm.phase, Phase::SearchOutputs);
    }

    // -------------------------------------------- distributable amount

    #[test]
    fn collection_takes_only_what_the_outputs_can_hold() {
        let mut w = World::default();
        w.chest([0, 0, 0], &[Some(("wheat", 40))]);
        w.chest([9, 0, 0], &[Some(("wheat", 50))]); // 14 free
        let mut arm = arm_with(&[[0, 0, 0]], &[[9, 0, 0]]);
        arm.progress = 1.0;
        arm.search_for_item(&mut w);
        assert_eq!(arm.phase, Phase::MoveToInput);
        arm.progress = 1.0;
        arm.collect_item(&mut w);
        assert_eq!(arm.held, Some(Stack { item: "wheat", count: 14 }));
        assert_eq!(w.count([0, 0, 0]), 26);
        assert_eq!((arm.phase, arm.chased_index, arm.progress), (Phase::SearchOutputs, -1, 0.0));
        assert_eq!(arm.drain_events(), [Event::PickupSound]);
    }

    #[test]
    fn nothing_distributable_means_no_pickup() {
        let mut w = World::default();
        w.chest([0, 0, 0], &[Some(("wheat", 4))]);
        w.chest([9, 0, 0], &[Some(("wheat", 64))]);
        let mut arm = arm_with(&[[0, 0, 0]], &[[9, 0, 0]]);
        arm.progress = 1.0;
        arm.search_for_item(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs);
        // If the input empties while travelling the arm returns to searching.
        arm.phase = Phase::MoveToInput;
        arm.chased_index = 0;
        arm.collect_item(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs);
        assert!(arm.held.is_none());
        assert_eq!(w.count([0, 0, 0]), 4);
    }

    #[test]
    fn first_slot_with_distributable_items_wins() {
        let mut w = World::default();
        w.chest([0, 0, 0], &[Some(("stone", 3)), None, Some(("wheat", 7))]);
        w.chest([9, 0, 0], &[None]);
        let mut arm = arm_with(&[[0, 0, 0]], &[[9, 0, 0]]);
        arm.phase = Phase::MoveToInput;
        arm.chased_index = 0;
        arm.collect_item(&mut w);
        assert_eq!(arm.held, Some(Stack { item: "stone", count: 3 }));
    }

    #[test]
    fn pickup_sound_only_when_the_item_changes() {
        let mut w = World::default();
        w.chest([0, 0, 0], &[Some(("wheat", 4))]);
        w.chest([9, 0, 0], &[None]);
        let mut arm = arm_with(&[[0, 0, 0]], &[[9, 0, 0]]);
        arm.held = Some(Stack { item: "wheat", count: 1 });
        arm.phase = Phase::MoveToInput;
        arm.chased_index = 0;
        arm.collect_item(&mut w);
        assert!(arm.drain_events().is_empty());
        assert_eq!(pickup_pitch(0.0), 0.5);
        assert_eq!(pickup_pitch(1.0), 0.75);
    }

    // ----------------------------------------------------- full cycle

    #[test]
    fn full_cycle_moves_items_and_counts_ticks() {
        let mut w = World::default();
        w.chest([0, 0, 0], &[Some(("wheat", 20))]);
        w.chest([3, 0, 0], &[None]);
        let mut arm = arm_with(&[[0, 0, 0]], &[[3, 0, 0]]);
        arm.progress = 1.0;
        let mut trace = Vec::new();
        for _ in 0..40 {
            arm.tick(256.0, &mut w);
            if trace.last() != Some(&arm.phase) {
                trace.push(arm.phase);
            }
            if w.count([3, 0, 0]) == 20 && arm.phase == Phase::SearchInputs {
                break;
            }
        }
        assert_eq!(w.count([3, 0, 0]), 20);
        assert_eq!(w.count([0, 0, 0]), 0);
        assert_eq!(
            trace,
            [Phase::MoveToInput, Phase::SearchOutputs, Phase::MoveToOutput, Phase::SearchInputs]
        );
        let ev = arm.drain_events();
        assert!(ev.contains(&Event::Award(Award::MechanicalArm)));
        assert!(w.keep_alive_calls > 0);
    }

    #[test]
    fn arm_that_cannot_unload_keeps_holding_and_retries() {
        let mut w = World::default();
        w.chest([3, 0, 0], &[None]);
        let mut arm = arm_with(&[], &[[3, 0, 0]]);
        arm.held = Some(Stack { item: "wheat", count: 5 });
        arm.phase = Phase::MoveToOutput;
        arm.chased_index = 0;
        w.blocks.get_mut(&[3, 0, 0]).unwrap().accepts = false;
        arm.deposit_item(&mut w);
        assert_eq!(arm.held, Some(Stack { item: "wheat", count: 5 }));
        assert_eq!(arm.phase, Phase::SearchOutputs);
        assert_eq!(arm.chased_index, -1);
    }

    #[test]
    fn deposit_into_invalid_output_leaves_the_stack() {
        let mut w = World::default();
        w.chest([3, 0, 0], &[None]);
        w.blocks.get_mut(&[3, 0, 0]).unwrap().valid = false;
        let mut arm = arm_with(&[], &[[3, 0, 0]]);
        arm.held = Some(Stack { item: "wheat", count: 5 });
        arm.phase = Phase::MoveToOutput;
        arm.chased_index = 0;
        arm.deposit_item(&mut w);
        assert_eq!(arm.held.as_ref().unwrap().count, 5);
        assert_eq!(arm.phase, Phase::SearchOutputs);
    }

    #[test]
    fn search_outputs_waits_for_half_progress() {
        let mut w = World::default();
        w.chest([3, 0, 0], &[None]);
        let mut arm = arm_with(&[], &[[3, 0, 0]]);
        arm.held = Some(Stack { item: "wheat", count: 5 });
        arm.phase = Phase::SearchOutputs;
        arm.progress = 0.25;
        arm.lazy_tick(&mut w);
        assert_eq!(arm.phase, Phase::SearchOutputs);
        arm.progress = 0.5;
        arm.lazy_tick(&mut w);
        assert_eq!(arm.phase, Phase::MoveToOutput);
    }

    // ------------------------------------------------------ redstone

    #[test]
    fn redstone_locks_searching_but_not_ongoing_moves() {
        let (mut arm, mut w) = three_inputs();
        assert!(arm.redstone_update(true, &mut w));
        assert!(!arm.redstone_update(true, &mut w), "no change, no effect");
        arm.search_for_item(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs);
        // a move already underway still completes
        arm.phase = Phase::MoveToInput;
        arm.chased_index = 0;
        arm.collect_item(&mut w);
        assert_eq!(arm.phase, Phase::SearchOutputs);
        // releasing the lock triggers a search straight away
        let (mut arm, mut w) = three_inputs();
        arm.redstone_locked = true;
        assert!(arm.redstone_update(false, &mut w));
        assert_eq!(arm.phase, Phase::MoveToInput);
    }

    // --------------------------------------------------------- music

    #[test]
    fn jukebox_with_record_makes_the_arm_dance() {
        let mut w = World::default();
        let mut arm = Arm::<Item>::new();
        arm.set_points([pt(PointKind::Jukebox, [1, 0, 0], Mode::Deposit)]);
        arm.progress = 1.0;
        arm.lazy_tick(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs);
        w.record = true;
        arm.lazy_tick(&mut w);
        assert_eq!(arm.phase, Phase::Dancing);
        arm.lazy_tick(&mut w);
        assert_eq!(arm.phase, Phase::Dancing);
        w.record = false;
        arm.lazy_tick(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs);
        arm.progress = 0.4;
        w.record = true;
        arm.lazy_tick(&mut w);
        assert_eq!(arm.phase, Phase::SearchInputs, "below half progress nothing happens");
    }

    #[test]
    fn dancing_arm_still_searches_for_items() {
        let (mut arm, mut w) = three_inputs();
        arm.phase = Phase::Dancing;
        arm.tick(256.0, &mut w);
        assert_eq!(arm.phase, Phase::MoveToInput);
    }

    #[test]
    fn musical_arm_award_needs_an_empty_hand_after_jukebox_deposit() {
        struct Jb;
        impl ArmWorld<Item> for Jb {
            fn is_valid(&mut self, _: &Point) -> bool { true }
            fn slot_count(&mut self, _: &Point) -> usize { 1 }
            fn extract(&mut self, _: &Point, _: usize, _: u32, _: bool) -> Option<Stack<Item>> { None }
            fn insert(&mut self, _: &Point, mut s: Stack<Item>, _: bool) -> Option<Stack<Item>> {
                s.count -= 1;
                if s.count == 0 { None } else { Some(s) }
            }
        }
        let mut arm = Arm::<Item>::new();
        arm.set_points([pt(PointKind::Jukebox, [1, 0, 0], Mode::Deposit)]);
        arm.held = Some(Stack { item: "disc", count: 1 });
        arm.phase = Phase::MoveToOutput;
        arm.chased_index = 0;
        arm.deposit_item(&mut Jb);
        assert!(arm.drain_events().contains(&Event::Award(Award::MusicalArm)));
        arm.held = Some(Stack { item: "disc", count: 2 });
        arm.phase = Phase::MoveToOutput;
        arm.chased_index = 0;
        arm.deposit_item(&mut Jb);
        assert!(!arm.drain_events().contains(&Event::Award(Award::MusicalArm)));
    }

    // ------------------------------------------------ points and items

    #[test]
    fn set_points_splits_by_mode_and_awards() {
        let mut arm = Arm::<Item>::new();
        let mut pts: Vec<Point> = (0..10).map(|i| pt(PointKind::Depot, [i, 0, 0], Mode::Deposit)).collect();
        pts.push(pt(PointKind::Basin, [0, 1, 0], Mode::Take));
        pts.push(pt(PointKind::BlazeBurner, [0, 2, 0], Mode::Deposit));
        arm.set_points(pts);
        assert_eq!(arm.inputs.len(), 1);
        assert_eq!(arm.outputs.len(), 11);
        assert_eq!(arm.drain_events(), [Event::Award(Award::ArmManyTargets), Event::Award(Award::ArmBlazeBurner)]);
        let mut few = Arm::<Item>::new();
        few.set_points((0..9).map(|i| pt(PointKind::Depot, [i, 0, 0], Mode::Deposit)));
        assert!(few.drain_events().is_empty());
    }

    #[test]
    fn held_item_goes_back_to_the_player_and_search_restarts() {
        let mut arm = Arm::<Item>::new();
        arm.held = Some(Stack { item: "wheat", count: 9 });
        arm.phase = Phase::SearchOutputs;
        assert_eq!(arm.take_held(), Some(Stack { item: "wheat", count: 9 }));
        assert_eq!(arm.phase, Phase::SearchInputs);
        assert_eq!(arm.take_held(), None);
        arm.held = Some(Stack { item: "x", count: 1 });
        assert!(arm.destroy().is_some());
        assert!(arm.apply_goggles());
        assert!(!arm.apply_goggles());
    }

    #[test]
    fn range_is_strictly_less_than_five_euclidean() {
        let arm = [0, 0, 0];
        assert!(in_range(arm, [4, 0, 0], DEFAULT_RANGE));
        assert!(!in_range(arm, [5, 0, 0], DEFAULT_RANGE));
        assert!(in_range(arm, [3, 3, 0], DEFAULT_RANGE)); // 18 < 25
        assert!(!in_range(arm, [4, 3, 0], DEFAULT_RANGE)); // 25 !< 25
        assert!(in_range(arm, [-4, -2, 1], DEFAULT_RANGE));
        assert_eq!(required_chunks([0, 64, 0], 5), vec![(-1, -1), (-1, 0), (0, -1), (0, 0)]);
        assert_eq!(required_chunks([8, 64, 8], 5), vec![(0, 0)]);
    }

    #[test]
    fn selection_click_cycle_matches_the_item_handler() {
        let basin = BlockProbe { basin: true, ..Default::default() };
        let blaze = BlockProbe { blaze_burner: true, ..Default::default() };
        let mut sel = Selection::default();
        assert_eq!(sel.right_click(&BlockProbe::default(), [0, 0, 0]), None);
        // First click on an ordinary block yields TAKE (default DEPOSIT, cycled).
        let c = sel.right_click(&basin, [1, 0, 0]).unwrap();
        assert_eq!((c.mode, c.added), (Mode::Take, true));
        let c = sel.right_click(&basin, [1, 0, 0]).unwrap();
        assert_eq!((c.mode, c.added), (Mode::Deposit, false));
        let c = sel.right_click(&basin, [1, 0, 0]).unwrap();
        assert_eq!(c.mode, Mode::Take);
        // Deposit-only blocks stay DEPOSIT.
        let c = sel.right_click(&blaze, [2, 0, 0]).unwrap();
        assert_eq!(c.mode, Mode::Deposit);
        assert_eq!(Mode::Take.colour(), 0x7FCDE0);
        assert_eq!(Mode::Deposit.colour(), 0xDDC166);
        assert!(sel.left_click([1, 0, 0]));
        assert!(!sel.left_click([1, 0, 0]));
        assert_eq!(sel.points.len(), 1);
    }

    #[test]
    fn flush_drops_points_out_of_range_and_counts_modes() {
        let basin = BlockProbe { basin: true, ..Default::default() };
        let mut sel = Selection::default();
        for (x, clicks) in [(1, 1), (2, 2), (9, 1), (4, 1)] {
            for _ in 0..clicks {
                sel.right_click(&basin, [x, 0, 0]);
            }
        }
        let f = sel.flush([0, 0, 0], DEFAULT_RANGE);
        assert_eq!((f.inputs, f.outputs, f.removed_out_of_range), (2, 1, 1));
        assert_eq!(f.points.iter().map(|p| p.pos[0]).collect::<Vec<_>>(), [1, 2, 4]);
        assert!(sel.points.is_empty());
        let mut arm = Arm::<Item>::new();
        arm.set_points(f.points);
        assert_eq!((arm.inputs.len(), arm.outputs.len()), (2, 1));
    }

    #[test]
    fn serialization_is_relative_and_validated() {
        let p = pt(PointKind::Basin, [12, 70, -3], Mode::Take);
        let r = p.serialize([10, 64, 0]);
        assert_eq!(r, PointRecord { kind: "create:basin".into(), pos: [2, 6, -3], mode: "TAKE".into() });
        let basin = |_: [i32; 3]| BlockProbe { basin: true, ..Default::default() };
        assert_eq!(Point::deserialize(&r, [10, 64, 0], basin), Some(p));
        // moved arm: same record resolves elsewhere
        assert_eq!(Point::deserialize(&r, [0, 0, 0], basin).unwrap().pos, [2, 6, -3]);
        // block gone
        assert_eq!(Point::deserialize(&r, [10, 64, 0], |_| BlockProbe::default()), None);
        let mut bad = r.clone();
        bad.kind = "create:nope".into();
        assert_eq!(Point::deserialize(&bad, [10, 64, 0], basin), None);
        bad = r.clone();
        bad.mode = "SIDEWAYS".into();
        assert_eq!(Point::deserialize(&bad, [10, 64, 0], basin), None);
        let mut moved = r;
        transform_record(&mut moved, |[x, y, z]| [-z, y, x]);
        assert_eq!(moved.pos, [3, 6, 2]);
    }

    #[test]
    fn enum_names_roundtrip() {
        for ph in [Phase::SearchInputs, Phase::MoveToInput, Phase::SearchOutputs, Phase::MoveToOutput, Phase::Dancing] {
            assert_eq!(Phase::parse(ph.name()), Some(ph));
        }
        assert_eq!(Phase::parse("IDLE"), None);
        assert_eq!(SelectionMode::PreferFirst.name(), "PREFER_FIRST");
        assert_eq!(
            SelectionMode::ForcedRoundRobin.translation_key(),
            "create.mechanical_arm.selection_mode.forced_round_robin"
        );
        assert_eq!(Mode::Take.translation_key(), "mechanical_arm.extract_from");
    }

    // ----------------------------------------------- per-kind rules

    #[test]
    fn per_kind_insertion_rules() {
        assert!(funnel_accepts(false, true, true));
        assert!(!funnel_accepts(true, true, true));
        assert!(!funnel_accepts(false, false, true));
        assert!(!funnel_accepts(false, true, false));
        assert_eq!(campfire_insert(4, true, true), 3);
        assert_eq!(campfire_insert(4, false, true), 4);
        assert_eq!(campfire_insert(4, true, false), 4);
        assert_eq!(respawn_anchor_insert(true, Some(3), 8), 7);
        assert_eq!(respawn_anchor_insert(true, Some(4), 8), 8);
        assert_eq!(respawn_anchor_insert(true, None, 8), 8);
        assert_eq!(respawn_anchor_insert(false, Some(0), 8), 8);
        assert_eq!(jukebox_insert(true, Some(false), true, 1), 0);
        assert_eq!(jukebox_insert(true, None, true, 1), 1);
        assert_eq!(jukebox_insert(true, Some(true), true, 1), 1);
        assert_eq!(jukebox_insert(false, Some(false), true, 1), 1);
        assert_eq!(jukebox_insert(true, Some(false), false, 1), 1);
        assert!(jukebox_can_extract(Some(true), true));
        assert!(!jukebox_can_extract(None, true));
        let mut locked = [false, false, false];
        lock_first(&mut locked);
        assert_eq!(locked, [true, false, false]);
        assert_eq!(COMPOSTER_INSERT_SIDE, Direction::Up);
        assert_eq!(COMPOSTER_EXTRACT_SIDE, Direction::Down);
    }

    #[test]
    fn deposit_only_points_are_never_extracted_from() {
        struct Panics;
        impl ArmWorld<Item> for Panics {
            fn is_valid(&mut self, _: &Point) -> bool { true }
            fn slot_count(&mut self, _: &Point) -> usize { panic!("fixed count") }
            fn extract(&mut self, _: &Point, _: usize, _: u32, _: bool) -> Option<Stack<Item>> { panic!("no extraction") }
            fn insert(&mut self, _: &Point, _: Stack<Item>, _: bool) -> Option<Stack<Item>> { None }
        }
        let mut arm = Arm::<Item>::new();
        arm.set_points([pt(PointKind::Funnel, [0, 0, 0], Mode::Take)]);
        arm.progress = 1.0;
        arm.search_for_item(&mut Panics);
        assert_eq!(arm.phase, Phase::SearchInputs);
    }

    #[test]
    fn partial_deposit_keeps_the_remainder_in_hand() {
        struct Burner;
        impl ArmWorld<Item> for Burner {
            fn is_valid(&mut self, _: &Point) -> bool { true }
            fn slot_count(&mut self, _: &Point) -> usize { 0 }
            fn extract(&mut self, _: &Point, _: usize, _: u32, _: bool) -> Option<Stack<Item>> { None }
            fn insert(&mut self, _: &Point, mut s: Stack<Item>, _: bool) -> Option<Stack<Item>> {
                s.count -= 1;
                Some(s)
            }
        }
        let mut arm = Arm::<Item>::new();
        arm.set_points([pt(PointKind::BlazeBurner, [0, 0, 0], Mode::Deposit)]);
        arm.drain_events();
        arm.held = Some(Stack { item: "coal", count: 3 });
        arm.phase = Phase::MoveToOutput;
        arm.chased_index = 0;
        arm.deposit_item(&mut Burner);
        assert_eq!(arm.phase, Phase::SearchOutputs);
        assert_eq!(arm.held.as_ref().unwrap().count, 2);
        assert_eq!(unique_positions(&arm.outputs), 1);
    }
}
