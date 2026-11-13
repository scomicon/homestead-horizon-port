// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Schematic item / Schematic and Quill / client SchematicHandler rules
// (Create 6.0.10): item component state, rotation + mirror transformation and
// anchor math (SchematicTransformation), tool list, 10 tick sync debounce,
// SchematicSyncPacket application, region selection (SchematicAndQuillHandler)
// and export file naming (SchematicExport / FilesHelper). Rendering, outlines,
// key bindings and screens belong to adapters. Rotation/scale animation
// (LerpedFloat chasers) is presentation and is not ported; only the logical
// targets and the 0.45 position chase are kept.

pub type Pos = [i32; 3];

pub const SCHEMATIC_ITEM: &str = "create:schematic";
pub const EMPTY_SCHEMATIC_ITEM: &str = "create:empty_schematic";
pub const SCHEMATIC_AND_QUILL_ITEM: &str = "create:schematic_and_quill";

/// Ticks between a `markDirty` and the sync packet (`SYNC_DELAY`).
pub const SYNC_DELAY: i32 = 10;
/// Default targeting range of the Schematic and Quill.
pub const QUILL_DEFAULT_RANGE: i32 = 10;
pub const QUILL_MIN_RANGE: i32 = 1;
pub const QUILL_MAX_RANGE: i32 = 100;
/// Ray length used when no key is held (`raycast(75)`).
pub const QUILL_TARGET_REACH: f64 = 75.0;
/// Ray length for face selection.
pub const QUILL_FACE_REACH: f64 = 70.0;
/// Chase factor of the deployed schematic position per tick.
pub const POSITION_CHASE: f64 = 0.45;

/// `net.minecraft.world.level.block.Rotation` in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    CounterClockwise90,
}

impl Rotation {
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    pub fn from_ordinal(i: i32) -> Self {
        match i.rem_euclid(4) {
            0 => Self::None,
            1 => Self::Clockwise90,
            2 => Self::Clockwise180,
            _ => Self::CounterClockwise90,
        }
    }
}

/// `Mirror` in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mirror {
    None,
    LeftRight,
    FrontBack,
}

/// Data components of a schematic stack (`SCHEMATIC_*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchematicStackData {
    pub file: Option<String>,
    pub owner: Option<String>,
    pub anchor: Option<Pos>,
    pub deployed: bool,
    pub rotation: Rotation,
    pub mirror: Mirror,
    pub bounds: Option<Pos>,
}

impl SchematicStackData {
    /// `SchematicItem.create`: not deployed, anchor ZERO, no rotation/mirror.
    pub fn create(owner: &str, file: &str, bounds: Pos) -> Self {
        Self {
            file: Some(file.to_owned()),
            owner: Some(owner.to_owned()),
            anchor: Some([0, 0, 0]),
            deployed: false,
            rotation: Rotation::None,
            mirror: Mirror::None,
            bounds: Some(bounds),
        }
    }

    /// `SchematicItem.loadSchematic` only reads files named `*.nbt` with an owner and a file.
    pub fn is_loadable(&self) -> bool {
        self.owner.is_some() && self.file.as_deref().is_some_and(|f| f.ends_with(".nbt"))
    }

    /// `SchematicItem.onItemUse`: sneak + main hand + stack has a file.
    pub fn opens_edit_screen(&self, sneaking: bool, main_hand: bool) -> bool {
        sneaking && main_hand && self.file.is_some()
    }

    /// `SchematicSyncPacket` handler body (after the item-type check). The anchor is
    /// stored as received, without any clamping or distance check.
    pub fn apply_sync(&mut self, sync: &SyncPacket) {
        self.deployed = sync.deployed;
        self.anchor = Some(sync.anchor);
        self.rotation = sync.rotation;
        self.mirror = sync.mirror;
    }
}

/// `SchematicSyncPacket`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncPacket {
    /// -1 selects the main hand item.
    pub slot: i32,
    pub rotation: Rotation,
    pub mirror: Mirror,
    pub anchor: Pos,
    pub deployed: bool,
}

/// Server-side check of `SchematicSyncPacket`: only schematic items are written.
pub fn sync_accepts(item_id: &str) -> bool {
    item_id == SCHEMATIC_ITEM
}

