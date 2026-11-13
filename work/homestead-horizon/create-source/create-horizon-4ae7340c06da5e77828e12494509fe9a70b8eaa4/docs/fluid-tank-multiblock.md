# Fluid Tank multiblock (game adapter)

Adapter: `mixins/create/basin.rs`, `mod tank`. Rules: `mixins/create/engine/src/core/tank_multiblock.rs`
(`TankWorld`, see `docs/tank-boiler-steam.md`). Content: `tools/fluid_tank_content.py`.
Status: written without a build; nothing here has been run in game.

## Behaviour

- Width 1..=3, height 1..=32, 8000 mB per block. The controller is the minimum corner; members point to it.
- Formation: the first server tick of a tank (no `LastKnownPos` yet) runs `form_multi` from that block
  (`placed(game, pos)` is the immediate entry point for a hook). A tank next to an unloaded chunk defers and retries.
- Dissolution: `destroyed(game, pos)` (to be called before the block is removed) runs the original split with the
  pool redistribution. Without that hook the controller's tick (every 5 ticks) notices a missing member and splits
  with the same rules (the vanished block's share is voided). If the *controller* block disappears without the hook,
  its pool is lost, because only the controller stores fluid.
- Self-check every 5 ticks per tank: block state matches the layout, member and controller agree, all footprint blocks exist.
- Fluid access through any member: `view`, `fill`, `drain`, bucket right click, `endpoint/sample/amount/transfer_pair`
  (two members of one multiblock are never a transfer pair), `comparator`.
- Wrench toggles the window flag of the whole multiblock (`Op::Toggle`); `top`/`bottom`/`shape` states follow the core.
- Rendering: the controller draws one fluid box over the interior of all layers (`FluidTankRenderer` geometry,
  generalised from the 1x1 constants); `render_bounds` widens the culling box to the multiblock.

## NBT (block entity `create:fluid_tank`)

| Key | Type | On | Meaning |
|---|---|---|---|
| `Size` | Int 1..3 | all | width of the multiblock (copy on members) |
| `Height` | Int 1..32 | all | height of the multiblock (copy on members) |
| `Window` | Byte 0/1 | all | window flag (copy on members) |
| `Controller` | IntArray[3] | members only | offset `controller - this`, every component <= 0 and inside the footprint; **absent on the controller** |
| `TankContent` | Compound | controller | `FluidTank.writeToNBT` (`Fluid{id,amount,components}`), capacity `Size*Size*Height*8000`; members hold an empty compound |
| `Fill` | Float | all | derived: pool amount / capacity. Lets `comparator(entity)` work from a member alone |
| `Luminosity` | Int | all | derived: per-layer light of lava (`layer_luminosity`), 0 without windows; informational (see gaps) |
| `Boiler` | Compound | all | unchanged; non-zero `Engines`/`Whistles` makes the tank opaque (never touched) |
| `LastKnownPos` | IntArray | all | set after the first successful formation |

Old saves (`Size=1, Height=1`, no `Controller`) load as 1x1 controllers; `Fill` is computed from `TankContent`
until the next write. `Controller` is relative (Create writes an absolute position); that survives schematic moves.
Only water and lava with empty components are supported by the shared fluid codec (`fluid_codec.rs`);
any other content marks the block unsupported and it is left alone.

## Gaps / unverified

- No block light per layer: the block has no light-level state, `Luminosity` is data only. Needs an engine
  hook or a `light_level` state property.
- `block_changed`/neighbour updates are not wired: changes made by `setblock`, pistons or explosions are
  picked up by the 5-tick self-check, not instantly.
- 2-wide corner windows draw glass on the two outer sides; 3-wide edge-middle `window` blocks draw glass on all
  four sides (the state does not say which side is outer); the original models are used when the player's JAR is imported.
- Fluid-box height uses `height - 2*0.25 - 1/16` and floor `5/16` (the 1x1 values, generalised). Gas (top-anchored)
  fluids are not rendered because only water and lava are supported.
- Reform after a split uses the core's deterministic order, not a verified Java tick order.
- Clusters above 8192 connected tank blocks defer forever (never form).
- Setting block-state properties of a block whose entity is currently ticking (first-tick formation) relies on
  `set_block` keeping the entity of the same block; untested.
- Boiler, steam, tank item drops (the fluid is not stored in the item), and goggles are not implemented.
