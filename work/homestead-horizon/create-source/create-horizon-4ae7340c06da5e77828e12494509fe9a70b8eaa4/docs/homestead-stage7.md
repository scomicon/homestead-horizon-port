# Homestead stage 7: Crops Love Rain and Better Days (`homestead` bundle)

Two more small mods in `mixins/homestead` (bundle version `0.1.0+homestead.2`). Settings come
from the pack's own configs in the CurseForge instance.

## Crops Love Rain (thepoultryman, MIT, 1.4.0)

Pack config `crops-love-rain.toml`: `useRainGrowthSpeed = true`, `rainGrowthSpeed = 3`,
`useIndividualSpeeds = false`, so the general speed 3 applies to everything.

Module `rain.rs`, hooked into `random_tick`. The rule was read from the mod's bytecode
(`CropsLoveRain.shouldGrowExtra`): when `Level.isRainingAt(pos)` is true and
`random.nextInt(3) == 0`, a random tick gives one extra growth step.

- Wheat, carrots, potatoes, beetroots: lit enough (raw brightness at least 9) and not ripe,
  age plus one.
- Sugar cane: the block above is air and the plant is under 3 high, then age plus one,
  or, at age 15, a new segment on top and age 0.
- The block's own random tick still runs as before.

Verified in game (`stage4/Run-Acceptance.ps1 -Rain`): on the same moist plots (water row,
random tick speed 100, 200 ticks) 90 wheat grew a total of 65 age steps in clear weather and
231 in rain; 60 sugar canes 295 and 426.

### Known differences

- Saplings and bamboo are not ported. The mod's sapling step calls the tree growth
  directly (no light check); its bamboo step needs the bamboo height helpers, and its
  height test looks inverted, so the original behaviour there is unclear.
- A beetroot stops at its own maximum age 3; the original tests `age < 7` and would try an
  invalid state.
- Only vanilla crops are covered; modded `CropBlock` subclasses are not.

## Better Days (Serilum, LGPL-3.0-or-later, 3.3.4.5)

Pack config `betterdays-common.toml`: `speedMethod = "MINUTES"`, `daySpeedMinutes = 25`,
`nightSpeedMinutes = 15`, `dayStart = 23500`, `nightStart = 12500`.

Module `days.rs`, a HEAD injection on `Game::advance_clock`: it sets the clock rate before the
tick. By day (13000 clock ticks) the rate is 13000 / (25 * 1200) = 0.4333; by night (11000
clock ticks) it is 11000 / (15 * 1200) = 0.6111. The clock rate is also what is sent to
clients, so they run the same speed.

Verified in game (`-Days`): 400 game ticks move the clock by 174 by day (expected 173) and by
246 by night (expected 244).

### Known differences

- Not ported: faster sleeping (`sleepSpeedMax 110`), the bed clock, sleep messages,
  `clearWeatherOnWake`, the random tick / potion / hunger / block entity effects (all off
  except `weatherEffect = SLEEPING`) and the client config. Sleeping uses vanilla's
  skip to the morning.
- A `/time rate` command is overridden every tick.
- The start of a day or night inside a very long game tick is not interpolated; the rate
  changes at the first tick after the boundary.

## Method notes

- `random_tick` is a named hook that gives the level (`GameLevel`) and the ticks; the helper
  reaches `native_set_block_property`, `native_level_random` and `is_raining_at`
  (`LevelAccess`) through the injected module's access to `crate::game`.
- A non-hook method such as `advance_clock` is targeted by path (`horizon_client::game::chat::Game<'_>::advance_clock`).
- Native bundle digests: the launcher hashes every file of the `.hnmod` archive, including
  `LICENSE` and `NOTICE.md`, in ordinal name order (name bytes, u64 LE length, bytes). This
  reproduces the stage 3 digest `be98fb1b...` exactly, and `Build-Package.ps1` now uses it.