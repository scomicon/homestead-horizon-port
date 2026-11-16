# Подготовка публичной копии до stage22 и скан приватности

Дата проверки и переноса: 2026-11-16. Режим: перенос в `publish-staging` с нейтральной идентичностью **без** `git push` и без сборки релизов.

## 1. Базовая точка и проверяемые коммиты

- Базовый `HEAD` публичной копии (`publish-staging`): `20e7be0` (соответствует состоянию `91dedfd` в основном репозитории, stage19).
- Проверяемый закоммиченный `HEAD` основного репозитория: `b54ca8a`.
- Диапазон коммитов после `91dedfd` (`91dedfd..b54ca8a`):
  1. `3f0b3d6` — Client defaults (dynamic lights, Mouse Tweaks, 3D skin layers), resource pack check in game, `-ClientDefaults` acceptance
  2. `5d321a8` — Client mods (tooltips, HUD, sound), client defaults suite wiring; stage20 package
  3. `254b79a` — Пакет 1 данных (batch1), патчи движка 20-22, исправления по игровой проверке (stage22, пакет не собран)
  4. `f783c30` — roadmap: статус К3 по результатам приёмок stage22 (41 из 42 зелёные, `-ClientS` ждёт аудиовыход)
  5. `1f4aa2d` — Приёмка stage22 на пересобранном бинарнике 51002ac6: 40 из 41 зелёные
  6. `b54ca8a` — Пакет stage22: `-AudioWaiver` в `Build-Package` (звук не проверен: нет аудиовыхода), журнал `regress`, скан публикации

## 2. Итог скана приватности (закоммиченный `HEAD` = `b54ca8a` и `publish-staging`)

Проверялись все текстовые blob-объекты закоммиченного `HEAD` (`b54ca8a`), переносимый набор (97 файлов из `91dedfd..b54ca8a`), обновлённые корневые документы `publish-staging` (`README.md`, `INSTALL.md`, `THIRD_PARTY.md`) и всё дерево `publish-staging` на:
- имя пользователя Windows;
- личные адреса электронной почты и авторские ники/хэндлы (кроме системных `noreply@anthropic.com` и `noreply@users.noreply.github.com`);
- абсолютные пути профиля Windows/macOS/Linux (каталоги пользователей на дисках Windows, `/Users/...`, `/home/...`, локальные пути рабочего пространства).

### Результаты

| Область скана | Находки (реальные) | Статус |
| --- | --- | --- |
| Переносимые файлы `91dedfd..b54ca8a` (97 файлов) | **0** | Чисто |
| Корневые документы `publish-staging` (`README.md`, `INSTALL.md`, `THIRD_PARTY.md`) | **0** | Чисто |
| Полное дерево `publish-staging` после переноса | **0** | Чисто |
| Вся публично-допустимая часть `HEAD` (`b54ca8a`, по белому списку `publish-staging/.gitignore` и правилам исключения) | **0** | Чисто |

Исключённые из публичной копии файлы в основном `HEAD` (`1f4aa2d`), где сработали паттерны (в `publish-staging` не попадают):
- `outputs/homestead-horizon-stage17/Start-Homestead.ps1:4` и `outputs/homestead-horizon-stage18/Start-Homestead.ps1:4` — устаревший путь профиля Windows в дефолтном параметре `$SourceInstance` (`<профиль_Windows>\curseforge\...`, исправлено начиная со stage19; каталог `outputs/` запрещён в `publish-staging/.gitignore`).
- `work/homestead-horizon/stage4/research/publish-audit.md:174` — пример команды `git grep` с хэндлом автора (файл не менялся в `91dedfd..HEAD` и уже исключён из `publish-staging` в `20e7be0`; не переносить).

