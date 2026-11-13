# Fluid Tank multiblocks, Boiler, Steam Engine and Steam Whistle

Pure engine modules for Create 6.0.10 (Creators-of-Create/Create, revision
`ac0c444d9828da3453ae8cc65338e8de063286fb`). These are algorithm oracles with
unit tests. They are not wired into `lib.rs`, the runtime or the content
pipeline, and nothing here shows presentation parity.

## Source reading caveat

The Java was read through a summarising fetch tool, which declined to return
files verbatim. Constants and formulas below were extracted as pseudo-code by
that tool, not read line by line by the author. Anything marked *unverified*
should be checked against the Java before it is relied on.

## Files

| File | Content |
|---|---|
| `mixins/create/engine/src/core/tank_multiblock.rs` | `TankWorld`: connectivity, capacity, fluid pooling and splitting, windows, light, comparator |
| `mixins/create/engine/src/core/boiler.rs` | `Boiler`: heat, water supply, level, efficiency, stress capacity, attachments |
| `mixins/create/engine/src/core/steam_engine.rs` | engine placement geometry, per-tick direction, `PoweredShaft` speed and stress |
| `mixins/create/engine/src/core/steam_whistle.rs` | whistle attachment, extender stacking, pitch, pitch id, sound parameters |
| `tools/steam_content.py` | draft block declarations, unwired |

66 unit tests (`cargo test --offline` in a scratch copy, 337 passed across the
whole engine crate with the four modules declared).

## Ported

Tanks (`tank_multiblock.rs`)
- Limits: width 3 (`MAX_SIZE`), height `fluidTankMaxHeight` = 32, capacity
  `fluidTankCapacity` = 8 buckets = 8000 mB per block (both configurable via
  `TankConfig`). A 3x3x32 tower is 288 blocks, 2,304,000 mB.
- Controller is the minimum corner; members point to it.
- `ConnectivityHandler.formMulti`: flood fill from the changed block towards
  smaller coordinates collecting controllers whose best simulated size is above
  one, then forming them largest first. `tryToFormNewMulti` tries widths 1..=3
  and keeps the biggest volume, larger width on a tie. `tryToFormNewMultiOfWidth`
  scans layers of `width x width` and stops at the first bad cell: missing
  part, wider controller, same-width full-height controller of another multi,
  controller outside the candidate footprint, or a different non-empty fluid.
- Merge pools the fluid into the new controller (capped at the new capacity);
  parts keep no fluid and copy the window flag.
- `splitMultiAndInvalidate`: the controller keeps one block of fluid, the
  surplus is spread bottom-up over the other parts, every part becomes a 1x1x1
  controller, the boiler flag is cleared, and the parts queue a connectivity
  update (`removeController` sets `updateConnectivity`). Breaking a block
  voids the share of that block.
- Block states `top`, `bottom` (from controller y and height) and `shape` via
  `setWindows` (width 1: window, width 2: corner windows NW/SW/NE/SE, width 3:
  window where `| |x|-|z| | == 1`, i.e. edge midpoints; same on every layer).
  Shape `rotate`/`mirror` tables. `toggleWindows` blocked while the boiler is
  active; activating a boiler forces windows off.
- `fill`/`drain` through the controller (single-fluid), fill state,
  comparator (`ComparatorUtil.fractionToRedstoneLevel`), per-layer luminosity
  (`onFluidStackChanged`), light emission gated by the window flag, bucket
  interaction pitch.

Boiler (`boiler.rs`)
- Heat per block under the controller footprint: `NO_HEAT = -1`,
  `PASSIVE_HEAT = 0`, Blaze Burner smouldering 0, fading and kindled 1,
  seething 2, `none` -1. Active heat is the sum; passive heat counts only when
  the active sum is zero.
- `maxHeatForSize = min(18, size / 4)`, `maxHeatForWater = min(18,
  ceil(waterSupply) / 10)`, level = min(activeHeat, both).
- Passive boiler: `isPassive` needs both caps above zero; efficiency
  `1/8 / engines`.
