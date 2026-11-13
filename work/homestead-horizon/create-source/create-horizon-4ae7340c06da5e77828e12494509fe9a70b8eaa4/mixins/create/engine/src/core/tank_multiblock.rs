// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//! Pure Fluid Tank multiblock rules after FluidTankBlock, FluidTankBlockEntity
//! and ConnectivityHandler (Create 6.0.10, revision ac0c444d). Unlike the Item
//! Vault model, tanks use the original greedy search: a controller keeps the
//! largest `width x width x height` column that is consistent with the
//! neighbouring tanks, and any stack of columns up to the height limit forms a
//! tower. Game effects (fluid handlers, block states, light, particles) belong
//! to adapters; this module owns shape, capacity and fluid redistribution.
//!
//! Reconstructed rather than copied: iteration order of a split's fluid
//! redistribution, tick order of re-forming parts and the tie-break between
//! equally ranked candidates are not specified by the source in a way that
//! could be verified offline. They are deterministic here and documented in
//! `docs/tank-boiler-steam.md`.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::kinetics::Pos;

/// `FluidTankBlockEntity.MAX_SIZE`: width limit of a tank.
pub const MAX_WIDTH: i32 = 3;
/// `CFluids.fluidTankMaxHeight` default.
pub const DEFAULT_MAX_HEIGHT: i32 = 32;
/// `CFluids.fluidTankCapacity` default (8 buckets) in millibuckets.
pub const DEFAULT_CAPACITY_PER_BLOCK: i64 = 8_000;

/// Registry id of a fluid, assigned by the adapter.
pub type FluidId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TankConfig {
    pub capacity_per_block: i64,
    pub max_height: i32,
}

impl Default for TankConfig {
    fn default() -> Self {
        Self {
            capacity_per_block: DEFAULT_CAPACITY_PER_BLOCK,
            max_height: DEFAULT_MAX_HEIGHT,
        }
    }
}

/// `FluidTankBlock.Shape`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Plain,
    Window,
    WindowNw,
    WindowSw,
    WindowNe,
    WindowSe,
}

impl Shape {
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Window => "window",
            Self::WindowNw => "window_nw",
            Self::WindowSw => "window_sw",
            Self::WindowNe => "window_ne",
            Self::WindowSe => "window_se",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "plain" => Self::Plain,
            "window" => Self::Window,
            "window_nw" => Self::WindowNw,
            "window_sw" => Self::WindowSw,
            "window_ne" => Self::WindowNe,
            "window_se" => Self::WindowSe,
            _ => return None,
        })
    }

    /// `FluidTankBlock.rotate` for one clockwise quarter turn.
    pub fn rotated_once(self) -> Self {
        match self {
            Self::WindowNe => Self::WindowSe,
            Self::WindowSe => Self::WindowSw,
            Self::WindowSw => Self::WindowNw,
            Self::WindowNw => Self::WindowNe,
            other => other,
        }
    }

    /// `FluidTankBlock.rotate` with `turns` clockwise quarter turns.
    pub fn rotated(self, turns: u32) -> Self {
        (0..turns % 4).fold(self, |s, _| s.rotated_once())
    }

    /// `FluidTankBlock.mirror`; `front_back` is Mirror.FRONT_BACK.
    pub fn mirrored(self, front_back: bool) -> Self {
        if front_back {
            match self {
                Self::WindowNw => Self::WindowNe,
                Self::WindowNe => Self::WindowNw,
                Self::WindowSw => Self::WindowSe,
                Self::WindowSe => Self::WindowSw,
                other => other,
            }
        } else {
            match self {
                Self::WindowNe => Self::WindowSe,
                Self::WindowSe => Self::WindowNe,
                Self::WindowNw => Self::WindowSw,
                Self::WindowSw => Self::WindowNw,
                other => other,
            }
        }
    }
}

/// `setWindows`: the shape of the block at the (x, z) offset inside a column
/// of the given width. The pattern is the same on every layer.
pub fn window_shape(window: bool, width: i32, x: i32, z: i32) -> Shape {
    if !window {
        return Shape::Plain;
    }
    match width {
        1 => Shape::Window,
        2 => match (x == 0, z == 0) {
            (true, true) => Shape::WindowNw,
            (true, false) => Shape::WindowSw,
            (false, true) => Shape::WindowNe,
            (false, false) => Shape::WindowSe,
        },
        3 => {
            if (x.abs() - z.abs()).abs() == 1 {
                Shape::Window
            } else {
                Shape::Plain
            }
        }
        _ => Shape::Plain,
    }
}

/// The `top`, `bottom` and `shape` block-state properties of one tank block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TankState {
    pub top: bool,
    pub bottom: bool,
    pub shape: Shape,
}

/// `ComparatorUtil.fractionToRedstoneLevel`.
pub fn comparator_level(fraction: f64) -> u8 {
    let v = fraction * 14.0 + if fraction > 0.0 { 1.0 } else { 0.0 };
    v.clamp(0.0, 15.0).floor() as u8
}