Технические ложные срабатывания (личными данными не являются):
- `mixins/create/engine/src/core/schematic_table.rs:435` — тестовая строка `"C:\\x.nbt"` в юнит-тесте валидации пути;
- `tools/burner_visual_oracle.py:113`, `tools/depot_decode_checks.py:78`, `tools/item_contact_source_checks.py:88` — путь upstream к `javac` внутри `Android Studio.app`;
- `outputs/horizon-fps/README.md:48,58` — шаблон-пример `C:\path\to\horizon-source` (вне белого списка);
- `stage4/research/vault-hook-plan.json:1280,2434,2474` — пути к файлам `admin.rs` / `server-admin-cli.json` внутри дерева upstream Horizon;
- `mixins/homestead/graves.rs:41,196` — константа уровня прав (`*_LEVEL = 2`) в основном репозитории (в `publish-staging@20e7be0` уже переименована в `OPERATOR_LEVEL`, в `91dedfd..HEAD` файл не менялся).

Дополнительное правило: `AGENTS.md` в публичную копию **не включается** (не входит в белый список `publish-staging/.gitignore`).

## 3. Проверка кодировки и BOM для Windows PowerShell 5

- Из **23** файлов `.ps1`, добавленных или изменённых в `91dedfd..b54ca8a`, 22 файла содержат только ASCII (`Cyr=False`, `NonASCII=0`) и не требуют BOM для чтения в Windows PowerShell 5 (включая `work/homestead-horizon/stage4/package/Apply-ClientDefaults.ps1` и `work/homestead-horizon/stage4/package/Start-Homestead.ps1`).
- Файл `work/homestead-horizon/stage4/Build-Package.ps1` в коммите `b54ca8a` получил русский текст для `SOUND-NOT-VERIFIED.ru.md` (340 не-ASCII байт) и при переносе в `publish-staging` сохранён с UTF-8 BOM (`EF BB BF`) для единообразия с остальными не-ASCII `.ps1` файлами публичной копии.
- Все `.ps1` с русским текстом, уже находящиеся в `publish-staging` (`Install-Homestead.ps1`, `work/rust-env.ps1`, `work/homestead-horizon/Setup-Paths.ps1`, `work/homestead-horizon/stage4/Build-LocalMods.ps1`, `work/homestead-horizon/stage4/package/Install-Homestead.ps1`), а также `work/homestead-horizon/stage4/acceptance/Test-HomesteadMotor.ps1` (содержит символ `≥`), сохранены с UTF-8 BOM (`EF BB BF`).
- В основном репозитории `work/homestead-horizon/stage4/acceptance/Test-HomesteadMotor.ps1` хранится без BOM, но в `91dedfd..HEAD` он не менялся — BOM в `publish-staging` сохранён.

## 4. Сводка фильтрации `91dedfd..b54ca8a` по `.gitignore` публичной копии

Всего в `91dedfd..b54ca8a` затронуто **984** пути:

| Категория | Кол-во | Решение |
| --- | --- | --- |
| Код, тесты, скрипты, доки в белом списке (`create-source/`, `stage4/`) | **97** | **Перенесено в `publish-staging`** |
| `outputs/**` | 789 | Исключено правилом `outputs/` в `publish-staging/.gitignore` |
| `work/homestead-horizon/stage4/extra-mods/*.hmod` | 54 | Исключено правилами `extra-mods/` и `*.hmod` в `publish-staging/.gitignore` |
| `work/homestead-horizon/stage4/run-*-result.json` | 41 | Исключено политикой публичной копии (локальные квитанции приёмки; отсутствуют в `20e7be0`) |
| `work/homestead-horizon/stage4/regress.txt`, `regress-stage22.txt` | 2 | Исключено политикой публичной копии (локальные логи регрессии; отсутствуют в `20e7be0`) |
| `.gitignore` (корень основного репо) | 1 | Не переносить (`publish-staging` хранит собственный белый список `.gitignore`) |

## 5. Полный список файлов для переноса в `publish-staging` (97 файлов)

Префикс `CS` ниже обозначает `work/homestead-horizon/create-source/create-horizon-4ae7340c06da5e77828e12494509fe9a70b8eaa4`, `ST` — `work/homestead-horizon/stage4`.

