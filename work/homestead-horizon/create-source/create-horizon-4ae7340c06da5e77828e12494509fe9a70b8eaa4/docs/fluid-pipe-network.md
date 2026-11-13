# Fluid pipe network (game adapter)

Modules: `mixins/create/pipe_net.rs` (module `create_port_pipe_net`, the adapter), `mixins/create/engine/src/core/pipes.rs`
(pure simulation, see `docs/fluid-pipes.md`) and `tools/pipes_content.py` (new block variants). Source: Create 6.0.10,
package `content/fluids`.

Status: written against the staged engine API by reading signatures; **not compiled and not run in game**. The pure
core has 32 passing unit tests (`cargo test --offline pipes`).

## What it does

One long-lived `PipeSim` per (world, dimension) replaces the straight-line `create_port_basin::pump` tick. Pipes may
branch, bend, end in open space and feed several tanks; a Mechanical Pump pushes or pulls through any such graph, the
Fluid Valve gates it, glass / encased / smart pipes are separate blocks.

| Piece | Where |
| --- | --- |
| World access (`PipeEnv`) | `Env` in `pipe_net.rs` |
| Fluid handlers (tank, basin, drain, spout, other `native_custom_fluid_*` providers) | `create_port_basin::{fluid_tanks, fluid_fill, fluid_drain}` with the neighbour's face as `side` |
| Open ends | `Env::neighbor` (`OpenEnd` = replaceable space or a fluid block, otherwise `Blocked`), `place_fluid`, `take_source` |
| Simulation tick | `maintenance(game)` from `level_tick_pre` (`motor_level.rs`): drain events, reconcile, `PipeSim::tick`, write shapes back |
| Per-block tick | `tick` / `tick_valve` from the block entity tick: register loaded blocks, restore NBT, pump speed, save NBT |
| Block events | `changed(...)` from the `block_changed` observer (no `Game` borrow): queues positions near known pipes |
| Placement | `placement` (`FluidPipeBlock.getStateForPlacement`) from `placement_runtime.rs` |
| Wrench | `right_click` / `reacts`: pipe -> glass pipe, glass -> pipe, encased -> pipe |
| Drawing | `draw`: `PipeAttachmentModel` quads for every open side of a plain pipe |

### Data flow

* **World -> sim.** `changed()` records every block change of a pipe-family block or of a position next to one
  (`Shared::near`). `maintenance` merges the events, reads the current block state (`parse`) and calls `place_block`
  (placed), `remove_block` (gone), `set_pump_facing` (wrench on the pump) or `place_block` (valve `enabled`, any other
  state change). For a non-pipe position it calls `notify_neighbors` (tank placed / broken next to a pipe).
* **Sim -> world.** Plain and encased pipes own their six side booleans: after every sim tick `write_back` compares them
  with the world and writes differences with `UPDATE_CLIENTS` (no neighbour or shape updates; the sim already cascades
  shape updates). The valve pointer and the pump facing stay world-owned (`PipeSim::external_valves = true`).
* **Loading.** Nothing is saved except block entity NBT. After a chunk load each pipe / pump / valve registers on the
  first tick of its block entity (`PipeSim::load_block`, no scheduled propagate), then `restore` reads the saved
  `<side>: {Pressure: [in, out], Flow: {Fluid, In, Progress}}` (same layout the straight-line pump wrote, so old
  worlds keep their flows) and the pump speed (`restore_pump_speed`, no `onSpeedChanged`). A sweep every 100 ticks drops
  blocks of unloaded chunks silently (`forget_block`) and blocks that vanished without an event.
* **Open ends.** `place_fluid` follows `OpenEndedPipe.provideFluidToSpace` (waterloggable block + water -> `waterlogged`,
  otherwise a replaceable cell gets the fluid's source block; a different fluid or an existing source refuses; water is
  not placed in the Nether). `take_source` follows `removeFluidFromSpace` (source blocks only; waterlogged blocks are
  dried, others become air). The adapter's own writes are registered in `Shared::ignore` so their `block_changed` echo
  does not wipe the network (`FluidPropagator.validateNeighbourChange` ignores liquid blocks).

### Blocks

Existing ids and states are unchanged (`fluid_pipe`: six booleans + `waterlogged`; `mechanical_pump`: `facing` +
`waterlogged`; `fluid_valve`: `facing`, `axis_along_first`, `enabled`, `waterlogged`; the valve pipe axis is derived with
`gauge_axis(facing, !axis_along_first)` exactly like `pump::valve_axis(state, false)`). New ids:

