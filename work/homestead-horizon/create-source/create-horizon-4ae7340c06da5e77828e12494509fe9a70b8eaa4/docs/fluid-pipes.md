# Fluid pipe network (`core/pipes.rs`)

Pure, deterministic port of the Create 6.0.10 fluid pipe network
(`com.simibubi.create.content.fluids`, revision `ac0c444d9828da3453ae8cc65338e8de063286fb`, MIT).
It generalises the straight-line subset in `fluid_transport.rs` (which is untouched) to arbitrary
graphs. All world access goes through the `PipeEnv` trait.

## What is ported

| Java | Rust (`pipes.rs`) | Notes |
| --- | --- | --- |
| `FluidTransportBehaviour.tick` | `PipeSim::tick_behaviour` | phases `WaitForPumps -> FlipFlows -> Idle`, collision check, per-face `manageFlows`, `tickFlowProgress`. |
| `PipeConnection` | `PipeConnection` | pressure couple, source, flow, `manageFlows`, `flipFlowsIfPressureReversed`, `tickFlowProgress` (speed `1/32 + clamp(p/128)*31/32`), `wipePressure`, `determineSource`. |
| `FlowSource` (`FluidHandler`, `OtherPipe`, `Blocked`) | `Source` | |
| `OpenEndedPipe` | `OpenEnd` | 1000 mB tank, placing a source block at 1000, drain tank first then world source, `wasPulling`. |
| `FluidNetwork` | `FluidNetwork`, `PipeSim::tick_network` | 2 tick start pause, 16 BFS cycles/tick, frontier/targets, `transferSpeed = max(1, inbound/2)`, simulate+execute transfer with even split + remainder. |
| `FluidPropagator.propagateChangedPipe`, `resetAffectedFluidNetworks` | `PipeSim::propagate_changed_pipe`, `reset_affected_fluid_networks` | |
| `PumpBlockEntity` | `PipeSim::{set_pump_speed, set_pump_facing, update_pressure_change, distribute_pressure_to}` + `search_for_endpoint` | BFS limited by `pump_range` (default 16 -> 15 pipes per side), recursive valid-face search, `|speed| / parallelBranches` pressure per distance set, `PumpFluidTransferBehaviour` pressure. Front/back update order and `hasReachedValidEndpoint` (pump-to-pump, handler, open end). |
| `FluidPipeBlock.updateBlockState` / `canConnectTo` | `update_block_state`, `PipeSim::{place_pipe, update_pipe_shape, after_state_change}` | straight / bend / branch rules, previous 2-side shape kept, neighbour shape cascade. |
| `FluidPipeBlock` wrench, `GlassFluidPipeBlock`, `EncasedPipeBlock` | `wrench_pipe`, `wrench_glass`, `encase`, `unencase` | flows/connection data kept across the swap (`cacheFlows/loadFlows`). |
| `FluidValveBlockEntity` | `set_valve_speed`, `tick_device` | pointer chase `clamp(|speed|/16/20,0,1)` per tick, `ENABLED` flips at 0 / 1, `canPullFluidFrom` gating. |
| `SmartFluidPipeBlockEntity` | `PipeBlock::Smart`, `set_smart_filter` | filter in `canPullFluidFrom`; no filter accepts any fluid. |
| scheduled ticks (`onPlace`, `neighborChanged`) | `PipeSim::schedule` | delay 1; run before block entities in `tick`. |
| `FluidReactions.handlePipeFlowCollision` | `PipeEnv::pipe_flow_collision` | hook only. |

## What is NOT ported / known gaps

* `FluidReactions` (lava/water collision effects, spill collisions) and potion/other `OpenPipeEffectHandler`s: only the hook exists.
  `place_fluid` returns false on a different fluid or an existing source; the effect is the adapter's job.
* `pipesPlaceFluidSourceBlocks` config, ultra-warm dimension extinguish, honey/cauldron `VanillaFluidTargets.drainBlock`, advancements: adapter side (`place_fluid` / `take_source`).
* Fluids without a block state in `OpenEnd` (always placed through `place_fluid`).
* Brackets, waterlogging, rim/attachment rendering (`AttachmentTypes`), particles, splash, flow animation, NBT serialisation, client-only code, `InterfaceFluidHandler` keep-alive (Portable Fluid Interface), hose pulley rim exception.
* `FluidNetwork` target handler cache and the 40-tick refresh (handlers are looked up live through `PipeEnv`); `targets` is de-duplicated (the Java list is not).
* Multi-tank handlers: `PipeEnv::drain/fill` take a fluid and an amount; the Java per-tank scan and the generic-drain fallback collapse into that call.
* Chunk loading: `is_loaded` is honoured in the same places as Java, but nothing re-propagates pressure over an edge that was unloaded when the pump distributed. Call `propagate_changed_pipe` when the adapter loads such a chunk.
* Block rotation/mirroring (`FluidPipeBlockRotation`), schematic transforms.

