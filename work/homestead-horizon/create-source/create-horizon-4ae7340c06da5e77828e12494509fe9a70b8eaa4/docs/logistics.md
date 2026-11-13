# Factory logistics (pure logic)

Pure, deterministic Rust for Create 6.0.10's package logistics. Nothing here
touches a world, renderer or network; inventories come in through the small
`ItemHandler` trait and randomness through `Random`. Source: Creators-of-Create/Create at
`ac0c444d9828da3453ae8cc65338e8de063286fb` (MIT). The Java was read through
a summarising fetch, not a local checkout, so see "Unverified".

Modules in `mixins/create/engine/src/core/`:

| Module | Content |
| --- | --- |
| `package.rs` | Items/stacks, `BigStack`, package data (contents, address, order data, context), address matching and glob, `PackageOrderWithCrafts`, `ItemHandler`, `SimpleInventory`, `Random` |
| `packager.rs` | Packager state machine (packing, unpacking, animation, queue, redstone/sign), `PackagingRequest`, Repackager and fragment helper |
| `stock_network.rs` | `InventorySummary`, promise queue, Stock Link data, `findPackagersForRequest`, broadcast, summary cache, global network registry |
| `factory_gauge.rs` | Factory Gauge: monitor, crafting requests, restocking, redstone input, connection rules |
| `package_post.rs` | Frogport/Postbox: 18-slot port, address filter, animation state machine, push/pull, targets, clipboard addresses |

83 unit tests (run in a scratch copy, see Integration).

## Ported

- **PackageItem**: nine slots, stacked insertion (`containing`), address
  get/set/clear, `matchAddress` (blank rule, `*`, equality, glob either
  direction), order data (`orderId`, `linkIndex`, `isFinalLink`,
  `fragmentIndex`, `isFinal`, context) with the `-1`/`false` defaults, the
  context lookup order (order data first, standalone component second),
  `getPackageVelocity`.
- **PackageOrderWithCrafts**: `empty/simple/singleRecipe`, `hasCraftingInformation`,
  `isEmpty` (ignores crafts), `orderedStacksMatchOrderedRecipes`.
- **Packager** (`PackagerBlockEntity`): `attemptToSend` for manual and
  request mode (64-per-extraction cap, bulky items alone, reuse of an existing
  package, address precedence, order data from the request, held box vs
  `queuedExitingPackages`), `unwrapBox` with the default all-or-nothing
  unpacking, 20-tick animation, `tick` hand-off from the queue,
  `isTooBusyFor` (50/20/10), `activate` with the 40-tick button cool-down,
  `lazyTick`, sign address rules (last non-blank side wins; front before back,
  lines joined by spaces), `performPackageRequests` (<= 100 passes),
  `PackagingRequest` with shared final-link flag and per-link package counter.
- **Repackager**: fragment collection, completeness walk (links x fragments,
  1000 caps), `repack` (recipe packages with random order id, greedy
  extraction following `orderedStacks`, nine-slot boxes, address, order data,
  context only on the last box), the "failed craft keeps earlier deductions"
  quirk, plain-package passthrough, `unwrapBox` needing one slot for the box.
- **Stock network**: `InventorySummary` (first-match merging, INF cap, erase,
  count, sort by count), `getSummaryOfNetwork` with inventory de-duplication,
  `getStockOf` without it, `processRequest` capping, `findPackagersForRequest`
  (one random link per shared inventory, one random order id per request,
  link indices, final-link tracker semantics, context only on the first used
  link), `broadcastPackageRequest` (empty/busy refusal, unfillable orders still
  accepted), `RequestPromiseQueue` (add, forceClear, itemEnteredSystem,
  expiry-aware total, change counter), `TickBasedCache`-like summary cache with
  `deductFromAccurateSummary`, `GlobalLogisticsManager` (owner set once,
  locked/mayInteract/mayAdministrate, loaded vs total links, per-network
  promises, tick).
- **Factory Gauge**: value settings (row 0 = items, else stacks), promise
  expiry (-1 / 600 ticks / N minutes), storage monitor with change detection,
  confirm sound and redstone-output events, crafting request flow (group by
  network and item, sufficiency, crafting context, busy check, perform,
  promise of `recipeOutput`), restocker flow (demand minus promised minus
  stored, clamp 0..9 stacks, cap by network stock, promise), redstone input,
  `addConnection` rules (links bypass the limit, restocker rejects, max 9,
  active source), arrow-bend cycling, missing-address flag.