| Block | Properties | `PipeBlock` | Item |
| --- | --- | --- | --- |
| `create:glass_fluid_pipe` | `axis`, `alt`, `waterlogged` | `Glass` | none (wrench) |
| `create:encased_fluid_pipe` | six side booleans | `Encased` | none (copper casing, not wired) |
| `create:smart_fluid_pipe` | `face`, `facing`, `waterlogged`; filter `create_horizon:FilterFluid` (string fluid id) in the block entity | `Smart` | yes |

## Differences and unverified points

* **Timing is the new simulation's, not the old fixture's.** The straight-line module was tick-exact against a Java
  fixture (`tools/fluid_pump_native_checks.py`, `Create fluid pump:` trace). `pipes.rs` is a port from summaries and its
  first-transfer ticks are self-derived regressions (docs/fluid-pipes.md). Expect `verify_startup` to fail on tick
  offsets until the sim is compared with the fixture; the other assertions (back-pressure, resume, reverse, stop,
  hand placement, assets) should hold. The trace line is now `Create fluid net:` (env `CREATE_TRACE_FLUID_NET`) with
  all faces per block entity.
* `Game.res.facts().get(id).replaceable` and `create_port::nearest` return types are assumed from `GameLevel.facts`
  usage and `pump::placement`; `Dir` order equals the `PipeAttachment::model(index)` order (down, up, north, south,
  west, east) as `pump::draw` relies on `side as usize`.
* `isOpenEnd` is approximated: replaceable cell or fluid -> open end, everything else a wall (no partial-face solidity).
  Cauldrons and other `VanillaFluidTargets` are not handled (`Neighbor::Vanilla` is never produced).
* Basin is treated as a handler on all faces; `fluid_tanks` filters by `can_drain` for contents, and fill/drain are
  whatever `create_port_basin::fluid_fill/drain` accept. The Item Drain is a handler except from above, the Spout
  except from below (`pump::storage_connection`).
* Only water and lava (`Fluid(1)`, `Fluid(2)`) and components-free stacks cross a pipe; other fluids are ignored.
* The smart pipe axis rule (`face == wall` -> Y, otherwise perpendicular to `facing`) is recalled, not read. The smart
  pipe has no filter UI, fallback model is a plain tube, and its placement orientation is approximate.
* NBT: the saved `Pressure` list may have both entries non-zero at junctions; the old `pump::stored` reader rejects that,
  so `mod pump` must not read NBT written by this adapter (bypass it fully).

## Known gaps

* Flowing fluid visuals (`FluidTransportBehaviour` renderer: fluid in glass pipes, pipe interior animation); `native_draw_fluid_box`
  (`native_fluid_render.rs`) is the building block. Rim / connection quads exist only for plain pipes.
* `FluidReactions` (lava/water collision, spill reactions) - `PipeEnv::pipe_flow_collision` is a no-op.
* Copper casing on pipes (`encase` exists, not routed: the casing item consumption path of `belt.rs` was not found),
  brackets, pipe rotation / mirror in schematics, Portable Fluid Interface, hose pulley.
* Infinite water sources (`take_source` always removes the source), `pipesPlaceFluidSourceBlocks` and
  `mechanicalPumpRange` configs (hard-coded to Create defaults 16 / true).
* Recipe for the smart pipe, advancements, goggles tooltips for pipes / pump.

## Integration checklist

All paths relative to `mixins/create/` unless noted. The lead owns these edits.

1. `horizon-mixin.toml`, after the `basin` module (line ~855):
   ```toml
   [[module]]
   id = "pipe-net"
   file = "crates/horizon-client/src/game.rs"
   name = "create_port_pipe_net"
   source = "pipe_net.rs"
   ```
2. `block_changed.rs` (and the matching inlined copy in `horizon-mixin.toml` helper), after
   `crate::game::create_port_drill::changed(dimension, pos);`:
   ```rust
       crate::game::create_port_pipe_net::changed(
           dimension, pos, level.native_state_block(old), level.native_state_block(new));
   ```
3. `motor_level.rs`, after `crate::game::create_port_basin::maintenance(game);`:
   ```rust
       crate::game::create_port_pipe_net::maintenance(game);
   ```
4. `basin.rs` `reset(reload)`, next to `pump::reset();`: `crate::game::create_port_pipe_net::reset();`
   `mod pump`: stop calling `pump::tick` / `pump::tick_valve` (see 5). Keep the consts, `placement` (pump and valve
   only), `reacts` / `right_click` (pump reversal) and `draw` (pump cog, valve pointer). `Line`, `TickState`, `Lines`,
   `find_line`, `line`, `controls`, `tick`, `tick_valve`, `trace*`, `stored`, `faces`, `save*` become dead code and can be
   deleted; `valve_nbt` and `Pointer` have copies in `pipe_net.rs`.
