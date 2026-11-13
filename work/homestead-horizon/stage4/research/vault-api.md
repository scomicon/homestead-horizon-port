# Vault inventory API research

Scope: read-only review of Horizon `4e5296d1800b3f89f2ae48dfd99d431801df4d46`,
native mixins `e8cd0aa38cffef49cb2e088ef716baa107148c02`, and MIT Create
`ac0c444d9828da3453ae8cc65338e8de063286fb`. No engine/Create source, stage-3
artifact, player save, issue or pull request was changed.

## Recommendation

Implement the dry horizontal Andesite Belt Funnel inventory-to-belt-to-inventory
flow for stage 4. A complete Vault increment is feasible, but needs inventory-view
integration, canonical per-part saves, topology repair and presentation together.
Adding a decorative Vault or a central oversized inventory would not satisfy it.
Keep Vault as a separate coherent implementation after the adapter below.

## Existing APIs and limits

There is **no container-parts/view override** in pinned `native-hooks.json` or
the native helpers. `block_entity_container` defines one static `ModContainer`
per block, with 256 slots maximum. `prepare_hopper_transfer` and
`hopper_inventory_search` are listen-only with `&self`; `inventory_move_item`
can only cancel an item move. Menu-linked containers apply to screens, not hopper
views. Those hooks cannot safely redirect every hopper/native item query.

`crates/horizon-client/src/game/machines.rs` already has ordered `View.parts`
and `View::locate` for double chests. Its item access, native item helpers and
slot arithmetic use `usize`, so an aggregate 1620-slot view is representable.
Three details prevent simply replacing `parts`:

1. A Mod view's faces, inputs and outputs are currently local static slot lists.
   A combined view needs expanded global rules, or a deliberate mapping of every
   rule to its owning part. Setting `parts` alone exposes only the first 20 slots
   through a side.
2. `Game::slot_mut_at` currently grows a short Mod entity's items to `view.len()`.
   Generic cold decoding trims trailing empty slots (`save.rs:901`). Without a
   local growth bound, a write after reload can grow one Vault part to 1620 slots.
3. Generic `Items` save slots are bytes and decode at most 256 slots. Keep every
   physical part at exactly 20 slots. Global indices must never be serialized
   into a part's `Items` or original `Inventory`. No 1620-slot menu is needed.

## Required original behavior

