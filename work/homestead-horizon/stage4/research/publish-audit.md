# Аудит перед публикацией на GitHub

Дата: 2026-11-13. Только чтение: ничего не удалено, история не менялась, ничего не отправлялось.
Это инженерный разбор, не юридическая консультация. Пометки: **ФАКТ** (есть файл-доказательство), **НЕИЗВЕСТНО** (не проверено или не удалось проверить).

## 0. Главное

1. **ФАКТ: в репозитории сейчас отслеживаются 46 файлов `.hmod` (51,2 МБ)** в `work/homestead-horizon/stage4/extra-mods/`. Это упакованные чужие моды (текстуры, модели, структуры, рецепты). Они есть в истории в трёх коммитах: `1d85d0e`, `ab56c63`, `38935a7`. Из них 45 штук по лицензии нельзя распространять или это неизвестно (см. п. 2). Это главный блокер публикации.
2. **ФАКТ: в `work/homestead-horizon/stage4/reference/` лежат 7 исходников Java из Create** (`AbstractFunnelBlock.java`, `AllBlocks.java` и др.), без заголовка об авторстве. Код Create под MIT, но MIT требует сохранить уведомление об авторских правах. Без заголовка и без файла лицензии рядом это нарушение условий.
3. **ФАКТ: в истории git записан адрес электронной почты автора** (все 50 коммитов, один автор). Если репозиторий публичный, адрес станет виден. Это приватность, не авторское право.
4. **ФАКТ: исходники Create и ресурсы в репозитории в остальном чистые по политике проекта.** Все текстуры из `content/assets/create/textures/*/fallback/` (213 PNG) по документам проекта нарисованы независимо (`docs/fallback-textures.md`), оригиналы Create в репозиторий не кладутся (`AGENTS.md`: «Never commit or distribute original assets/JARs»). Но происхождение двух моделей `.obj` у водяных колёс **НЕИЗВЕСТНО** (п. 1.4).
5. Размер истории небольшой: 50 коммитов, ~61 МБ (8,6 МБ упаковано + 52,7 МБ свободных объектов). Переписать историю дёшево.

## 1. Что отслеживает git

Всего отслеживается **7356 файлов**. Ветка одна: `main`. Удалённых репозиториев нет (`git remote -v` пусто).

### 1.1 По каталогам

| Каталог | Файлов | Что это |
| --- | --- | --- |
| `work/homestead-horizon/create-source/create-horizon-…/` | 6216 | Скопированное дерево проекта create-horizon (MIT, автор Delete228 + The Create Team) с нашими добавлениями |
| `work/homestead-horizon/stage4/` | 141 | Наши скрипты, приёмка, исследования, патчи движка |
| `outputs/homestead-horizon-stageN/{validation,acceptance,source}` и метаданные | ~1100 | Наши квитанции, логи, скрипты приёмки |
| `outputs/horizon-fps/` | 14 | Отдельный проект в этом же репозитории |

### 1.2 По типам

`.json` 5695, `.rs` 461, `.ps1` 348, `.py` 332, `.png` 214, `.md` 111, **`.hmod` 46**, `.java` 21, `.jpg` 6, `.obj` 3, `.gif` 2.

### 1.3 Сторонний контент, найденный в треке