5. `runtime.rs`:
   * `entity()` (line ~169): replace
     `if block == super::create_port_basin::pump::PIPE && entity.is_none()` with
     `if super::create_port_pipe_net::is_passive(block) && entity.is_none()` (the body is unchanged);
     optionally `nbt = super::create_port_pipe_net::valve_nbt()` in the valve branch.
   * `ticks()` (line ~278): `|| block == super::create_port_basin::pump::PIPE` ->
     `|| super::create_port_pipe_net::is_passive(block)`.
   * `tick()` (line ~1154):
     ```rust
     if super::create_port_pipe_net::is_passive(&block) {
         if super::create_port_pipe_net::tick(game, pos, entity, 0.0) {
             game.native_block_entity_sync(pos);
         }
         return;
     }
     ```
   * pump / valve branch (lines ~1351-1355): `pump::tick` -> `super::create_port_pipe_net::tick`,
     `pump::tick_valve` -> `super::create_port_pipe_net::tick_valve`.
   * `update_tag()` (line ~1544): add `|| super::create_port_pipe_net::is_passive(&entity.id)` to the condition that
     returns `entity.nbt.clone()` (clients read `Pressure` / `Flow`).
   * `draw()` (line ~1730): `PUMP | VALVE` stay on `pump::draw`; add
     `if block == super::create_port_basin::pump::PIPE { super::create_port_pipe_net::draw(game, pos, entity, draw); return; }`
     remove `pump::PIPE` from the `pump::draw` arm.
   * `predicts_use()` (line ~492): `|| crate::game::create_port_pipe_net::reacts(game, pos)`.
   * `right_click()` (line ~549), before the pump entry:
     `|| crate::game::create_port_pipe_net::right_click(game, pos, face)`.
6. `placement_runtime.rs` (line ~74): split the arm:
   ```rust
   if matches!(block, super::create_port_pipe_net::PIPE | super::create_port_pipe_net::SMART) {
       *answer = Some(super::create_port_pipe_net::placement(game, state, ctx, block));
       return;
   }
   if matches!(block, super::create_port_basin::pump::PUMP | super::create_port_basin::pump::VALVE) {
       *answer = Some(super::create_port_basin::pump::placement(game, state, ctx, block));
       return;
   }
   ```
7. Content (`tools/`):
   * `content.py` line 26: `from pipes_content import pipes as fluid_pipe_variants, ITEM_BLOCKS as PIPE_ITEMS, LANG_EN as PIPES_EN, LANG_RU as PIPES_RU`;
     line ~966 after `fluid_pipes(write, model, axial_boxes)`: `fluid_pipe_variants(write, model)`;
     line ~1076: `extra_blocks.update(PIPES_EN)`; line ~1222 (Russian): `lang.update({'block.create.' + k: v for k, v in PIPES_RU.items()})`.
     Blocks without an item need `block.create.<id>` names; confirm how `extra_blocks` keys are turned into lang keys.
   * `catalog.py` line 13/35: `from pipes_content import ITEM_BLOCKS as PIPE_ITEMS` and `PROCESSORS = [..., PIPE, PUMP, VALVE, *PIPE_ITEMS, SPOUT]`
     (smart pipe gets the generic item loot / creative entry; glass and encased have no item).
   * `mixins/create/assets/src/catalog.rs` line ~43: add `"smart_fluid_pipe",` after `"fluid_valve"` (mirrors `catalog.py`
     `BLOCKS`; check `tools/block_catalog_checks.py`). Original-asset model parts for the new blocks are not imported yet.
   * `data/minecraft/tags/block/mineable/pickaxe.json` generation (content.py ~1008): add the three ids.
   * Hidden items: `glass_fluid_pipe` and `encased_fluid_pipe` have no item, so nothing to hide; the encased loot drops
     `create:fluid_pipe` and `create:copper_casing`.
8. `runtime.rs` `owns()` / kinetic lists: no change (`PUMP` and `VALVE` are already there, passive pipes are not kinetic).
9. Tests to run after the build: `cargo test --offline pipes` in the engine (done: 32 pass); then the scenario below.

## Acceptance of the straight line (what to keep green)

