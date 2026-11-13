# AGENTS.md: что это за проект и что в нём сделано

Файл для ИИ-агентов (Codex и других), которые продолжают работу. Читайте его первым. Все пути относительные от корня репозитория. Личных данных здесь нет и быть не должно (см. раздел «Публикация»).

## 1. Что это

Перенос сборки **Homestead** (CurseForge, Minecraft с модами) на **Rust** для нативного клиента **Horizon** (Minecraft 26.3 на Rust). Моды Java не запускаются: их поведение переписывается как «нативные миксины» Horizon (Rust), а контент (блоки, предметы, рецепты, текстуры, структуры) упаковывается в `.hmod`.

Порядок приоритета, выбранный владельцем: **Create глубже, без поездов** → **Farmer's Delight** → мелкие logic-моды → контент-моды как оболочки. Поезда Create владелец явно отложил.

Общение с владельцем идёт на русском, короткими командами («дальше», «давай ...»). Работа идёт волнами с параллельными субагентами.

## 2. Состояние на сейчас

- **Принят и упакован stage18** (31 набор приёмки на одном бинарнике, все зелёные). Пакет: `outputs/homestead-horizon-stage18` (в git не входит).
- **stage19 в работе:** logic-моды групп A-D (Double Doors, Shulker Drops Two, Invisible Frames, Accelerated Decay, No Animal Tempt Delay, Falling Trees, Stack Refill, CraftingPad, SpawnFilter, Universal Graves), правки запуска/кэша, `Install-Homestead` в пакете. Код написан, интеграция и приёмка шли в момент написания этого файла. Проверьте `git log` и `outputs/`.
- Публичный репозиторий-копия (только код и документы) ведётся отдельно, см. раздел 9.

## 3. Раскладка репозитория

```
work/
  rust-env.ps1                       окружение сборки (Rust 1.99.1, llvm-mingw, git)
  toolchain/ llvm-mingw/ mingit/     инструменты (не в git)
  horizon-source/ ...                исходники клиента Horizon (не в git)
  homestead-horizon/
    create-source/create-horizon-<rev>/   форк create-horizon (MIT) + наши модули  (CS)
      mixins/create/                  порт Create (Rust, миксины); engine/src/core/*.rs чистые ядра с тестами
      mixins/homestead/               мелкие моды логики и срезы Farmer's Delight
      fd/                             наложение компонентов Farmer's Delight (overlay.py и срезы)
      tools/                          генераторы контента (content.py, catalog.py, *_content.py)
      docs/                           документ по каждому срезу
      mixins/*/fragments/             правки общих файлов от срезов (*.toml/.md)
    stage4/                           сборка, приёмка, пакетирование (ST)
      build.ps1, Prepare-Candidate.ps1, Run-Acceptance.ps1, regress.ps1, Build-Package.ps1,
      Build-FarmersDelight.ps1, Build-DataMods.ps1, Build-Graves.ps1, Build-CraftingPad.ps1, Build-Cache.ps1
      acceptance/Test-Homestead*.ps1  сценарии приёмки
      package/                        Start-Homestead.ps1, Play-Create.cmd, Install-Homestead.*
      research/roadmap.md             план, долги, решения (главный документ по состоянию)
    Setup-Paths.ps1                   пути к исходникам (для публичного репо)
outputs/homestead-horizon-stageNN/    готовые пакеты (не в git)
```

`CS` = `work/homestead-horizon/create-source/create-horizon-<rev>`, `ST` = `work/homestead-horizon/stage4`.

## 4. Как это устроено технически

- **Миксины Horizon.** Бандл = каталог `mixins/<id>/` с `horizon-mixin.toml` (`[[module]]` с Rust-файлами, `[[injection]]` с `hook` и `target`). Утилита `horizon-mixins apply` накладывает бандлы на копию движка в `work/homestead-horizon/staged-engine`, затем cargo собирает клиент (профиль `fast`). Бандлы: `create` (версия 0.11.x+) и `homestead` (0.1.0+homestead.N).
- **Несколько инъекций на один хук** безопасны и складываются (block_entity, tick, right_click_block и т.д.). Новая функциональность добавляется новыми инъекциями, а не правкой старых.
- **Контент.** `tools/content.py` генерирует блоки/предметы/рецепты/модели/языки Create. Farmer's Delight и «данные»-моды собираются в `.hmod` (`Build-FarmersDelight.ps1`, `Build-DataMods.ps1`). Вкладки креатива: `data/<ns>/horizon/creative_tab/*.json`.
- **Ресурсы Create.** Игра импортирует оригинальные ресурсы из локального JAR Create. Если импорт падает (в логе «using provisional resources»), всё рисуется нарисованными заглушками из `fallback_textures.py`. Модели с `porting_lib:composite`-загрузчиком (Nixie, Display Link/Board) импортёр не читает, они исключены из списка импорта.
- **Ядра и адаптеры.** Чистая логика Create лежит в `engine/src/core/*.rs` (юнит-тесты, 775 штук на момент stage14, ядра с тех пор почти не менялись). Адаптеры к игре (`*_runtime.rs`, `*_entity.rs` и т.д.) подключают их к блокам и хукам.
- **Запуск без входа Microsoft.** Клиент Horizon сам умеет офлайн-игру (`--name`, `HORIZON_PLAYER_NAME`, `horizon-player.txt`). Никакой авторизации мы не обходим и обходить не должны. Ванильные ресурсы берутся из папки `horizon-client/26.3`, которую готовит собственный Horizon Launcher пользователя.

