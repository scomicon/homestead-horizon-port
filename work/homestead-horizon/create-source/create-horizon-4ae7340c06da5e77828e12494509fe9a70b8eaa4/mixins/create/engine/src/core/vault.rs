// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Item Vault multiblock rules after ItemVaultBlock / ItemVaultBlockEntity
// and ConnectivityHandler (Create 6.0.10). Simplification: a group of
// face-adjacent vaults of one axis merges only when it is an exact
// width x width x length cuboid; otherwise every member stays a single vault.
// Game effects (inventory view, block states) belong to adapters.
use std::collections::BTreeSet;

pub const SLOTS_PER_BLOCK: usize = 20;
pub const MAX_WIDTH: i32 = 3;

/// Longest side along the vault axis for a given width (`getMaxLength`).
pub fn max_length(width: i32) -> i32 {
    width * 3
}

/// Horizontal long axis of a vault block (`HORIZONTAL_AXIS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Long {
    X,
    Z,
}

impl Long {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "x" => Some(Self::X),
            "z" => Some(Self::Z),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Z => "z",
        }
    }

    fn step(self) -> [i32; 3] {
        match self {
            Self::X => [1, 0, 0],
            Self::Z => [0, 0, 1],
        }
    }
}

/// A formed (or single) vault. `origin` is the controller, the minimum corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cuboid {
    pub origin: [i32; 3],
    pub axis: Long,
    pub width: i32,
    pub length: i32,
}

impl Cuboid {
    pub fn single(pos: [i32; 3], axis: Long) -> Self {
        Self { origin: pos, axis, width: 1, length: 1 }
    }

    pub fn volume(&self) -> usize {
        (self.width * self.width * self.length) as usize
    }

    pub fn slots(&self) -> usize {
        self.volume() * SLOTS_PER_BLOCK
    }

    /// `ItemVaultBlock.LARGE`: the block belongs to a multi-block vault.
    pub fn large(&self) -> bool {
        self.volume() > 1
    }

    /// Members in the order their slots appear in the shared inventory:
    /// along the long axis first, then up, then across.
    pub fn members(&self) -> Vec<[i32; 3]> {
        let along = self.axis.step();
        let across = match self.axis {
            Long::X => [0, 0, 1],
            Long::Z => [1, 0, 0],
        };
        let mut out = Vec::with_capacity(self.volume());
        for c in 0..self.width {
            for y in 0..self.width {
                for l in 0..self.length {
                    out.push([
                        self.origin[0] + along[0] * l + across[0] * c,
                        self.origin[1] + y,
                        self.origin[2] + along[2] * l + across[2] * c,
                    ]);
                }
            }
        }
        out
    }

    pub fn contains(&self, pos: [i32; 3]) -> bool {
        self.members().contains(&pos)
    }
}

/// The face-adjacent group of same-axis vaults that contains `seed`.
pub fn group(
    seed: [i32; 3],
    axis: Long,
    axis_at: impl Fn([i32; 3]) -> Option<Long>,
) -> BTreeSet<[i32; 3]> {
    let mut seen = BTreeSet::new();
    if axis_at(seed) != Some(axis) {
        return seen;
    }
    let mut stack = vec![seed];
    seen.insert(seed);
    while let Some(at) = stack.pop() {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            let next = [at[0] + d[0], at[1] + d[1], at[2] + d[2]];
            if seen.len() < 4096 && axis_at(next) == Some(axis) && seen.insert(next) {
                stack.push(next);
            }
        }
    }
    seen
}

/// A group that is an exact cuboid within the size limits, else `None`.
pub fn cuboid_of(group: &BTreeSet<[i32; 3]>, axis: Long) -> Option<Cuboid> {
    let min = [0, 1, 2].map(|i| group.iter().map(|p| p[i]).min().unwrap_or(0));
    let max = [0, 1, 2].map(|i| group.iter().map(|p| p[i]).max().unwrap_or(0));
    let size = [0, 1, 2].map(|i| max[i] - min[i] + 1);
    let (length, across) = match axis {
        Long::X => (size[0], size[2]),
        Long::Z => (size[2], size[0]),
    };
    let width = size[1];
    if group.is_empty()
        || width != across
        || width > MAX_WIDTH
        || length > max_length(width)
        || group.len() != (width * width * length) as usize
    {
        return None;
    }
    Some(Cuboid { origin: min, axis, width, length })
}

/// The vault a block belongs to: the group's cuboid, or the block alone.
pub fn resolve(
    pos: [i32; 3],
    axis_at: impl Fn([i32; 3]) -> Option<Long>,
) -> Option<Cuboid> {
    let axis = axis_at(pos)?;
    let group = group(pos, axis, &axis_at);
    Some(cuboid_of(&group, axis).unwrap_or_else(|| Cuboid::single(pos, axis)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(blocks: &[[i32; 3]], axis: Long) -> impl Fn([i32; 3]) -> Option<Long> {
        let set: BTreeSet<[i32; 3]> = blocks.iter().copied().collect();
        move |p| set.contains(&p).then_some(axis)
    }

    fn solid(w: i32, l: i32, axis: Long) -> Vec<[i32; 3]> {
        Cuboid { origin: [4, 70, 8], axis, width: w, length: l }.members()
    }

    #[test]
    fn a_single_vault_is_its_own_controller() {
        let at = world(&[[1, 2, 3]], Long::X);
        let c = resolve([1, 2, 3], at).unwrap();
        assert_eq!(c, Cuboid::single([1, 2, 3], Long::X));
        assert!(!c.large());
        assert_eq!(c.slots(), SLOTS_PER_BLOCK);
    }

    #[test]
    fn exact_cuboids_merge_with_the_minimum_corner_as_controller() {
        let blocks = solid(3, 9, Long::Z);
        let at = world(&blocks, Long::Z);
        let c = resolve([6, 72, 12], &at).unwrap();
        assert_eq!(c.origin, [4, 70, 8]);
        assert_eq!((c.width, c.length), (3, 9));
        assert!(c.large());
        assert_eq!(c.slots(), 3 * 3 * 9 * SLOTS_PER_BLOCK);
        assert_eq!(c.members().len(), 81);
        assert_eq!(c.members()[0], c.origin);
        assert!(c.contains([6, 72, 16]));
    }

    #[test]
    fn size_limits_and_shape_are_enforced() {
        // 1 x 1 x 4 is longer than three times the width: stays single.
        let blocks = solid(1, 4, Long::X);
        let at = world(&blocks, Long::X);
        assert_eq!(resolve([5, 70, 8], &at).unwrap().volume(), 1);
        // 2 x 2 x 6 is the longest 2-wide vault.
        let blocks = solid(2, 6, Long::X);
        assert_eq!(resolve([4, 70, 8], world(&blocks, Long::X)).unwrap().length, 6);
        // An incomplete layer breaks the cuboid.
        let mut blocks = solid(2, 2, Long::X);
        blocks.pop();
        assert_eq!(resolve([4, 70, 8], world(&blocks, Long::X)).unwrap().volume(), 1);
    }

    #[test]
    fn different_axes_never_join() {
        let at = |p: [i32; 3]| match p {
            [0, 0, 0] => Some(Long::X),
            [1, 0, 0] => Some(Long::Z),
            _ => None,
        };
        assert_eq!(resolve([0, 0, 0], at).unwrap().volume(), 1);
    }
}