/// `Mth.sin` / `Mth.cos`: 65536 entry float table, indexed by `(int)(x * 10430.378f)`.
fn mth_table(i: i32) -> f64 {
    let i = (i & 65535) as f64;
    ((i * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32) as f64
}

fn rotate_y(v: [f64; 3], degrees: f64) -> [f64; 3] {
    // VecHelper.rotate(vec, deg, Axis.Y): no-op for 0, otherwise Mth float tables.
    if degrees == 0.0 {
        return v;
    }
    let angle = (degrees / 180.0 * std::f64::consts::PI) as f32;
    let s = mth_table((angle * 10430.378f32) as i32);
    let c = mth_table((angle * 10430.378f32 + 16384.0f32) as i32);
    [v[0] * c + v[2] * s, v[1], v[2] * c - v[0] * s]
}

fn floor_pos(v: [f64; 3]) -> Pos {
    [v[0].floor() as i32, v[1].floor() as i32, v[2].floor() as i32]
}

/// Logical state of `SchematicTransformation`.
#[derive(Clone, Debug, PartialEq)]
pub struct Transformation {
    /// Rotation chase target in degrees (unnormalised, `-90` after one clockwise turn).
    pub rotation: i32,
    /// +1 / -1 targets. `scale_fb` mirrors FRONT_BACK, `scale_lr` LEFT_RIGHT.
    pub scale_fb: i32,
    pub scale_lr: i32,
    pub x_origin: f64,
    pub z_origin: f64,
    pub target: Pos,
    pub chasing: [f64; 3],
    pub prev_chasing: [f64; 3],
}

impl Transformation {
    /// `init(anchor, settings, bounds)`: `bounds` is the schematic size.
    ///
    /// Assumption: `chase(0..).startWithValue(v)` leaves the chase target at `v`
    /// (LerpedFloat is outside the pinned source tree); the later `toSettings`
    /// round trip only works that way.
    pub fn init(anchor: Pos, rotation: Rotation, mirror: Mirror, size: Pos) -> Self {
        let mut t = Self {
            rotation: -(rotation.ordinal() * 90),
            scale_fb: if mirror == Mirror::FrontBack { -1 } else { 1 },
            scale_lr: if mirror == Mirror::LeftRight { -1 } else { 1 },
            x_origin: size[0] as f64 / 2.0,
            z_origin: size[2] as f64 / 2.0,
            target: [0; 3],
            chasing: [0.0; 3],
            prev_chasing: [0.0; 3],
        };
        t.target = t.from_anchor(anchor);
        t.chasing = [t.target[0] as f64, t.target[1] as f64, t.target[2] as f64];
        t.prev_chasing = t.chasing;
        t
    }

    pub fn rotation_target(&self) -> i32 {
        self.rotation
    }

    /// `getMirrorModifier`: Z reads the left/right scale, anything else the front/back scale.
    pub fn mirror_modifier(&self, z_axis: bool) -> i32 {
        if z_axis { self.scale_lr } else { self.scale_fb }
    }

    pub fn is_flipped(&self) -> bool {
        (self.mirror_modifier(false) < 0) != (self.mirror_modifier(true) < 0)
    }

    /// `getRotationOffset(ignoreMirrors)`: half block pivot correction applied
    /// when exactly one of the half extents is a half integer.
    pub fn rotation_offset(&self, ignore_mirrors: bool) -> [f64; 3] {
        let mut off = [0.0; 3];
        if ((self.z_origin * 2.0) as i32) % 2 != ((self.x_origin * 2.0) as i32) % 2 {
            let (mut x, mut z) = if self.x_origin > self.z_origin { (0.0, 0.5) } else { (0.5, 0.0) };
            if !ignore_mirrors {
                x *= self.mirror_modifier(false) as f64;
                z *= self.mirror_modifier(true) as f64;
            }
            off = [x, 0.0, z];
        }
        off
    }

    fn anchor_vector(&self) -> [f64; 3] {
        let ro = self.rotation_offset(false);
        let v = [0.5 - self.x_origin - ro[0], 0.0, 0.5 - self.z_origin - ro[2]];
        let v = [v[0] * self.scale_fb as f64, v[1], v[2] * self.scale_lr as f64];
        let v = rotate_y(v, self.rotation as f64);
        [v[0] + self.x_origin, v[1], v[2] + self.z_origin]
    }

    /// `getAnchor`: world position of the schematic's origin block.
    pub fn anchor(&self) -> Pos {
        let v = self.anchor_vector();
        floor_pos([
            v[0] + self.target[0] as f64,
            v[1] + self.target[1] as f64,
            v[2] + self.target[2] as f64,
        ])
    }

    /// `fromAnchor`: the target that produces `anchor` under the current transform.
    pub fn from_anchor(&self, anchor: Pos) -> Pos {
        let o = floor_pos(self.anchor_vector());
        [anchor[0] - o[0], anchor[1] - o[1], anchor[2] - o[2]]
    }

    /// `rotate90(clockwise)`: clockwise adds -90 to the target, counter-clockwise +90.
    pub fn rotate90(&mut self, clockwise: bool) {
        self.rotation += if clockwise { -90 } else { 90 };
    }

    /// `flip(axis)`: X negates the left/right scale, Z the front/back scale, Y is a no-op.
    pub fn flip(&mut self, axis: char) {
        match axis {
            'x' | 'X' => self.scale_lr *= -1,
            'z' | 'Z' => self.scale_fb *= -1,
            _ => {}
        }
    }

    pub fn move_to(&mut self, pos: Pos) {
        self.target = pos;
    }

    pub fn move_by(&mut self, d: Pos) {
        self.move_to([self.target[0] + d[0], self.target[1] + d[1], self.target[2] + d[2]]);
    }

    /// `startAt`: snap the chase position and set the target.
    pub fn start_at(&mut self, pos: Pos) {
        self.chasing = [pos[0] as f64, pos[1] as f64, pos[2] as f64];
        self.prev_chasing = self.chasing;
        self.move_to(pos);
    }

    /// `tick`: position eases towards the target by `0.45` per tick.
    pub fn tick(&mut self) {
        self.prev_chasing = self.chasing;
        for i in 0..3 {
            self.chasing[i] += (self.target[i] as f64 - self.chasing[i]) * POSITION_CHASE;
        }
    }

    /// `toSettings`: normalised rotation and a single mirror. Both mirrors together
    /// become a 180 degree turn. When both mirror flags are set the original applies
    /// FRONT_BACK first and LEFT_RIGHT second, but that case cannot reach here.
    pub fn to_settings(&self) -> (Rotation, Mirror) {
        let mut i = self.rotation;
        let mut lr = self.scale_lr < 0;
        let mut fb = self.scale_fb < 0;
        if lr && fb {
            lr = false;
            fb = false;
            i += 180;
        }
        i %= 360;
        if i < 0 {
            i += 360;
        }
        let rotation = match i {
            90 => Rotation::CounterClockwise90,
            180 => Rotation::Clockwise180,
            270 => Rotation::Clockwise90,
            _ => Rotation::None,
        };
        let mirror = if lr {
            Mirror::LeftRight
        } else if fb {
            Mirror::FrontBack
        } else {
            Mirror::None
        };
        (rotation, mirror)
    }
}

/// `ToolType` constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolType {
    Deploy,
    Move,
    MoveY,
    Rotate,
    Flip,
    Print,
}