The [pinned ItemVaultBlockEntity](https://github.com/Creators-of-Create/Create/blob/ac0c444d9828da3453ae8cc65338e8de063286fb/src/main/java/com/simibubi/create/content/logistics/vault/ItemVaultBlockEntity.java)
owns one inventory per physical block and exposes their ordered concatenation
from every member and side. Width is 1..3, length is at most three times width;
the largest group contains 81 parts. For X, iterate longitudinal offset, then Y,
then Z; for Z, iterate longitudinal offset, then X, then Y. Preserve controller,
size/length, last-known-position and local canonical inventory fields. Breaking
a member drops only its local inventory, then repairs the remaining topology.
[ConnectivityHandler](https://github.com/Creators-of-Create/Create/blob/ac0c444d9828da3453ae8cc65338e8de063286fb/src/main/java/com/simibubi/create/api/connectivity/ConnectivityHandler.java)
also defines merge/split selection; an invented flood-fill rectangle would not
establish parity. The player's `scripts/mods/Create/vault.zs` replaces crafting
with two Vaults from one barrel and two iron sheets.

## Minimal deliberate adapter

Prefer an engine-native read-only parts resolver in a later API revision. It
should receive the world, registry, queried position and block entity, then
answer one of: unrelated (keep original view), supported ordered parts, or owned
but unsupported (refuse, never fall back to a local partial view). Validate all
positions, unique membership, matching block/entity/controller/axis, local slot
counts, dimensions and limits before constructing a combined view. An unloaded
member refuses this bounded implementation; it must not trigger chunk loading
or erase the saved topology.

For this pinned source only, an exact-revision cancellable HEAD mixin at
`horizon_client::game::machines::Containers<'_>::view` can provide the same
boundary before the vanilla Mod branch. The helper can borrow `self.world` and
`self.registry`, call a Create resolver, and return an owned `Option<View>`.
Keep `Kind::Mod` and create/cache combined metadata with full side/input/output
indices so ordinary validation hooks remain active. Avoid copying inventories
or moving items into a synthetic controller. Fingerprint this private method.

Pair it with either (a) a bounded local resize in `slot_mut_at`, or (b) canonical
Vault decode/factory initialization to exactly 20 slots plus a resolver that
refuses every differently sized part. The general extension should use (a).
A `MODIFY_ARG` of `m.items.resize` was tested with the actual pinned planner and
rejected (`E_UNSUPPORTED_CONTROL_FLOW`, because the method contains `?`). An
alternative regular HEAD can initialize only the located Vault member to its
local 20 slots before the original method, so its oversized-resize branch never
runs. The resolver must still refuse oversized or malformed parts. The general
engine extension should directly correct the original local resize bound.

The scratch `vault-hook-probe/` only researches planner compatibility for those
two targets; its view callback always continues. It implements no Vault and must
not be included in a gameplay package. See its plan log for actual resolver
status; planning alone is not Rust compile or gameplay acceptance.

Reuse normal transfer transactions: `native_move_items` detects shared physical
parts, takes one item, attempts insertion, restores refusals, and notifies all
parts. `mod_container_accepts` already resolves a global slot to a local entity
and local index. Keep this path for hoppers, droppers and native machine IO.

## Files and acceptance for a Vault increment

* Engine/API: `machines.rs`, `machines/native_items.rs`, native hook/API catalog
  and docs, or the two explicitly pinned mixin helpers above.
* Create: pure `engine/src/core/vault.rs`, `vault_runtime.rs`, `runtime.rs`,
  `inventory_codec.rs`, canonical inventory serialization and `horizon-mixin.toml`.
  Add catalog/content/loot/recipe and private asset-import entries. Preserve ARR
  artwork in the player's JAR workflow. Extend strict schematic support only
  when its complete topology/NBT contract is implemented.
* Tests: both axes; 1/3/24/81 parts; first/last aggregate slot; insertion/extraction
  through every member; marked stacks; full/partial refusal; two hoppers; moves
  within one group; cold-load empty/trailing-empty parts; width/axis edits and
  malformed/unknown NBT; boundary chunks/unload; merge/split; break controller
  and noncontroller with exact local-only drops and no duplicates. Check whole
  group comparator signals. Compare original topology and slot-order fixtures,
  then normal player builds and real process restarts in an isolated world.

## Belt Funnel alternative

`chute_runtime.rs::funnel` already supplies ordinary filters, redstone, cooldown
and inventory access; `belt.rs` already has controller ownership, insertion
simulation and persisted transported items. Add the real Andesite Belt Funnel
block/states and hook it to the ordinary belt simulation. The [original belt
funnel block](https://github.com/Creators-of-Create/Create/blob/ac0c444d9828da3453ae8cc65338e8de063286fb/src/main/java/com/simibubi/create/content/logistics/funnel/BeltFunnelBlock.java)
defines four shapes and Wrench transitions. The [original funnel entity](https://github.com/Creators-of-Create/Create/blob/ac0c444d9828da3453ae8cc65338e8de063286fb/src/main/java/com/simibubi/create/content/logistics/funnel/FunnelBlockEntity.java)
distinguishes pushing, taking and redstone pause, and simulates belt insertion
before removing stock. A dry bare horizontal belt with ordinary Chest/Barrel IO,
legitimate inputs, blocked-output handling, stopped/reversed movement and an
actual cold restart is a useful complete automation increment without a new
aggregate inventory API. Brass filters, wet/sloped belts and custom handlers
can remain explicitly unsupported.
