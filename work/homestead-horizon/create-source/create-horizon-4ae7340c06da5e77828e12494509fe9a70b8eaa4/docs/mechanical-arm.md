# Mechanical Arm (pure rules)

Module: `mixins/create/engine/src/core/arm.rs` (rules, 45 unit tests) and the game adapter`mixins/create/arm_runtime.rs`. Source: Create 6.0.10
(`ac0c444d9828da3453ae8cc65338e8de063286fb`), package
`content/kinetics/mechanicalArm`.

## In game (stage 10)

The block `create:mechanical_arm` (property `ceiling`) exists and is accepted by `Run-Acceptance.ps1 -Arm`:
it is a small cog on the Y axis in the kinetic graph (stress 2, like the Crafter), the Arm item selects
points (right click on a point block: first click = input, second = output; sneak to place instead;
left click removes), a placed Arm takes the selection within range 5, ticks with the kinetic speed and moves
a stack between two ordinary inventories. Points persist in the block entity NBT (`Points`, relative
positions).

Transfers are connected for: ordinary inventories (point type `Container`, added by the port), Basin, Depot, Millstone, Saw, Crushing Wheels (deposit), Belt (deposit and extraction), and, covered by the per-point acceptance (`-Points`, 14 scenarios), Funnel, Chute, Mechanical Crafter, Deployer, Blaze Burner (fuel), Campfire, Composter, Jukebox and Respawn Anchor. The held item is drawn in the claw and dropped when the arm is destroyed. Packager is supported as a point type: inserting a package unpacks it into the inventory behind the Packager, and extraction reads the Packager's output slot (the finished box). Acceptance: `-Arm` (chest to barrel with point selection) and `-Points` (one scenario per point type; the checks look for the item in the target block, not for exact Create amounts). Not done: goggles use, wrench, ponder, advancements. The pose drawing uses guessed pivots
and angle signs and has not been looked at in game.

Note on provenance: the Java was read through a summarising fetch tool, not as
raw files. Constants and control flow below come from those summaries and were
not diffed against the raw source; see "Unverified".

## Ported

| Java | Rust |
| --- | --- |
| `ArmBlockEntity.Phase`, `SelectionMode` | `Phase`, `SelectionMode` (names, language keys) |
| `ArmBlockEntity.tick/lazyTick/tickMovementProgress` (server) | `Arm::tick`, `lazy_tick`, `tick_movement`, `progress_step` (`min(256,\|speed\|)/1024`, clamp 1) |
| `searchForItem/searchForDestination/selectIndex` | `Arm::search_for_item`, `search_for_destination` (round robin, forced round robin, prefer first, index wrap) |
| `getDistributableAmount/simulateInsertion` | private `distributable`, `simulate_insertion` |
| `collectItem/depositItem` | `Arm::collect_item`, `deposit_item` (events: pickup sound, awards) |
| `redstoneUpdate`, `checkForMusic` | `Arm::redstone_update`, `check_for_music` |
| `initInteractionPoints` (split by mode, awards at 10 outputs / blaze burner) | `Arm::set_points` |
| `ArmBlock` empty-hand use, goggles, destroy | `take_held`, `apply_goggles`, `destroy` |
| `ArmInteractionPoint` (mode, cycle, serialize/deserialize, transformPos) | `Point`, `Mode`, `PointRecord`, `transform_record` |
| `ArmInteractionPointType.getPrimaryType` + `AllArmInteractionPointTypes` | `PointKind`, `BlockProbe`, `primary_type` (16 types, registration order) |
| per-type geometry (`getInteractionPositionVector/Direction`) | `PointKind::target_vector`, `interaction_direction` |
| deposit-only types, fixed slot counts (jukebox 1, composter 2) | `deposit_only`, `fixed_slot_count` |
| Funnel/Campfire/Respawn Anchor/Jukebox insert preconditions, Belt `keepAlive` | `funnel_accepts`, `campfire_insert`, `respawn_anchor_insert`, `jukebox_insert`, `jukebox_can_extract`, `lock_first` |
| `ArmAngleTarget` | `AngleTarget::solve` (two-link IK, floor and ceiling), `NO_TARGET`, `wrist` check helper |
| client angle interpolation | `pose`, `angle_lerp`, `shortest_angle_diff` |
| `ArmInteractionPointHandler` (right/left click, `flushSettings`) | `Selection`, `in_range` (strictly < range^2, `DEFAULT_RANGE` = 5) |
| `isAreaActuallyLoaded(range + 1)` | `required_chunks` |