## 5. Как работать (команды)

Всё на PowerShell 7 (`pwsh`), кроме `Play-Create.cmd`/`Install-Homestead.*`, которые запускает Windows PowerShell 5.

```
pwsh ST/build.ps1                  # контент + apply + engine-patches + cargo fast, ~11-13 мин; одна копия за раз
pwsh ST/Prepare-Candidate.ps1      # копирует собранный клиент в stage4/candidate (ОБЯЗАТЕЛЬНО после build)
pwsh ST/Run-Acceptance.ps1 -<Набор>   # один набор приёмки (-Arm -Link -Tank -Pipes -Cannon -Motor -DataMods ...)
pwsh ST/regress.ps1                # все наборы подряд, ~1 час; квитанции run-<имя>-result.json с binary_sha256
pwsh ST/Build-Package.ps1          # пакет outputs/homestead-horizon-stageNN; требует квитанций ВСЕХ наборов на ОДНОМ бинарнике
```

Ошибки компиляции: `ST/game-build.log` (искать `^error`). Тестовый мир: `ST/runtime/game/saves/HomesteadAutomationAcceptance`; если в нём остался мёртвый игрок, следующие наборы падают («the dead screen is open»), решение: переименовать папку сохранения.

Правила, выученные кровью:
1. **Сборки и запуски игры строго по одной.** Две копии клиента портят кэш контента. Закрытие игры посреди сборки кэша начинает его заново (кэш ~1 ГБ, 3-5 минут).
2. `build.ps1` не обновляет `candidate/`: после него всегда `Prepare-Candidate.ps1`.
3. Пакет проверяет, что все квитанции относятся к бинарнику кандидата. Любой красный набор блокирует пакет.
4. Тики идут медленнее при 48 модах: тесты не должны полагаться на секундные окна, используйте пошаговый `tick freeze`/`tick step`.
5. Блок-сущность во время тика не лежит в карте мира, используйте аргумент `entity`. `sounds` в описании блока должен быть id блока. Шаблонные свойства состояния нельзя объявлять заново.
6. `setblock ... destroy` не вызывает хук ломания (только игрок). Меню модов открывает шаблон `barrel`, стекло нет.
7. Скрипты, которые читает Windows PowerShell 5, должны быть ASCII или UTF-8 с BOM. Русский текст без BOM превращается в кракозябры и ломает разбор.
8. Файлы CRLF/LF: при правках markdown-таблиц регулярками учитывайте CRLF.
9. Тесты, управляемые через сокет, не умеют приседать, респавнить игрока и видят не все сущности (рамки). Такие сценарии помечайте как «не проверено», а не как успех.

## 6. Как добавлять новые срезы (рабочий шаблон)

1. Субагенты/авторы пишут **только новые файлы** (`*_runtime.rs`, вспомогательные хук-файлы, `tools/*_content.py`, `docs/*.md`, `acceptance/Test-*.ps1`) и **фрагмент** `fragments/<срез>.toml` + `.md` с точными правками общих файлов. Общие файлы (`horizon-mixin.toml`, `content.py`, `catalog.py`, `Run-Acceptance.ps1`, `regress.ps1`, `Build-Package.ps1`) не трогают.
2. Один агент интеграции применяет фрагменты, собирает один бинарник, чинит ошибки компиляции, прогоняет новые наборы, затем полный regress, затем пакет и коммит.
3. Параллелятся только авторство и чтение. Сборки и запуски игры строго последовательны.
4. Всё, что не реализуется имеющимися хуками, не делается «как-нибудь»: срез пропускается, причина и нужный хук записываются в документ.

## 7. Что сделано (кратко, по этапам)

