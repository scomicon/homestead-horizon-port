# Homestead stage 5: Item Vault and its multiblock rules

Status: implemented, built and accepted in the native client. Scope is the plain
Item Vault (`create:item_vault`, Create 6.0.10).

Verified in game (`stage4/Run-Acceptance.ps1 -Vault`, PowerShell 7, disposable
world, normal saved exit): a lone vault has 20 slots; a hopper under the far
member of a row empties stock held in the first member; a hopper above a full
member inserts into the rest of the shared inventory; a 2x2x2 cuboid shares one
inventory over eight blocks; removing the middle of a row splits it into single
vaults. Also verified: a Chute inserts into and pulls out of a vault; Belt Funnels
move ingots between two vaults and back when the Belt reverses; a comparator
reads 5 for one full block of a three-block vault and 15 for a lone full vault
(`calcRedstoneFromInventory` arithmetic over the shared slots); a destroyed
vault drops its contents and itself; a cold restart in a separate process
restores a three-block vault with its contents, its comparator reading and a
working shared view. Not yet checked: the Create recipe in survival, and
interaction with contraptions, Mechanical Arms and Item Drain.

Adapter changes for this: the Chute bridge and Belt Funnel accept the vault as an
ordinary container, and the vault declares an analog output (`has_analog_output`
and `analog_output` hooks). Comparators only update on a neighbour change, so the
acceptance toggles an adjacent lamp after changing a vault.

Engine detail found on the way: the container table is built only for blocks in
the ticking table, so the vault is listed there with an empty tick.

## What is ported

- `engine/src/core/vault.rs`: pure connectivity rules (4 unit tests). A group of
  face-adjacent vaults of one `axis` merges when it is an exact
  `width x width x length` cuboid with width 1..3 and length up to `3 * width`.
  The controller is the minimum corner.
- `vault_runtime.rs`: block entity and container declaration, 20 slots per block.
- `vault_view.rs` + `vault_view_hook.rs`: a cancellable HEAD injection on
  `Containers::view` returns a multi-part `View` over every member, like a
  double chest, so hoppers, Chutes, belt funnels and comparators see one shared
  inventory. No engine fork.
- `placement_runtime.rs`: `ItemVaultBlock.getStateForPlacement` (continue the
  clicked vault's axis unless sneaking, otherwise the player's horizontal axis).
- `tools/vault_content.py`: block `create:item_vault` (hardness 3, blast
  resistance 1200, axis and large states), blockstate rotation as in the
  original, loot table, localization (en/ru), pickaxe tag, shaped recipe
  `iron plate / wooden barrel / iron plate`, vanilla-textured fallback model.

## Design notes

Each member keeps its own 20 slots; the shared inventory is the concatenation in
controller order (along the axis, then up, then across). Items therefore never
move when a vault merges or splits, and nothing can be lost on a rebuild.

## Known differences

- Saved NBT is Horizon's `Items` per block, not Create's `Inventory`/`Controller`
  compound. Worlds saved by the Java mod are not read for vault contents.
- The `large` state is declared but not updated, and connected textures are not
  rendered (the original blockstates use one model for every state).
- Merging is the simplified exact-cuboid rule above, not the full
  `ConnectivityHandler` search; an irregular blob stays as single vaults.
- Breaking a member keeps its own slots; Create redistributes the shared handler.
- Max length per width (`width * 3`) and 20 slots per block are from memory of the
  6.0.10 config and need a check against the pinned source.
- Mechanical Arm, contraption-mounted vault storage and the Vault ponder are not
  ported.

## Local build notes

`mixins/create/engine` must not hold a `Cargo.lock` or `target/` when the mixin
stage runs (`E_CRATE`); delete them after `cargo test`. A cancellable injection
helper must return `core::ops::ControlFlow<Option<View>>` spelled with `core::`.
