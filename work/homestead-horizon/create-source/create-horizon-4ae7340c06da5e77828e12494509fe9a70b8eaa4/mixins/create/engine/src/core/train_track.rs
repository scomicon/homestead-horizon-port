// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure track geometry after Create 6.0.10 content/trains/track: TrackShape,
// BezierConnection (handles, length, step LUT, tangents, normals) and the
// geometric validity rules of TrackPlacement.tryConnect. No world access.
// See docs/trains.md for the Java mapping and the list of unverified details.

/// Plain 3-vector (x, y, z).
pub type V3 = [f64; 3];

pub const UP: V3 = [0.0, 1.0, 0.0];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}
pub fn dist(a: V3, b: V3) -> f64 {
    len(sub(a, b))
}
pub fn normalize(a: V3) -> V3 {
    let l = len(a);
    if l < 1e-12 { [0.0; 3] } else { scale(a, 1.0 / l) }
}
fn lerp(a: V3, b: V3, t: f64) -> V3 {
    add(scale(a, 1.0 - t), scale(b, t))
}

/// Spherical interpolation of two unit vectors (`VecHelper.slerp`).
pub fn slerp(t: f64, a: V3, b: V3) -> V3 {
    let d = dot(a, b).clamp(-1.0, 1.0);
    let omega = d.acos();
    let so = omega.sin();
    if so.abs() < 1e-9 {
        return lerp(a, b, t);
    }
    add(
        scale(a, ((1.0 - t) * omega).sin() / so),
        scale(b, (t * omega).sin() / so),
    )
}

// ---------------------------------------------------------------- TrackShape

/// Clockwise quarter turns (`Rotation` ordinal).
pub type Quarter = u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mirror {
    None,
    FrontBack,
    LeftRight,
}

/// `TrackShape` (block state property `shape`). Variant order is the Java order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrackShape {
    None,
    Zo,
    Xo,
    Pd,
    Nd,
    An,
    As,
    Ae,
    Aw,
    Tn,
    Ts,
    Te,
    Tw,
    CrO,
    CrD,
    CrPdx,
    CrPdz,
    CrNdx,
    CrNdz,
}

impl TrackShape {
    pub const ALL: [TrackShape; 19] = [
        Self::None,
        Self::Zo,
        Self::Xo,
        Self::Pd,
        Self::Nd,
        Self::An,
        Self::As,
        Self::Ae,
        Self::Aw,
        Self::Tn,
        Self::Ts,
        Self::Te,
        Self::Tw,
        Self::CrO,
        Self::CrD,
        Self::CrPdx,
        Self::CrPdz,
        Self::CrNdx,
        Self::CrNdz,
    ];

