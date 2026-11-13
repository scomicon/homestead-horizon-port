# Stock network: Stock Link, Stock Ticker, Package Frogport, Package Postbox

This slice puts the pure stock-network rules (`engine/src/core/stock_network.rs`) and
the package-port rules (`engine/src/core/package_post.rs`) into the game, on top of the
Packager of `docs/logistics.md`. Nothing was run when this was written: the code was
authored against the engine API and the existing runtime modules and has not been compiled.

Files (all under `mixins/create/` unless noted):

| File | Content |
| --- | --- |
| `stock_link_runtime.rs` | module `create_port_stock_link`: Stock Link and Stock Ticker block entities, frequency items, network scan, summary, ordering, queuing requests on Packagers |
| `stock_ticker_runtime.rs` | module `create_port_stock_ticker`: the `create stock ...` operator commands |
| `frogport_runtime.rs` | module `create_port_frogport`: Package Frogport and Package Postbox |
| `stock_*.rs` | hook helpers (`create_stock_*`) |
| `fragments/stock.toml`, `fragments/stock.md` | mixin entries to append and the shared-file edits |
| `tools/stock_content.py` | blocks, models, recipes, creative tab, labels |
| `stage4/acceptance/Test-HomesteadStock.ps1` | acceptance script |

## What works

**Stock Link** (`create:stock_link`, state `facing`). It attaches to a Packager: the Packager
is at `step(link, facing.opposite())`, so a link on top of a Packager has `facing=up`. A link
placed by hand picks the facing of an adjacent Packager. The link offers the items of the
inventory behind that Packager (`facing.opposite()` of the Packager) to its network; only
slots the Packager may take from count. Full redstone power (15) on a link switches it off
(`StockLink::active_packager`). Several links on one inventory count once in a summary
(`summary_of_network`) and share one random representative per request
(`find_packagers_for_request`).

**Network identity** is two frequency items on the Stock Link and the Stock Ticker, the
same way as the Redstone Link: right click with an item sets the first, then the last
slot; sneak + empty hand clears both; an empty hand shows the network in the action bar.
No frequency items is the "blank" network. Create keys networks by a UUID, so this is a
port decision (the Linked Controller and Redstone Link already use item frequencies).

**Stock Ticker** (`create:stock_ticker`) is the terminal of a network. The original opens a
request screen; here it is driven by operator commands (permission 2):

```
create stock list    X Y Z                        stock of the network of the ticker or link at X Y Z
create stock status  X Y Z                        network size, or the state of a Frogport/Postbox
create stock order   X Y Z ITEM COUNT [ADDRESS]   order; boxes leave the Packagers
create stock address X Y Z ADDRESS                address filter of a Frogport/Postbox
```

Words: a bare item is `minecraft:`; `create.brass_ingot` means `create:brass_ingot` (command
words have no colon). In addresses `+` is `*`, `all` is `*`, `none` makes a port stop
accepting packages and `clear` empties the filter.

**Ordering** is `LogisticsManager.broadcastPackageRequest` with `RequestType.PLAYER`: an
order for more than the network holds is capped at the stock; an order for an item it lacks is
accepted and makes nothing (Java returns true as well); a Packager with 50 boxes already queued
refuses it. The requests of each Packager are run through `PackagerState::perform_requests`
at once: the items leave the chest immediately, the first box is held by the Packager, the
rest queue on it and are handed out by the Packager's own tick (so the Packager keeps its
existing behaviour: the box shows in its output slot, goes into an inventory in front of it,
or is dropped after 60 ticks in front of open air). Order id, link index, final-link flag
and address are written to the boxes as in the logistics slice.

**Package Frogport** and **Package Postbox** (`create:package_frogport`,
`create:package_postbox`): 18 package slots (one per slot; other items are spat out), an
address filter (`Filter`, `Accepts`). Every 10 ticks a port, when idle,
1. pushes each package into the block below it (a chest takes it as an item; a Packager
   unpacks it into its inventory),
2. pulls one package that has somewhere to go: first from its own slots (those the port
   would not keep: addresses that do not match its filter), then from the block below (a
   Packager's finished box regardless of filter, other inventories only packages that do
   not match the filter, the original's quirk).

A package that leaves is delivered by the **virtual network** (below) with a 10-tick
animation; the receiving port plays the arrival and inserts the package. A package whose
delivery fails after the animation has started goes back into the sender.

Right click: a package in hand goes into the first free slot, an empty hand takes the last
package, sneak + empty hand shows the filter.

## Differences from Create

* **No chain conveyors and no trains.** Create connects Frogports with chain conveyors and
  Postboxes with train stations. Neither exists in this port. The virtual network connects
  every other port of the same kind within 32 blocks (same dimension): the nearest one that
  accepts the address, has a free slot and is idle receives the package. A Frogport never
  talks to a Postbox. When chain conveyors and trains are ported, `find_destination` is the
  only place to replace.
* No frog or Postbox model animation, no tongue, no mail sound; the arriving or leaving
  package rises or sinks above the port.
* No Stock Ticker screen: no categories, search, bulk ordering or recipes (crafting requests
  belong to the Factory Gauge slice). One item per order.
* No network ownership, lock or UUID, no request promises across orders (the promise queue
  of the core is not wired), no summary cache (the network is scanned for every command).
* A Stock Link is a block entity that is never ticked, so a Packager under a link still reacts
  to redstone and signs (`PackagerState::linked` is not written); keep such Packagers unpowered.
  Create disables redstone packing on a linked Packager.
* Only one dimension is scanned (the current one); a link is "loaded" if its chunk is.
* The Stock Link does not check its support: breaking the Packager leaves the link in place
  (it then offers nothing).
* Packager state is read and written by `stock_link_runtime.rs` with a copy of the layout in
  `packager_runtime.rs` (`HeldBox`, `PrevUnwrapped`, `AnimTicks`, `AnimInward`, `Cooldown`,
  `RedstonePowered`, `SignAddress`, `Queue`). If that layout changes, both files change.
* Models, shapes, recipes (all four are placeholders) and labels are this port's own.

## NBT

* Stock Link, Stock Ticker: `FrequencyFirst`, `FrequencyLast` (item), `NextSlot` (byte).
* Frogport, Postbox: items 0..17 (packages), `Filter` (string), `Accepts` (byte, default 1),
  animation state `Anim`, `Prog` (progress in thousandths), `Depositing`, `AnimPkg` (item),
  `AnimStart` (game time at start, for the client).

## Unverified (nothing was compiled or run)

* The whole slice compiles only on paper. Likely trouble spots: the `Debug` formatting of the
  dye colour in the frequency key, `ModContainer::slots` literal type (cast with `as _`),
  `Game::native_container_slots` field names (`slot`, `stack`, `can_take`, copied from the
  Packager), and `block_entities.iter()` key type `(i32, i32, i32)`.
* Whether `setblock ... {FrequencyFirst:{id:"minecraft:apple",count:1}}` merges onto the default
  NBT (the gauge test relies on the same behaviour with `Filter`).
* Command words: `word` arguments are assumed to accept `:` free ids only; `create.brass_ingot`
  is the namespaced spelling.
* Whether a Frogport above a Packager sees the Packager's output slot through
  `native_container_slots` with `can_take` set (the Packager exposes slot 1 as an output).
* Behaviour when a hopper or an arm inserts into a port (non-packages are spat out on the next tick).