- **Frogport/Postbox**: 18 package-only slots, `isBackedUp`, filter string,
  `startAnimation` rules, outgoing hand-over after 50 % progress, drop on
  failed export, incoming insert/drop at the end, push to the inventory below,
  pull with the filter quirk, station deposit-immediately, target flip/mirror,
  yaw, clipboard address writing.

## Not ported

- Rendering, models, sounds, particles, animation interpolation beyond the
  numeric progress, tooltips, goggle text, UI screens, keyboard/network packets.
- Package entity (throwing, placing, physics), Package Filter/Address items'
  data components beyond the address string (the item is only `matchAddress`
  here), the rare package styles and `getRandomBox` (only the four standard
  style names/sizes are listed).
- Stock Ticker/Stock Link block entities and screens, `LogisticalStockResponse`
  paging, hidden categories, `PlayerOrder`/ordering UI, keeper (seat/heater)
  check, tick-based passive refresh.
- Table Cloth shop rules (shopping list, price checks); they were not read, so
  nothing is claimed.
- Chain conveyor routing and Train station delivery (`export` is the
  `PackageSink` boundary), `ChainConveyorPackage` motion.
- Computer events, advancements (only emitted as events), ComputerCraft
  peripherals, `PortableStorageInterface` exclusion beyond the doc note,
  `moveTo` geometry (only the 24-block constant), panel shape/path/bulb
  drawing, `FactoryPanelSupportBehaviour` (redstone link input is a boolean
  per support), saved-data (de)serialisation (all state is plain structs).

## Java -> Rust mapping

| Java (`content/logistics/...`) | Rust |
| --- | --- |
| `box/PackageItem` (static helpers) | `package::containing`, `add_address`, `get_address`, `match_address`, `set_order`, `get_order_id`, `get_order_context`, ... |
| `PackageItem.PackageOrderData` | `package::PackageOrderData` |
| `box/PackageStyles` | `package::STANDARD_STYLES` (partial) |
| `stockTicker/PackageOrderWithCrafts`, `CraftingEntry` | `package::OrderWithCrafts`, `CraftingEntry` |
| `BigItemStack` | `package::BigStack`, `INF` |
| `ItemHandlerHelper.insertItemStacked` | `package::insert_stacked` |
| `packager/PackagerBlockEntity` | `packager::PackagerState` |
| `packager/PackagingRequest` | `packager::PackagingRequest` |
| `packager/unpacking/UnpackingHandler.DEFAULT` | `packager::unpack_default` |
| `repackager/RepackagerBlockEntity` | `packager::RepackagerState` |
| `repackager/PackageRepackageHelper` | `packager::RepackageHelper` |
| `InventorySummary` | `stock_network::InventorySummary` |
| `packagerLink/RequestPromiseQueue`, `RequestPromise` | `stock_network::RequestPromiseQueue`, `RequestPromise` |
| `packagerLink/LogisticsManager` | `stock_network::{summary_of_network, stock_of, find_packagers_for_request, broadcast_package_request, group_by_packager}` |
| `PackagerLinkBlockEntity.processRequest/getPackager` | `stock_network::StockLink` |
| `LogisticallyLinkedBehaviour` (freq, power 15, cache) | `StockLink` fields, `SummaryCache` |
| `GlobalLogisticsManager`, `LogisticsNetwork` | `stock_network::LogisticsNetworks`, `LogisticsNetwork` |
| `factoryBoard/FactoryPanelBehaviour` | `factory_gauge::FactoryPanel`, `GaugeEnv`, `GaugeEvent` |
| `packagePort/PackagePortBlockEntity` | `package_post::PackagePort` |
| `packagePort/frogport/FrogportBlockEntity` | `package_post::PackagePort` (animation, push/pull) |
| `packagePort/PackagePortTarget`, `*FrogportTarget` | `package_post::PackageTarget`, `PackageSink` |

## Decisions and deliberate differences

- Items are `ItemKey { id, components }` plus `max_stack`/`bulky`; a package is
  an `ItemStack` with an inline `Package`. `components` is an opaque
  string; the adapter fills it from the item's data components so
  `isSameItemSameComponents` holds.
- `InventorySummary` keeps first-insertion order (Java's identity-hash order is
  unspecified) and never serves a stale sorted cache; `isEmpty` is true when no
  entry remains (Java keeps empty keys after `erase`).
- Random picks use the `Random` trait. Groups of shared inventories are visited
  in first-appearance order where Java uses hash-map order. `XorShift` (a
  splitmix64 step generator, name kept for the first draft) gives
  reproducible tests.
- `PackagingRequest::subtract` saturates at zero.
- The restocker skips a zero-count order instead of recording a zero promise.
- `PackagerState::perform_requests` runs one packager. The adapter groups
  requests (`group_by_packager`) and calls it per packager.