`tools/fluid_pump_native_checks.py` asserts, for single/double/direct lines: startup tick-exact comparison with the Java
fixture (storage amounts, `inlet`/`outlet` flow and progress, pressure == rpm, `front == east`) - **expected to differ**,
see above; back-pressure (incompatible fluid and full target park the line, 3 mB of room moves exactly 3 mB);
cold restart reproduces every NBT `Pressure` / `Flow` (restored by `restore`) and the transfer resumes after refilling;
wrench reversal sets `facing=west`, speed -32 and pressure `[32,0]` / `[0,32]` on the faces; removing a block
stops the flow and clears pressure and flow; hand placement gives `east=true,west=true` and `facing=east` and
`waterlogged=false`; original pipe models (`connection`, `rim_connector`, `drain`) are drawn (`Create fluid attachment:` trace
is emitted by `pump::draw` only: `pipe_net::draw` does not trace - add the same `trace_attachment` if the check stays).

## Proposed in-game scenario (ctl harness, style of `Test-HomesteadArm.ps1`)

Disposable world `HomesteadAutomationPipes`, origin (100,80,100). Never teleport the player above a placement cell and
wait; stand beside; use `.state` of the `block` query, not the name; do not `look-at` thin side faces (interact with
`up` faces of the floor).

```powershell
$null=Command 'tick unfreeze';$null=Command 'gamemode creative'
$null=Command 'forceload add 90 90 120 120'; $null=Command 'tp @s 96 80 96'; Wait-Ticks 40
$null=Command 'fill 92 77 92 112 90 112 minecraft:air'
$null=Command 'fill 92 79 92 112 79 112 minecraft:smooth_stone'
# Source tank, 3 pipes, pump facing east, a branch (tee) with two arms to two tanks.
$null=Command 'setblock 100 80 100 create:fluid_tank'
$null=Command 'data merge block 100 80 100 {TankContent:{Fluid:{id:"minecraft:water",amount:8000}}}'
$null=Command 'setblock 101 80 100 create:mechanical_pump[facing=east,waterlogged=false]'
$null=Command 'setblock 102 80 100 create:fluid_pipe[east=true,west=true]'
$null=Command 'setblock 103 80 100 create:fluid_pipe'          # becomes a tee once neighbours exist
$null=Command 'setblock 103 80 98 create:fluid_pipe'; $null=Command 'setblock 103 80 99 create:fluid_pipe'
$null=Command 'setblock 103 80 102 create:fluid_pipe';$null=Command 'setblock 103 80 101 create:fluid_pipe'
$null=Command 'setblock 103 80 97 create:fluid_tank'; $null=Command 'setblock 103 80 103 create:fluid_tank'
$null=Command 'setblock 101 79 99 create:cogwheel[axis=x]'   # meshes with the pump cog; drive from a creative motor
$null=Command 'setblock 102 78 99 create:creative_motor[facing=up]{ScrollValue:64}'   # adjust to the real drive layout
Wait-Ticks 20
# 1. Shapes: the tee is a branch (west, north, south open), the arms are straight.
Expect ((Invoke-Ctl @('block','103','80','100')).state -match 'north=true' -and (Invoke-Ctl @('block','103','80','100')).state -match 'south=true' -and (Invoke-Ctl @('block','103','80','100')).state -match 'west=true') 'branch shape'
Wait-Ticks 300
# 2. Both target tanks fill from the source: amounts nonzero, conserved, within one transfer of an even split.
$a=Amount 103 80 97; $b=Amount 103 80 103; $s=Amount 100 80 100
Expect (($a -gt 0) -and ($b -gt 0) -and ($a+$b+$s -eq 8000) -and ([math]::Abs($a-$b) -le 16)) 'branch split'
# 3. Valve: replace the pipe at 103 80 99 with a closed valve; the north tank stops, the south keeps filling.
$null=Command 'setblock 103 80 99 create:fluid_valve[facing=up,axis_along_first=false,enabled=false,waterlogged=false]'
Wait-Ticks 40; $a1=Amount 103 80 97; $b1=Amount 103 80 103; Wait-Ticks 60
Expect ((Amount 103 80 97) -eq $a1 -and (Amount 103 80 103) -gt $b1) 'closed valve gates one branch'
# 4. Open end: break the south tank; the south arm spills a water source at 103 80 103 after 1000 mB.
$null=Command 'setblock 103 80 103 minecraft:air'; Wait-Ticks 200
Expect ((Block 103 80 103) -eq 'minecraft:water') 'open end placed a source block'
# 5. Reversal: wrench the pump (Click on its up face with create:wrench); source and targets swap direction.
# 6. Persistence: save, quit to title, reopen; pressure/flow NBT equals the pre-quit values on the first tick and the transfer resumes.
```

`Amount x y z` is the `amount` field of `data get block x y z TankContent.Fluid`. Valve geometry (`axis_along_first`) must
be chosen so that its pipe axis is Z (`gauge_axis(facing=up, !axis_along_first)`); verify with the placed state.