impl ToolType {
    /// Lang key suffix (`schematic.tool.<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Deploy => "deploy",
            Self::Move => "move",
            Self::MoveY => "move_y",
            Self::Rotate => "rotate",
            Self::Flip => "flip",
            Self::Print => "print",
        }
    }

    /// `ToolType.getTools(creative)`: MOVE, MOVE_Y, DEPLOY, ROTATE, FLIP and PRINT
    /// only in creative.
    pub fn tools(creative: bool) -> Vec<ToolType> {
        let mut v = vec![Self::Move, Self::MoveY, Self::Deploy, Self::Rotate, Self::Flip];
        if creative {
            v.push(Self::Print);
        }
        v
    }
}

/// Client `SchematicHandler` state machine without rendering.
#[derive(Clone, Debug)]
pub struct SchematicHandler {
    pub active: bool,
    pub deployed: bool,
    pub displayed_schematic: Option<String>,
    pub current_tool: ToolType,
    pub sync_cooldown: i32,
    pub hotbar_slot: i32,
    pub transformation: Option<Transformation>,
    pub tools: Vec<ToolType>,
}

impl Default for SchematicHandler {
    fn default() -> Self {
        Self {
            active: false,
            deployed: false,
            displayed_schematic: None,
            current_tool: ToolType::Deploy,
            sync_cooldown: 0,
            hotbar_slot: 0,
            transformation: None,
            tools: Vec::new(),
        }
    }
}

impl SchematicHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// `loadSettings` + the tool menu part of `init`.
    pub fn init(&mut self, held: &SchematicStackData, creative: bool) {
        self.deployed = held.deployed;
        self.transformation = held.bounds.map(|size| {
            Transformation::init(held.anchor.unwrap_or([0; 3]), held.rotation, held.mirror, size)
        });
        self.displayed_schematic = held.file.clone();
        self.active = true;
        if self.deployed {
            self.tools = ToolType::tools(creative);
            if !self.tools.contains(&self.current_tool) {
                self.current_tool = self.tools[0];
            }
        } else {
            self.tools = vec![ToolType::Deploy];
            self.current_tool = ToolType::Deploy;
        }
    }

    /// `deploy()`; returns true if the tool menu changed.
    pub fn deploy(&mut self, creative: bool) -> bool {
        let first = !self.deployed;
        if first {
            self.tools = ToolType::tools(creative);
        }
        self.deployed = true;
        first
    }

    /// `markDirty`.
    pub fn mark_dirty(&mut self) {
        self.sync_cooldown = SYNC_DELAY;
    }

    fn sync_packet(&self) -> Option<SyncPacket> {
        let t = self.transformation.as_ref()?;
        let (rotation, mirror) = t.to_settings();
        Some(SyncPacket { slot: self.hotbar_slot, rotation, mirror, anchor: t.anchor(), deployed: self.deployed })
    }

    /// One client tick. `held` is the main-hand schematic with a file, if any.
    /// Returns the packet to send. The debounce fires when the countdown reaches 1,
    /// i.e. on the 9th tick after `mark_dirty`.
    pub fn tick(&mut self, held: Option<&SchematicStackData>, spectator: bool, creative: bool) -> Option<SyncPacket> {
        if spectator {
            if self.active {
                self.active = false;
                self.sync_cooldown = 0;
                self.hotbar_slot = 0;
            }
            return None;
        }
        if self.active {
            if let Some(t) = self.transformation.as_mut() {
                t.tick();
            }
        }
        let held = held.filter(|h| h.file.is_some());
        let Some(held) = held else {
            self.active = false;
            self.sync_cooldown = 0;
            return None;
        };
        if !self.active || held.file != self.displayed_schematic {
            self.init(held, creative);
        }
        if self.sync_cooldown > 0 {
            self.sync_cooldown -= 1;
        }
        if self.sync_cooldown == 1 {
            return self.sync_packet();
        }
        None
    }

    /// `onMouseInput`: right click press only, never while sneaking or aiming at a
    /// schematicannon or deployer. Returns whether the active tool handles the click.
    pub fn handles_right_click(&self, button: i32, pressed: bool, sneaking: bool, targets_cannon_or_deployer: bool) -> bool {
        self.active && pressed && button == 1 && !sneaking && !targets_cannon_or_deployer
    }

    /// `printInstantly`: creative only on the server. Un-deploys and goes inactive.
    pub fn print_instantly(&mut self) {
        self.deployed = false;
        self.active = false;
        self.mark_dirty();
    }
}