### 5.1 Документация срезов (`CS/docs`, 6 файлов)
- `A` `CS/docs/batch1.md`
- `A` `CS/docs/client-hud.md`
- `A` `CS/docs/client-sound.md`
- `A` `CS/docs/client-tooltips.md`
- `A` `CS/docs/comforts-grind.md`
- `A` `CS/docs/playtest-fixes-2.md`

### 5.2 Модули и фрагменты миксинов (`CS/mixins/homestead`, 46 файлов)
- `M` `CS/mixins/homestead/horizon-mixin.toml`
- `A` `CS/mixins/homestead/armor_bar.rs`
- `A` `CS/mixins/homestead/armor_bar_layer.rs`
- `A` `CS/mixins/homestead/client_tooltips.rs`
- `A` `CS/mixins/homestead/comforts.rs`
- `A` `CS/mixins/homestead/comforts_spawn.rs`
- `A` `CS/mixins/homestead/ctip_color.rs`
- `A` `CS/mixins/homestead/ctip_command.rs`
- `A` `CS/mixins/homestead/ctip_register.rs`
- `A` `CS/mixins/homestead/ctip_tooltip.rs`
- `A` `CS/mixins/homestead/grind.rs`
- `A` `CS/mixins/homestead/grind_result.rs`
- `A` `CS/mixins/homestead/mount_hud.rs`
- `A` `CS/mixins/homestead/mount_hud_health.rs`
- `A` `CS/mixins/homestead/mount_hud_hide.rs`
- `A` `CS/mixins/homestead/peaceful_hunger.rs`
- `A` `CS/mixins/homestead/peaceful_hunger_tick.rs`
- `A` `CS/mixins/homestead/sound_ambient.rs`
- `A` `CS/mixins/homestead/sound_bridge.rs`
- `A` `CS/mixins/homestead/sound_config.rs`
- `A` `CS/mixins/homestead/sound_footsteps.rs`
- `A` `CS/mixins/homestead/sound_muffler.rs`
- `A` `CS/mixins/homestead/sound_play_hook.rs`
- `A` `CS/mixins/homestead/sound_tick_hook.rs`
- `A` `CS/mixins/homestead/status.rs`
- `A` `CS/mixins/homestead/status_command.rs`
- `A` `CS/mixins/homestead/status_register.rs`
- `A` `CS/mixins/homestead/travel_titles.rs`
- `A` `CS/mixins/homestead/travel_titles_layer.rs`
- `A` `CS/mixins/homestead/travel_titles_tick.rs`
- `A` `CS/mixins/homestead/welcome_click.rs`
- `A` `CS/mixins/homestead/welcome_draw.rs`
- `A` `CS/mixins/homestead/welcome_key.rs`
- `A` `CS/mixins/homestead/welcome_screen.rs`
- `A` `CS/mixins/homestead/welcome_tick.rs`
- `A` `CS/mixins/homestead/fragments/batch1.md`
- `A` `CS/mixins/homestead/fragments/clientH.md`
- `A` `CS/mixins/homestead/fragments/clientH.toml`
- `A` `CS/mixins/homestead/fragments/clientS.md`
- `A` `CS/mixins/homestead/fragments/clientS.toml`
- `A` `CS/mixins/homestead/fragments/clientT.md`
- `A` `CS/mixins/homestead/fragments/clientT.toml`
- `A` `CS/mixins/homestead/fragments/engine20.md`
- `A` `CS/mixins/homestead/fragments/engine20.toml`
- `A` `CS/mixins/homestead/fragments/fixes2.md`
- `A` `CS/mixins/homestead/fragments/status.toml`

### 5.3 Инструменты контента и тесты (`CS/tools`, 10 файлов)
- `A` `CS/tools/advancement_fix.py`
- `A` `CS/tools/batch1_content.py`
- `A` `CS/tools/block_looks.py`
- `A` `CS/tools/comforts_content.py`
- `A` `CS/tools/hide_unplaceable.py`
- `A` `CS/tools/sound_content.py`
- `A` `CS/tools/test_advancement_fix.py`
- `A` `CS/tools/test_block_looks.py`
- `A` `CS/tools/test_hide_unplaceable.py`
- `A` `CS/tools/unplaceable_items.json`