| Этапы | Что |
| --- | --- |
| 2-7 | Create: Spout, Item Drain, насосы, Belt Funnel, Item Vault; `homestead`: Right Click Harvest, Secure Crops, Too Cheap!, Preferred Gamerules, Crops Love Rain, Better Days |
| 8 | Farmer's Delight: 109 блоков, 91 предмет, еда, эффекты, ножи, nourishment |
| 9 | Create: 350 декоративных блоков; патч коллизии лесов |
| 10 | Mechanical Arm и 14 типов точек (позже ещё Packager) |
| 11 | Redstone Link |
| 12-14 | Fluid Tank (мультиблок), сеть труб, котёл, паровой двигатель, свисток |
| 15-18 | Farmer's Delight: разделочная доска, печь, сковорода, котёл с экраном |
| 19-22 | Create: выключатели и Nixie, Display Link/Board, Schematicannon/Table/Checklist, Packager и Factory Gauge (stage14) |
| 23-25 | Create: Linked Controller, инструменты схем (Schematic, Schematic and Quill), Stock Link/Ticker, Frogport/Postbox (stage15) |
| 26-27 | Farmer's Delight: посевы, богатая почва, компост; пиры, пироги, Comfort, напитки (stage16) |
| stage17 | 45 «данных» модов как `.hmod`-оболочки, приёмка `-DataMods` |
| stage18 | Исправления по игровой проверке: импорт оригинальных ресурсов Create, творческий мотор (допуск пакета с доски), вкладки креатива для модов, пресс, устойчивый `-Switches`, набор `-Motor` |

Подробности по каждому срезу: `CS/docs/*.md` и `ST/research/roadmap.md`.

## 8. Что не сделано и известные проблемы

- **Create:** поезда (ядра есть, не подключены, отложено владельцем), копикэты, createdeco, предпросмотр схем, экран Stock Ticker, анимация Display Board, цепные конвейеры для склада, Lectern Controller, правый клик контроллером в воздухе. Много рецептов и форм оценены, не сверены с Java. Ядра писались по пересказу исходников. Выживание и внешний вид почти не проверены.
- **Пресс** клипается сквозь ленту: в stage18 исправлено заменой модели головки, визуально не подтверждено.
- **Не перенесены (нужны хуки движка):** Grind Enchantments (меню наждака), Trade Cycling (торговля), Bridging Mod (клиентский луч), Comforts (хуки сна только для LAN-гостя), SimpleHats (слоты Accessories).
- **Farmer's Delight:** колонии грибов, дикие культуры в генерации, корзины, шкафы, циновки, канатные заборы и др.; зависимые моды (nethersdelight, oceansdelight, ...) не сделаны.
- **Не начато:** machines-уровень (sophisticatedstorage, toms_storage, lootr, ...), deep (сущности, рендер), клиентские моды.
- Мёртвый код `mod pump` в `basin.rs`, красный «Too Expensive!» на наковальне, незакрытые долги: список в `ST/research/roadmap.md`.

## 9. Публикация, лицензии, приватность (ВАЖНО)

- Лицензии модов: у многих «All Rights Reserved». **Готовые пакеты, `.hmod`, JAR, `.exe`, текстуры и всё извлечённое из чужих JAR в публичный репозиторий не кладутся.** Код Create и Farmer's Delight под MIT (заголовки сохранять). Оригинальные ресурсы Create идут по отдельной лицензии. Подробности и таблица лицензий: `ST/research/publish-audit.md`.
- Публичная копия репозитория (приватный GitHub-репозиторий владельца) собирается отдельно: экспорт закоммиченного HEAD по белому списку, чистка личных данных, одна нейтральная идентичность автора, свои `README.md`, `NOTICE.md`, `THIRD_PARTY.md`, `LICENSE`, `INSTALL.md`. Историю основного репозитория не переписывать.
- **Никаких личных данных** в файлах, скриптах, логах, пакетах: имена пользователей Windows, абсолютные пути с ними, адреса почты, ники аккаунтов. Пути берутся из `local-settings.json` (в `.gitignore`), образец `local-settings.example.json`.
- Релизные архивы собираются из чистой копии без `private/`, `local-settings.json` и логов с путями; перед отправкой сканируются на личные данные.
- Отправка наружу (push, релизы) только по явной просьбе владельца. Токены GitHub не запрашивать и не использовать; push идёт через сохранённые у владельца данные Git.

## 10. Установка и запуск для пользователей

- `Install-Homestead.cmd` (в корне публичного репозитория и в пакете): спрашивает путь к нашей сборке (папка или `.zip`) и к Homestead из CurseForge, сам находит ресурсы Horizon 26.3, проверяет файл Create (хэш), пишет `local-settings.json`, предлагает ник (`playerName`) и запуск. Архив нужно распаковать целиком, из открытого `.zip` не запускается.
- `Start-Homestead.ps1` / `Play-Create.cmd`: запуск обзорного мира в творческом режиме. Ник: `-Name`, `playerName` в настройках, по умолчанию `HomesteadTest`. Первый запуск долгий (3-5 минут, строится кэш), вторую копию игры скрипт не пускает.
- Сборка из исходников описана в `INSTALL.md`, путь Б, и на чистой машине целиком не проверялась.

## 11. Соглашения

- Коммиты и документы на русском (код и идентификаторы на английском). В коммитах агентов Claude стоит строка `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`; другим агентам достаточно своей атрибуции.
- Каждый срез: документ `docs/*.md` с тремя блоками «сделано / отличается от оригинала / не проверено», набор приёмки с префиксом `stageNN-<имя>-`, запись в `research/roadmap.md`.
- Честность в отчётах: если тест не проверен, красный или нестабилен, это пишется прямо.
