# Client sound mods (group S): shared-file edits for the lead

New files (paths relative to the create-source checkout `CS`; `ST` = `work/homestead-horizon/stage4`):

* `mixins/homestead/sound_config.rs`, `sound_muffler.rs`, `sound_footsteps.rs`, `sound_ambient.rs`
  (modules `homestead_sound_config|muffler|footsteps|ambient`), hook helpers `sound_play_hook.rs`,
  `sound_tick_hook.rs`
* `mixins/homestead/fragments/clientS.toml` (4 modules, 2 injections; parses, ids and module names
  are unique against the bundle, every source and helper file exists) and this file
* `tools/sound_content.py`, `docs/client-sound.md`
* `ST/Build-SoundMod.ps1`, `ST/acceptance/Test-HomesteadClientS.ps1` (prefix `stage26-clients-`)

Nothing was built, run or committed.

## 1. `mixins/homestead/horizon-mixin.toml`

Append the whole of `fragments/clientS.toml`. Bump the version after the last slice
(`0.1.0+homestead.9` -> `.10`, or whatever follows the other stage 20 slices).
One injection targets a free function of the client crate
(`horizon_client::sound_playback::native_sound_play`): it passes `sound`, `volume`, `pitch`,
`cancel` by the parameter names of that function. The helper calls
`crate::game::homestead_sound_*`, which is why the entry functions are `pub(crate)`. If the tool
rejects a target that is not a `Game` method, the fallback is a one-line call at the head of
`sound_playback.rs`'s function in an engine patch (`stage4/engine-patches`), same helper body.

## 2. Content mod

`ST/Build-SoundMod.ps1` builds `homestead_sound-0.1.0-homestead.1.hmod` into `ST/extra-mods`
(the name does not match `Build-DataMods.ps1`'s cleanup pattern). Run it before
`Prepare-Candidate.ps1`; `Build-Package.ps1` copies `extra-mods/*.hmod` into the package and
`Start-Homestead.ps1` enables every `.hmod` in `mods/` by itself. Without it the footstep and
ambience tests fail with `play_failed` (the placeholder events do not exist).

## 3. `ST/Run-Acceptance.ps1`

* `param(...)`: add `,[switch] $ClientS`.
* `$reportName` chain: start with `if ($ClientS) {'run-clients-result.json'} elseif (...)` (the
  existing first `if` becomes `elseif`).
* `$runs` chain: add a branch
  `} elseif ($ClientS) { $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadClientS.ps1'),'ClientS','stage26-clients-')) `
  next to the `-ClientDefaults` branch (same shape).

## 4. `ST/regress.ps1`

In `$sets` add `,@('clients',@{ClientS=$true})`.

## 5. `ST/Start-Test.ps1`

Like it already removes `horizon-extras.txt`, it should delete
`config/homestead-muffler.txt`, `homestead-footsteps.txt`, `homestead-ambient.txt` and
`homestead-sound-status.txt` of the disposable instance before each run, so an interrupted
sound suite cannot leave rules for the next suites (the test deletes them in `finally`, but a
killed run does not).

## 6. `ST/Build-Package.ps1`

Add `'run-clients-result.json'` to `$runs`, a provenance line "Client sound mods: Extreme Sound
Muffler, Presence Footsteps, AmbientSounds (stage 26)", and run `Build-SoundMod.ps1` before the
`extra-mods` copy. README rows (`README.ru.md` of the package): the three mods, with the honest
list: config files instead of screens, no sounds shipped, audio not verified.

## 7. Docs

`ST/research/roadmap.md`: a row for the three sound mods; `ST/research/client-mods.md`: mark
Extreme sound muffler, AmbientSounds and Presence Footsteps as "ported (engine only)".

## Risks (never compiled)

* Visibility: the helper in `sound_playback.rs` calls `crate::game::homestead_sound_muffler`
  (`pub(crate) mod`, entry functions `pub(crate)`). `Game::native_config_dir` and
  `native_playing` are `pub(super)` in `native_hooks`: visible to modules inside `game.rs`.
* `NativePlayer` fields (`x`, `y`, `z`, `dimension`) are used as in `native_hooks.rs`
  (`dimension` through `.to_string()`).
* `winit::keyboard::KeyCode` is used the way `create_port_controller` does; `game.keys.contains`.
* `native_sound_play` is documented as game-thread only; if a build calls it from the audio
  thread the mutex is still safe, but `native_play_local_sound` is never called from it.
* The status file is rewritten twice a second (`.tmp` + rename): negligible, but visible in
  the config folder.
* The mutex is global to the process: split screen would share the rules (the engine says
  instance ids are not unique across split-screen engines; the mods do not use them).