| Что | Где | Кол-во | Оценка |
| --- | --- | --- | --- |
| Упакованные чужие моды | `work/homestead-horizon/stage4/extra-mods/*.hmod` | 46, 51,2 МБ | **Убрать из треки и из истории.** Самый большой файл: `chipped-3.0.7-horizon.2.hmod` 17,3 МБ |
| Исходники Create (Java, MIT) без заголовка | `work/homestead-horizon/stage4/reference/*.java` | 7 | Либо убрать, либо добавить заголовок и `LICENSE` Create |
| Инструменты Java проекта create-horizon | `create-source/…/tools/java/*.java` | 14 | Из upstream, их заголовки не проверены (3 без заголовка) |
| Fallback-текстуры | `content/assets/create/textures/{block,item}/fallback/*.png` | 213 | По документам проекта независимые (сгенерированы ImageGen по описанию палитры Create 6.0.10). Оставить, пометить в NOTICE |
| Модели `.obj`, `.mtl` | `content/assets/create/models/block/{water_wheel,large_water_wheel}/` | 5 | **НЕИЗВЕСТНО**, чьи это меши (первый коммит `099fa52`, «Initial import»). `fallback-textures.md` говорит «wheel meshes use separate beam grain…», то есть свои. Подтвердить автору |
| Скриншоты игры | `create-source/…/docs/screenshots/*.jpg,gif` | 8 | Игровые кадры upstream-проекта, показывают встроенные текстуры. Низкий риск |
| Данные Create (рецепты, теги) | `create-source/…/content/data/create/` | 2182 json | По `THIRD_PARTY_NOTICES.md`: «Selected MIT datapack recipes, behavior/ingredient tags… exported from `src/generated/resources/data/`». MIT, уведомление уже есть |
| Модели, blockstates, lang | `create-source/…/content/assets/create/{models,blockstates,items,lang}` | 2800 json | По документам проекта свои («our provisional JSON models… refers to vanilla textures by ID»). Генерируются `content.py`. Подтвердить, что в них нет копий из JAR (выборочная проверка не делалась) |

Тексты Mojang и оригинальные текстуры Create в треке **не найдены** (расширения `.png`/`.ogg` только в каталогах `fallback/` и в скриншотах).

### 1.4 История

