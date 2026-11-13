# Railways, first layer (pure logic)

Status: pure engine modules with unit tests; **no gameplay integration yet**.
Source: Create 6.0.10 (`ac0c444d9828da3453ae8cc65338e8de063286fb`), packages
`content/trains/{track,graph,entity,signal,schedule}` and `CTrains`. Behaviour
is ported, not pasted. Numeric tests are *internal oracles derived from the
Java formulas*; none was cross-checked against a running Java client.

## Modules

| Rust file (`engine/src/core/`) | Scope |
|---|---|
| `train_track.rs` | `TrackShape` (19 shapes, axes, normals, rotate/mirror, junction/portal), `TrackType`, `BezierConnection` (handle length, length, step LUT, tangent/normal, arc lookup, item cost), `check_connection` (TrackPlacement geometry rules), `TrackConnections` |
| `train_graph.rs` | `TrackGraph`, nodes, directed edges, `canTravelTo` (7/8 rule), edge points, connect/disconnect/remove, components + split, merge with node unification, crossings, `GraphSave` |
| `train_motion.rs` | `TravellingPoint.travel`, selectors (steer, follow, path, random), `Train` (speed limits, passive slowdown, stress, derail, crash, collision power, stall, refresh triggers, penalties), `Navigation.tick_speed`, uniform-cost `find_path` |
| `train_signals.rs` | `SignalNetwork`: boundaries, edge groups, propagate/removal, colours, occupancy/reservations, states (red/green/yellow/invalid), cross-signal chains, crossings as intersecting groups, `SignalSave`, `Observer` |
| `train_schedule.rs` | `Schedule`, instructions (destination, rename, throttle), conditions (delay, idle cargo, time of day, fluid/item threshold, redstone link, player count, unloaded, powered), `ScheduleRuntime` state machine |

All modules are `std`-only, deterministic, and take the world through plain
data or small traits (`ConditionEnv`, `ScheduleEnv`, `TrackSelector`).

## Java -> Rust mapping

| Java | Rust |
|---|---|
| `TrackShape` | `train_track::TrackShape` (`rotate`, `mirror`, `axes`, `normal`, `as_portal`) |
| `TrackMaterial.TrackType` | `TrackType(String)` (identity only) |
| `BezierConnection` + `Runtime` | `BezierConnection::new` (handles, 16 chords, `(int)(len*2)` segments, `stepLUT`), `segment_t`, `position`, `derivative`, `normal`, `t_at_length` |
| `TrackPlacement.tryConnect` | `check_connection` (geometry/limits only) |
| `TrackBlockEntity.connections` | `TrackConnections` |
| `TrackGraph`, `TrackNode`, `TrackEdge` | `TrackGraph`, `TrackNode`, `TrackEdge` (edges stored both ways, one `EdgeData` per canonical pair) |
| `EdgeData`, `TrackEdgePoint` | `EdgePoint` (kind Signal/Station/Observer, pos from smaller node id) |
| `TrackGraph.transfer/merge`, `removeNode` + split | `merge`, `remove_node`, `components`, `split_disconnected` |
| `TravellingPoint` | `TravellingPoint::travel`, `TrackSelector` impls, `TravelHooks` |
| `Train`, `Carriage` | `Train`, `Carriage` (`tick`, `compute_stress`, `crash`, `collision_power`) |
| `Navigation.tick` (speed part), `Navigation.search` | `Navigation::tick_speed`, `find_path`, `SignalWeight`, `penalties` |
| `CTrains` | `TrainConfig` (defaults 28/14/3 and 40/20/3, assembly 128, bogeys 20) |
| `SignalBoundary`, `SignalEdgeGroup`, `SignalPropagator` | `SignalNetwork`, `SignalBoundary`, `EdgeGroup` |
| `TrackObserver` | `Observer` (activation rule only) |
| `Schedule`, `ScheduleEntry`, `*Instruction`, `*Condition`, `ScheduleRuntime` | `train_schedule::*` |

## Decisions and deliberate deviations

* Group, point, node ids are integers, not UUIDs (deterministic, cheap to save).
* Node keys are `(dimension, position*16 rounded)`; Java rounds to its own grid.
* Edge points are stored once per canonical edge; positions are measured from
  the smaller node id. `directed_pos` converts.
* Edge position is arc length; curves convert through `BezierConnection::t_at_length`.
  `increment_t` uses the dimensionally consistent `dt = d/|B'|` (the Java text
  divides by `|B'|/length`).
* The signal state of side `s` is decided by the group **ahead** (`groups[1-s]`).
  Cached states feed cross-signal chains, so a chain lags one tick like Java.
* When the front point stops early (station listener, dead end), the rest of the
  train moves the same distance instead of the full requested distance.
* Followers pick junction branches by `FollowSelector` (20 BFS rounds) toward
  the point ahead; the front point uses the supplied selector.
* Group occupancy can be recomputed from bogey points (`set_train_groups`) or
  driven by `occupy`/`release` events; recompute misses groups a long train spans
  between its end bogeys, so adapters should use events for long trains.

## Not ported

