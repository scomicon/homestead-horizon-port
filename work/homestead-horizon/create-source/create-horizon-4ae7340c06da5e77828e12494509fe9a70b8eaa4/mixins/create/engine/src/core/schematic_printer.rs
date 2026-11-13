// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure print-order cursor after SchematicPrinter (Create 6.0.10): the
// BLOCKS -> DEFERRED_BLOCKS -> ENTITIES stages, the X-fastest / Z / Y scan with
// its off-by-one edge columns, and the FIFO queue of deferred ("brittle")
// blocks. Loading the schematic into a level, requirement lookup and world
// queries stay with the adapter (`PrinterBackend` in schematicannon.rs).
use std::collections::VecDeque;

pub type Pos = [i32; 3];

/// Inclusive integer box (`BoundingBox`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub min: Pos,
    pub max: Pos,
}

impl Bounds {
    pub fn new(min: Pos, max: Pos) -> Self {
        Self { min, max }
    }
    /// Box spanning `[0, size-1]` on each axis.
    pub fn from_size(size: Pos) -> Self {
        Self { min: [0; 3], max: [size[0] - 1, size[1] - 1, size[2] - 1] }
    }
    /// `getXSpan` etc.: the inclusive extent (`max - min + 1`).
    pub fn span(&self) -> Pos {
        [
            self.max[0] - self.min[0] + 1,
            self.max[1] - self.min[1] + 1,
            self.max[2] - self.min[2] + 1,
        ]
    }
    pub fn contains(&self, p: Pos) -> bool {
        (0..3).all(|i| p[i] >= self.min[i] && p[i] <= self.max[i])
    }
    /// `BoundingBox.encapsulate(pos)`.
    pub fn encapsulate(&mut self, p: Pos) {
        for i in 0..3 {
            self.min[i] = self.min[i].min(p[i]);
            self.max[i] = self.max[i].max(p[i]);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintStage {
    Blocks,
    DeferredBlocks,
    Entities,
}

#[derive(Clone, Debug)]
pub struct PrintCursor {
    pub bounds: Bounds,
    /// Position relative to the anchor.
    pub current_pos: Pos,
    pub stage: PrintStage,
    pub entity_index: i32,
    pub deferred: VecDeque<Pos>,
    /// Entity block positions relative to the anchor, in list order.
    pub entities: Vec<Pos>,
}

impl PrintCursor {
    /// `loadSchematic` tail: cursor starts one column before the first cell.
    pub fn new(bounds: Bounds, entities: Vec<Pos>) -> Self {
        Self {
            current_pos: [bounds.min[0] - 1, bounds.min[1], bounds.min[2]],
            bounds,
            stage: PrintStage::Blocks,
            entity_index: -1,
            deferred: VecDeque::new(),
            entities,
        }
    }

    /// `tryAdvanceCurrentPos`: step east, wrap rows/layers, and report whether the
    /// new cell must be deferred. Mirrors the original checks exactly, including
    /// comparing the absolute `y` with the Y span and using the pre-wrap Z.
    fn try_advance(&mut self, is_deferred: &dyn Fn(Pos) -> bool) -> bool {
        let b = self.bounds;
        let span = b.span();
        self.current_pos[0] += 1;
        let in_bounds = [
            self.current_pos[0] - b.min[0],
            self.current_pos[1] - b.min[1],
            self.current_pos[2] - b.min[2],
        ];
        if in_bounds[0] > span[0] {
            // (minX, y, z + 1).west()
            self.current_pos = [b.min[0] - 1, self.current_pos[1], self.current_pos[2] + 1];
        }
        if in_bounds[2] > span[2] {
            // (x, y + 1, minZ).west()
            self.current_pos = [self.current_pos[0] - 1, self.current_pos[1] + 1, b.min[2]];
        }
        if self.current_pos[1] > span[1] {
            self.stage = PrintStage::DeferredBlocks;
            return false;
        }
        is_deferred(self.current_pos)
    }

    /// `advanceCurrentPos`: true while there is something left to print.
    /// `is_deferred` receives anchor-relative positions (Gantry Carriage, Mechanical
    /// Arm and `BlockMovementChecks.isBrittle` blocks are deferred in the original).
    pub fn advance(&mut self, is_deferred: &dyn Fn(Pos) -> bool) -> bool {
        loop {
            if self.stage == PrintStage::Blocks {
                while self.try_advance(is_deferred) {
                    self.deferred.push_back(self.current_pos);
                }
            }
            if self.stage == PrintStage::DeferredBlocks {
                match self.deferred.pop_front() {
                    None => self.stage = PrintStage::Entities,
                    Some(p) => self.current_pos = p,
                }
            }
            if self.stage == PrintStage::Entities {
                if ((self.entity_index + 1) as usize) < self.entities.len() {
                    self.entity_index += 1;
                    self.current_pos = self.entities[self.entity_index as usize];
                } else {
                    return false;
                }
            }
            if self.bounds.contains(self.current_pos) {
                return true;
            }
        }
    }

    /// `getCurrentTarget` in world space.
    pub fn target(&self, anchor: Pos) -> Pos {
        [
            anchor[0] + self.current_pos[0],
            anchor[1] + self.current_pos[1],
            anchor[2] + self.current_pos[2],
        ]
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.bounds, std::mem::take(&mut self.entities));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(c: &mut PrintCursor, defer: &dyn Fn(Pos) -> bool) -> Vec<Pos> {
        let mut out = Vec::new();
        while c.advance(defer) {
            out.push(c.current_pos);
            assert!(out.len() < 10_000);
        }
        out
    }

    #[test]
    fn scans_x_fastest_then_z_then_y() {
        let mut c = PrintCursor::new(Bounds::from_size([2, 2, 2]), vec![]);
        let v = collect(&mut c, &|_| false);
        assert_eq!(
            v,
            vec![
                [0, 0, 0], [1, 0, 0], [0, 0, 1], [1, 0, 1],
                [0, 1, 0], [1, 1, 0], [0, 1, 1], [1, 1, 1],
            ]
        );
    }

    #[test]
    fn visits_every_cell_exactly_once_for_odd_sizes() {
        let mut c = PrintCursor::new(Bounds::from_size([3, 4, 5]), vec![]);
        let v = collect(&mut c, &|_| false);
        assert_eq!(v.len(), 3 * 4 * 5);
        let mut s = v.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), v.len());
        assert!(v.iter().all(|p| Bounds::from_size([3, 4, 5]).contains(*p)));
    }

    #[test]
    fn deferred_blocks_come_last_in_discovery_order() {
        let mut c = PrintCursor::new(Bounds::from_size([3, 1, 1]), vec![]);
        let v = collect(&mut c, &|p| p[0] == 0 || p[0] == 1);
        assert_eq!(v, vec![[2, 0, 0], [0, 0, 0], [1, 0, 0]]);
    }

    #[test]
    fn entities_follow_blocks_and_out_of_bounds_ones_are_skipped() {
        let mut c = PrintCursor::new(
            Bounds::from_size([1, 1, 1]),
            vec![[0, 0, 0], [5, 5, 5], [0, 0, 0]],
        );
        let v = collect(&mut c, &|_| false);
        assert_eq!(v, vec![[0, 0, 0], [0, 0, 0], [0, 0, 0]]);
        assert_eq!(c.stage, PrintStage::Entities);
        assert_eq!(c.entity_index, 2);
    }

    #[test]
    fn deferred_stage_then_entities() {
        let mut c = PrintCursor::new(Bounds::from_size([2, 1, 1]), vec![[1, 0, 0]]);
        let v = collect(&mut c, &|p| p == [0, 0, 0]);
        assert_eq!(v, vec![[1, 0, 0], [0, 0, 0], [1, 0, 0]]);
    }

    #[test]
    fn finished_cursor_keeps_returning_false() {
        let mut c = PrintCursor::new(Bounds::from_size([1, 1, 1]), vec![]);
        assert!(c.advance(&|_| false));
        assert!(!c.advance(&|_| false));
        assert!(!c.advance(&|_| false));
    }

    #[test]
    fn nonzero_min_y_keeps_original_absolute_y_check() {
        // The original compares absolute y with the Y span: with minY = 2 and a
        // span of 2 the second layer (y = 3) is already past the limit.
        let b = Bounds::new([0, 2, 0], [0, 3, 0]);
        let mut c = PrintCursor::new(b, vec![]);
        let v = collect(&mut c, &|_| false);
        assert_eq!(v, vec![[0, 2, 0]]);
    }

    #[test]
    fn target_is_anchor_plus_relative() {
        let mut c = PrintCursor::new(Bounds::from_size([2, 1, 1]), vec![]);
        c.advance(&|_| false);
        c.advance(&|_| false);
        assert_eq!(c.target([10, 20, 30]), [11, 20, 30]);
    }

    #[test]
    fn reset_restarts_scan() {
        let mut c = PrintCursor::new(Bounds::from_size([2, 1, 1]), vec![[0, 0, 0]]);
        while c.advance(&|_| false) {}
        c.reset();
        assert_eq!(c.stage, PrintStage::Blocks);
        assert_eq!(c.entity_index, -1);
        assert_eq!(c.entities.len(), 1);
        assert!(c.advance(&|_| false));
        assert_eq!(c.current_pos, [0, 0, 0]);
    }

    #[test]
    fn bounds_encapsulate_extends_for_rotated_corner() {
        let mut b = Bounds::from_size([2, 2, 2]);
        b.encapsulate([-3, 0, 1]);
        assert_eq!(b.min, [-3, 0, 0]);
        assert_eq!(b.max, [1, 1, 1]);
    }
}