/// Block faces used for the Schematic and Quill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub fn normal(self) -> Pos {
        match self {
            Self::Down => [0, -1, 0],
            Self::Up => [0, 1, 0],
            Self::North => [0, 0, -1],
            Self::South => [0, 0, 1],
            Self::West => [-1, 0, 0],
            Self::East => [1, 0, 0],
        }
    }
    pub fn is_positive(self) -> bool {
        matches!(self, Self::Up | Self::South | Self::East)
    }
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
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
}

/// Result of a right click with the Schematic and Quill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuillClick {
    NotHandled,
    /// `schematicAndQuill.abort`
    Discarded,
    /// Box complete: open the name prompt.
    OpenPrompt,
    /// `schematicAndQuill.noTarget`
    NoTarget,
    /// `schematicAndQuill.secondPos`
    SecondPos,
    /// `schematicAndQuill.firstPos`
    FirstPos,
}

/// Selection state of `SchematicAndQuillHandler`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuillSelection {
    pub first: Option<Pos>,
    pub second: Option<Pos>,
    pub selected: Option<Pos>,
    pub selected_face: Option<Direction>,
    pub range: i32,
}

impl Default for QuillSelection {
    fn default() -> Self {
        Self { first: None, second: None, selected: None, selected_face: None, range: QUILL_DEFAULT_RANGE }
    }
}

/// Result of resizing via the scroll wheel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuillResize {
    pub first: Pos,
    pub second: Pos,
    /// Dimensions announced in `schematicAndQuill.dimensions` (xSize+1, ...).
    pub dimensions: Pos,
}

impl QuillSelection {
    pub fn new() -> Self {
        Self::default()
    }

    /// `onMouseInput(button, pressed)`; only a right click press counts.
    pub fn right_click(&mut self, active: bool, sneaking: bool) -> QuillClick {
        if !active {
            return QuillClick::NotHandled;
        }
        if sneaking {
            self.discard();
            return QuillClick::Discarded;
        }
        if self.second.is_some() {
            return QuillClick::OpenPrompt;
        }
        let Some(sel) = self.selected else {
            return QuillClick::NoTarget;
        };
        if self.first.is_some() {
            self.second = Some(sel);
            return QuillClick::SecondPos;
        }
        self.first = Some(sel);
        QuillClick::FirstPos
    }

    pub fn discard(&mut self) {
        self.first = None;
        self.second = None;
    }

    /// Plain ray hit: a vertical face on a non-replaceable block selects the
    /// neighbouring block instead.
    pub fn target_from_hit(hit: Pos, face: Direction, hit_replaceable: bool) -> Pos {
        if face.is_vertical() && !hit_replaceable {
            let n = face.normal();
            [hit[0] + n[0], hit[1] + n[1], hit[2] + n[2]]
        } else {
            hit
        }
    }

    /// Range scrolling (Ctrl + wheel) before the box is complete.
    pub fn scroll_range(&mut self, delta: f64) {
        if self.second.is_none() {
            self.range = (self.range + delta as i32).clamp(QUILL_MIN_RANGE, QUILL_MAX_RANGE);
        }
    }

    /// Face the camera is aiming at, given the ray result: when the camera is
    /// inside the box the opposite face is selected.
    pub fn face_from_ray(hit_face: Option<Direction>, camera_inside: bool) -> Option<Direction> {
        hit_face.map(|f| if camera_inside { f.opposite() } else { f })
    }

    /// Ctrl + wheel resizing of a finished box along the selected face.
    /// Returns `None` when no face is selected or the box is incomplete.
    pub fn resize(&mut self, delta: f64, camera_inside: bool) -> Option<QuillResize> {
        let face = self.selected_face?;
        let (a, b) = (self.first?, self.second?);
        let mut delta = if camera_inside { -delta } else { delta };
        // delta > 0 ? ceil : floor, i.e. rounded away from zero
        delta = if delta > 0.0 { delta.ceil() } else { delta.floor() };
        let n = face.normal();
        let v = [n[0] as f64 * delta, n[1] as f64 * delta, n[2] as f64 * delta];
        let step = if face.is_positive() { 1.0 } else { -1.0 };
        let mut min = [0.0f64; 3];
        let mut max = [0.0f64; 3];
        for i in 0..3 {
            min[i] = a[i].min(b[i]) as f64;
            max[i] = a[i].max(b[i]) as f64;
        }
        if !face.is_positive() {
            for i in 0..3 {
                min[i] -= v[i];
                max[i] -= v[i];
            }
        }
        for i in 0..3 {
            max[i] = (max[i] - v[i] * step).max(min[i]);
        }
        let first = floor_pos(min);
        let second = floor_pos(max);
        self.first = Some(first);
        self.second = Some(second);
        Some(QuillResize {
            first,
            second,
            dimensions: [
                (second[0] - first[0]).abs() + 1,
                (second[1] - first[1]).abs() + 1,
                (second[2] - first[2]).abs() + 1,
            ],
        })
    }