- Engine efficiency: 1 when engines <= level, else `level / engines`; 0 when
  there is no active heat.
- Goggle total stress: `efficiency * 16 * max(level, engines) * 1024`; status
  text (passive / idle / max / level n); water-limited condition; gauge target.
- Water supply: samples every 5 ticks into a 10 slot ring, `waterSupply` rises
  to any sample and is recomputed as the ring maximum on wrap (every 50 ticks);
  creative tanks report 200. Max-level flag at level 18. Sound volume
  `3 / max(2, engines / 6)`.
- Attachment counting over all tank blocks (engine facing away, whistle
  attached towards the tank) and the Pipe Organ check (12 distinct pitch ids).

Steam Engine (`steam_engine.rs`)
- Geometry: connected direction by face (floor up, ceiling down, wall facing),
  tank one block behind, shaft two blocks ahead, valid shaft axis differs from
  the engine's axis, first-valid-axis choice when the engine places a shaft.
- Per tick: direction +-1 from the facing sign (horizontal FACING for vertical
  engines), vertical shafts ignore the sign, Z-axis shafts negate, the
  rotation-direction scroll negates, an externally driven shaft running the
  other way flips the scroll.
- `PoweredShaft`: 3 tick warm-up, single owning engine keyed by relative
  offset, `update`/`remove` semantics, speed `direction * 16 * modifier`
  with modifier 1..4 from efficiency quarters, capacity
  `efficiency * 1024 / modifier` (so network stress is `efficiency * 16384`
  for every modifier), angle offset parity.

Whistle (`steam_whistle.rs`)
- Attached direction, survival, placement, extender survival rule, pitch scan
  (each non-single extender +2, a single on top +1, capped at 26), pitch id
  `pitch + 100 * size`, `incrementSize` search over six slots, sound pitch
  `2^(-pitch/12)` (+0.75 for a new small sound), chiff volume, sounding
  condition.

## Not ported / gaps

- Creative tank (fill to a set fluid, always full), gaseous (lighter than air)
  fluids in the split redistribution and fill order, `SilenceTankSound`.
- Multi-fluid conflict beyond one fluid per tank: merging compares against the
  first non-empty fluid seen (a stricter, deliberate simplification of the
  original check against the origin only).
- Tick order of re-forming parts after a split: the engine processes pending
  parts in ascending coordinate order (the real order is the block-entity tick
  order). Results that depend on it are only deterministic, not proven equal.
- Order of redistribution during a split (y, then x, then z) and the loss of
  the removed block's share are reconstructions, *unverified*.
- Search bounds of the formation flood fill are reduced to "3 blocks below the
  seed on x and z, unbounded on y".
- `BoilerHeaters` tag membership (`create:passive_boiler_heaters`) and
  `BlockHelper.isNotUnheated` are adapter data; only the mapping is ported.
- Steam Engine stress capacity 1024 was read from the `AllBlocks` registration
  line (`CStress.setCapacity(1024.0)`); the generated data file could not be
  fetched.
- `WhistleBlock` placement facing: whether the original flips the placer's
  direction was not visible; the adapter passes the final facing.
- Whistle `SIZE` for newly placed extenders is copied from the whistle; the
  size-from-height rule, if any, was not found. Extender `DOUBLE` assignment
  when stacking was not seen either.
- Particles (steam jet), whistle/boiler sounds beyond the numbers above,
  renderers (tank fluid level lerp, gauge needle easing 0.125, whistle
  animation chase 0.5/0.4), goggle tooltip layout, advancements, moving
  contraption tanks and the boiler fluid handler as a capability object.
- Recipes and loot: not verified, therefore not included.

## Java -> Rust mapping