Rendering, bogey/track models, UI, entity networking, station/train maps,
conductors and seats, fuel inventory scan, mail/portals beyond zero-length
interdimensional edges, `EdgeInteraction`/portal listeners, remote graph sync
packets, train collision geometry (only the explosion power rule), whistle and
honk, `TrainStatus` messages, schedule predictions/display links, track
pavement/material checks and the distribution of excess length into track
extents in `check_connection`, NBT/StringTag codecs, Sable physics.

## Unverified / reconstructed (be sceptical)

The Java was read through a summarising fetch, not as raw text. Items that I
could not confirm against the source and that need a real-Java comparison:

* `BezierConnection`: 16 chords covering `t=0..1` (Java wording "16 uniform samples"),
  the parallel-axes handle rule (`min>1.2`, `1<max/min<3`) and which of `t/u`
  is forward vs lateral.
* `check_connection`: slope-to-slope transitions are approximated
  (`turn_size = run/2`); excess-length distribution omitted; the `-0.1` margin
  makes a 90 degree turn need ~7.1 blocks between block centres. Parallel
  check tolerances are mine.
* `TrackShape` rotate/mirror tables follow the fetched summary; portal shapes do
  not rotate in that table.
* Signal colour count (9) and the "lowest free colour" choice; cross-signal
  `Yellow` treats nested yellow as free.
* Idle-cargo condition, throttle percent range (5..100), `startCooldown` call
  sites, empty-condition entries leaving immediately, station-name matching being
  case-sensitive, and the delay/time units (ticks, seconds, minutes).
* The `TrainConfig` defaults are the `CTrains` defaults reported by the fetch.
* Per-point travel order in `Train::tick` is my reading of `Carriage.travel`.

## Integration

### `lib.rs` mod lines (not applied; mirror the vault declaration)

```rust
#[path = "core/train_track.rs"]
pub mod train_track;
#[path = "core/train_graph.rs"]
pub mod train_graph;
#[path = "core/train_motion.rs"]
pub mod train_motion;
#[path = "core/train_signals.rs"]
pub mod train_signals;
#[path = "core/train_schedule.rs"]
pub mod train_schedule;
```

The modules refer to each other as `crate::train_*`, so they must be declared at
the crate root with these names.

### Adapter hooks needed

1. **Track blocks**: block state `shape` (`TrackShape::name/parse`), `has_be`,
   material -> `TrackType`; on place/break update `TrackGraph` nodes/edges (node
   positions from shape axes and block centre; `TrackNode.normal` from
   `TrackShape::normal`). Curved connections come from `check_connection` +
   `BezierConnection::new`; keep `TrackConnections` per track block entity.
2. **Graph registry** (world level): graph ids, `split_disconnected` after
   removals, `merge` when placement joins graphs, `find_crossings` +
   `SignalNetwork::refresh_intersections` after topology changes.
3. **Edge points**: station, signal, observer block entities call
   `add_point`/`remove_point`; signals additionally
   `SignalNetwork::add_boundary` / `on_signal_removed`, `add_signal_block`,
   `set_powered` (redstone), `cycle_signal_type`.
4. **Tick order** per server tick: `SignalNetwork::tick_pre` -> schedule runtime
   -> navigation scouting (adapter probes with `TravellingPoint::travel` and
   hooks, fills `NavInput`, `distance_to_signal`, reservations) ->
   `Navigation::tick_speed` -> `Train::tick` (front-point listener for
   station/signal edge points) -> occupancy updates -> `tick_post`.
5. **Trains**: adapter builds `Carriage` points from assembled bogeys, spawns
   carriage entities at `TravellingPoint::world_position` + `EdgeData`
   direction, applies `Train::tick` results (`blocked`, `derailed_now`,
   `collision_power` explosion, crash).
6. **Schedules**: implement `ScheduleEnv` (path start using `find_path` +
   station name `matches_filter`), `ConditionEnv` (train inventory/fluid, link
   network, day time, passengers, station redstone).

### Block entities / entities / content needed

Blocks: track (+ placement item logic), girder-reinforced variant, track
station, train signal, train observer, controls (manual driving via
`manual_tick`), schedule item, bogey blocks. Entities: carriage contraption
entity (follow a `Carriage`), train. Items: schedule, train map are optional.
Tags/recipes belong to the content pipeline.

### Save format proposal

World-level `create_trains.dat`: `{ graphs: [GraphSave], signals: [SignalSave per
graph], trains: [{id, graph, carriages:[{lead:{n1,n2,pos,flip},trail:{...},
bogey_spacing}], spacing:[f64], speed, throttle, fuel_ticks, derailed,
nav:{destination, behind, ...}, runtime:{schedule, current_entry, state,
progress, paused, completed}}] }`. `GraphSave`/`SignalSave` are plain data with
no UUIDs; the adapter encodes them (NBT or JSON). Beziers are rebuilt from axes
and normals on load (`TrackGraph::load` verified by a round-trip test).

## Tests

61 unit tests in the five modules (track geometry numbers, graph ops, travel
and listeners, full run to a station with braking, stress/derail, path search,
signal propagation/chains/removal, schedule runtime). Run in a scratch copy with
the five `mod` lines added: `cargo test --offline` (331 tests total with the
existing 270 all pass at time of writing).