- Пути, добавленные хотя бы раз и подходящие под бинарные типы (jar, hmod, zip, ogg, png, jpg, gif, obj, nbt, exe, dll): `fallback/` текстуры 213, `extra-mods` **46**, `docs/screenshots` 9, `.obj` 3. Больше ничего.
- Самые большие объекты в истории: четыре `.hmod` (17,3 / 7,3 / 4,7 / 4,0 МБ) и ~30 версий `mixins/create/data/content.json` (3,5-4,0 МБ каждая, свои сгенерированные данные).
- Коммиты, добавившие `.hmod`: `1d85d0e` (2026-11-10, первый Farmer's Delight), `ab56c63` (2026-11-10), `38935a7` (2026-11-13, stage18).
- `.jar`, `.zip`, `.exe` в истории **не добавлялись**.

## 2. Лицензии модов

Источник: `work/homestead-horizon/pack/inventory.csv` (поле `license`, взято с Modrinth) и `fabric.mod.json` Create (проверен прямо в JAR). **Проверялась метка лицензии, а не текст файлов LICENSE.** Для многих модов метка относится к коду, а лицензия ресурсов (текстуры, звуки, модели) может быть другой. Файлы LICENSE самих модов локально отсутствуют: `pack/jars/` пуста.

### 2.1 Create и Farmer's Delight

| Мод | Метка | Код | Ресурсы |
| --- | --- | --- | --- |
| create (6.0.8.1 в JAR экземпляра, порт закреплён на 6.0.10) | MIT (`fabric.mod.json`) | MIT, подтверждено лицензией проекта create-horizon, `LICENSE` + `THIRD_PARTY_NOTICES.md` | **По `THIRD_PARTY_NOTICES.md` и `docs/assets.md`: «Create's original assets are All Rights Reserved»**, отдельная лицензия. Сам файл LICENSE Create локально не найден, ссылка на upstream. Считать ARR |
| farmersdelight 2.4.1 Refabricated | MIT License (Modrinth) | MIT | **НЕИЗВЕСТНО**: лицензия ресурсов FD не проверена, файла LICENSE нет |
| create_oxidized | MIT License | MIT | НЕИЗВЕСТНО |

### 2.2 Моды, упакованные в `.hmod` (45 штук)

Классификация по метке: **P** = разрешающая, **C** = copyleft, **R** = нельзя распространять, **U** = неизвестно или нестандартная.

| Мод | Метка | Класс |
| --- | --- | --- |
| alcocraftplus, darksmelting, echoes_of_the_end__structures_, mcwholidays, mcwpaintings, natures_delight, geophilic, regions_unexplored | All Rights Reserved / ARR | **R** |
| mr_flower_tweaks, mr_mastercutter, mr_tidal_towns | LicenseRef-All-Rights-Reserved | **R** |
| mr_typewriter_daycounter | LicenseRef-Custom | U |
| villagesandpillages | CC BY-NC-ND 4.0 | **R** (ND запрещает производные, в том числе в виде `.hmod`) |
| mr_endercon, t_and_t | CC BY-NC-SA 4.0 | C/NC: нельзя для коммерческого использования, производные только под той же лицензией |
| betterendrod, froglightsreimagined, perfectplushies | CC0-1.0 | P |
| blahaj | Unlicense | P |
| beautify, dustydecorations, dyedend, fairylights, justoutdoorstuffs, letsdoaddonstructures, mcwpaths, modrinth_logo_mod, ends_delight | MIT / MIT License | P (код); ресурсы НЕИЗВЕСТНО |
| diagonalwalls, diagonalwindows, universalbonemeal | MPL-2.0 | C (файловый copyleft) |
| repurposed_structures, connectiblechains, betterdungeons, betterendisland, betterfortresses, bettermineshafts, betterstrongholds, yungsextras | LGPL-3.0 / LGPLv3 | C |
| luphieclutteredmod | Academic Free License v3.0 | P (с уведомлением) |
| bakery, candlelight | Custom | U |
| chipped | Terrarium Licence | U |
| incendium | Stardust Labs License | U |
| suppsquared | Supplementaries Team License v1.1 | U |

Итого: 12 штук с меткой ARR или ND, 2 CC BY-NC-SA, 10 разрешающих, 10 с LGPL, 3 MPL, 1 AFL, 5 нестандартных, 3 «CC0/Unlicense», остальное в таблице. **Для публикации годятся только CC0, Unlicense и, с оговорками, MIT-метки, но для публикации самих `.hmod` ресурсы каждого мода нужно проверять отдельно.**

### 2.3 Моды логики (наши срезы, `mixins/homestead`)

Наш код написан «по поведению мода». Как именно агенты получали описание поведения, не проверялось, поэтому **НЕИЗВЕСТНО**, не содержит ли наш код переписанных кусков чужого.

| Мод | Метка | Риск |
| --- | --- | --- |
| stackrefill, doubledoors, noanimaltemptdelay, shulkerdropstwo, trade_cycling | All rights reserved | **Высокий**: переписывание поведения допустимо, копирование кода или структуры нет. Нужна проверка, что код написан без чтения исходников |
| fallingtrees (GPL-3), accelerateddecay (GPL3) | GPL | Если наш код производный, он должен быть под GPL |
| comforts (LGPL-3.0-or-later), universal-graves (LGPLv3), grindenchantments, betterdays (LGPL-3.0-or-later) | LGPL | Умеренный |
| bridgingmod, craftingpad, invisibleframes, crops_love_rain, rightclickharvest, securecrops, too-cheap, preferred-gamerules | MIT | Низкий (нужно уведомление, если код портирован) |
| spawnfilter | CC0-1.0 | Нет |
| simplehats | «Code: MIT, Assets: ARR» | Код можно, ресурсы нет |

## 3. Лицензии проектов, из которых мы собираем

| Проект | Лицензия | Что требуется | Доказательство |
| --- | --- | --- | --- |
| Horizon (клиент), `work/horizon-source/horizon-5f7cad…/LICENSE` | MIT, © 2026 Ludovic Marie и Horizon contributors | Включить уведомление в любой пакет с бинарником | ФАКТ |
| horizon-mixins, `work/mixins-source/horizon-mixins-5a64af…/LICENSE` | MIT, © 2026 levzzz | То же | ФАКТ |
| create-horizon (дерево `create-source`), `LICENSE` | MIT, © The Create Team + create-horizon contributors | Сохранить `LICENSE`, `THIRD_PARTY_NOTICES.md` | ФАКТ |
| llvm-mingw, `LICENSE.TXT` | Apache-2.0 с LLVM-исключением; включает `busybox` (GPL-2.0) и `python` (PSF) | Только если распространять инструменты. В репозиторий они не входят | ФАКТ, текст не вычитан |
| mingit, `LICENSE.txt` | GPL-2.0 (Git) и др. | Только если распространять | ФАКТ, текст не вычитан |
| Rust toolchain | MIT / Apache-2.0 | Только если распространять | НЕИЗВЕСТНО |
| Крейты Rust внутри `horizon-client.exe` | разные (MIT, Apache-2.0, MPL и др.) | Если распространять бинарник, нужны их уведомления (`cargo about` или аналог) | **НЕИЗВЕСТНО**, не генерировалось |

Пакеты `outputs/homestead-horizon-stage*` содержат `client/horizon-client.exe` и `.hmod`: **распространять их публично нельзя, пока не решены лицензии модов и уведомления крейтов**. В git они не отслеживаются (бинарники и `.hmod` внутри пакетов игнорируются), кроме метаданных.

## 4. Заголовки в собственных файлах

Проверялись первые 8 строк на `Copyright` или `SPDX`.

| Тип | Всего | Без заголовка |
| --- | --- | --- |
| `.rs` | 461 | 34 (13 в `examples/`, 7 в `tools/rust/`, 3 в `mixins/homestead/`, остальные в `outputs/horizon-fps`, `stage3`, `vault-hook-probe`) |
| `.py` | 332 | 165 (159 в `create-source/…/tools/`) |
| `.ps1` | 348 | 83 |
| `.java` | 21 | 10 (7 в `stage4/reference/` — **копии из Create без уведомления**, 3 в `tools/java/`) |

Основные модули порта Create (`mixins/create/*.rs`) заголовки имеют (`// Copyright (c) The Create Team… SPDX: MIT`). Без заголовка остаются 3 файла `mixins/homestead/*.rs` и часть инструментов. Это не нарушение для собственного кода, но для файлов, **адаптированных из MIT-источников**, уведомление обязательно.

## 5. План публикации

### 5.1 Что публиковать

Один репозиторий с белым списком `.gitignore`:

```gitignore
/*
!/README.md
!/LICENSE
!/NOTICE.md
!/THIRD_PARTY.md
!/.gitignore
!/.gitattributes
!/docs/
!/mixins/              # наши миксины create и homestead (без data/content.json если он производный)
!/tools/               # content.py, catalog.py, build-скрипты
!/acceptance/          # Test-*.ps1
!/stage4-scripts/      # Build-*.ps1, Run-Acceptance.ps1, regress.ps1 (без extra-mods, receipts можно)
!/research/            # roadmap.md и т.п.
# запретить явно, на случай ошибок:
*.jar
*.hmod
*.hnmod
*.zip
*.exe
*.dll
*.ogg
extra-mods/
private/
.horizon-cache/
pack/
carry/
*/fallback/*.png       # только если подтвердим происхождение, иначе оставить как свои
```

Идеальнее опубликовать **как форк upstream `Delete228/create-horizon`**, а не как копию внутри `work/homestead-horizon/create-source`: тогда и лицензия, и история авторов сохраняются, а наши модули живут в `mixins/homestead` и `fd`, `stage4` в отдельном репозитории или подкаталоге.

### 5.2 Что вычистить из истории

Надо удалить (`git filter-repo --path … --invert-paths`) и проверить, что после этого нет объектов:

1. `work/homestead-horizon/stage4/extra-mods/` целиком (46 `.hmod`).
2. `work/homestead-horizon/stage4/reference/*.java` (7 файлов Create) или вернуть их с заголовком и `LICENSE`.
3. Адрес электронной почты в авторстве коммитов: переписать (`git filter-repo --mailmap`), если репозиторий публичный.
4. Проверить после очистки: `git rev-list --objects --all | git cat-file --batch-check` на отсутствие `.hmod`/`.jar`, и `git grep -I -l "<личный идентификатор владельца>"`.
5. Необязательно: старые версии `content.json` (3,5-4,0 МБ, ~30 штук) раздувают историю, но проблем авторского права не создают.

Репозиторий пока локальный и без удалённого, поэтому перепись безопасна (сделать резервную копию `.git` до этого).

### 5.3 Схема файлов `NOTICE.md` / `THIRD_PARTY.md`

1. **Проект:** название, лицензия нашего кода (предложение: MIT, как у upstream), автор.
2. **Upstream:** Create (MIT, © The Create Team / The Creators of Create, коммит `ac0c444d…`, версия 6.0.10), create-horizon (MIT, Delete228), Horizon (MIT, Ludovic Marie), horizon-mixins (MIT, levzzz). Полный текст лицензии каждого.
3. **Моды:** таблица из п. 2 (имя, версия, лицензия, ссылка на страницу Modrinth, «не включён, ставится пользователем»).
4. **Ресурсы:** «Оригинальные ресурсы Create распространяются по отдельной лицензии (All Rights Reserved) и в репозиторий не входят. Встроенные текстуры нарисованы независимо».
5. **Крейты:** список зависимостей Rust с лицензиями (`cargo about generate`).
6. **Торговые знаки:** «Minecraft — торговая марка Mojang/Microsoft. Проект не связан с Mojang и авторами модов».

### 5.4 Формулировка для README

> Этот репозиторий содержит только исходный код и документацию. Он не содержит модов, текстур, моделей и звуков других авторов. Чтобы собрать игру, установите нужные моды сами (ссылки в `THIRD_PARTY.md`) и укажите путь к вашей папке с ними в `local-settings.json`. Скрипты соберут нужные файлы `.hmod` из ваших JAR локально и положат их в игнорируемую папку `local/`. Не публикуйте полученные файлы и пакеты.

### 5.5 Изменения в скриптах (чтобы пользователь собирал из своих JAR)

Факты о текущем состоянии:
- `stage4/Build-FarmersDelight.ps1` уже читает путь к ресурсам из `outputs/homestead-horizon-stage7/local-settings.json` и вызывает `horizon-port-pack carry … farmersdelight` из JAR игрока.
- `stage4/Build-DataMods.ps1` берёт **уже подготовленные** данные из `work/homestead-horizon/pack/mods/<мод>/carry` (папка локальная, не в git) и пишет `.hmod` в `stage4/extra-mods`. Входных JAR в `pack/jars` сейчас нет.
- Импорт ресурсов Create уже сделан по принципу «пользователь кладёт JAR» (`horizon-mods/create-resources/`, проверка SHA-256).

Что нужно сделать:
1. Единый скрипт `Build-LocalMods.ps1`: читает `local-settings.json` (`modsDir`, `createJar`), для каждого мода из списка поддерживаемых находит JAR, проверяет версию и SHA-256 по таблице в репозитории, запускает `horizon-port-pack carry` и пишет `.hmod` в `local/mods/`. Моды, у которых лицензия ARR/ND, не разбирать из чужого пакета, если пользователь не поставил JAR сам.
2. Перенести `extra-mods/` и результат `Build-DataMods.ps1` в `local/` и добавить его в `.gitignore`. Исправить путь в `Start-Test.ps1`, который подгружает все `.hmod` из `stage4/extra-mods`.
3. Заменить жёсткий путь `outputs/homestead-horizon-stage7/local-settings.json` на `local-settings.json` в корне.
4. Добавить `Build-Package.ps1` режим `-NoThirdParty` (по умолчанию в публичной сборке): пакет без `mods/*.hmod` и без `horizon-client.exe`, только скрипты запуска и инструкция.
5. Таблица поддерживаемых версий модов (`mods-lock.json`: имя, версия, SHA-256, лицензия, URL страницы) вместо данных `pack/`.

## 6. Что неизвестно и надо проверить до публикации

1. Лицензия ресурсов Farmer's Delight (код MIT; ресурсы не проверены) и всех 10 «MIT»-модов в `.hmod`: нужны LICENSE-файлы из их JAR.
2. Происхождение `.obj` колёс и содержимого `content/assets/create/models` (не копии ли из JAR Create).
3. Как писался код модулей по модам с лицензией ARR и GPL (читали ли исходники).
4. Лицензии 5 нестандартных (bakery, candlelight, chipped, incendium, suppsquared) и mr_typewriter_daycounter.
5. Список зависимостей Rust и их лицензии в `horizon-client.exe` (если распространять бинарник).
6. Полный текст LICENSE Create (assets): сверить с upstream, локальной копии нет.
7. Нужно ли указывать upstream (Delete228) как источник и согласовать форк.