| Java | Rust |
|---|---|
| `FluidTankBlock` (TOP/BOTTOM/SHAPE, `Shape`, rotate/mirror) | `tank_multiblock::{TankState, Shape}` |
| `FluidTankBlockEntity.setWindows` / `toggleWindows` / `notifyMultiUpdated` | `window_shape`, `TankWorld::{toggle_windows, block_state}` |
| `FluidTankBlockEntity.applyFluidTankSize`, `getFillState`, `onFluidStackChanged` | `TankWorld::{capacity, fill_state}`, `layer_luminosity` |
| `ConnectivityHandler.formMulti` / `tryToFormNewMulti(OfWidth)` / `splitMultiAndInvalidate` | `TankWorld::{form_multi, try_form, simulate_width, absorb, split_multi}` |
| `FluidTankBlock.onPlace` / `onRemove` | `TankWorld::{place, remove, update_connectivity}` |
| `ComparatorUtil.fractionToRedstoneLevel` | `comparator_level` |
| `BoilerHeater` / `BoilerHeaters` | `boiler::{NO_HEAT, PASSIVE_HEAT, blaze_burner_heat, passive_heat}` |
| `BoilerData` | `boiler::Boiler` (`evaluate`, `update_temperature`, `tick_server`, ...) |
| `SteamEngineBlock` | `steam_engine::{connected_direction, can_survive, shaft_pos, is_shaft_valid, placed_shaft_axis}` |
| `SteamEngineBlockEntity.tick` | `steam_engine::engine_tick` |
| `PoweredShaftBlockEntity` | `steam_engine::PoweredShaft`, `speed_modifier` |
| `WhistleBlock` / `WhistleExtenderBlock` | `steam_whistle::{Size, ExtenderShape, attached_direction, placement_state, extender_can_survive, increment}` |
| `WhistleBlockEntity.updatePitch` / `getPitchId` | `pitch_of_stack`, `pitch_id`, `Whistle` |

## Integration

1. `lib.rs` (not edited by this work), in the vault style:
   ```rust
   #[path = "core/tank_multiblock.rs"]
   pub mod tank_multiblock;
   #[path = "core/boiler.rs"]
   pub mod boiler;
   #[path = "core/steam_engine.rs"]
   pub mod steam_engine;
   #[path = "core/steam_whistle.rs"]
   pub mod steam_whistle;
   ```
   `boiler` uses `crate::burner::Heat`; all modules use `crate::kinetics` and
   `crate::placement`.
2. `horizon-mixin.toml` / `runtime.rs`: hooks for place, break and neighbour
   change of tanks (call `TankWorld::place` / `remove`, then apply
   `block_state()` of every member), wrench (`toggle_windows`), fluid
   capability (`fill`/`drain` on any member; with an active boiler route water
   to `Boiler::fill_water`), server tick of controllers
   (`Boiler::tick_server`), neighbour change below a controller (set
   `needs_heat_update`), engine and whistle place/remove (re-run
   `Boiler::evaluate`, then `TankWorld::set_boiler_active`).
   `TankWorld` is a model: an adapter either mirrors world cells into it per
   operation or reuses its functions against block-entity data.
3. Block states needed: fluid tank `top`, `bottom`, `shape` (six values, already
   present on the standalone tank blockstate); steam engine `face`, `facing`,
   `waterlogged`; powered shaft `axis`; steam whistle `facing`, `wall`,
   `powered`, `size`; extender `shape`, `size`. Draft in `tools/steam_content.py`.
4. Block entities / persistent data: tank controller, width, height, window,
   fluid; boiler counts, heat, water supply and ring state; powered shaft
   engine offset, efficiency, direction; whistle pitch and size.
5. Kinetics: `PoweredShaft::generated_speed` and `added_stress_capacity` feed the
   generator side of `kinetics.rs`; engine capacity 1024.
6. Content still needed: Steam Engine, Steam Whistle, Powered Shaft and
   extender blocks and items, models, lang, loot, recipes (verify against the
   pinned datapack), tag `create:passive_boiler_heaters`, advancements, and
   lang keys for the boiler tooltip.
7. Schematics: validate `top`/`bottom`/`shape` and the engine/whistle
   properties before placement, rejecting unknown values.