/// `onFluidStackChanged`: light level of every layer of a column. `window`
/// false means `FluidTankBlock.getLightEmission` is 0 regardless.
pub fn layer_luminosity(
    fill: f32,
    height: i32,
    fluid_light_level: i32,
    lighter_than_air: bool,
) -> Vec<u8> {
    let base = (fluid_light_level as f32 / 1.2) as i32;
    let max_y = (fill * height as f32 + 1.0) as i32;
    (0..height)
        .map(|y| {
            let bright = if lighter_than_air { height - y <= max_y } else { y < max_y };
            if bright {
                base.clamp(0, 15) as u8
            } else if base > 0 {
                1
            } else {
                0
            }
        })
        .collect()
}

/// Sound pitch of a bucket interaction (`FluidTankBlock.useItemOn`), before
/// the +-0.125 random offset: `clamp(1 - amount / (cap * 16), 0, 1) / 1.5 + 0.5`.
pub fn interaction_pitch(amount: i64, capacity_per_block: i64) -> f32 {
    let base = (1.0 - amount as f32 / (capacity_per_block as f32 * 16.0)).clamp(0.0, 1.0);
    base / 1.5 + 0.5
}

/// Raw per-cell record exposed to adapters (see `TankWorld::snapshot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSnapshot {
    pub controller: Pos,
    pub width: i32,
    pub height: i32,
    /// Only a controller holds fluid.
    pub content: Option<(FluidId, i64)>,
    pub window: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cell {
    controller: Pos,
    width: i32,
    height: i32,
    fluid: Option<FluidId>,
    amount: i64,
    window: bool,
    boiler_active: bool,
}

impl Cell {
    fn single(pos: Pos) -> Self {
        Self {
            controller: pos,
            width: 1,
            height: 1,
            fluid: None,
            amount: 0,
            window: true,
            boiler_active: false,
        }
    }
}

/// Outcome of removing a tank.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RemoveReport {
    /// Millibuckets that fitted nowhere after the split.
    pub voided: i64,
}

fn add(p: Pos, d: [i32; 3]) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// A set of tank block entities and their connectivity. All mutation keeps the
/// invariants: members point at an existing controller whose footprint holds
/// them; only controllers store fluid.
#[derive(Clone, Debug)]
pub struct TankWorld {
    cfg: TankConfig,
    cells: BTreeMap<Pos, Cell>,
    /// Parts waiting for their next `updateConnectivity()` tick.
    pending: BTreeSet<Pos>,
}

impl TankWorld {
    pub fn new(cfg: TankConfig) -> Self {
        Self { cfg, cells: BTreeMap::new(), pending: BTreeSet::new() }
    }

    pub fn config(&self) -> TankConfig {
        self.cfg
    }

    pub fn contains(&self, pos: Pos) -> bool {
        self.cells.contains_key(&pos)
    }

    pub fn controller_of(&self, pos: Pos) -> Option<Pos> {
        self.cells.get(&pos).map(|c| c.controller)
    }

    pub fn is_controller(&self, pos: Pos) -> bool {
        self.cells.get(&pos).is_some_and(|c| c.controller == pos)
    }

    /// `(width, height)` of the multiblock `pos` belongs to.
    pub fn size_of(&self, pos: Pos) -> Option<(i32, i32)> {
        let c = self.cells.get(&pos)?;
        let ctrl = self.cells.get(&c.controller)?;
        Some((ctrl.width, ctrl.height))
    }

    /// `getTotalTankSize` of the multiblock.
    pub fn total_size(&self, pos: Pos) -> Option<i32> {
        self.size_of(pos).map(|(w, h)| w * w * h)
    }

    pub fn capacity(&self, pos: Pos) -> Option<i64> {
        self.total_size(pos).map(|n| i64::from(n) * self.cfg.capacity_per_block)
    }

    pub fn fluid(&self, pos: Pos) -> Option<(FluidId, i64)> {
        let ctrl = self.cells.get(&self.cells.get(&pos)?.controller)?;
        ctrl.fluid.filter(|_| ctrl.amount > 0).map(|f| (f, ctrl.amount))
    }

    /// `getFillState`.
    pub fn fill_state(&self, pos: Pos) -> Option<f32> {
        let cap = self.capacity(pos)?;
        let ctrl = self.cells.get(&self.cells.get(&pos)?.controller)?;
        Some(ctrl.amount as f32 / cap as f32)
    }

    pub fn comparator(&self, pos: Pos) -> u8 {
        self.fill_state(pos).map_or(0, |f| comparator_level(f64::from(f)))
    }

    pub fn window(&self, pos: Pos) -> Option<bool> {
        let c = self.cells.get(&self.cells.get(&pos)?.controller)?;
        Some(c.window)
    }

    pub fn boiler_active(&self, pos: Pos) -> bool {
        self.cells
            .get(&pos)
            .and_then(|c| self.cells.get(&c.controller))
            .is_some_and(|c| c.boiler_active)
    }

    /// Positions of all blocks of the multiblock in y, x, z order.
    pub fn members(&self, pos: Pos) -> Vec<Pos> {
        let Some(ctrl_pos) = self.controller_of(pos) else { return Vec::new() };
        let Some((w, h)) = self.size_of(ctrl_pos) else { return Vec::new() };
        let mut out = Vec::new();
        for y in 0..h {
            for x in 0..w {
                for z in 0..w {
                    out.push(add(ctrl_pos, [x, y, z]));
                }
            }
        }
        out
    }

