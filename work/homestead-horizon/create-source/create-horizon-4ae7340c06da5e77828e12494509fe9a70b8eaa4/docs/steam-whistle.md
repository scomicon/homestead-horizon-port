# Steam Whistle

Rules: `mixins/create/engine/src/core/steam_whistle.rs` (15 whistle tests, run with
`cargo test --offline whistle`). Game adapter: `mixins/create/whistle_runtime.rs`
(module `create_port_whistle`). Content: `tools/whistle_content.py`. Boiler and tank side:
`docs/tank-boiler-steam.md`, `docs/fluid-tank-multiblock.md`.
Status: written without a build; nothing here has been run in game.

## Blocks and states

| Block | Item | States |
|---|---|---|
| `create:steam_whistle` | yes | `facing` (north/south/west/east), `wall`, `powered`, `size` (small/medium/large) |
| `create:steam_whistle_extension` | no (drops nothing) | `shape` (single/double/double_connected), `size` |

* `wall=false`: the whistle stands on a Fluid Tank (attached DOWN). `wall=true`: it hangs on the tank
  in direction `facing` (the model's base plate is on that side). `can_survive` needs a
  `create:fluid_tank` block on the attached side; losing it removes the whistle.
* Extensions stand on the whistle or on a non-single extension. Only the top one may be `single`;
  one below another is `double_connected`; a double top keeps its rim (`double`).
* Recipe: `create:golden_sheet` over `minecraft:copper_ingot` (the original's gold plate over copper
  ingot). Loot: the whistle drops itself, the extension drops nothing.

## Behaviour

* **Placement** (`state_for_placement`): clicking the top of a tank gives a floor whistle facing the
  player; clicking a tank side gives a wall whistle with `facing` toward the tank. No tank on the
  attached side refuses the placement.
* **Pitch** (`updatePitch`): the extender stack above the whistle, every non-single extender +2 and a single
  on top +1 (cap 26). Playback rate `2^(-pitch/12)`. Pitch id `pitch + 100 * size` (for the Pipe Organ
  advancement, not wired).
* **Steam Whistle item on a whistle or its extension** (`incrementSize`): adds a single extender or
  turns the top single into a double (search over six slots), copies the whistle size, consumes the item and
  plays a chime at `2^(exponent/12)`. Sneak places a block instead.
* **Wrench on a whistle or extension**: cycles `size` small, medium, large for the whole stack.
* **Power**: each tick the whistle polls `native_neighbor_signal` and mirrors it into `powered`
  (flag 2, clients only).
* **Sound**: while `powered` and the tank's boiler is active and hot (`Boiler.Level > 0`, and
  `Boiler.Whistles > 0` when that key exists) the whistle plays. On start: the chiff `create:steam`
  (volume 0.175, rate `start_pitch`: `sound_pitch`, plus 0.75 for a small whistle); then the sustained note,
  `create:whistle_high` (small), `create:whistle` (medium), `create:whistle_low` (large) at rate
  `2^(-pitch/12)`, replayed every `floor(60 / clamp(rate, 0.5, 2)) - 2` ticks (the original samples
  last 3.00 s, measured on the JAR files). Source `RECORDS` (2) for the note, `BLOCKS` (4) for the
  confirmation chime.

## NBT

Whistle block entity `create:steam_whistle`: `Pitch` Int (cached stack pitch), `Sounding` Byte,
`SoundTimer` Int (ticks to the next replay). Synced to clients: `Pitch`, `Sounding`.
Extensions have no block entity.

Contract with the boiler code (read only, never written by the whistle). The controller's
`create:fluid_tank` entity holds `Boiler` (Compound): `Level` Int 0..18 (0 = cold), optional
`Whistles` Int (attached count). A member tank block holds `Controller` IntArray[3] (offset
controller minus this block, absent on the controller). The whistle takes the tank at its attached
side, follows `Controller`, and reads `Boiler` there.

## Sound assets

The Create JAR (checked on the 6.0.8.1 fabric probe) has `assets/create/sounds/whistle.ogg`,
`whistle_high.ogg`, `whistle_low.ogg` (mono, 44.1 kHz, 3.00 s) and `steam.ogg` (mono, 48 kHz, 0.70 s) and
the events `create:whistle`, `create:whistle_high`, `create:whistle_low`, `create:steam` in its
`sounds.json`. The importer copies `sounds.json` and every `assets/create/sounds/*.ogg`, so with the JAR
the original sounds play. Without it `SOUND_ALIASES` in `whistle_content.py` maps the events to vanilla
sounds: `minecraft:block.note_block.flute` (pitch 1.5 / 1.0 / 0.75 for small / medium / large) and
`minecraft:block.fire.extinguish` for the chiff. A flute note is much shorter than three seconds, so
without the JAR the note pulses once per replay period.

## Trace

With `CREATE_HORIZON_TRACE` set, each played sound logs one line:
`Create whistle: {"action":"sound","kind":"chiff|note","event":...,"pos":[x,y,z],"volume":...,"pitch":...,"size":...,"pitch_id":...,"level":...}`.
The harness counts those lines (audio is off with `HORIZON_NO_AUDIO=1`; the sound is still queued).

## Not ported / gaps

* A looping, stoppable sound instance: a server cannot stop a one-shot, so the last replay rings for up
  to three seconds after the power is cut, and a pitch change is heard at the next replay.
* Per-listener distance falloff of the chiff (`chiff_volume(distance)`): the server plays it at
  distance 0; the client attenuates by its own rules.
* Steam jet particles, the whistle mouth animation (chase 0.5 / 0.4), the goggle pitch line, Train whistles, the
  `steam_whistle` and `pipe_organ` advancements, a Ponder scene.
* Which sound event each size uses, the gain of the sustained note, the confirmation chime
  and whether the confirmation consumes the item were not read from the source (assumptions).
* Placement facing for floor whistles (`player.opposite()`) and the wall `facing` convention (toward
  the tank, consistent with the model and `getAttachedDirection`) are unverified.
* The shapes are estimates.
* The extension `shape` assignment of the stack (`stack_shapes`) is the port's rule.

## Integration

See the checklist the author handed over with this work: runtime.rs (`entity`, `ticks`, `tick`,
`update_tag`), `right_click.rs`, `placement_runtime.rs`, three hook helpers
(`whistle_survival.rs`, `whistle_shape.rs`) with `horizon-mixin.toml` entries, `catalog.py`,
`content.py`, `fallback_textures.py`.