### 5.4 Сборка, патчи, пакет и приёмка (`ST/`, 24 файла)
- `A` `ST/Build-Batch1.ps1`
- `A` `ST/Build-Comforts.ps1`
- `M` `ST/Build-DataMods.ps1`
- `M` `ST/Build-Package.ps1`
- `A` `ST/Build-SoundMod.ps1`
- `A` `ST/Experiment-Hunger.ps1`
- `M` `ST/Run-Acceptance.ps1`
- `M` `ST/Start-Test.ps1`
- `A` `ST/batch1-policy.json`
- `M` `ST/regress.ps1`
- `A` `ST/acceptance/Test-HomesteadAcceptanceHarness.ps1`
- `A` `ST/acceptance/Test-HomesteadBatch1.ps1`
- `A` `ST/acceptance/Test-HomesteadClientDefaults.ps1`
- `A` `ST/acceptance/Test-HomesteadClientH.ps1`
- `A` `ST/acceptance/Test-HomesteadClientS.ps1`
- `A` `ST/acceptance/Test-HomesteadClientT.ps1`
- `A` `ST/acceptance/Test-HomesteadEngine20.ps1`
- `A` `ST/acceptance/Test-HomesteadFixes2.ps1`
- `A` `ST/acceptance/Test-HomesteadSoundPlayback.ps1`
- `A` `ST/engine-patches/20-sleep-host.ps1`
- `A` `ST/engine-patches/21-peaceful-hunger.ps1`
- `A` `ST/engine-patches/22-grind-take.ps1`
- `A` `ST/package/Apply-ClientDefaults.ps1`
- `M` `ST/package/Start-Homestead.ps1`

### 5.5 Исследования и статус (`ST/research`, 11 файлов)
- `A` `ST/research/client-mods.md`
- `A` `ST/research/clients-audio.md`
- `A` `ST/research/install-client-section.md`
- `A` `ST/research/logic-machines-triage.md`
- `A` `ST/research/playtest-bugs-2.md`
- `A` `ST/research/playtest-bugs-3.md`
- `A` `ST/research/publish-stage22-scan.md`
- `A` `ST/research/roadmap-refresh-notes.md`
- `M` `ST/research/roadmap.md`
- `A` `ST/research/sound-acceptance.md`
- `A` `ST/research/sound-path-audit.md`

## 6. Итог переноса в `publish-staging`

1. Перенесены все **97** файлов диапазона `91dedfd..b54ca8a` по белому списку (включая обновлённый `ST/research/publish-stage22-scan.md`; `ST/Build-Package.ps1` сохранён в UTF-8 BOM).
2. В корне `publish-staging` включены обновлённые под stage22 `README.md`, `INSTALL.md` (с разделом из `ST/research/install-client-section.md` в «Путь А») и `THIRD_PARTY.md` (добавлены `batch1-policy.json` и клиентские модули). Всего в коммите `publish-staging`: **100** изменённых/добавленных файлов.
3. Не копировались: `AGENTS.md`, `ST/research/publish-audit.md`, `ST/run-*-result.json`, `ST/regress.txt`, `ST/regress-stage22.txt`, `ST/extra-mods/`, `outputs/`, `.hmod`, JAR, текстуры из чужих JAR. В `publish-staging` сохранены правки `graves.rs` (`OPERATOR_LEVEL`) и BOM на `Test-HomesteadMotor.ps1`.
4. Повторный скан на личные данные (имя пользователя Windows, личные адреса почты, авторский хэндл, абсолютные пути профиля Windows/macOS/Linux) по всему дереву `publish-staging` дал **0** находок.
5. Коммит в `publish-staging` выполнен локально от нейтральной идентичности `Homestead Horizon <noreply@users.noreply.github.com>`. `git push` и публикация релизов не выполнялись.
