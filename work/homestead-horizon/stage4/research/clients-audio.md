# Почему красный набор `-ClientS` и как его разблокировать

## 1. Причина падения `-ClientS`

Набор `-ClientS` падает на строгой проверке живого звукового движка в фазе `Playback` (`work/homestead-horizon/stage4/acceptance/Test-HomesteadSoundPlayback.ps1`).

Факты из квитанций и окружения:

- В квитанции `work/homestead-horizon/stage4/run-clients-result.json` зафиксировано:
  - `passed`: `false`
  - `phases`: `["ClientS"]` (фаза `ClientS` прошла, но вторая обязательная фаза `Playback` завершилась ошибкой до добавления в `phases`)
  - `process_id`: `23312`
  - `binary_sha256`: `2964252b617197f9a0d1fb16ce426a6cb3cee004361b471503231c249f8ce4b6`
  - `error`: `"The client has no healthy audio backend: Horizon: no sound: no audio output device on the WASAPI host. This cannot pass as sound-playback acceptance."`
- В квитанции фазы воспроизведения `work/homestead-horizon/stage4/runtime/game/stage33-soundplayback-playback-result.json` (от более раннего запуска с `process_id`: `14192`, `binary_sha256`: `3abafb37372aa016fefece674f179cdbc085df5ea69a4099f5050f60c02a19d7`) зафиксированы `passed: false`, `playback_hook_verified: false`, `audio_verified: false` и та же ошибка бэкенда: `audio_backend_error: "Horizon: no sound: no audio output device on the WASAPI host"`.
- Проверка аудиоподсистемы Windows на текущей машине показывает, почему WASAPI не видит ни одного выхода:
  - служба `Audiosrv` запущена (`Status: Running`, `StartType: Automatic`);
  - аудиоадаптеры класса `MEDIA` присутствуют и исправны (`Realtek(R) Audio` и `High Definition Audio Device`, статус `OK`);
  - при этом `Get-PnpDevice -Class AudioEndpoint` пуст (`0` устройств): в Windows нет ни одной активной конечной точки вывода звука (к разъёмам Realtek не подключены наушники или колонки и нет виртуального аудиоустройства воспроизведения).

При отсутствии конечной точки WASAPI инициализация звука в Horizon (`Audio::with_output`) завершается без открытого потока вывода (`loaded = false`), поэтому `SoundEngine::play_inner` возвращает `NotStarted` ещё до вызова `playback_parameters` и `native_sound_play`. Мост `homestead_sound_bridge` не вызывается, и `hook_seen` остаётся равным `0`.

## 2. Условие допуска в `Build-Package.ps1`

В `work/homestead-horizon/stage4/Build-Package.ps1` (строки 21–37, блок для `run-clients-result.json` на строках 27–36) действуют следующие обязательные условия сборки пакета:

1. Для каждой квитанции из списка `$runs` (строки 23–26): `passed` должно быть `true`, а `binary_sha256` должно совпадать с SHA-256 текущего бинарника-кандидата `work/homestead-horizon/stage4/candidate/client/horizon-client.exe`.
2. Дополнительно для `run-clients-result.json` (строки 27–36):
   - массив `phases` должен содержать ровно 2 элемента, и среди них обязаны быть и `'ClientS'`, и `'Playback'`;
   - должен существовать файл `work/homestead-horizon/stage4/runtime/game/stage33-soundplayback-playback-result.json`;
   - в этой квитанции фазы `Playback` должны выполняться одновременно четыре условия:
     - `passed -eq $true`,
     - `playback_hook_verified -eq $true`,
     - `binary_sha256` совпадает с хэшем текущего кандидата (`$binary`),
     - `process_id` совпадает с `process_id` в `run-clients-result.json` (обе фазы выполнены в рамках одного и того же запущенного процесса клиента).

## 3. Как разблокировать прогон

1. Создать в Windows активную конечную точку вывода звука (`AudioEndpoint`):
   - физически подключить наушники или колонки к выходу `Realtek(R) Audio` (чтобы Windows активировала устройство воспроизведения), **либо**
   - установить виртуальное аудиоустройство воспроизведения (виртуальный аудиодрайвер / virtual audio cable).
2. Убедиться, что `Get-PnpDevice -Class AudioEndpoint` возвращает хотя бы одно активное устройство вывода со статусом `OK`.
3. Дождаться завершения других запусков клиента (убедиться, что `horizon-client` не запущен) и на актуальном кандидате выполнить:

```powershell
pwsh work/homestead-horizon/stage4/Run-Acceptance.ps1 -ClientS
```

`Run-Acceptance.ps1` до запуска клиента выставит `HOMESTEAD_AUDIO=1` и `HOMESTEAD_WINDOWED=1`, последовательно пройдёт `Test-HomesteadClientS.ps1` (`ClientS`) и `Test-HomesteadSoundPlayback.ps1` (`Playback`) в одном процессе и запишет согласованные `run-clients-result.json` и `runtime/game/stage33-soundplayback-playback-result.json` с общими `binary_sha256` и `process_id`.

## 4. Почему обходить проверку нельзя

- **Первая фаза `ClientS` не проверяет перехват звука.** При `hook_seen = 0` сценарий `Test-HomesteadClientS.ps1` переводит проверки `muffler` и `footsteps` в примечания (`notes`) и всё равно выставляет `passed = true`, подтверждая только парсинг конфигов и постановку запросов `ambient` в очередь.
- **Без живого WASAPI-выхода хук `native_sound_play` не выполняется.** Пока движок не открыл аудиовыход, `SoundEngine::play_inner` не доходит до `playback_parameters`, и перехватчик `homestead_sound_bridge` вообще не получает ни одного звукового события.
- **Фаза `Playback` — единственный автоматический gate реального callback.** `Test-HomesteadSoundPlayback.ps1` проверяет отсутствие ошибок аудиодвижка в `stderr.log`, поступление ванильных звуков (`block.note_block.hat` -> `block.note_block.bell`) в callback, реальное срабатывание правил `muffler` (отмена 0 %, масштабирование 25 %, пропуск без правила) и появление `homestead_sound:tone.high` по идентификатору `last_sound` в `homestead-sound-status.txt`.
- **Отключение или подмена проверки скроет нерабочий звуковой мост.** Ослабление условия в `Build-Package.ps1` приведёт к выпуску пакета, в котором работа клиентских звуковых модов (`Extreme Sound Muffler`, `Presence Footsteps`, `AmbientSounds`) на текущем бинарнике не подтверждена ни разу.