    /// The block-state properties `notifyMultiUpdated` and `setWindows` keep.
    pub fn block_state(&self, pos: Pos) -> Option<TankState> {
        let c = self.cells.get(&pos)?;
        let ctrl = self.cells.get(&c.controller)?;
        let ctrl_pos = c.controller;
        Some(TankState {
            bottom: ctrl_pos[1] == pos[1],
            top: ctrl_pos[1] + ctrl.height - 1 == pos[1],
            shape: window_shape(
                ctrl.window,
                ctrl.width,
                pos[0] - ctrl_pos[0],
                pos[2] - ctrl_pos[2],
            ),
        })
    }

    /// `FluidTankBlock.getLightEmission`.
    pub fn light_emission(&self, pos: Pos, fluid_light_level: i32, lighter_than_air: bool) -> u8 {
        let (Some(c), Some(fill)) = (self.cells.get(&pos), self.fill_state(pos)) else { return 0 };
        let Some(ctrl) = self.cells.get(&c.controller) else { return 0 };
        if !ctrl.window {
            return 0;
        }
        let layers = layer_luminosity(fill, ctrl.height, fluid_light_level, lighter_than_air);
        layers[(pos[1] - c.controller[1]) as usize]
    }

    // ----- fluid access (always through the controller) -----

    /// `FluidTank.fill`; returns the amount accepted.
    pub fn fill(&mut self, pos: Pos, fluid: FluidId, amount: i64, simulate: bool) -> i64 {
        let Some(cap) = self.capacity(pos) else { return 0 };
        let ctrl_pos = self.cells[&pos].controller;
        let ctrl = self.cells.get_mut(&ctrl_pos).unwrap();
        if amount <= 0 || (ctrl.amount > 0 && ctrl.fluid != Some(fluid)) {
            return 0;
        }
        let accepted = amount.min(cap - ctrl.amount).max(0);
        if !simulate && accepted > 0 {
            ctrl.fluid = Some(fluid);
            ctrl.amount += accepted;
        }
        accepted
    }

    /// `FluidTank.drain`; returns what was taken.
    pub fn drain(&mut self, pos: Pos, amount: i64, simulate: bool) -> Option<(FluidId, i64)> {
        let ctrl_pos = self.cells.get(&pos)?.controller;
        let ctrl = self.cells.get_mut(&ctrl_pos)?;
        let fluid = ctrl.fluid.filter(|_| ctrl.amount > 0)?;
        let taken = amount.min(ctrl.amount);
        if taken <= 0 {
            return None;
        }
        if !simulate {
            ctrl.amount -= taken;
            if ctrl.amount == 0 {
                ctrl.fluid = None;
            }
        }
        Some((fluid, taken))
    }

    // ----- windows / boiler hooks -----

    /// Wrench on a tank (`toggleWindows`); blocked while the boiler is active.
    pub fn toggle_windows(&mut self, pos: Pos) -> bool {
        let Some(ctrl_pos) = self.controller_of(pos) else { return false };
        let ctrl = self.cells.get_mut(&ctrl_pos).unwrap();
        if ctrl.boiler_active {
            return false;
        }
        ctrl.window = !ctrl.window;
        true
    }

    /// `updateBoilerState` result: becoming active forces windows off.
    pub fn set_boiler_active(&mut self, pos: Pos, active: bool) {
        let Some(ctrl_pos) = self.controller_of(pos) else { return };
        let ctrl = self.cells.get_mut(&ctrl_pos).unwrap();
        if active && !ctrl.boiler_active {
            ctrl.window = false;
        }
        ctrl.boiler_active = active;
    }

    // ----- placement / removal -----

    /// Place a tank (optionally with a stored fluid, e.g. from an item) and run
    /// `onPlace` -> `updateConnectivity` plus the following re-form ticks.
    pub fn place(&mut self, pos: Pos, content: Option<(FluidId, i64)>) {
        if self.cells.contains_key(&pos) {
            return;
        }
        let mut cell = Cell::single(pos);
        if let Some((f, a)) = content {
            cell.fluid = Some(f);
            cell.amount = a.clamp(0, self.cfg.capacity_per_block);
        }
        self.cells.insert(pos, cell);
        self.form_multi(pos);
        self.update_connectivity();
    }

    /// Break a tank: `onRemove` -> `splitMulti`, then the surviving parts
    /// re-form on their next tick.
    pub fn remove(&mut self, pos: Pos) -> RemoveReport {
        let Some(ctrl) = self.controller_of(pos) else { return RemoveReport::default() };
        let voided = self.split_multi(ctrl, Some(pos));
        self.cells.remove(&pos);
        self.pending.remove(&pos);
        self.update_connectivity();
        RemoveReport { voided }
    }

    /// Restore a persisted cell without running any formation (an adapter
    /// loads saved block entities with this). `content` is only meaningful on
    /// a controller. Replaces an existing cell at `pos`.
    pub fn restore(
        &mut self,
        pos: Pos,
        controller: Pos,
        width: i32,
        height: i32,
        content: Option<(FluidId, i64)>,
        window: bool,
    ) {
        let (fluid, amount) = match content {
            Some((f, a)) if a > 0 && controller == pos => (Some(f), a),
            _ => (None, 0),
        };
        self.cells.insert(
            pos,
            Cell {
                controller,
                width: width.clamp(1, MAX_WIDTH),
                height: height.clamp(1, self.cfg.max_height),
                fluid,
                amount,
                window,
                boiler_active: false,
            },
        );
    }

