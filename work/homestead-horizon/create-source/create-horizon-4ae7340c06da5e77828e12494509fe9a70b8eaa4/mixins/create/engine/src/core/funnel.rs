// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure BeltFunnelBlock / BeltFunnelInteractionHandler geometry, Create 6.0.10
// ac0c444d9828da3453ae8cc65338e8de063286fb. Inventory effects belong to adapters.
use crate::{kinetics::Axis, placement::Direction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Retracted,
    Extended,
    Pushing,
    Pulling,
}

impl Shape {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "retracted" => Some(Self::Retracted),
            "extended" => Some(Self::Extended),
            "pushing" => Some(Self::Pushing),
            "pulling" => Some(Self::Pulling),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Retracted => "retracted",
            Self::Extended => "extended",
            Self::Pushing => "pushing",
            Self::Pulling => "pulling",
        }
    }

    pub fn for_belt(
        belt_facing: Direction,
        funnel_facing: Direction,
        extracting: bool,
    ) -> Option<Self> {
        if belt_facing.axis() == Axis::Y || funnel_facing.axis() == Axis::Y {
            None
        } else if belt_facing.axis() == funnel_facing.axis() {
            Some(Self::Retracted)
        } else if extracting {
            Some(Self::Pushing)
        } else {
            Some(Self::Pulling)
        }
    }

    /// Horizontal bare belts support all four original Wrench transitions.
    pub fn wrenched(self) -> Self {
        match self {
            Self::Retracted => Self::Extended,
            Self::Extended => Self::Retracted,
            Self::Pushing => Self::Pulling,
            Self::Pulling => Self::Pushing,
        }
    }
}

/// A source traversal visits the current segment before later ones, including
/// extended mouths crossed before their segment's center. No allocation.
#[derive(Clone, Debug)]
pub struct Segments {
    segment: i32,
    length: i32,
    next: f32,
    positive: bool,
    valid: bool,
}

pub fn segments(position: f32, next: f32, length: i32, positive: bool) -> Segments {
    let valid = length > 0 && position.is_finite() && next.is_finite();
    Segments {
        segment: if valid {
            (position.floor() as i32).clamp(0, length - 1)
        } else {
            0
        },
        length,
        next,
        positive,
        valid,
    }
}

impl Iterator for Segments {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.valid || self.segment < 0 || self.segment >= self.length {
            return None;
        }
        let segment = self.segment;
        if if self.positive {
            segment as f32 > self.next
        } else {
            ((segment + 1) as f32) < self.next
        } {
            self.valid = false;
            return None;
        }
        self.segment += if self.positive { 1 } else { -1 };
        Some(segment)
    }
}

pub fn entry(segment: i32, shape: Shape, positive: bool) -> f32 {
    let center = segment as f32 + 0.5;
    if shape == Shape::Extended {
        center + 0.499 * if positive { -1.0 } else { 1.0 }
    } else {
        center
    }
}

pub fn crosses(next: f32, entry: f32, positive: bool) -> bool {
    if positive { next > entry } else { next < entry }
}

/// None: this funnel does not take from this movement. Some(true): opposed,
/// blocking mouth. Some(false): lateral collection, whose refusal passes cargo.
pub fn taking(facing: Direction, movement: Direction, shape: Shape) -> Option<bool> {
    if facing.axis() == Axis::Y
        || movement.axis() == Axis::Y
        || facing == movement
        || shape == Shape::Pushing
    {
        None
    } else {
        Some(facing == movement.opposite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_uses_belt_axis_and_wrench_preserves_each_original_pair() {
        assert_eq!(
            Shape::for_belt(Direction::South, Direction::North, false),
            Some(Shape::Retracted)
        );
        assert_eq!(
            Shape::for_belt(Direction::South, Direction::West, true),
            Some(Shape::Pushing)
        );
        assert_eq!(
            Shape::for_belt(Direction::South, Direction::West, false),
            Some(Shape::Pulling)
        );
        assert_eq!(Shape::for_belt(Direction::South, Direction::Up, true), None);
        assert_eq!(Shape::Retracted.wrenched(), Shape::Extended);
        assert_eq!(Shape::Extended.wrenched(), Shape::Retracted);
        assert_eq!(Shape::Pushing.wrenched(), Shape::Pulling);
        assert_eq!(Shape::Pulling.wrenched(), Shape::Pushing);
    }

    #[test]
    fn traversal_covers_multiple_mouths_and_reverses_from_the_current_segment() {
        assert_eq!(segments(0.8, 2.2, 5, true).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(segments(2.2, 0.8, 5, false).collect::<Vec<_>>(), [2, 1, 0]);
        assert_eq!(segments(-0.25, 0.1, 5, true).collect::<Vec<_>>(), [0]);
        assert_eq!(segments(5.2, 4.8, 5, false).collect::<Vec<_>>(), [4]);
        assert!(segments(f32::NAN, 1.0, 5, true).next().is_none());
        assert!(segments(0.0, f32::INFINITY, 5, true).next().is_none());
        assert!(segments(0.0, 1.0, 0, true).next().is_none());
    }

    #[test]
    fn interception_is_strict_and_extended_mouths_use_the_leading_edge() {
        assert!(!crosses(1.5, entry(1, Shape::Retracted, true), true));
        assert!(crosses(1.5001, entry(1, Shape::Retracted, true), true));
        assert!(!crosses(1.5, entry(1, Shape::Pulling, false), false));
        assert!(crosses(1.4999, entry(1, Shape::Pulling, false), false));
        assert!((entry(1, Shape::Extended, true) - 1.001).abs() < 0.000001);
        assert!((entry(1, Shape::Extended, false) - 1.999).abs() < 0.000001);
    }

    #[test]
    fn opposed_mouth_blocks_while_side_mouth_can_pass_rejected_cargo() {
        assert_eq!(
            taking(Direction::North, Direction::South, Shape::Retracted),
            Some(true)
        );
        assert_eq!(
            taking(Direction::West, Direction::South, Shape::Pulling),
            Some(false)
        );
        assert_eq!(
            taking(Direction::West, Direction::South, Shape::Pushing),
            None
        );
        assert_eq!(
            taking(Direction::South, Direction::South, Shape::Extended),
            None
        );
        assert_eq!(
            taking(Direction::South, Direction::North, Shape::Extended),
            Some(true)
        );
    }
}