    /// Serialized (lowercase) block state name.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Zo => "zo",
            Self::Xo => "xo",
            Self::Pd => "pd",
            Self::Nd => "nd",
            Self::An => "an",
            Self::As => "as",
            Self::Ae => "ae",
            Self::Aw => "aw",
            Self::Tn => "tn",
            Self::Ts => "ts",
            Self::Te => "te",
            Self::Tw => "tw",
            Self::CrO => "cr_o",
            Self::CrD => "cr_d",
            Self::CrPdx => "cr_pdx",
            Self::CrPdz => "cr_pdz",
            Self::CrNdx => "cr_ndx",
            Self::CrNdz => "cr_ndz",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.name() == name)
    }

    /// Model name used by the original blockstate models.
    pub fn model(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Zo => "z_ortho",
            Self::Xo => "x_ortho",
            Self::Pd => "diag",
            Self::Nd => "diag_2",
            Self::An | Self::As | Self::Ae | Self::Aw => "ascending",
            Self::Tn | Self::Ts | Self::Te | Self::Tw => "teleport",
            Self::CrO => "cross_ortho",
            Self::CrD => "cross_diag",
            Self::CrPdx => "cross_d1_xo",
            Self::CrPdz => "cross_d1_zo",
            Self::CrNdx => "cross_d2_xo",
            Self::CrNdz => "cross_d2_zo",
        }
    }

    pub fn model_rotation(self) -> i32 {
        match self {
            Self::An | Self::Tn => 180,
            Self::Ae | Self::Te => 270,
            Self::Aw | Self::Tw => 90,
            _ => 0,
        }
    }

    /// Axis vectors (a junction has several). Not normalised, as in Java.
    pub fn axes(self) -> &'static [V3] {
        match self {
            Self::None => &[[0.0, 0.0, 0.0]],
            Self::Zo | Self::Ts => &[[0.0, 0.0, 1.0]],
            Self::Xo | Self::Te => &[[1.0, 0.0, 0.0]],
            Self::Pd => &[[1.0, 0.0, 1.0]],
            Self::Nd => &[[-1.0, 0.0, 1.0]],
            Self::An => &[[0.0, 1.0, -1.0]],
            Self::As => &[[0.0, 1.0, 1.0]],
            Self::Ae => &[[1.0, 1.0, 0.0]],
            Self::Aw => &[[-1.0, 1.0, 0.0]],
            Self::Tn => &[[0.0, 0.0, -1.0]],
            Self::Tw => &[[-1.0, 0.0, 0.0]],
            Self::CrO => &[[0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
            Self::CrD => &[[1.0, 0.0, 1.0], [-1.0, 0.0, 1.0]],
            Self::CrPdx => &[[1.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            Self::CrPdz => &[[0.0, 0.0, 1.0], [1.0, 0.0, 1.0]],
            Self::CrNdx => &[[1.0, 0.0, 0.0], [-1.0, 0.0, 1.0]],
            Self::CrNdz => &[[0.0, 0.0, 1.0], [-1.0, 0.0, 1.0]],
        }
    }

    /// Surface normal of the shape (ascending shapes are tilted 45 degrees).
    pub fn normal(self) -> V3 {
        let h = std::f64::consts::FRAC_1_SQRT_2;
        match self {
            Self::An => [0.0, h, h],
            Self::As => [0.0, h, -h],
            Self::Ae => [-h, h, 0.0],
            Self::Aw => [h, h, 0.0],
            _ => UP,
        }
    }

    pub fn is_junction(self) -> bool {
        self.axes().len() > 1
    }

    pub fn is_portal(self) -> bool {
        matches!(self, Self::Tn | Self::Ts | Self::Te | Self::Tw)
    }

    pub fn is_ascending(self) -> bool {
        matches!(self, Self::An | Self::As | Self::Ae | Self::Aw)
    }

    /// `asPortal(Direction)`: horizontal direction given as a (dx, dz) unit step.
    pub fn as_portal(dx: i32, dz: i32) -> Self {
        match (dx, dz) {
            (1, 0) => Self::Te,
            (0, -1) => Self::Tn,
            (0, 1) => Self::Ts,
            _ => Self::Tw,
        }
    }

    fn rotate_once(self) -> Self {
        match self {
            Self::Pd => Self::Nd,
            Self::Nd => Self::Pd,
            Self::Xo => Self::Zo,
            Self::Zo => Self::Xo,
            Self::Ae => Self::As,
            Self::As => Self::Aw,
            Self::Aw => Self::An,
            Self::An => Self::Ae,
            Self::CrPdx => Self::CrNdz,
            Self::CrNdz => Self::CrPdx,
            Self::CrNdx => Self::CrPdz,
            Self::CrPdz => Self::CrNdx,
            other => other,
        }
    }

    pub fn rotate(self, quarters: Quarter) -> Self {
        let mut s = self;
        for _ in 0..(quarters % 4) {
            s = s.rotate_once();
        }
        s
    }

    pub fn mirror(self, mirror: Mirror) -> Self {
        let x_mirror = |s: Self| match s {
            Self::Pd => Self::Nd,
            Self::Nd => Self::Pd,
            Self::Ae => Self::Aw,
            Self::Aw => Self::Ae,
            Self::CrPdx => Self::CrNdx,
            Self::CrNdx => Self::CrPdx,
            Self::CrPdz => Self::CrNdz,
            Self::CrNdz => Self::CrPdz,
            o => o,
        };
        let z_mirror = |s: Self| match s {
            Self::Pd => Self::Nd,
            Self::Nd => Self::Pd,
            Self::An => Self::As,
            Self::As => Self::An,
            Self::CrPdx => Self::CrNdx,
            Self::CrNdx => Self::CrPdx,
            Self::CrPdz => Self::CrNdz,
            Self::CrNdz => Self::CrPdz,
            o => o,
        };
        match mirror {
            Mirror::None => self,
            Mirror::FrontBack => x_mirror(self),
            Mirror::LeftRight => z_mirror(self),
        }
    }
}

/// Track material identity. Graph edges only connect when the material type
/// matches (`TrackMaterial.TrackType`); a train needs every bogey's type.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackType(pub String);

impl TrackType {
    pub fn standard() -> Self {
        Self("create:standard".into())
    }
}

// ----------------------------------------------------------- BezierConnection

const SCAN_COUNT: usize = 16;

/// One curved track connection. `end1`/`end2` are the track node positions,
/// `axis1`/`axis2` point from each end into the curve (towards the control
/// point), `normal*` are the surface normals at the ends.
#[derive(Clone, Debug, PartialEq)]
pub struct BezierConnection {
    pub end1: V3,
    pub end2: V3,
    pub axis1: V3,
    pub axis2: V3,
    pub normal1: V3,
    pub normal2: V3,
    pub girder: bool,
    pub finish1: V3,
    pub finish2: V3,
    length: f64,
    segments: usize,
    step_lut: Vec<f64>,
    /// Cumulative chord length at each baked segment boundary (for `t_at_length`).
    arc_table: Vec<f64>,
}

fn cross_y(a: V3) -> V3 {
    cross(a, UP)
}

/// 2D (xz) line intersection `p + t*r == q + u*s`; returns `(t, u)`.
fn intersect_xz(p: V3, r: V3, q: V3, s: V3) -> Option<(f64, f64)> {
    let den = r[0] * s[2] - r[2] * s[0];
    if den.abs() < 1e-9 {
        return None;
    }
    let d = [q[0] - p[0], q[2] - p[2]];
    let t = (d[0] * s[2] - d[1] * s[0]) / den;
    let u = (d[0] * r[2] - d[1] * r[0]) / den;
    Some((t, u))
}

impl BezierConnection {
    pub fn new(end1: V3, end2: V3, axis1: V3, axis2: V3, normal1: V3, normal2: V3, girder: bool) -> Self {
        let handle = Self::determine_handle_length(end1, end2, axis1, axis2);
        let finish1 = add(end1, scale(axis1, handle));
        let finish2 = add(end2, scale(axis2, handle));
        let mut b = Self {
            end1,
            end2,
            axis1,
            axis2,
            normal1,
            normal2,
            girder,
            finish1,
            finish2,
            length: 0.0,
            segments: 0,
            step_lut: Vec::new(),
            arc_table: Vec::new(),
        };
        b.bake();
        b
    }

    /// `Runtime.determineHandles`.
    fn determine_handle_length(end1: V3, end2: V3, axis1: V3, axis2: V3) -> f64 {
        let a1 = (-axis2[2]).atan2(-axis2[0]);
        let a2 = axis1[2].atan2(axis1[0]);
        let pi = std::f64::consts::PI;
        let mut angle = (a1 - a2) % (2.0 * pi);
        if angle > pi {
            angle -= 2.0 * pi;
        }
        if angle < -pi {
            angle += 2.0 * pi;
        }
        let angle = angle.abs();
        let fallback = dist(end1, end2) / 3.0;

        if angle < 1e-4 {
            match intersect_xz(end1, axis1, end2, cross_y(axis2)) {
                Some((t, u)) => {
                    let (t, u) = (t.abs(), u.abs());
                    let (min, max) = (t.min(u), t.max(u));
                    if min > 1.2 && max / min > 1.0 && max / min < 3.0 {
                        max - min
                    } else {
                        fallback
                    }
                }
                None => fallback,
            }
        } else {
            let n = 2.0 * pi / angle;
            let factor = (4.0 / 3.0) * (pi / (2.0 * n)).tan();
            match intersect_xz(end1, cross_y(axis1), end2, cross_y(axis2)) {
                None => fallback,
                Some((t, _)) => {
                    let h = t.abs() * len(cross_y(axis1)) * factor;
                    if h.abs() < 1e-9 { 1.0 } else { h }
                }
            }
        }
    }

    fn bake(&mut self) {
        let mut length = 0.0;
        let mut prev = self.end1;
        for i in 1..=SCAN_COUNT {
            let p = self.position(i as f64 / SCAN_COUNT as f64);
            length += dist(p, prev);
            prev = p;
        }
        self.length = length;
        self.segments = (length * 2.0) as usize;
        let n = self.segments;
        let mut lut = vec![1.0; n + 1];
        let mut arc = vec![0.0; n + 1];
        let mut combined = 0.0;
        let mut prev = self.end1;
        for i in 0..=n {
            let t = if n == 0 { 1.0 } else { i as f64 / n as f64 };
            let p = self.position(t);
            if i > 0 {
                let d = dist(p, prev);
                combined += d / length;
                arc[i] = arc[i - 1] + d;
                lut[i] = t / combined;
            }
            prev = p;
        }
        self.step_lut = lut;
        self.arc_table = arc;
    }

    pub fn length(&self) -> f64 {
        self.length
    }

    pub fn segment_count(&self) -> usize {
        self.segments
    }

    /// `getSegmentT`: parameter of baked segment `index` (uniform in arc length).
    pub fn segment_t(&self, index: usize) -> f64 {
        if index >= self.segments {
            1.0
        } else {
            index as f64 * self.step_lut[index] / self.segments as f64
        }
    }

    /// Cubic Bezier through `end1`..`end2` with control points `finish1/2`.
    pub fn position(&self, t: f64) -> V3 {
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        let mut r = [0.0; 3];
        for k in 0..3 {
            r[k] = a * self.end1[k] + b * self.finish1[k] + c * self.finish2[k] + d * self.end2[k];
        }
        r
    }

    pub fn derivative(&self, t: f64) -> V3 {
        let u = 1.0 - t;
        let d0 = sub(self.finish1, self.end1);
        let d1 = sub(self.finish2, self.finish1);
        let d2 = sub(self.end2, self.finish2);
        add(
            add(scale(d0, 3.0 * u * u), scale(d1, 6.0 * u * t)),
            scale(d2, 3.0 * t * t),
        )
    }

    /// Unit tangent (`derivative` normalised).
    pub fn tangent(&self, t: f64) -> V3 {
        normalize(self.derivative(t))
    }

    /// `incrementT`: advance parameter `t` by arc distance `d` (first-order,
    /// `dt = d / |B'(t)|`). The summarised Java divides by `|B'|/length`; the
    /// Rust port uses the dimensionally consistent form. Train travel uses
    /// `t_at_length` instead, so this is a helper for adapters only.
    pub fn increment_t(&self, t: f64, d: f64) -> f64 {
        let speed = len(self.derivative(t));
        if speed < 1e-12 { t } else { t + d / speed }
    }

    /// `getNormal`: surface normal at `t` (face normal slerped when the ends differ).
    pub fn normal(&self, t: f64) -> V3 {
        let d = self.tangent(t);
        let face = if dot(sub(self.normal1, self.normal2), sub(self.normal1, self.normal2)) < 1e-12 {
            self.normal1
        } else {
            slerp(t, self.normal1, self.normal2)
        };
        let n = normalize(cross(face, d));
        cross(d, n)
    }

    /// Parameter `t` for a distance along the curve measured from `end1`.
    pub fn t_at_length(&self, s: f64) -> f64 {
        if self.segments == 0 {
            return (s / self.length).clamp(0.0, 1.0);
        }
        let total = *self.arc_table.last().unwrap();
        if s <= 0.0 {
            return 0.0;
        }
        if s >= total {
            return 1.0;
        }
        let i = self.arc_table.partition_point(|&a| a < s).max(1);
        let (a0, a1) = (self.arc_table[i - 1], self.arc_table[i]);
        let f = if a1 > a0 { (s - a0) / (a1 - a0) } else { 0.0 };
        let t0 = self.segment_t(i - 1);
        let t1 = self.segment_t(i);
        t0 + (t1 - t0) * f
    }

    /// Position at arc distance `s` from `end1`.
    pub fn position_at_length(&self, s: f64) -> V3 {
        self.position(self.t_at_length(s))
    }

    /// Reverse orientation (the edge seen from the other end).
    pub fn reversed(&self) -> Self {
        Self::new(self.end2, self.end1, self.axis2, self.axis1, self.normal2, self.normal1, self.girder)
    }

    /// `getTrackItemCost`.
    pub fn track_item_cost(&self) -> usize {
        (self.segments + 1) / 2
    }

    /// `getGirderItemCost`.
    pub fn girder_item_cost(&self) -> usize {
        if self.girder { 2 * self.track_item_cost() } else { 0 }
    }

    /// Baked rail sample: `(position, normal)` at segment `index` (rails sit at +-0.965 normal).
    pub fn rail_offsets(&self, index: usize) -> (V3, V3) {
        let t = self.segment_t(index);
        let p = self.position(t);
        let n = normalize(cross(self.normal_face(t), self.tangent(t)));
        (add(p, scale(n, 0.965)), sub(p, scale(n, 0.965)))
    }

    fn normal_face(&self, t: f64) -> V3 {
        if dot(sub(self.normal1, self.normal2), sub(self.normal1, self.normal2)) < 1e-12 {
            self.normal1
        } else {
            slerp(t, self.normal1, self.normal2)
        }
    }

    /// Largest vertical drop of the curve between the ends; used by turn scouting.
    pub fn vertical_drop(&self) -> f64 {
        (self.end2[1] - self.end1[1]).abs()
    }
}

// ------------------------------------------------------------ placement rules

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementError {
    TooFar,
    SecondPoint,
    Perpendicular,
    AscendingSCurve,
    TooSharp,
    TooSteep,
    SlopeTurn,
    OpposingSlopes,
    LeaveSlopeAscending,
    LeaveSlopeDescending,
    Turn90,
}

impl PlacementError {
    /// Translation key suffix (`track.<key>`).
    pub fn key(&self) -> &'static str {
        match self {
            Self::TooFar => "too_far",
            Self::SecondPoint => "second_point",
            Self::Perpendicular => "perpendicular",
            Self::AscendingSCurve => "ascending_s_curve",
            Self::TooSharp => "too_sharp",
            Self::TooSteep => "too_steep",
            Self::SlopeTurn => "slope_turn",
            Self::OpposingSlopes => "opposing_slopes",
            Self::LeaveSlopeAscending => "leave_slope_ascending",
            Self::LeaveSlopeDescending => "leave_slope_descending",
            Self::Turn90 => "turn_90",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConnectionKind {
    /// Straight run; extent is `round((distance + 1) / |axis|)` tiles.
    Straight { extent: i32 },
    /// Parallel-axis offset curve; `t` forward, `u` lateral.
    SCurve { forward: f64, lateral: f64 },
    /// Corner curve; `turn_size` is `min(dist1, dist2) - 0.1`.
    Turn { turn_size: f64, ninety: bool },
    /// Slope transition (flat <-> ascending).
    Slope { turn_size: f64 },
}

/// One end of a prospective connection.
#[derive(Clone, Copy, Debug)]
pub struct PlacementEnd {
    pub pos: V3,
    pub axis: V3,
    pub normal: V3,
}

fn horizontal(a: V3) -> V3 {
    [a[0], 0.0, a[2]]
}

/// Geometric checks of `TrackPlacement.tryConnect`. `max_length` is
/// `trains.maxTrackPlacementLength` (default 32). Axes need not be oriented;
/// they are flipped to face the other end. Material/pavement checks and the
/// distribution of excess length into extents are NOT part of this function.
pub fn check_connection(a: PlacementEnd, b: PlacementEnd, max_length: i32) -> Result<ConnectionKind, PlacementError> {
    let diff = sub(b.pos, a.pos);
    let l2 = dot(diff, diff);
    if l2 > (max_length as f64) * (max_length as f64) {
        return Err(PlacementError::TooFar);
    }
    if l2 < 1e-9 {
        return Err(PlacementError::SecondPoint);
    }
    let dh = horizontal(diff);
    let ascend = diff[1];
    let abs_ascend = ascend.abs();
    let mut axis1 = a.axis;
    if dot(horizontal(axis1), dh) < 0.0 {
        axis1 = scale(axis1, -1.0);
    }
    let mut axis2 = b.axis;
    if dot(horizontal(axis2), dh) > 0.0 {
        axis2 = scale(axis2, -1.0);
    }
    let a1 = horizontal(axis1);
    let a2 = horizontal(axis2);
    let (l1, l2h) = (len(a1), len(a2));
    let cross2 = a1[0] * a2[2] - a1[2] * a2[0];
    let parallel = cross2.abs() < 1e-6 * l1 * l2h;
    let slopes_differ = dot(sub(a.normal, b.normal), sub(a.normal, b.normal)) > 1e-6;

    if parallel {
        let unit1 = scale(a1, 1.0 / l1);
        let forward = dot(unit1, dh);
        let lateral = (unit1[0] * dh[2] - unit1[2] * dh[0]).abs();
        if lateral > 1e-3 {
            if slopes_differ {
                return Err(PlacementError::SlopeTurn);
            }
            if abs_ascend > 1e-3 || axis1[1].abs() > 1e-3 {
                return Err(PlacementError::AscendingSCurve);
            }
            let target_t = if lateral <= 1.0 { 3.0 } else { 2.0 * lateral };
            if forward < target_t {
                return Err(PlacementError::TooSharp);
            }
            return Ok(ConnectionKind::SCurve { forward, lateral });
        }
        if slopes_differ {
            let d = dot(a.normal, b.normal);
            if d.abs() < 1e-3 {
                return Err(PlacementError::OpposingSlopes);
            }
            // Approximation: symmetric transition, each half is half the run.
            let turn_size = len(dh) / 2.0;
            if ascend > 1e-3 && a.normal[2].abs() + a.normal[0].abs() < 1e-6 && a.normal[1] > 0.99 {
                // starting flat and ascending is only legal if axis climbs
                if axis1[1] < -1e-3 {
                    return Err(PlacementError::LeaveSlopeAscending);
                }
            }
            if turn_size < 2.0 {
                return Err(PlacementError::TooSharp);
            }
            return Ok(ConnectionKind::Slope { turn_size });
        }
        // plain straight (possibly with elevation change)
        if abs_ascend > 1e-3 {
            if axis1[1].abs() > 1e-3 && axis1[1].signum() != ascend.signum() {
                return Err(PlacementError::AscendingSCurve);
            }
            let min_h = (if abs_ascend < 4.0 { abs_ascend * 4.0 } else { abs_ascend * 3.0 }).max(6.0) / l1;
            if len(dh) < min_h {
                return Err(PlacementError::TooSteep);
            }
        }
        let extent = ((len(dh) + 1.0) / l1).round() as i32;
        return Ok(ConnectionKind::Straight { extent });
    }

    // Non-parallel: a corner.
    if slopes_differ {
        return Err(PlacementError::SlopeTurn);
    }
    let pi = std::f64::consts::PI;
    let s1 = a1[2].atan2(a1[0]);
    let s2 = a2[2].atan2(a2[0]);
    let mut angle = (s2 - s1).to_degrees();
    while angle < 0.0 {
        angle += 360.0;
    }
    while angle >= 360.0 {
        angle -= 360.0;
    }
    let _ = pi;
    if !(60.0..=300.0).contains(&angle) {
        return Err(PlacementError::Turn90);
    }
    let shifted = (angle + 0.25) % 90.0;
    let ninety = shifted < 1.25 || shifted > 89.0 + 0.25;
    let (t, u) = intersect_xz(a.pos, a1, b.pos, a2).ok_or(PlacementError::TooSharp)?;
    if t < 0.0 || u < 0.0 {
        return Err(PlacementError::TooSharp);
    }
    let turn_size = (t * l1).min(u * l2h) - 0.1;
    let min_turn = if ninety { 7.0 } else { 3.25 };
    let fit = min_turn
        + if ninety {
            (abs_ascend - 3.0).max(0.0) * 2.0
        } else {
            (abs_ascend - 1.5).max(0.0) * 1.5
        };
    if turn_size < min_turn {
        return Err(PlacementError::TooSharp);
    }
    if turn_size < fit {
        return Err(PlacementError::TooSteep);
    }
    Ok(ConnectionKind::Turn { turn_size, ninety })
}

// ---------------------------------------------------------- block connections

/// Per-track-block set of curved connections (`TrackBlockEntity.connections`),
/// keyed by the other end's block position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackConnections {
    items: Vec<([i32; 3], BezierConnection)>,
}

impl TrackConnections {
    pub fn add(&mut self, other: [i32; 3], bezier: BezierConnection) -> bool {
        if self.items.iter().any(|(p, b)| *p == other && b.end1 == bezier.end1 && b.end2 == bezier.end2) {
            return false;
        }
        self.items.push((other, bezier));
        true
    }

    pub fn remove_to(&mut self, other: [i32; 3]) -> usize {
        let before = self.items.len();
        self.items.retain(|(p, _)| *p != other);
        before - self.items.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &([i32; 3], BezierConnection)> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Total item cost to refund when all connections are removed.
    pub fn refund(&self) -> usize {
        self.items.iter().map(|(_, b)| b.track_item_cost()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter(r: f64) -> BezierConnection {
        // heading +z at the start, ending heading +x after a quarter turn
        BezierConnection::new(
            [0.0, 0.0, 0.0],
            [r, 0.0, r],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            UP,
            UP,
            false,
        )
    }

    #[test]
    fn shape_table() {
        assert_eq!(TrackShape::ALL.len(), 19);
        assert!(TrackShape::CrD.is_junction());
        assert!(!TrackShape::Xo.is_junction());
        assert!(TrackShape::Tn.is_portal());
        assert_eq!(TrackShape::parse("cr_ndz"), Some(TrackShape::CrNdz));
        for s in TrackShape::ALL {
            assert_eq!(TrackShape::parse(s.name()), Some(s));
        }
        assert_eq!(TrackShape::As.axes()[0], [0.0, 1.0, 1.0]);
        let n = TrackShape::Ae.normal();
        assert!((n[0] + 0.70710678).abs() < 1e-6 && (n[1] - 0.70710678).abs() < 1e-6);
        assert_eq!(TrackShape::as_portal(1, 0), TrackShape::Te);
        assert_eq!(TrackShape::as_portal(-1, 0), TrackShape::Tw);
    }

    #[test]
    fn shape_rotate_mirror() {
        assert_eq!(TrackShape::Xo.rotate(1), TrackShape::Zo);
        assert_eq!(TrackShape::Xo.rotate(2), TrackShape::Xo);
        assert_eq!(TrackShape::Ae.rotate(1), TrackShape::As);
        assert_eq!(TrackShape::Ae.rotate(4), TrackShape::Ae);
        assert_eq!(TrackShape::Pd.mirror(Mirror::FrontBack), TrackShape::Nd);
        assert_eq!(TrackShape::Ae.mirror(Mirror::FrontBack), TrackShape::Aw);
        assert_eq!(TrackShape::An.mirror(Mirror::FrontBack), TrackShape::An);
        assert_eq!(TrackShape::An.mirror(Mirror::LeftRight), TrackShape::As);
        assert_eq!(TrackShape::CrPdx.mirror(Mirror::None), TrackShape::CrPdx);
        for s in TrackShape::ALL {
            assert_eq!(s.rotate(4), s);
        }
    }

    #[test]
    fn quarter_circle_handle_and_length() {
        let b = quarter(7.0);
        let k = (4.0 / 3.0) * (std::f64::consts::PI / 8.0).tan();
        let handle = 7.0 * k;
        assert!((dist(b.finish1, b.end1) - handle).abs() < 1e-9);
        assert!((dist(b.finish2, b.end2) - handle).abs() < 1e-9);
        let exact = std::f64::consts::FRAC_PI_2 * 7.0;
        assert!((b.length() - exact).abs() / exact < 0.005, "{}", b.length());
        assert_eq!(b.segment_count(), (b.length() * 2.0) as usize);
        assert_eq!(b.segment_count(), 21);
        assert_eq!(b.track_item_cost(), 11);
    }

    #[test]
    fn endpoints_and_tangents() {
        let b = quarter(7.0);
        assert_eq!(b.position(0.0), [0.0, 0.0, 0.0]);
        let e = b.position(1.0);
        assert!(dist(e, [7.0, 0.0, 7.0]) < 1e-9);
        let t0 = b.tangent(0.0);
        assert!(dist(t0, [0.0, 0.0, 1.0]) < 1e-9);
        let t1 = b.tangent(1.0);
        assert!(dist(t1, [1.0, 0.0, 0.0]) < 1e-9);
        // midpoint of a circle arc lies on the circle of radius 7 around (7,0,0)
        let m = b.position(0.5);
        assert!((dist(m, [7.0, 0.0, 0.0]) - 7.0).abs() < 0.01);
        assert!(dist(b.normal(0.3), UP) < 1e-9);
    }

    #[test]
    fn step_lut_makes_segments_uniform() {
        let b = quarter(10.0);
        let n = b.segment_count();
        assert_eq!(b.segment_t(0), 0.0);
        assert_eq!(b.segment_t(n), 1.0);
        let mut prev = b.position(0.0);
        let mut chords = Vec::new();
        for i in 1..=n {
            let p = b.position(b.segment_t(i));
            chords.push(dist(p, prev));
            prev = p;
        }
        let mean = chords.iter().sum::<f64>() / chords.len() as f64;
        for c in &chords {
            assert!((c - mean).abs() / mean < 0.03, "{c} vs {mean}");
        }
    }

    #[test]
    fn increment_and_arc_length_lookup() {
        let b = quarter(7.0);
        let t = b.increment_t(0.0, 0.5);
        assert!(t > 0.0 && t < 0.2);
        // half length lands at mid parameter for a symmetric curve
        let th = b.t_at_length(b.length() / 2.0);
        assert!((th - 0.5).abs() < 0.02);
        assert_eq!(b.t_at_length(-1.0), 0.0);
        assert_eq!(b.t_at_length(1e9), 1.0);
        let p = b.position_at_length(b.length());
        assert!(dist(p, b.end2) < 0.05);
    }

    #[test]
    fn s_curve_handle_rule_and_reverse() {
        // forward 8, lateral 4 -> min 4 > 1.2, ratio 2 -> handle = 4
        let b = BezierConnection::new(
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 8.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            UP,
            UP,
            false,
        );
        assert!((dist(b.finish1, b.end1) - 4.0).abs() < 1e-9);
        // forward 9, lateral 3 -> ratio 3 not < 3 -> |d|/3
        let c = BezierConnection::new(
            [0.0, 0.0, 0.0],
            [3.0, 0.0, 9.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            UP,
            UP,
            false,
        );
        let expect = dist(c.end1, c.end2) / 3.0;
        assert!((dist(c.finish1, c.end1) - expect).abs() < 1e-9);
        let r = b.reversed();
        assert!(dist(r.position(0.0), b.end2) < 1e-9);
        assert!((r.length() - b.length()).abs() < 1e-6);
        assert_eq!(r.reversed().end1, b.end1);
    }

    #[test]
    fn sloped_normals_slerp() {
        let n2 = TrackShape::As.normal();
        let b = BezierConnection::new(
            [0.0, 0.0, 0.0],
            [0.0, 2.0, 12.0],
            [0.0, 0.0, 1.0],
            [0.0, -0.3, -1.0],
            UP,
            n2,
            true,
        );
        let n = b.normal(0.5);
        assert!(n[1] > 0.7);
        assert_eq!(b.girder_item_cost(), 2 * b.track_item_cost());
    }

    fn end(pos: V3, axis: V3) -> PlacementEnd {
        PlacementEnd { pos, axis, normal: UP }
    }

    #[test]
    fn placement_straight_and_range() {
        let r = check_connection(end([0.0; 3], [0.0, 0.0, 1.0]), end([0.0, 0.0, 10.0], [0.0, 0.0, 1.0]), 32);
        assert_eq!(r, Ok(ConnectionKind::Straight { extent: 11 }));
        let r = check_connection(end([0.0; 3], [0.0, 0.0, 1.0]), end([0.0, 0.0, 40.0], [0.0, 0.0, 1.0]), 32);
        assert_eq!(r, Err(PlacementError::TooFar));
        let r = check_connection(end([0.0; 3], [0.0, 0.0, 1.0]), end([0.0; 3], [0.0, 0.0, 1.0]), 32);
        assert_eq!(r, Err(PlacementError::SecondPoint));
        assert_eq!(PlacementError::TooFar.key(), "too_far");
    }

    #[test]
    fn placement_s_curve_rules() {
        let a = end([0.0; 3], [0.0, 0.0, 1.0]);
        // lateral 4 needs forward >= 8
        assert!(matches!(
            check_connection(a, end([4.0, 0.0, 8.0], [0.0, 0.0, 1.0]), 32),
            Ok(ConnectionKind::SCurve { .. })
        ));
        assert_eq!(
            check_connection(a, end([4.0, 0.0, 7.0], [0.0, 0.0, 1.0]), 32),
            Err(PlacementError::TooSharp)
        );
        // lateral <= 1 needs forward >= 3
        assert_eq!(
            check_connection(a, end([1.0, 0.0, 2.0], [0.0, 0.0, 1.0]), 32),
            Err(PlacementError::TooSharp)
        );
        assert_eq!(
            check_connection(a, end([1.0, 2.0, 12.0], [0.0, 0.0, 1.0]), 32),
            Err(PlacementError::AscendingSCurve)
        );
    }

    #[test]
    fn placement_turns() {
        let a = end([0.0; 3], [0.0, 0.0, 1.0]);
        let b = end([8.0, 0.0, 8.0], [1.0, 0.0, 0.0]);
        match check_connection(a, b, 32) {
            Ok(ConnectionKind::Turn { turn_size, ninety }) => {
                assert!(ninety);
                assert!((turn_size - 7.9).abs() < 1e-9);
            }
            other => panic!("{other:?}"),
        }
        // 6.9 turn size is below the 7 minimum for a 90 degree turn
        let tight = end([6.0, 0.0, 6.0], [1.0, 0.0, 0.0]);
        assert_eq!(check_connection(a, tight, 32), Err(PlacementError::TooSharp));
        // too much height for the turn
        let high = end([8.0, 6.0, 8.0], [1.0, 0.0, 0.0]);
        assert_eq!(check_connection(a, high, 32), Err(PlacementError::TooSteep));
    }

    #[test]
    fn placement_steep_straight() {
        let a = end([0.0; 3], [0.0, 0.0, 1.0]);
        // ascent 2 needs horizontal run >= max(8, 6) = 8
        assert_eq!(
            check_connection(a, end([0.0, 2.0, 7.0], [0.0, 0.0, 1.0]), 32),
            Err(PlacementError::TooSteep)
        );
        assert!(check_connection(a, end([0.0, 2.0, 8.0], [0.0, 0.0, 1.0]), 32).is_ok());
    }

    #[test]
    fn block_connections() {
        let mut c = TrackConnections::default();
        let b = quarter(7.0);
        assert!(c.add([7, 0, 7], b.clone()));
        assert!(!c.add([7, 0, 7], b.clone()));
        assert_eq!(c.len(), 1);
        assert_eq!(c.refund(), 11);
        assert_eq!(c.remove_to([7, 0, 7]), 1);
        assert!(c.is_empty());
    }
}