    /// All cell positions in key order.
    pub fn positions(&self) -> Vec<Pos> {
        self.cells.keys().copied().collect()
    }

    /// The raw record of one cell (not resolved through its controller).
    pub fn snapshot(&self, pos: Pos) -> Option<CellSnapshot> {
        self.cells.get(&pos).map(|c| CellSnapshot {
            controller: c.controller,
            width: c.width,
            height: c.height,
            content: c.fluid.filter(|_| c.amount > 0).map(|f| (f, c.amount)),
            window: c.window,
        })
    }

    /// Break several tanks at once (cells that vanished from the world without
    /// a break event): every affected multiblock is split first, the broken
    /// cells are dropped, and only then do the survivors re-form. Their shares
    /// of the pool are voided like in `remove`.
    pub fn remove_all(&mut self, positions: &[Pos]) -> RemoveReport {
        let mut voided = 0;
        for &pos in positions {
            let Some(ctrl) = self.controller_of(pos) else { continue };
            voided += self.split_multi(ctrl, Some(pos));
            self.cells.remove(&pos);
            self.pending.remove(&pos);
        }
        // A broken cell that was a single still owns its own share.
        self.update_connectivity();
        RemoveReport { voided }
    }

    /// Run queued `updateConnectivity()` ticks until the world is stable.
    pub fn update_connectivity(&mut self) {
        let mut guard = 0;
        while let Some(&pos) = self.pending.iter().next() {
            self.pending.remove(&pos);
            guard += 1;
            if guard > 10_000 {
                break;
            }
            if self.is_controller(pos) {
                self.form_multi(pos);
            }
        }
    }

    // ----- ConnectivityHandler -----

    fn part_ok(&self, origin: Pos, width: i32, pos: Pos, fluid: &mut Option<FluidId>) -> bool {
        let Some(part) = self.cells.get(&pos) else { return false };
        let Some(ctrl) = self.cells.get(&part.controller) else { return false };
        let c = part.controller;
        if ctrl.width > width
            || (c != origin && ctrl.width == width && ctrl.height == self.cfg.max_height)
        {
            return false;
        }
        if c != origin
            && (c[0] < origin[0]
                || c[1] < origin[1]
                || c[2] < origin[2]
                || c[0] > origin[0] + width - 1
                || c[2] > origin[2] + width - 1)
        {
            return false;
        }
        if ctrl.amount > 0 {
            match *fluid {
                Some(f) if Some(f) != ctrl.fluid => return false,
                _ => *fluid = ctrl.fluid,
            }
        }
        true
    }