- Packager extraction: one stack per package slot, taken from the first
  eligible inventory slot each pass (see Unverified).

## Unverified (honest list)

The Java was read through a summarising fetch; no class was compared line by line and
nothing was run against a Java client. Specific gaps:

1. `attemptToSend`: the inner slot loop and the bulky rule were
   reconstructed. The summary said a bulky item ends packing "unless the
   request still has items left"; the port ends the box after any bulky item
   and lets the remaining request go into the next box. The `package_counter`
   carry between consecutive requests of one order is inferred from "its
   `packageCounter` is set to the previous value".
2. `Glob.toRegexPattern`: the glob dialect (`*` not crossing `/`, `**`,
   `[]`, `{}`) is assumed to be `java.nio` style. Case sensitivity assumed.
3. `factoryGaugeTimer` default (100) is a guess; the adapter passes the config.
4. `PackagerItemHandler.extractItem` gating (`take_held_box` only at
   `animation_ticks == 0`) is assumed.
5. `getAllPresent(freq, flag)` ordering/priority flag was not read; links are
   used in the order given (the adapter should order by link id).
6. Package style sizes are taken from the style names; rare styles and the
   random box selection were not read.
7. Table Cloth, Stock Ticker ordering/paging and `PackageOrder` internals were
   not read.
8. Repackager `attemptToSend` row "second pass removes only matching fragments"
   removes one item per matching slot; the Java may remove whole stacks (equal
   for non-stackable packages).
9. The `RequestPromise.CODEC`, saved data and NBT layouts are not ported.

No test compares against Java output; the numbers come from the source
reading. These are algorithm oracles only, not presentation parity.

## Integration

### Engine crate lines

Add next to the `vault` declaration in `src/lib.rs`:

```rust
#[path = "core/package.rs"]
pub mod package;
#[path = "core/packager.rs"]
pub mod packager;
#[path = "core/stock_network.rs"]
pub mod stock_network;
#[path = "core/factory_gauge.rs"]
pub mod factory_gauge;
#[path = "core/package_post.rs"]
pub mod package_post;
```

Modules reference each other as `crate::package`, `crate::packager`,
`crate::stock_network`; they use only `std`. No Cargo changes.

To test without touching the main tree: copy `engine/` to a scratch folder,
replace `lib.rs` with just these five declarations (or add them), set
`CARGO_TARGET_DIR` elsewhere and run `cargo test --offline`.

### Adapter hooks

- **Inventory bridge**: implement `ItemHandler` over the game inventory
  (`extract`/`insert` per slot, `duplicate` as a snapshot, `slot_limit`). Convert
  game stacks to `ItemStack` (`components` = canonical serialized components;
  `bulky` from `canFitInsideContainerItems`; packages from the
  `PACKAGE_*` components).
- **Packager block entity**: own a `PackagerState`; call `tick` each server tick
  and react to `TickReport.animation_finished` (wake a Frogport above);
  call `activate` on redstone/sign trigger and `lazy_tick` every 10 ticks;
  feed `update_sign_address` with the six neighbour signs (`sign_text`); expose
  `take_held_box` / `unwrap_box` through its item-handler capability; set
  `linked` from the adjacent Stock Link; after a `SendReport` with `extracted`,
  call `SummaryCache::deduct` on the accurate cache.
- **Repackager**: own a `RepackagerState`, call `attempt_to_send` in its tick.
- **Stock Link block entity**: maintain a `StockLink` (freq, packager id,
  inventory identity, power, loaded, `available` from the packager's cached
  inventory count). Order `links` by link id. Call
  `broadcast_package_request` and then `PackagerState::perform_requests` per
  packager; call `LogisticsNetworks::{link_added, link_loaded, link_removed,
  link_invalidated}` from the block entity lifecycle and `tick` once per
  overworld tick.
- **Stock Ticker**: sums `summary_of_network` (use `SummaryCache` 20 ticks;
  accurate 1 tick), sends `broadcast_package_request` with `RequestType::Player`.
- **Factory Gauge**: own a `FactoryPanel`, implement `GaugeEnv` over the
  world (summaries, packager lookup, promise queues), call `tick` per
  server tick and `check_for_redstone_input` every 40; drain `events` into
  packets, sounds and advancement grants.
- **Frogport / Postbox**: own a `PackagePort`, implement `PackageSink`
  (chain conveyor `addLoopingPackage`/`addTravellingPackage`, station
  delivery), call `tick` and `lazy_tick` (every 10 ticks with the inventory
  below), act on `PortEvent` (spawn dropped package entity, emit events).