    /// `getCurrentSelectionBox` as inclusive block corners (min, max) of the
    /// outline, or `None`. A box under construction follows the targeted block.
    pub fn selection_box(&self) -> Option<(Pos, Pos)> {
        let norm = |a: Pos, b: Pos| {
            (
                [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
                [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
            )
        };
        match (self.first, self.second, self.selected) {
            (Some(a), Some(b), _) => Some(norm(a, b)),
            (Some(a), None, Some(s)) => Some(norm(a, s)),
            (Some(a), None, None) => Some((a, a)),
            (None, _, Some(s)) => Some((s, s)),
            (None, _, None) => None,
        }
    }

    /// After a successful save both corners are cleared; after a failure they stay.
    pub fn finish_save(&mut self, success: bool) {
        if success {
            self.first = None;
            self.second = None;
        }
    }
}

/// `SchematicExport.saveSchematic` bounds: origin is the min corner, size is
/// inclusive (`BoundingBox.fromCorners` spans). Identical corners give 1x1x1;
/// there is no emptiness or size check.
pub fn export_bounds(first: Pos, second: Pos) -> (Pos, Pos) {
    let origin = [first[0].min(second[0]), first[1].min(second[1]), first[2].min(second[2])];
    let size = [
        (first[0] - second[0]).abs() + 1,
        (first[1] - second[1]).abs() + 1,
        (first[2] - second[2]).abs() + 1,
    ];
    (origin, size)
}

/// `FilesHelper.slug`: `Lang.asId` (assumed lower-casing) then every run of
/// non-word characters (`\W+`, ASCII) collapses into one `_`.
pub fn slug(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut in_run = false;
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('_');
            in_run = true;
        }
    }
    out
}

