// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//! Bounded item-filling phase and callbacks from Create 6.0.10's
//! SpoutBlockEntity at ac0c444d9828da3453ae8cc65338e8de063286fb.
//! The adapter owns recipes, item/fluid transactions, sounds and transport.

pub const FILLING_TIME: i32 = 20;
pub const EXCHANGE_TICK: i32 = 5;

/// The current input and fluid lookup, re-evaluated by the platform adapter.
/// An absent requirement means this fluid has no filling recipe for the input.
/// Amounts are millibuckets, not Fabric transfer units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub can_item_be_filled: bool,
    pub fluid_present: bool,
    pub required_amount: Option<u32>,
    pub available_fluid: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Pass,
    Hold,
    /// Set the source phase to 20 and publish its ordinary start effects.
    Start,
    /// Attempt one item transaction; retain the source phase at five.
    Fill,
}

/// Entry never starts the timer. A fillable item waits for an empty tank;
/// an input that cannot use the current fluid is allowed through instead.
pub fn received(context: Context) -> Decision {
    if !context.can_item_be_filled {
        Decision::Pass
    } else if !context.fluid_present {
        Decision::Hold
    } else if context.required_amount.is_none() {
        Decision::Pass
    } else {
        Decision::Hold
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cycle {
    ticks: i32,
}

impl Default for Cycle {
    fn default() -> Self {
        Self { ticks: -1 }
    }
}

impl Cycle {
    /// Unknown save phases are unsupported; do not silently reset their work.
    pub fn from_ticks(ticks: i32) -> Option<Self> {
        (-1..=FILLING_TIME)
            .contains(&ticks)
            .then_some(Self { ticks })
    }

    pub fn ticks(self) -> i32 {
        self.ticks
    }

    /// The ordinary block-entity tick. It continues through zero to idle.
    pub fn tick(&mut self) -> bool {
        if self.ticks >= 0 {
            self.ticks -= 1;
            true
        } else {
            false
        }
    }

    /// Source held-item callback. Starting and exchanging recheck input,
    /// fluid and amount. World writes must still be atomic in the adapter.
    /// As in the source, an exchange does not introduce a new saved latch;
    /// the host calls the callback once per transported item per game tick.
    pub fn held(&mut self, context: Context) -> Decision {
        if self.ticks != -1 && self.ticks != EXCHANGE_TICK {
            return Decision::Hold;
        }
        if !context.can_item_be_filled {
            return Decision::Pass;
        }
        if !context.fluid_present {
            return Decision::Hold;
        }
        let Some(required) = context.required_amount else {
            return Decision::Pass;
        };
        if required > context.available_fluid {
            return Decision::Hold;
        }
        if self.ticks == -1 {
            self.ticks = FILLING_TIME;
            Decision::Start
        } else {
            Decision::Fill
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUCKET: Context = Context {
        can_item_be_filled: true,
        fluid_present: true,
        required_amount: Some(1000),
        available_fluid: 1000,
    };

    #[test]
    fn receipt_waits_for_empty_tanks_but_passes_incompatible_inputs_and_fluids() {
        assert_eq!(received(BUCKET), Decision::Hold);
        assert_eq!(
            received(Context {
                available_fluid: 999,
                ..BUCKET
            }),
            Decision::Hold
        );
        assert_eq!(
            received(Context {
                fluid_present: false,
                required_amount: None,
                available_fluid: 0,
                ..BUCKET
            }),
            Decision::Hold
        );
        assert_eq!(
            received(Context {
                required_amount: None,
                ..BUCKET
            }),
            Decision::Pass
        );
        assert_eq!(
            received(Context {
                can_item_be_filled: false,
                fluid_present: false,
                ..BUCKET
            }),
            Decision::Pass
        );
    }

    #[test]
    fn idle_start_rechecks_amount_and_does_not_consume_on_receipt() {
        let mut cycle = Cycle::default();
        assert_eq!(received(BUCKET), Decision::Hold);
        assert_eq!(cycle.ticks(), -1);
        assert_eq!(
            cycle.held(Context {
                available_fluid: 999,
                ..BUCKET
            }),
            Decision::Hold
        );
        assert_eq!(cycle.ticks(), -1);
        assert_eq!(cycle.held(BUCKET), Decision::Start);
        assert_eq!(cycle.ticks(), 20);
        assert_eq!(cycle.held(BUCKET), Decision::Hold);
    }

    #[test]
    fn one_callback_each_tick_exchanges_once_and_finishes_its_cooldown() {
        let mut cycle = Cycle::default();
        assert_eq!(cycle.held(BUCKET), Decision::Start);
        let mut exchanges = Vec::new();
        for elapsed in 1..=20 {
            assert!(cycle.tick());
            match cycle.held(BUCKET) {
                Decision::Fill => exchanges.push((elapsed, cycle.ticks())),
                Decision::Hold => {}
                other => panic!("unexpected {other:?} during the active cycle"),
            }
        }
        assert_eq!(exchanges, [(15, 5)]);
        assert_eq!(cycle.ticks(), 0);
        assert!(cycle.tick());
        assert_eq!(cycle.ticks(), -1);
        // A held remainder starts a new cycle, rather than bulk-converting
        // again during the first cycle's six trailing phase ticks.
        assert_eq!(cycle.held(BUCKET), Decision::Start);
        assert_eq!(cycle.ticks(), 20);
    }

    #[test]
    fn exchange_rechecks_inputs_fluid_and_amount_before_the_adapter_writes() {
        let at_exchange = Cycle::from_ticks(5).unwrap();
        for (context, expected) in [
            (
                Context {
                    can_item_be_filled: false,
                    ..BUCKET
                },
                Decision::Pass,
            ),
            (
                Context {
                    fluid_present: false,
                    ..BUCKET
                },
                Decision::Hold,
            ),
            (
                Context {
                    required_amount: None,
                    ..BUCKET
                },
                Decision::Pass,
            ),
            (
                Context {
                    available_fluid: 999,
                    ..BUCKET
                },
                Decision::Hold,
            ),
            (BUCKET, Decision::Fill),
        ] {
            let mut cycle = at_exchange;
            assert_eq!(cycle.held(context), expected);
            assert_eq!(cycle.ticks(), 5);
        }
    }

    #[test]
    fn active_and_trailing_phases_hold_even_when_the_current_input_changes() {
        let unsupported = Context {
            can_item_be_filled: false,
            fluid_present: false,
            required_amount: None,
            available_fluid: 0,
        };
        for ticks in (0..=20).filter(|&ticks| ticks != 5) {
            let mut cycle = Cycle::from_ticks(ticks).unwrap();
            assert_eq!(cycle.held(unsupported), Decision::Hold);
            assert_eq!(cycle.ticks(), ticks);
        }
        assert_eq!(Cycle::default().held(unsupported), Decision::Pass);
    }

    #[test]
    fn restored_work_keeps_its_exchange_phase_and_unknown_phases_are_rejected() {
        let mut restored = Cycle::from_ticks(6).unwrap();
        assert_eq!(restored.held(BUCKET), Decision::Hold);
        restored.tick();
        assert_eq!(restored.held(BUCKET), Decision::Fill);
        restored.tick();
        assert_eq!(restored.held(BUCKET), Decision::Hold);
        for unsupported in [i32::MIN, -2, 21, i32::MAX] {
            assert!(Cycle::from_ticks(unsupported).is_none());
        }
        let mut idle = Cycle::from_ticks(-1).unwrap();
        for _ in 0..30 {
            assert!(!idle.tick());
        }
        assert_eq!(idle.ticks(), -1);
    }
}