### Block entities and registry content (all `create` namespace)

`packager`, `repackager`, `stock_link` (`PackagerLinkBlockEntity`),
`stock_ticker`, `factory_gauge` (panel behaviours on `FactoryPanelBlockEntity`),
`frogport`, `postbox`, chain conveyor (for delivery), `table_cloth`
(deferred), `redstone_requester` (not part of this port).

Items: the package items (`cardboard_package_*` plus rare styles), the
package filter/address item, `stock_link` item, clipboard (for port
addresses). Data components: `PACKAGE_CONTENTS` (9 slots),
`PACKAGE_ADDRESS`, `PACKAGE_ORDER_DATA`
(`order_id`, `link_index`, `is_final_link`, `fragment_index`, `is_final`,
`order_context`), `PACKAGE_ORDER_CONTEXT`.

Sounds/particles/advancements (events only): packager, trapdoor, stock link,
frogport open/catch/deposit, factory gauge confirm, `factory_gauge`
advancement, WiFi particle.

## Game integration status (Packager slice)

Authored but **not built, not run and not compared with Java** at the time of writing; the
lead applies `mixins/create/fragments/logistics.md` and runs
`Run-Acceptance.ps1 -Packager`. Nothing below is verified in game.

Wired in `mixins/create/` (modules `create_port_package`, `create_port_packager`,
`create_port_gauge`; hooks and module registration in `fragments/logistics.toml`):

* **Package items**: `create:cardboard_package_12x12|10x12|10x8|12x10`, max stack 1. Contents (nine
  slots), address and order data live in `minecraft:custom_data` (the port's own layout, not
  Create's data components). `package_runtime.rs` converts game stacks to core stacks and back
  (`components` = canonical text of the saved component patch).
* **Packager** (`create:packager`, state `facing`; works on the inventory behind it):
  redstone rising edge packs the inventory (`PackagerState::activate`, lazy re-send while powered,
  40-tick cool-down, 20-tick animation); address from adjacent signs (`sign_text`); a package in
  the input slot (hopper, chute, arm, or right click with a package) is unpacked all-or-nothing into
  the inventory behind (`unwrap_box`); empty-hand right click or any puller takes the finished box
  from the output slot. The packager is a two-slot mod container (slot 0 input, slot 1 output).
* **Mechanical Arm**: `PointKind::Packager` (selected with the Arm item like any point); the arm
  inserts packages and extracts the finished box. Needs the two additive edits in the .md.
* **Factory Gauge** (`create:factory_gauge`, state `facing`, `satisfied`): a local storage monitor
  over the inventory behind it using `FactoryPanel::tick_storage_monitor`.

Deliberate differences and caveats:

1. A finished box is pushed into an inventory in front of the Packager, or dropped as an item after
   60 ticks when the front is open air. Create holds the box until something takes it (Frogport, belt,
   hopper). The auto-drop is the port's addition for "output ejected".
2. Non-package items inserted into the input slot are passed to the inventory behind (Create's handler
   refuses them). Request mode, `linked`, the Stock Link, Stock Ticker, Repackager block and
   `perform_requests` are not wired (the cores are, the blocks are not).
3. Packing runs on a snapshot of the inventory: slot limits other than 64, slot-specific insert rules
   (furnaces) and `can_take` per face are only partly modelled; unpack leftovers are dropped.
4. `PackageOrderWithCrafts` (order context) is not persisted in the stack. Package styles are chosen at
   random from the four standard ones; rare styles are missing. No tooltip showing contents/address, no
   package entity (throwing, placing, physics), no Package Filter / Address Tag items, no clipboard
   addresses.
5. Packager and Gauge models are authored boxes, the draw of the box moving through the tray uses guessed
   offsets (0.45 block, halves of the 20-tick cycle). Gauge item drawing rotation is unchecked. No sounds
   other than a stand-in two-note chime on gauge confirm; no packager sound/particles.
6. Factory Gauge: no network, no promise queue content (it is always empty), no requests, no restocker,
   no panel connections or multiple panels per block, no value-settings screen (click cycles 0,10..100 and
   sneak toggles items/stacks), no goggles text. Redstone output is 15 while satisfied; the polarity of
   Create's panel output was not read. An unconfigured gauge (no item) stays unlit.
7. Recipes for Packager and Gauge are placeholders (the pinned recipes were not read).
8. Not started: Package Post / Frogport and Postbox. They need a package entity and an animated
   entity-like block layer; the core (`package_post.rs`) stays unwired. Stock Link needs a cross-chunk
   frequency registry like the Redstone Link plus packager lookup, so it was left out rather than faked.