/// `FilesHelper.findFirstValidFilename`: `slug`, `slug_1`, `slug_2`, ... with the
/// first index whose `<name>.<ext>` does not exist. Gaps are filled from the lowest index.
pub fn find_first_valid_filename(name: &str, ext: &str, exists: &dyn Fn(&str) -> bool) -> String {
    let slug = slug(name);
    let mut index = 0;
    loop {
        let candidate = if index == 0 { format!("{slug}.{ext}") } else { format!("{slug}_{index}.{ext}") };
        if !exists(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

/// File name chosen by `SchematicExport.saveSchematic`. An empty name falls back
/// to `fallback` (the translated `schematicAndQuill.fallbackName`); without
/// `overwrite` a free slugged name is searched; `.nbt` is appended when missing.
pub fn export_file_name(name: &str, overwrite: bool, fallback: &str, exists: &dyn Fn(&str) -> bool) -> String {
    let mut file = if name.is_empty() { fallback.to_owned() } else { name.to_owned() };
    if !overwrite {
        file = find_first_valid_filename(&file, "nbt", exists);
    }
    if !file.ends_with(".nbt") {
        file.push_str(".nbt");
    }
    file
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_matches_schematic_item_defaults() {
        let d = SchematicStackData::create("Steve", "house.nbt", [3, 4, 5]);
        assert!(!d.deployed);
        assert_eq!(d.anchor, Some([0, 0, 0]));
        assert_eq!((d.rotation, d.mirror), (Rotation::None, Mirror::None));
        assert!(d.is_loadable());
        assert_eq!(d.bounds, Some([3, 4, 5]));
    }

    #[test]
    fn loadable_requires_nbt_extension_and_owner() {
        let mut d = SchematicStackData::create("a", "x.schem", [1, 1, 1]);
        assert!(!d.is_loadable());
        d.file = Some("x.nbt".into());
        assert!(d.is_loadable());
        d.owner = None;
        assert!(!d.is_loadable());
    }

    #[test]
    fn edit_screen_needs_sneak_main_hand_and_file() {
        let d = SchematicStackData::create("a", "x.nbt", [1, 1, 1]);
        assert!(d.opens_edit_screen(true, true));
        assert!(!d.opens_edit_screen(false, true));
        assert!(!d.opens_edit_screen(true, false));
    }

    #[test]
    fn rotation_ordinals_follow_vanilla_order() {
        assert_eq!(Rotation::from_ordinal(3), Rotation::CounterClockwise90);
        assert_eq!(Rotation::Clockwise90.ordinal(), 1);
    }

    #[test]
    fn identity_transform_anchor_equals_target() {
        let t = Transformation::init([10, 5, 10], Rotation::None, Mirror::None, [3, 3, 3]);
        assert_eq!(t.target, [10, 5, 10]);
        assert_eq!(t.anchor(), [10, 5, 10]);
        assert_eq!(t.to_settings(), (Rotation::None, Mirror::None));
    }

    #[test]
    fn clockwise_turn_of_3x3_moves_anchor_by_two_blocks() {
        // Hand computed: corner (-1,-1) about the centre rotates to (1,-1) -> (2,0).
        let t = Transformation::init([0, 0, 0], Rotation::Clockwise90, Mirror::None, [3, 3, 3]);
        assert_eq!(t.rotation, -90);
        assert_eq!(t.anchor(), [0, 0, 0]);
        assert_eq!(t.target, [-2, 0, 0]);
        assert_eq!(t.to_settings().0, Rotation::Clockwise90);
    }

    #[test]
    fn rotation_ordinal_survives_init_to_settings_roundtrip() {
        for r in [Rotation::None, Rotation::Clockwise90, Rotation::Clockwise180, Rotation::CounterClockwise90] {
            for m in [Mirror::None, Mirror::LeftRight, Mirror::FrontBack] {
                let t = Transformation::init([4, 2, -7], r, m, [5, 2, 4]);
                assert_eq!(t.to_settings(), (r, m), "{r:?} {m:?}");
                // anchor() and from_anchor() floor at different points, so float noise
                // can shift a rotated anchor by one block (same in the original)
                let a = t.anchor();
                let off = (0..3).map(|i| (a[i] - [4, 2, -7][i]).abs()).max().unwrap();
                assert!(off <= 1, "{r:?} {m:?} {a:?}");
                if r == Rotation::None {
                    assert_eq!(a, [4, 2, -7]);
                }
            }
        }
    }

    #[test]
    fn rotate90_and_flip_change_targets() {
        let mut t = Transformation::init([0; 3], Rotation::None, Mirror::None, [2, 1, 2]);
        t.rotate90(true);
        assert_eq!(t.to_settings().0, Rotation::Clockwise90);
        t.rotate90(false);
        t.rotate90(false);
        assert_eq!(t.rotation_target(), 90);
        assert_eq!(t.to_settings().0, Rotation::CounterClockwise90);
        t.flip('x');
        assert_eq!(t.to_settings().1, Mirror::LeftRight);
        t.flip('z');
        // both mirrors: no mirror, plus 180 degrees
        assert_eq!(t.to_settings(), (Rotation::Clockwise90, Mirror::None));
        t.flip('y');
        assert_eq!(t.scale_lr, -1);
    }

    #[test]
    fn four_clockwise_turns_return_to_none() {
        let mut t = Transformation::init([0; 3], Rotation::None, Mirror::None, [2, 1, 2]);
        for _ in 0..4 {
            t.rotate90(true);
        }
        assert_eq!(t.to_settings().0, Rotation::None);
        assert_eq!(t.rotation_target(), -360);
    }

    #[test]
    fn mirror_modifier_axis_quirk_is_kept() {
        let mut t = Transformation::init([0; 3], Rotation::None, Mirror::LeftRight, [2, 1, 2]);
        assert_eq!(t.mirror_modifier(true), -1); // Z reads the left/right scale
        assert_eq!(t.mirror_modifier(false), 1);
        assert!(t.is_flipped());
        t.flip('z');
        assert!(!t.is_flipped());
    }

    #[test]
    fn rotation_offset_only_for_mixed_parity() {
        let even = Transformation::init([0; 3], Rotation::None, Mirror::None, [4, 1, 4]);
        assert_eq!(even.rotation_offset(true), [0.0; 3]);
        let mixed = Transformation::init([0; 3], Rotation::None, Mirror::None, [4, 1, 3]);
        assert_eq!(mixed.rotation_offset(true), [0.0, 0.0, 0.5]);
        let mixed2 = Transformation::init([0; 3], Rotation::None, Mirror::None, [3, 1, 4]);
        assert_eq!(mixed2.rotation_offset(true), [0.5, 0.0, 0.0]);
        let mut m = mixed2.clone();
        m.flip('z'); // scale_fb = -1 -> X modifier
        assert_eq!(m.rotation_offset(false), [-0.5, 0.0, 0.0]);
        assert_eq!(m.rotation_offset(true), [0.5, 0.0, 0.0]);
    }

    #[test]
    fn move_and_chase() {
        let mut t = Transformation::init([0; 3], Rotation::None, Mirror::None, [1, 1, 1]);
        t.move_by([10, 0, -10]);
        assert_eq!(t.target, [10, 0, -10]);
        assert_eq!(t.chasing, [0.0; 3]);
        t.tick();
        assert!((t.chasing[0] - 4.5).abs() < 1e-9);
        assert!((t.chasing[2] + 4.5).abs() < 1e-9);
        assert_eq!(t.prev_chasing, [0.0; 3]);
        t.start_at([7, 7, 7]);
        assert_eq!(t.chasing, [7.0; 3]);
        assert_eq!(t.target, [7, 7, 7]);
    }

    #[test]
    fn tool_lists() {
        assert_eq!(
            ToolType::tools(false),
            vec![ToolType::Move, ToolType::MoveY, ToolType::Deploy, ToolType::Rotate, ToolType::Flip]
        );
        assert_eq!(ToolType::tools(true).last(), Some(&ToolType::Print));
        assert_eq!(ToolType::MoveY.id(), "move_y");
    }

    #[test]
    fn handler_init_undeployed_offers_only_deploy() {
        let mut h = SchematicHandler::new();
        let d = SchematicStackData::create("a", "x.nbt", [2, 2, 2]);
        h.tick(Some(&d), false, false);
        assert!(h.active);
        assert_eq!(h.tools, vec![ToolType::Deploy]);
        assert_eq!(h.current_tool, ToolType::Deploy);
        assert!(h.deploy(false));
        assert!(!h.deploy(false));
        assert_eq!(h.tools.len(), 5);
    }

    #[test]
    fn handler_sync_fires_on_ninth_tick_after_mark_dirty() {
        let mut h = SchematicHandler::new();
        let mut d = SchematicStackData::create("a", "x.nbt", [2, 2, 2]);
        d.deployed = true;
        h.tick(Some(&d), false, false);
        h.mark_dirty();
        let mut fired = None;
        for tick in 1..=12 {
            if let Some(p) = h.tick(Some(&d), false, false) {
                fired = Some((tick, p));
                break;
            }
        }
        let (tick, p) = fired.unwrap();
        assert_eq!(tick, 9);
        assert!(p.deployed);
        assert_eq!(p.slot, 0);
        assert_eq!(p.anchor, [0, 0, 0]);
    }

    #[test]
    fn mark_dirty_again_restarts_debounce() {
        let mut h = SchematicHandler::new();
        let d = SchematicStackData::create("a", "x.nbt", [2, 2, 2]);
        h.tick(Some(&d), false, false);
        h.mark_dirty();
        for _ in 0..5 {
            assert!(h.tick(Some(&d), false, false).is_none());
        }
        h.mark_dirty();
        for _ in 0..8 {
            assert!(h.tick(Some(&d), false, false).is_none());
        }
        assert!(h.tick(Some(&d), false, false).is_some());
    }

    #[test]
    fn handler_deactivates_without_item_and_in_spectator() {
        let mut h = SchematicHandler::new();
        let d = SchematicStackData::create("a", "x.nbt", [2, 2, 2]);
        h.tick(Some(&d), false, false);
        h.mark_dirty();
        h.tick(None, false, false);
        assert!(!h.active);
        assert_eq!(h.sync_cooldown, 0);
        h.tick(Some(&d), false, false);
        assert!(h.active);
        h.hotbar_slot = 3;
        h.tick(Some(&d), true, false);
        assert!(!h.active);
        assert_eq!(h.hotbar_slot, 0);
    }

    #[test]
    fn handler_reinitialises_when_file_changes() {
        let mut h = SchematicHandler::new();
        let a = SchematicStackData::create("a", "a.nbt", [2, 2, 2]);
        let mut b = SchematicStackData::create("a", "b.nbt", [2, 2, 2]);
        b.deployed = true;
        h.tick(Some(&a), false, false);
        assert!(!h.deployed);
        h.tick(Some(&b), false, false);
        assert!(h.deployed);
        assert_eq!(h.displayed_schematic.as_deref(), Some("b.nbt"));
    }

    #[test]
    fn handler_right_click_rules() {
        let mut h = SchematicHandler::new();
        assert!(!h.handles_right_click(1, true, false, false));
        h.active = true;
        assert!(h.handles_right_click(1, true, false, false));
        assert!(!h.handles_right_click(0, true, false, false));
        assert!(!h.handles_right_click(1, false, false, false));
        assert!(!h.handles_right_click(1, true, true, false));
        assert!(!h.handles_right_click(1, true, false, true));
    }

    #[test]
    fn print_instantly_undeploys() {
        let mut h = SchematicHandler::new();
        h.active = true;
        h.deployed = true;
        h.print_instantly();
        assert!(!h.deployed && !h.active);
        assert_eq!(h.sync_cooldown, SYNC_DELAY);
    }

    #[test]
    fn sync_packet_writes_components_without_clamping() {
        let mut d = SchematicStackData::create("a", "x.nbt", [2, 2, 2]);
        d.apply_sync(&SyncPacket {
            slot: -1,
            rotation: Rotation::Clockwise180,
            mirror: Mirror::FrontBack,
            anchor: [1_000_000, -64, 9],
            deployed: true,
        });
        assert!(d.deployed);
        assert_eq!(d.anchor, Some([1_000_000, -64, 9]));
        assert_eq!((d.rotation, d.mirror), (Rotation::Clockwise180, Mirror::FrontBack));
        assert!(sync_accepts(SCHEMATIC_ITEM));
        assert!(!sync_accepts(SCHEMATIC_AND_QUILL_ITEM));
    }

    #[test]
    fn quill_click_sequence() {
        let mut q = QuillSelection::new();
        assert_eq!(q.right_click(false, false), QuillClick::NotHandled);
        assert_eq!(q.right_click(true, false), QuillClick::NoTarget);
        q.selected = Some([1, 2, 3]);
        assert_eq!(q.right_click(true, false), QuillClick::FirstPos);
        q.selected = Some([4, 5, 6]);
        assert_eq!(q.right_click(true, false), QuillClick::SecondPos);
        assert_eq!(q.right_click(true, false), QuillClick::OpenPrompt);
        assert_eq!(q.right_click(true, true), QuillClick::Discarded);
        assert_eq!((q.first, q.second), (None, None));
    }

    #[test]
    fn quill_target_from_hit() {
        assert_eq!(QuillSelection::target_from_hit([0, 0, 0], Direction::Up, false), [0, 1, 0]);
        assert_eq!(QuillSelection::target_from_hit([0, 0, 0], Direction::Down, false), [0, -1, 0]);
        assert_eq!(QuillSelection::target_from_hit([0, 0, 0], Direction::Up, true), [0, 0, 0]);
        assert_eq!(QuillSelection::target_from_hit([0, 0, 0], Direction::East, false), [0, 0, 0]);
    }

    #[test]
    fn quill_range_clamps_and_freezes_after_second_corner() {
        let mut q = QuillSelection::new();
        q.scroll_range(500.0);
        assert_eq!(q.range, 100);
        q.scroll_range(-500.0);
        assert_eq!(q.range, 1);
        q.second = Some([0; 3]);
        q.scroll_range(5.0);
        assert_eq!(q.range, 1);
    }

    #[test]
    fn quill_face_selection_inside_box_is_opposite() {
        assert_eq!(QuillSelection::face_from_ray(Some(Direction::North), false), Some(Direction::North));
        assert_eq!(QuillSelection::face_from_ray(Some(Direction::North), true), Some(Direction::South));
        assert_eq!(QuillSelection::face_from_ray(None, true), None);
    }

    #[test]
    fn quill_selection_box_cases() {
        let mut q = QuillSelection::new();
        assert_eq!(q.selection_box(), None);
        q.selected = Some([3, 3, 3]);
        assert_eq!(q.selection_box(), Some(([3; 3], [3; 3])));
        q.first = Some([5, 0, 0]);
        assert_eq!(q.selection_box(), Some(([3, 0, 0], [5, 3, 3])));
        q.selected = None;
        assert_eq!(q.selection_box(), Some(([5, 0, 0], [5, 0, 0])));
        q.second = Some([1, 1, 1]);
        assert_eq!(q.selection_box(), Some(([1, 0, 0], [5, 1, 1])));
    }

    #[test]
    fn quill_resize_positive_face() {
        let mut q = QuillSelection::new();
        q.first = Some([0, 0, 0]);
        q.second = Some([4, 4, 4]);
        q.selected_face = Some(Direction::East);
        // outside the box a positive wheel step pulls the east face inwards
        let r = q.resize(1.0, false).unwrap();
        assert_eq!(r.second, [3, 4, 4]);
        assert_eq!(r.dimensions, [4, 5, 5]);
        // inside the box the delta flips and the box grows
        let r = q.resize(1.0, true).unwrap();
        assert_eq!(r.second, [4, 4, 4]);
    }

    #[test]
    fn quill_resize_negative_face_moves_min_and_never_collapses() {
        let mut q = QuillSelection::new();
        q.first = Some([0, 0, 0]);
        q.second = Some([4, 4, 4]);
        q.selected_face = Some(Direction::West);
        let r = q.resize(-2.0, false).unwrap();
        assert_eq!(r.first, [-2, 0, 0]);
        assert_eq!(r.second, [4, 4, 4]);
        // fractional deltas round away from zero
        let r = q.resize(0.2, false).unwrap();
        assert_eq!(r.first, [-1, 0, 0]);
        // cannot shrink below the min corner
        q.selected_face = Some(Direction::East);
        let r = q.resize(100.0, false).unwrap();
        assert_eq!(r.second[0], r.first[0]);
        q.selected_face = None;
        assert!(q.resize(1.0, false).is_none());
    }

    #[test]
    fn quill_save_keeps_corners_on_failure() {
        let mut q = QuillSelection::new();
        q.first = Some([0; 3]);
        q.second = Some([1; 3]);
        q.finish_save(false);
        assert!(q.second.is_some());
        q.finish_save(true);
        assert!(q.first.is_none() && q.second.is_none());
    }

    #[test]
    fn export_bounds_are_inclusive_and_order_independent() {
        assert_eq!(export_bounds([5, 5, 5], [5, 5, 5]), ([5, 5, 5], [1, 1, 1]));
        assert_eq!(export_bounds([3, 0, 9], [-1, 4, 2]), ([-1, 0, 2], [5, 5, 8]));
    }

    #[test]
    fn slug_rules() {
        assert_eq!(slug("My Build!!"), "my_build_");
        assert_eq!(slug("a  b---c"), "a_b_c");
        assert_eq!(slug("../etc/passwd"), "_etc_passwd");
        assert_eq!(slug("under_score9"), "under_score9");
        assert_eq!(slug("house.nbt"), "house_nbt");
    }

    #[test]
    fn first_valid_filename_numbers_with_underscore_and_fills_gaps() {
        let taken = ["house.nbt", "house_1.nbt"];
        let ex = |n: &str| taken.contains(&n);
        assert_eq!(find_first_valid_filename("House", "nbt", &ex), "house_2.nbt");
        let taken2 = ["house_1.nbt"];
        let ex2 = |n: &str| taken2.contains(&n);
        assert_eq!(find_first_valid_filename("house", "nbt", &ex2), "house.nbt");
    }

    #[test]
    fn export_file_name_fallback_overwrite_and_suffix() {
        let none = |_: &str| false;
        assert_eq!(export_file_name("", false, "Unnamed", &none), "unnamed.nbt");
        assert_eq!(export_file_name("Tower", false, "x", &none), "tower.nbt");
        // overwrite keeps the name as typed and only appends the extension
        assert_eq!(export_file_name("Tower", true, "x", &none), "Tower.nbt");
        assert_eq!(export_file_name("Tower.nbt", true, "x", &none), "Tower.nbt");
        let taken = |n: &str| n == "tower.nbt";
        assert_eq!(export_file_name("tower", false, "x", &taken), "tower_1.nbt");
    }
}
