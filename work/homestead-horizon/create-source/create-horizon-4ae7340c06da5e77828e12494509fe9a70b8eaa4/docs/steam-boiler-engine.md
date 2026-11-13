# Boiler, Steam Engine and Powered Shaft (game adapter)

Written without a build; nothing here has been run in game. Rules:
`engine/src/core/{boiler,steam_engine}.rs` (see `tank-boiler-steam.md`).
Adapters: `mixins/create/steam_runtime.rs` (module `create_port_steam`) and the
boiler section of `mod tank` in `mixins/create/basin.rs`. Content: `tools/steam_content.py`.

## Behaviour

- A Fluid Tank multiblock becomes a boiler while a Steam Engine (or Steam Whistle)
  touches any of its blocks. The tank controller counts them (`Boiler.Engines`,
  `Boiler.Whistles`) every 5 ticks while active, every 20 ticks while idle, and at
  once when an engine's first tick calls `tank::request_update`.
- Heat: the blocks directly under the controller's footprint. A Blaze Burner
  (`blaze` state) gives smouldering 0 (passive), fading/kindled 1, seething 2;
  blocks of tag `create:passive_boiler_heaters` give passive heat unless unlit.
- Water: while the boiler is active the tank stores no fluid. Water that flows in
  (bucket, pipe transfer) is counted as supply, sampled every 5 ticks into a
  10-slot ring (`Boiler::tick_server`); other fluids are refused and the boiler
  cannot be drained. This is Create's `BoilerFluidHandler`. It means a bucket
  gives 200 mB/tick of supply for about 50 to 100 ticks, then the boiler starves
  unless a pump keeps feeding it. `POOL_FEEDS_BOILER` (basin.rs, default false)
  makes a water pool count as a steady supply for debugging only.
- Level = min(active heat, water supply / 10, tank blocks / 4), at most 18. A boiler
  needs at least 4 tank blocks for level 1 (2x2x1 or 1x1x4).
- Efficiency per engine = 1 if engines <= level, else level / engines
  (passive heat: 1/8 / engines). Written to `Boiler.Efficiency`.
- Windows are forced off when the boiler activates and the wrench no longer
  toggles them while active.
- Engine: placed on a tank face (`face` floor/wall/ceiling, clicked face = the face
  of the tank, `facing` = clicked side for walls, player direction for floor/ceiling);
  needs a tank behind it (`can_survive`). On its first tick it places a Powered Shaft two
  blocks ahead if that cell is empty; a plain `create:shaft` there (axis not along
  the engine) is converted. Every tick it computes the direction (`engine_tick`),
  reads the boiler efficiency and calls `PoweredShaft::update`.
- Powered Shaft: `Kind::Generator { rpm, capacity }`. rpm = direction * 16 *
  modifier (1..4), capacity per RPM = efficiency * 1024 / modifier, so the network
  gets efficiency * 16384 SU in total. It warms up 3 ticks, belongs to one engine, and
  becomes a plain Shaft ten ticks after its engine is gone (the original shaft
  reverting is recalled, not verified).

## NBT

Tank controller `Boiler` compound (inactive tanks keep the pristine compound):

| Key | Type | Meaning |
|---|---|---|
| ActiveHeat | Int | sum of active heater values |
| PassiveHeat | Byte | passive heat present (counts only if ActiveHeat is 0) |
| Engines, Whistles | Int | attachments over all tank blocks |
| Supply | Float | water supply, mB/tick |
| Update | Byte | re-count / heat refresh requested |
| Level | Int | boiler level, `Boiler::level(size)` (read by the whistle) |
| Efficiency | Float | engine efficiency 0..1 |
| Gathered, Ring, RingIndex, SampleIn | Int, List<Float>, Int, Int | sampling state |

Powered Shaft (`create:powered_shaft`): Engine IntArray (shaft - engine), Efficiency,
Direction, Warmup, CapacityBase, GeneratedSpeed, GeneratedCapacity (per RPM), AppliedSpeed,
AppliedCapacity, OrphanTicks, plus the kinetic keys (Speed, ...). Engine
(`create:steam_engine`): Placed, CounterClockwise.

## Gaps and unverified items

- No steam particles, no engine/boiler/whistle sounds, no goggles tooltip for the
  boiler or engine (heat/level/water lines), no pressure gauge, no status text.
- Engine animation: only one piston sliding 0.3 block with the shaft angle; Create's
  linkage and rotating connector are not drawn. Model boxes are authored guesses; the
  selection box is the full cube.
- Tank light and opacity rules for an active boiler (`isOpaque`) are not applied
  (no light state exists on the tank).
- The externally-driven-shaft direction flip (`shaft_has_source`) is not modelled.
- Shaft axis chosen for a vertical engine (perpendicular to `facing`) is a guess; the
  original uses the placer's nearest-first look directions.
- Recipe, hardness and resistance of the engine are recalled, not verified.
- Whistle counting assumes block `create:steam_whistle` with states `wall`, `facing`
  (`steam_runtime::WHISTLE`).
- A powered shaft joins a network through `runtime::node`; a stopped boiler publishes
  speed 0 through the same source-change path as the windmill.
- The idle re-count is polled (20 ticks) and by request; neighbour-change events are
  not wired.

## Integration checklist (files this work does not own)

See the report of this task: `runtime.rs` (owns/entity/ticks/node/tick/update_tag/draw),
`horizon-mixin.toml` (module + survival injection), `placement_runtime.rs`,
`content.py`, `catalog.py`, `catalog.rs`, `fallback_textures.py`, tag
`create:passive_boiler_heaters`, lang.