World access is the `ArmWorld<I>` trait (`is_valid`, `slot_count`, `extract`,
`insert`, `keep_alive`, `jukebox_has_record`); stacks are `Stack<I>` with an
adapter-defined identity `I` (item + components).

## Not ported

- Rendering (`ArmRenderer`, visual, instancing), the model/animation, sounds.
- `ArmItem`/`ArmPlacementPacket` networking, outline rendering, wrench loading.
- NBT encode/decode (only record and enum-name shapes are provided).
- Config file (`mechanicalArmRange` is a parameter), advancement registration.
- Real handlers: composter container wrapper, campfire cooking recipe lookup,
  jukebox/anchor block entities, funnel filtering/flap, blaze burner
  `tryInsert` and its dropped remainder, crafter extraction window, belt
  `lockedExternally`. Only the pure preconditions exist.
- `SmartBlockEntity` lazy-tick cadence (the adapter calls `lazy_tick`).
- Harvester, plough and portable storage interface are not arm point types in
  6.0.10 (nothing registered); Track Station maps to the Depot type.

## Unverified

- Raw Java was not diffed. Specifically unconfirmed: client pose details in
  `tickMovementProgress` (how the base angle falls back), the exact period of
  `lazyTick`, whether `redstoneUpdate` guards by phase, and the blaze burner
  `insert` remainder handling.
- Angle numbers: the solver was validated by forward kinematics (the wrist
  reaches the target for floor and ceiling arms), not against Java output. Float
  vs double rounding (Java mixes both) is not reproduced.
- Pure rules are not evidence of visual or in-game parity.

## Integration

1. `lib.rs` (add next to `vault`):
   ```rust
   #[path = "core/arm.rs"]
   pub mod arm;
   ```
2. Block entity data (per arm): `inputs`/`outputs` points as `PointRecord` list
   (`Type`, relative `Pos`, `Mode`), `Phase`, `HeldItem`, `TargetPointIndex`
   (`chased_index`), `MovementProgress`, `Powered` (`redstone_locked`),
   `Goggles`, `selection_mode`, `lastInputIndex/lastOutputIndex`. Block state:
   `ceiling` boolean.
3. Ticks: call `Arm::tick(kinetic_speed, &mut world)` each server tick, plus
   `lazy_tick` on the block-entity lazy cadence; drain `Arm::drain_events()`
   (pickup sound volume 0.125 / pitch `pickup_pitch(random)`, awards). Call
   `redstone_update(has_neighbor_signal, ..)` on place and neighbor change.
   Reload: re-resolve saved records with `Point::deserialize` only when
   `required_chunks` are loaded.
4. `ArmWorld` adapter: probe blocks into `BlockProbe`; route item handlers
   (side UP; composter insert UP / extract DOWN); `keep_alive` for belts;
   `jukebox_has_record`.
5. Interaction hooks: the Arm item right/left click on blocks while held
   (`Selection`), placement packet carrying points to the server (apply with
   `Selection::flush` then `Arm::set_points`), empty-hand use (`take_held`),
   goggles use, wrench/transform support (`transform_record`), arm block
   placing `ceiling` when clicked face is DOWN, shape variants.
6. Menus: none. A scroll-value option for `SelectionMode`
   (`create.mechanical_arm.selection_mode.*`) and the
   `logistics.when_multiple_outputs_available` label.
7. Packets: arm item selection -> server (points + arm position); block entity
   sync of phase/index/progress/held item for the client pose.
8. Content/recipes: block `mechanical_arm` (property `ceiling`; rotation axis Y,
   cogwheel-style), item, loot table, blockstate/model, block entity type;
   advancements `mechanical_arm`, `arm_many_targets`, `arm_blaze_burner`,
   `musical_arm`; recipe and language keys from the original data. Original
   models/textures must come from the player-supplied JAR import.