    /// `tryToFormNewMultiOfWidth` in simulation: returns the block count.
    fn simulate_width(&self, origin: Pos, width: i32) -> i32 {
        let mut fluid = self
            .cells
            .get(&origin)
            .and_then(|c| c.fluid.filter(|_| c.amount > 0));
        let mut amount = 0;
        'layers: for y in 0..self.cfg.max_height {
            for x in 0..width {
                for z in 0..width {
                    if !self.part_ok(origin, width, add(origin, [x, y, z]), &mut fluid) {
                        break 'layers;
                    }
                }
            }
            amount += width * width;
        }
        amount
    }

    fn best_width(&self, origin: Pos) -> (i32, i32) {
        let mut best = (1, -1);
        for w in 1..=MAX_WIDTH {
            let amount = self.simulate_width(origin, w);
            if amount >= best.1 {
                best = (w, amount);
            }
        }
        best
    }

    /// `tryToFormNewMulti`; returns the best simulated block count.
    fn try_form(&mut self, origin: Pos, simulate: bool) -> i32 {
        if !self.is_controller(origin) {
            return 0;
        }
        let (width, amount) = self.best_width(origin);
        if simulate || amount <= 0 {
            return amount.max(0);
        }
        let (cw, ch) = self.size_of(origin).unwrap();
        if cw == width && ch == amount / width / width {
            return amount;
        }
        self.split_multi(origin, None);
        self.absorb(origin, width, amount / width / width);
        amount
    }

    /// Real formation: the origin takes over every block of the footprint.
    fn absorb(&mut self, origin: Pos, width: i32, height: i32) {
        let mut positions = Vec::new();
        for y in 0..height {
            for x in 0..width {
                for z in 0..width {
                    positions.push(add(origin, [x, y, z]));
                }
            }
        }
        let window = self.cells[&origin].window;
        let mut fluid = self.cells[&origin].fluid.filter(|_| self.cells[&origin].amount > 0);
        let mut total = self.cells[&origin].amount;
        for &pos in &positions {
            if pos == origin {
                continue;
            }
            let old_ctrl = self.cells[&pos].controller;
            if old_ctrl != pos || self.cells[&pos].width != 1 || self.cells[&pos].height != 1 {
                self.split_multi(old_ctrl, None);
            }
            // After the split `pos` is a single again.
            let part = self.cells.get_mut(&pos).unwrap();
            if part.amount > 0 {
                fluid = fluid.or(part.fluid);
                total += part.amount;
            }
            part.amount = 0;
            part.fluid = None;
            part.controller = origin;
            part.width = width;
            part.height = height;
            part.window = window;
            self.pending.remove(&pos);
        }
        let cap = i64::from(width * width * height) * self.cfg.capacity_per_block;
        let o = self.cells.get_mut(&origin).unwrap();
        o.width = width;
        o.height = height;
        o.amount = total.min(cap);
        o.fluid = if o.amount > 0 { fluid } else { None };
    }

    /// `ConnectivityHandler.formMulti`: gather candidate controllers by a flood
    /// fill towards the minimum corner, then form them from the largest down.
    pub fn form_multi(&mut self, seed: Pos) {
        if !self.cells.contains_key(&seed) {
            return;
        }
        let bound = [seed[0] - MAX_WIDTH, seed[2] - MAX_WIDTH];
        let mut visited = BTreeSet::new();
        let mut frontier = VecDeque::from([seed]);
        let mut queue: Vec<(i32, usize, Pos)> = Vec::new();
        while let Some(pos) = frontier.pop_front() {
            if !visited.insert(pos) {
                continue;
            }
            let best = self.try_form(pos, true);
            if best > 1 {
                queue.push((best, queue.len(), pos));
            }
            for d in [[-1, 0, 0], [0, -1, 0], [0, 0, -1]] {
                let next = add(pos, d);
                let out = (d[0] != 0 && next[0] <= bound[0]) || (d[2] != 0 && next[2] <= bound[1]);
                if !out && self.cells.contains_key(&next) && !visited.contains(&next) {
                    frontier.push_back(next);
                }
            }
        }
        queue.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut done = BTreeSet::new();
        for (_, _, pos) in queue {
            if done.contains(&pos) || !self.is_controller(pos) {
                continue;
            }
            self.try_form(pos, false);
            if let Some((w, h)) = self.size_of(pos) {
                for y in 0..h {
                    for x in 0..w {
                        for z in 0..w {
                            done.insert(add(pos, [x, y, z]));
                        }
                    }
                }
            }
        }
    }

    /// `splitMultiAndInvalidate`: every part becomes a single again, the
    /// controller keeps one block of fluid and the surplus fills the other
    /// parts bottom-up. Returns the voided amount.
    fn split_multi(&mut self, origin: Pos, removed: Option<Pos>) -> i64 {
        let Some(ctrl) = self.cells.get(&origin).cloned() else { return 0 };
        if ctrl.controller != origin || (ctrl.width == 1 && ctrl.height == 1) {
            return 0;
        }
        let per = self.cfg.capacity_per_block;
        let kept = ctrl.amount.min(per);
        let mut surplus = ctrl.amount - kept;
        let fluid = ctrl.fluid;
        let mut members = Vec::new();
        for y in 0..ctrl.height {
            for x in 0..ctrl.width {
                for z in 0..ctrl.width {
                    members.push(add(origin, [x, y, z]));
                }
            }
        }
        for pos in members {
            let Some(cell) = self.cells.get_mut(&pos) else { continue };
            if Some(pos) == removed {
                continue;
            }
            cell.controller = pos;
            cell.width = 1;
            cell.height = 1;
            cell.boiler_active = false;
            if pos == origin {
                cell.amount = kept;
                if kept == 0 {
                    cell.fluid = None;
                }
            } else {
                let take = surplus.min(per);
                cell.amount = take;
                cell.fluid = if take > 0 { fluid } else { None };
                surplus -= take;
            }
            self.pending.insert(pos);
        }
        if removed == Some(origin) {
            // The removed controller's own share is lost with the block.
            return surplus + kept;
        }
        surplus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATER: FluidId = 1;
    const LAVA: FluidId = 2;

    fn world() -> TankWorld {
        TankWorld::new(TankConfig::default())
    }

    fn build(w: &mut TankWorld, origin: Pos, width: i32, height: i32) {
        for y in 0..height {
            for x in 0..width {
                for z in 0..width {
                    w.place(add(origin, [x, y, z]), None);
                }
            }
        }
    }

    #[test]
    fn a_single_tank_holds_eight_buckets() {
        let mut w = world();
        w.place([0, 0, 0], None);
        assert_eq!(w.capacity([0, 0, 0]), Some(8_000));
        assert_eq!(w.total_size([0, 0, 0]), Some(1));
        let s = w.block_state([0, 0, 0]).unwrap();
        assert!(s.top && s.bottom);
        assert_eq!(s.shape, Shape::Window);
    }

    #[test]
    fn stacking_forms_a_tower_with_the_lowest_block_as_controller() {
        let mut w = world();
        for y in 0..5 {
            w.place([2, 10 + y, 2], None);
        }
        for y in 0..5 {
            assert_eq!(w.controller_of([2, 10 + y, 2]), Some([2, 10, 2]));
        }
        assert_eq!(w.size_of([2, 12, 2]), Some((1, 5)));
        assert_eq!(w.capacity([2, 14, 2]), Some(40_000));
        assert!(w.block_state([2, 10, 2]).unwrap().bottom);
        assert!(!w.block_state([2, 10, 2]).unwrap().top);
        assert!(w.block_state([2, 14, 2]).unwrap().top);
        let mid = w.block_state([2, 12, 2]).unwrap();
        assert!(!mid.top && !mid.bottom);
    }

    #[test]
    fn placing_below_an_existing_tower_adopts_it() {
        let mut w = world();
        for y in 1..4 {
            w.place([0, y, 0], None);
        }
        w.place([0, 0, 0], None);
        assert_eq!(w.size_of([0, 3, 0]), Some((1, 4)));
        assert_eq!(w.controller_of([0, 3, 0]), Some([0, 0, 0]));
    }

    #[test]
    fn a_full_three_by_three_column_forms() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 3, 3);
        assert_eq!(w.size_of([1, 1, 1]), Some((3, 3)));
        assert_eq!(w.controller_of([2, 2, 2]), Some([0, 0, 0]));
        assert_eq!(w.capacity([0, 0, 0]), Some(27 * 8_000));
    }

    #[test]
    fn two_by_two_towers_use_corner_windows() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 2, 2);
        assert_eq!(w.size_of([1, 1, 1]), Some((2, 2)));
        assert_eq!(w.block_state([0, 0, 0]).unwrap().shape, Shape::WindowNw);
        assert_eq!(w.block_state([0, 1, 1]).unwrap().shape, Shape::WindowSw);
        assert_eq!(w.block_state([1, 0, 0]).unwrap().shape, Shape::WindowNe);
        assert_eq!(w.block_state([1, 1, 1]).unwrap().shape, Shape::WindowSe);
    }

    #[test]
    fn three_wide_windows_sit_on_the_edge_midpoints() {
        let windows: Vec<(i32, i32)> = (0..3)
            .flat_map(|x| (0..3).map(move |z| (x, z)))
            .filter(|&(x, z)| window_shape(true, 3, x, z) == Shape::Window)
            .collect();
        assert_eq!(windows, vec![(0, 1), (1, 0), (1, 2), (2, 1)]);
        assert_eq!(window_shape(true, 3, 1, 1), Shape::Plain);
        assert_eq!(window_shape(false, 2, 0, 0), Shape::Plain);
    }

    #[test]
    fn height_is_limited_by_the_config() {
        let mut w = TankWorld::new(TankConfig { capacity_per_block: 8_000, max_height: 4 });
        for y in 0..6 {
            w.place([0, y, 0], None);
        }
        // The first four form a tower; the search never exceeds the limit.
        assert!(w.size_of([0, 0, 0]).unwrap().1 <= 4);
        let members: i32 = (0..6).filter(|&y| w.controller_of([0, y, 0]) == Some([0, 0, 0])).count() as i32;
        assert_eq!(members, 4);
    }

    #[test]
    fn default_height_limit_is_thirty_two() {
        let mut w = world();
        for y in 0..40 {
            w.place([0, y, 0], None);
        }
        assert_eq!(w.size_of([0, 0, 0]), Some((1, 32)));
        assert_eq!(w.capacity([0, 0, 0]), Some(256_000));
    }

    #[test]
    fn fluid_is_pooled_on_merge() {
        let mut w = world();
        w.place([0, 0, 0], Some((WATER, 5_000)));
        w.place([0, 1, 0], Some((WATER, 6_000)));
        assert_eq!(w.fluid([0, 1, 0]), Some((WATER, 11_000)));
        assert_eq!(w.fill_state([0, 0, 0]), Some(11_000.0 / 16_000.0));
        // Non-controllers hold nothing themselves.
        assert_eq!(w.cells[&[0, 1, 0]].amount, 0);
    }

    #[test]
    fn different_fluids_do_not_merge() {
        let mut w = world();
        w.place([0, 0, 0], Some((WATER, 100)));
        w.place([0, 1, 0], Some((LAVA, 100)));
        assert_eq!(w.size_of([0, 0, 0]), Some((1, 1)));
        assert_eq!(w.size_of([0, 1, 0]), Some((1, 1)));
        assert_eq!(w.fluid([0, 1, 0]), Some((LAVA, 100)));
    }

    #[test]
    fn an_empty_tank_joins_a_filled_one() {
        let mut w = world();
        w.place([0, 0, 0], Some((LAVA, 100)));
        w.place([0, 1, 0], None);
        assert_eq!(w.size_of([0, 1, 0]), Some((1, 2)));
        assert_eq!(w.fluid([0, 0, 0]), Some((LAVA, 100)));
    }

    #[test]
    fn filling_and_draining_go_through_the_controller() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 1, 2);
        assert_eq!(w.fill([0, 1, 0], WATER, 20_000, true), 16_000);
        assert_eq!(w.fill([0, 1, 0], WATER, 20_000, false), 16_000);
        assert_eq!(w.fill([0, 0, 0], LAVA, 1, false), 0);
        assert_eq!(w.drain([0, 1, 0], 4_000, false), Some((WATER, 4_000)));
        assert_eq!(w.fluid([0, 0, 0]), Some((WATER, 12_000)));
        assert_eq!(w.drain([0, 0, 0], 99_999, false), Some((WATER, 12_000)));
        assert_eq!(w.fluid([0, 0, 0]), None);
    }

    #[test]
    fn removing_the_top_keeps_a_smaller_tower_and_drops_the_overflow() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 1, 3);
        w.fill([0, 0, 0], WATER, 24_000, false);
        let report = w.remove([0, 2, 0]);
        assert_eq!(w.size_of([0, 0, 0]), Some((1, 2)));
        // 24000 -> origin 8000 + surplus 16000 over the one remaining part
        // (8000) + the removed block's 8000 which is voided.
        assert_eq!(report.voided, 8_000);
        assert_eq!(w.fluid([0, 0, 0]), Some((WATER, 16_000)));
    }

    #[test]
    fn splitting_the_middle_redistributes_bottom_up() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 1, 5);
        w.fill([0, 0, 0], LAVA, 40_000, false);
        w.remove([0, 2, 0]);
        // Lower half: y0 keeps one block, y1 takes the next block. The
        // upper half only holds what flowed into y3, y4 afterwards.
        assert_eq!(w.size_of([0, 0, 0]), Some((1, 2)));
        assert_eq!(w.size_of([0, 3, 0]), Some((1, 2)));
        assert_eq!(w.fluid([0, 0, 0]), Some((LAVA, 16_000)));
        assert_eq!(w.fluid([0, 3, 0]), Some((LAVA, 16_000)));
    }

    #[test]
    fn removing_the_controller_promotes_the_next_lowest_block() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 1, 3);
        w.fill([0, 0, 0], WATER, 12_000, false);
        w.remove([0, 0, 0]);
        assert_eq!(w.controller_of([0, 1, 0]), Some([0, 1, 0]));
        assert_eq!(w.size_of([0, 2, 0]), Some((1, 2)));
        assert_eq!(w.fluid([0, 1, 0]), Some((WATER, 4_000)));
    }

    #[test]
    fn a_wide_tower_splits_into_columns_that_regroup() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 2, 2);
        w.remove([1, 1, 1]);
        // 7 blocks remain; the greedy search keeps the biggest consistent
        // columns and nothing is left dangling.
        for pos in [[0, 0, 0], [1, 0, 0], [0, 0, 1], [1, 0, 1], [0, 1, 0], [1, 1, 0], [0, 1, 1]] {
            let c = w.controller_of(pos).unwrap();
            let (cw, ch) = w.size_of(pos).unwrap();
            assert!(pos[0] >= c[0] && pos[0] < c[0] + cw);
            assert!(pos[1] >= c[1] && pos[1] < c[1] + ch);
            assert!(pos[2] >= c[2] && pos[2] < c[2] + cw);
        }
        let total: i32 = w
            .cells
            .keys()
            .filter(|&&p| w.is_controller(p))
            .map(|&p| w.total_size(p).unwrap())
            .sum();
        assert_eq!(total, 7);
    }

    #[test]
    fn a_two_wide_layer_merges_into_an_existing_column() {
        let mut w = world();
        for y in 0..3 {
            w.place([0, y, 0], None);
        }
        // Add the neighbouring column block by block.
        for y in 0..3 {
            w.place([1, y, 0], None);
        }
        // Three singles stacked are columns; the 2-wide group is 6 blocks.
        let cells: BTreeSet<Pos> = (0..3).flat_map(|y| [[0, y, 0], [1, y, 0]]).collect();
        assert_eq!(cells.len(), 6);
        // A 2-wide search needs a full 2x2 layer; a 1x2 strip is not one, so
        // the two columns of height three stay separate.
        assert_eq!(w.size_of([0, 0, 0]), Some((1, 3)));
        assert_eq!(w.size_of([1, 0, 0]), Some((1, 3)));
        // Completing the second row of z merges everything into a 2x3.
        for y in 0..3 {
            w.place([0, y, 1], None);
            w.place([1, y, 1], None);
        }
        assert_eq!(w.size_of([1, 1, 1]), Some((2, 3)));
        assert_eq!(w.controller_of([1, 2, 1]), Some([0, 0, 0]));
    }

    #[test]
    fn an_over_wide_neighbour_is_not_swallowed() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 3, 2);
        // A far away column of width 1 on the same level does not join.
        w.place([5, 0, 0], None);
        assert_eq!(w.size_of([5, 0, 0]), Some((1, 1)));
        assert_eq!(w.size_of([0, 0, 0]), Some((3, 2)));
    }

    #[test]
    fn toggling_windows_changes_every_shape_and_is_blocked_by_a_boiler() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 2, 2);
        assert!(w.toggle_windows([1, 1, 1]));
        assert_eq!(w.block_state([1, 1, 1]).unwrap().shape, Shape::Plain);
        assert!(w.toggle_windows([0, 0, 0]));
        assert_eq!(w.block_state([1, 1, 1]).unwrap().shape, Shape::WindowSe);
        w.set_boiler_active([0, 0, 0], true);
        assert_eq!(w.window([0, 0, 0]), Some(false));
        assert!(!w.toggle_windows([0, 0, 0]));
        assert!(w.boiler_active([1, 1, 1]));
    }

    #[test]
    fn a_split_clears_the_boiler_flag() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 1, 3);
        w.set_boiler_active([0, 2, 0], true);
        w.remove([0, 1, 0]);
        assert!(!w.boiler_active([0, 0, 0]));
    }

    #[test]
    fn window_flag_survives_merging() {
        let mut w = world();
        w.place([0, 0, 0], None);
        w.toggle_windows([0, 0, 0]);
        w.place([0, 1, 0], None);
        assert_eq!(w.window([0, 1, 0]), Some(false));
        assert_eq!(w.block_state([0, 1, 0]).unwrap().shape, Shape::Plain);
    }

    #[test]
    fn comparator_matches_the_original_formula() {
        assert_eq!(comparator_level(0.0), 0);
        assert_eq!(comparator_level(0.0001), 1);
        assert_eq!(comparator_level(0.5), 8);
        assert_eq!(comparator_level(1.0), 15);
        let mut w = world();
        w.place([0, 0, 0], Some((WATER, 4_000)));
        assert_eq!(w.comparator([0, 0, 0]), 8);
        assert_eq!(w.comparator([9, 9, 9]), 0);
    }

    #[test]
    fn luminosity_follows_the_fluid_level() {
        // Lava (15) -> 12 at full strength; layers above the level fall to 1.
        let l = layer_luminosity(0.5, 4, 15, false);
        assert_eq!(l, vec![12, 12, 12, 1]);
        let full = layer_luminosity(1.0, 4, 15, false);
        assert_eq!(full, vec![12; 4]);
        assert_eq!(layer_luminosity(0.5, 2, 0, false), vec![0, 0]);
        // Gases fill from the top.
        assert_eq!(layer_luminosity(0.0, 3, 15, true), vec![1, 1, 12]);
    }

    #[test]
    fn light_emission_requires_windows() {
        let mut w = world();
        w.place([0, 0, 0], Some((LAVA, 8_000)));
        assert_eq!(w.light_emission([0, 0, 0], 15, false), 12);
        w.toggle_windows([0, 0, 0]);
        assert_eq!(w.light_emission([0, 0, 0], 15, false), 0);
    }

    #[test]
    fn interaction_pitch_range() {
        assert!((interaction_pitch(0, 8_000) - (1.0 / 1.5 + 0.5)).abs() < 1e-6);
        assert!((interaction_pitch(8_000 * 16, 8_000) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn shape_rotation_and_mirror() {
        assert_eq!(Shape::WindowNe.rotated(1), Shape::WindowSe);
        assert_eq!(Shape::WindowNe.rotated(4), Shape::WindowNe);
        assert_eq!(Shape::WindowNw.rotated(2), Shape::WindowSe);
        assert_eq!(Shape::Window.rotated(3), Shape::Window);
        assert_eq!(Shape::WindowNw.mirrored(true), Shape::WindowNe);
        assert_eq!(Shape::WindowNw.mirrored(false), Shape::WindowSw);
        assert_eq!(Shape::parse("window_se"), Some(Shape::WindowSe));
        assert_eq!(Shape::parse("nope"), None);
    }

    #[test]
    fn restore_round_trips_a_saved_tower_without_reforming() {
        let mut w = world();
        w.restore([0, 0, 0], [0, 0, 0], 1, 3, Some((WATER, 12_000)), true);
        w.restore([0, 1, 0], [0, 0, 0], 1, 3, None, true);
        w.restore([0, 2, 0], [0, 0, 0], 1, 3, None, true);
        assert_eq!(w.size_of([0, 2, 0]), Some((1, 3)));
        assert_eq!(w.fluid([0, 1, 0]), Some((WATER, 12_000)));
        let s = w.snapshot([0, 0, 0]).unwrap();
        assert_eq!(s.content, Some((WATER, 12_000)));
        assert_eq!(w.snapshot([0, 1, 0]).unwrap().content, None);
        assert_eq!(w.positions().len(), 3);
    }

    #[test]
    fn remove_all_splits_once_and_reforms_the_survivors() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 1, 5);
        w.fill([0, 0, 0], WATER, 40_000, false);
        let report = w.remove_all(&[[0, 2, 0], [0, 3, 0]]);
        assert!(w.snapshot([0, 2, 0]).is_none() && w.snapshot([0, 3, 0]).is_none());
        assert_eq!(w.size_of([0, 0, 0]), Some((1, 2)));
        assert_eq!(w.size_of([0, 4, 0]), Some((1, 1)));
        // 40000 = 8000 kept + 32000 surplus spread over y1, y3, y4; the broken
        // y2's share (8000) is the reported void. y3's later share is lost with
        // its cell, like removing a single.
        assert_eq!(report.voided, 8_000);
        assert_eq!(w.fluid([0, 0, 0]), Some((WATER, 16_000)));
        assert_eq!(w.fluid([0, 4, 0]), Some((WATER, 8_000)));
    }

    #[test]
    fn a_tower_of_three_by_three_reaches_288_blocks() {
        let mut w = world();
        build(&mut w, [0, 0, 0], 3, 32);
        assert_eq!(w.size_of([1, 31, 1]), Some((3, 32)));
        assert_eq!(w.total_size([0, 0, 0]), Some(288));
        assert_eq!(w.capacity([0, 0, 0]), Some(288 * 8_000));
    }
}