## Assumptions and unverified items

Source was read through summaries of the raw Java files, not line by line; verify before claiming parity:

* `FluidNetwork.tick` frontier details (keep-alive rules, inbound handling) follow the summary of the Java file; the exact `continue` placement after inbound flows is inferred.
* `PipeConnection.tryStartingNewFlow` was not visible; it is modelled as "create an incomplete flow with 0 progress if the fluid is non-empty".
* The valve `ENABLED` flip schedules `propagateChangedPipe` through the generic "state changed -> `onPlace` -> scheduled tick" path (`AxisPipeBlock.onPlace`); not confirmed for `FluidValveBlock` specifically.
* `set_pump_facing` (pump reversal by re-orienting the block) runs `updatePressureChange`; the real trigger chain (`PumpBlock` state change / wrench) was not read.
* Smart pipe `canHaveFlowToward` uses the axis rule like `StraightPipeFluidTransportBehaviour`; the summary of the inner behaviour was contradictory.
* `canConnectTo` ignores brackets; a neighbouring `FluidPipeBlock` always counts as connectable (Java), other transport blocks must accept flow back.
* `first_transfer_tick_regression` pins values produced by this port (25, 53, 235, 25 for 1/3/3/3 pipes per side at pressure 32/32/4/64). They are **not** measured from a Java client and are not comparable with the `fluid_transport.rs` test numbers (which use a different block count and omit the scheduled propagate tick and phase delays).
* BE tick order: `PipeSim` ticks in registration order (`place_block` order); Java uses the level's block-entity order. Feed `place_block` in load order if it matters.

Verification: `cargo test --offline` in a scratch copy of the engine crate with the `pipes` mod added: 30 pipe tests, all 301 engine tests pass. No cross-client or in-game comparison was done.

## Integration

1. `lib.rs`: add next to `fluid_transport`:
   ```rust
   #[path = "core/pipes.rs"]
   pub mod pipes;
   ```
2. Adapter implements `pipes::PipeEnv` over the Horizon world: `neighbor` (Handler if the block exposes a fluid capability on that face, Vanilla for `canProvideFluidWithoutCapability`, OpenEnd for `FluidPropagator.isOpenEnd` rules 4-7, else Blocked), `contents/fill/drain`, `place_fluid` (replaceable / waterlogged / pipesPlaceFluidSourceBlocks / dimension rules), `take_source` (source block removal incl. infinite water), `is_loaded`, `pipe_flow_collision`.
3. Hooks the adapter must call on a long-lived `PipeSim`:
   * block placed/changed: `place_block` (exact state) or `place_pipe(look_dir)` (placement connection logic);
   * block broken: `remove_block`; other block changed next to a pipe: `notify_neighbors(pos)`;
   * wrench/casing: `wrench_pipe`, `wrench_glass`, `encase`, `unencase`;
   * kinetics: `set_pump_speed(pos, speed)` on speed change, `set_valve_speed`; pump re-orientation: `set_pump_facing`; filter slot: `set_smart_filter`;
   * `tick(env)` once per server tick; read back `block(pos)` (pipe/valve `enabled` state changes), `connection/flow/pressure/open_end` for rendering and NBT.
   * on chunk load of a boundary: `propagate_changed_pipe`.
4. Blocks and properties needed (content): `fluid_pipe` (north/east/south/west/up/down booleans, waterlogged), `glass_fluid_pipe` (axis, alt, waterlogged), `encased_fluid_pipe` (six booleans), `fluid_valve` (axis, enabled), `smart_fluid_pipe` (face, facing, waterlogged + filter slot), `mechanical_pump` (facing). Block entities: pipe behaviour on all, kinetic on pump and valve, filter on smart pipe. See `tools/pipes_content.py` for declarations. Straight/glass/valve/smart pipes map to `PipeBlock::{Glass, Valve, Smart}`, plain and encased pipes to the six-boolean variants.
5. Registry mapping: `Fluid(u32)` is opaque; the adapter maps it to resource ids (water = 1, lava = 2 in the constants).
