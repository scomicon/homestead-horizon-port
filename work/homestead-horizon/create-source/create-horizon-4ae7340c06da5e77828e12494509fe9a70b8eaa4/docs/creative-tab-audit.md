# Аудит вкладок креатива

## Сделано

Статический аудит текущих runtime `.hmod`, 2026-11-16. Архивы распакованы во временную папку вне репозитория и удалены после чтения. Сборка и игра не запускались. Исходные архивы проверены SHA-256 до и после аудита.

Архивов: **62**; вкладок: **34**; записей: **10340**. Кандидатов на пустую иконку: **304**. Нарушений политик: **0**.

Это диагностика ресурсов, а не подтверждение игрового рендера. Отсутствующая текстура может дать missing-texture, а не пустую ячейку; настенный дубликат может иметь исправную иконку. Категории пересекаются.

Ресурсы Minecraft разрешались по локальному подготовленному снимку `runtime-01/resources`; доступных дополнительных корней: 1. Ссылки между `.hmod` проверяются по общему индексу. Конфликтующих путей assets с разным содержимым: 0. Общие data-теги и отдельные манифесты архивов не считаются конфликтами ресурсов.

| Архив | Вкладки | Записи | Нет items JSON | Нет модели/родителя | Нет текстуры | Пустая геометрия | builtin/entity без геометрии | Настенная пара | Скрыто политикой | Кандидаты пустых |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alcocraftplus-2.1.1-horizon.3.hmod | 1 | 16 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| bakery-2.0.6-horizon.3.hmod | 1 | 66 | 7 | 0 | 7 | 0 | 0 | 1 | 0 | 7 |
| beautify-3.5.5-horizon.3.hmod | 1 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterdungeons-1.20_Fabric_4.0.4-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterendisland-1.20_Fabric_2.0.6-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterendrod-1.1.0-horizon.3.hmod | 1 | 9 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterfortresses-1.20_Fabric_2.0.6-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| bettermineshafts-1.20_Fabric_4.0.4-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterstrongholds-1.20_Fabric_4.0.3-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| blahaj-0.3.2-horizon.3.hmod | 1 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| candlelight-2.0.5-horizon.3.hmod | 1 | 123 | 2 | 0 | 4 | 0 | 0 | 0 | 0 | 2 |
| chipped-3.0.7-horizon.3.hmod | 1 | 6942 | 0 | 6 | 0 | 0 | 0 | 0 | 0 | 6 |
| comforts-0.1.0-homestead.1.hmod | 1 | 32 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| connectiblechains-2.5.5_1.20.1-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| cratedelight-26.07.01_1.20_fabric-batch2.1.hmod | 1 | 27 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| create_oxidized-0.1.1-horizon.1.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| createdeco-2.1.1_1.20.1_fabric-batch2.1.hmod | 1 | 326 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| darksmelting-1.0.6-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| diagonalwalls-8.0.4-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| diagonalwindows-8.1.5-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| dustydecorations-1.1_1.20.1_1.20.2-horizon.3.hmod | 1 | 79 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| dyedend-1.20.1_1.0.0-horizon.3.hmod | 1 | 64 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| echoes_of_the_end__structures_-12.00.13-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| ends_delight-refabricated_1.20.1_2.5-horizon.3.hmod | 1 | 60 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 |
| expandeddelight-0.3.2-batch1.1.hmod | 1 | 59 | 6 | 0 | 1 | 0 | 0 | 0 | 0 | 6 |
| fairylights-1.1.5_fabric-horizon.3.hmod | 1 | 31 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| farmersdelight-2.4.1-horizon.10.hmod | 1 | 215 | 39 | 0 | 0 | 1 | 0 | 18 | 0 | 40 |
| farmersknives-3.2-batch1.1.hmod | 1 | 9 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| froglightsreimagined-1.0-horizon.3.hmod | 1 | 48 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| geophilic-3.7-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| homestead_sound-0.1.0-homestead.1.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| incendium-5.3.5-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| justoutdoorstuffs-1.0.2-horizon.3.hmod | 1 | 52 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| letsdoaddonstructures-1.7.2-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| luphieclutteredmod-2.1.0_1.20.1-horizon.3.hmod | 1 | 509 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mcaccel-1.0.0-batch2.1.hmod | 1 | 28 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mcwholidays-1.1.2-horizon.3.hmod | 1 | 309 | 0 | 0 | 0 | 0 | 0 | 15 | 0 | 0 |
| mcwpaintings-1.1.0-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mcwpaths-1.1.2-horizon.3.hmod | 1 | 258 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mo_glass-1.7_MC1.20.1-batch1.1.hmod | 1 | 36 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| modrinth_logo_mod-1.3.4-horizon.3.hmod | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mr_endercon-1.0.2-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mr_flower_tweaks-1-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mr_mastercutter-1.1-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mr_tidal_towns-2.0-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mr_typewriter_daycounter-1.0-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| natures_delight-1.12_1.20.1-horizon.3.hmod | 1 | 28 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| nethersdelight-1.20.1_4.1.1-batch1.1.hmod | 1 | 23 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| obsidianequipmentrework-1.1-batch2.1.hmod | 1 | 14 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| oceansdelight-fdrf_fabric_1.0.2_1.20-batch2.1.hmod | 1 | 22 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| perfectplushies-1.13.3-horizon.3.hmod | 1 | 48 | 0 | 0 | 1 | 0 | 15 | 0 | 0 | 15 |
| phomesteadplushies-1.1.0-batch1.1.hmod | 1 | 53 | 0 | 50 | 3 | 0 | 3 | 0 | 0 | 53 |
| regions_unexplored-0.5.6_1.20.1-horizon.3.hmod | 1 | 814 | 152 | 0 | 7 | 0 | 0 | 44 | 0 | 152 |
| repurposed_structures-7.1.25_1.20.1_fabric-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| sootychimneys-1.3.4-batch2.1.hmod | 1 | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| spuds_shops-1.9.0-batch1.1.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| suppsquared-1.20_1.1.29-horizon.3.hmod | 1 | 27 | 0 | 19 | 5 | 0 | 0 | 0 | 0 | 19 |
| t_and_t-1.10.2-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| universal_graves-3.0.3-homestead.1.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| universalbonemeal-8.0.1-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| villagesandpillages-2.0.0-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| yungsextras-1.20_Fabric_4.0.3-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

## Правила для creative_tab.py

1. До выбора `icon` отфильтровать записи, у которых нет `assets/<ns>/items/<id>.json`. Не добавлять автоматически все объявления блоков.
2. Рекурсивно проверить item-модель и родителей по общему набору ресурсов (включая Minecraft). Объединять текстуры родителей с переопределениями потомков, разрешать `#alias`, обнаруживать циклы. Само наличие JSON и `particle` не означает наличие иконки. Для обычной модели нужны faces, для `builtin/generated` — `layerN`; `builtin/entity` без геометрии требует настоящего special renderer или замены модели.
3. Исключать настенный ID только при наличии объявленного парного ID в том же namespace: `wall_X → X`, `X_wall_sign → X_sign`, `X_wall_hanging_sign → X_hanging_sign`, `X_wall_deco` / `X_wall_deco_N → X`. Не удалять все имена с `wall`: многие самостоятельные декорации имеют рабочие иконки и не имеют пары.
4. Применять `unplaceable_items.json` (`fnmatchcase` по имени), явные `drop_items` из всех `batch*-policy.json` и точные удаления из отчётов overlay для `drop_uncraftable_items`. Не считать каждый предмет без рецепта скрытым. Генерировать вкладки после overlay, проверять финальный `.hmod`.
5. Выбирать `icon` только из оставшихся проверенных записей; если их нет — не создавать вкладку. Удалённые из вкладки ID сохранять зарегистрированными для совместимости миров, если отдельная политика не требует удаления регистрации.
6. Не скрывать автоматически custom-loader/special-модели: отмечать их отдельно для проверки поддерживаемого рендера. Недостающие текстуры лучше исправлять, а не выдавать за доказанный пустой слот.

## Особое внимание

- `regions_unexplored`: записей 814, кандидатов пустых 152, настенных пар 44, нарушений политик 0; объединение рекомендуемого фильтра 152.
- `perfectplushies`: записей 48, кандидатов пустых 15, настенных пар 0, нарушений политик 0; объединение рекомендуемого фильтра 15.
- `phomesteadplushies`: записей 53, кандидатов пустых 53, настенных пар 0, нарушений политик 0; объединение рекомендуемого фильтра 53.
- `mcwholidays`: записей 309, кандидатов пустых 0, настенных пар 15, нарушений политик 0; объединение рекомендуемого фильтра 15.

## Полные списки по модам

`missing_item` — нет нового items JSON; `missing_model` — отсутствует модель или родитель; `missing_texture` — отсутствует PNG; `empty_geometry` — нет faces и генерируемых слоёв; `entity_without_geometry` — builtin/entity без собственной геометрии; `wall_pair` — найден парный не-настенный ID; `policy_hidden` — нарушение политики. Прочие типы/ветки/рендереры и ошибки выводятся явно. Все ID с проблемами перечислены без усечения; ID без замечаний не перечисляются.

### alcocraftplus-2.1.1-horizon.3.hmod

SHA-256: `5e4033df3ef7cecdc759223a03af46f21769490858a727d3e5d0f7a3549a1264`.

Замечаний к записям нет.


### bakery-2.0.6-horizon.3.hmod

SHA-256: `778dee734c9ee93e9ae3733fa0dcd770198c936dfe5d0dc77c96614021ec1286`.

- `bakery:cake_tower` — missing_texture: assets/bakery/textures/item/cake_tower.png.
- `bakery:chocolate_bread` — missing_texture: assets/bakery/textures/item/chocolate_bread.png.
- `bakery:donut` — missing_texture: assets/bakery/textures/item/donut.png.
- `bakery:faucet` — missing_texture: assets/bakery/textures/item/faucet.png.
- `bakery:jam_sandwich` — missing_texture: assets/bakery/textures/item/jam_sandwich.png.
- `bakery:pancake` — missing_texture: assets/bakery/textures/item/food/pancake.png.
- `bakery:shelf` — missing_texture: assets/bakery/textures/item/shelf.png.
- `bakery:apple_cupcake_block` — missing_item: assets/bakery/items/apple_cupcake_block.json.
- `bakery:bakery_wall_banner` — missing_item: assets/bakery/items/bakery_wall_banner.json.
- `bakery:chocolate_cookie_block` — missing_item: assets/bakery/items/chocolate_cookie_block.json.
- `bakery:strawberry_cookie_block` — missing_item: assets/bakery/items/strawberry_cookie_block.json.
- `bakery:strawberry_cupcake_block` — missing_item: assets/bakery/items/strawberry_cupcake_block.json.
- `bakery:sweetberry_cookie_block` — missing_item: assets/bakery/items/sweetberry_cookie_block.json.
- `bakery:sweetberry_cupcake_block` — missing_item: assets/bakery/items/sweetberry_cupcake_block.json.
- `bakery:wall_cabinet` — wall_pair: bakery:cabinet.

### beautify-3.5.5-horizon.3.hmod

SHA-256: `4a3d12e6e594b26e7ded1717497feedab0f69b358a71556583a520cd95ee8df7`.

Замечаний к записям нет.


### betterdungeons-1.20_Fabric_4.0.4-horizon.3.hmod

SHA-256: `ac6e7bb3b39bbbdb9d6be79d7c9eb025a7608b1b2544bc13dd5a32c16ca60a19`.

Вкладок нет; записей для проверки нет.


### betterendisland-1.20_Fabric_2.0.6-horizon.3.hmod

SHA-256: `0400c360fa7380bb2db1fc68c26e9b525bf15fab3fa7656d598eb14ee4573530`.

Вкладок нет; записей для проверки нет.


### betterendrod-1.1.0-horizon.3.hmod

SHA-256: `d1f5d36d273a1fd94b9d12dd78a823277f40e4e8f793a6b32ada68e71d9184ee`.

Замечаний к записям нет.


### betterfortresses-1.20_Fabric_2.0.6-horizon.3.hmod

SHA-256: `5d99cad3d36bcab7e7c4da34ccfd02e513b244f10d7efcdc8529a6f69243f8cb`.

Вкладок нет; записей для проверки нет.


### bettermineshafts-1.20_Fabric_4.0.4-horizon.3.hmod

SHA-256: `c1748d909f494f4f36fc82a116e454a8a0d3bf02e2bc4132a4bdc9ca91276d42`.

Вкладок нет; записей для проверки нет.


### betterstrongholds-1.20_Fabric_4.0.3-horizon.3.hmod

SHA-256: `8a93c2430aca13a32de74ae9eca1cc8132f09ac27cfa1168ec5398b8fda0fb28`.

Вкладок нет; записей для проверки нет.


### blahaj-0.3.2-horizon.3.hmod

SHA-256: `bc981c9257a683435576cd77e61ccea4b16aa9d85c81af3eadc7584c450a4318`.

Замечаний к записям нет.


### candlelight-2.0.5-horizon.3.hmod

SHA-256: `93dc37e5545a4c14785aa4073334c917cc177d94dadfa3588cc7146cc27f72b0`.

- `candlelight:dough` — missing_texture: assets/candlelight/textures/item/dough.png.
- `candlelight:faucet` — missing_texture: assets/candlelight/textures/item/faucet.png.
- `candlelight:toast` — missing_texture: assets/candlelight/textures/item/toast.png.
- `candlelight:tomato_sauce` — missing_texture: assets/candlelight/textures/item/tomato_sauce.png.
- `candlelight:bamboo_stove` — texture_alias: candlelight:item/bamboo_stove:#missing.
- `candlelight:candlelight_wall_banner` — missing_item: assets/candlelight/items/candlelight_wall_banner.json.
- `candlelight:potted_rose` — missing_item: assets/candlelight/items/potted_rose.json.

### chipped-3.0.7-horizon.3.hmod

SHA-256: `a9aa7d083cf4e33395506f9b5d01354e4701d69c70ed7b5a21ee647519e598ef`.

- `chipped:carved_pumpkin_20` — missing_model: assets/chipped/models/block/carved_pumpkin_20.json.
- `chipped:carved_pumpkin_21` — missing_model: assets/chipped/models/block/carved_pumpkin_21.json.
- `chipped:carved_pumpkin_22` — missing_model: assets/chipped/models/block/carved_pumpkin_22.json.
- `chipped:jack_o_lantern_20` — missing_model: assets/chipped/models/block/jack_o_lantern_20.json.
- `chipped:jack_o_lantern_21` — missing_model: assets/chipped/models/block/jack_o_lantern_21.json.
- `chipped:jack_o_lantern_22` — missing_model: assets/chipped/models/block/jack_o_lantern_22.json.

Значки вкладок (отдельно от числа записей):
- `chipped:carved_pumpkin_20`: missing_model: assets/chipped/models/block/carved_pumpkin_20.json.

### comforts-0.1.0-homestead.1.hmod

SHA-256: `588a94bb42fed1a7ebabc6d77102061051b6b8314864249790f2d1b89137adff`.

Замечаний к записям нет.


### connectiblechains-2.5.5_1.20.1-horizon.3.hmod

SHA-256: `2a6a839b7ae4abfededd28abb07919af8ef8a203504620a9b2315a741810e661`.

Вкладок нет; записей для проверки нет.


### cratedelight-26.07.01_1.20_fabric-batch2.1.hmod

SHA-256: `e0d52fd9c446c89afd5e9217bff526b3c5ff948224dede7c4e3d43bb0f34b278`.

- `cratedelight:blueberry_crate` — missing_model: assets/cratedelight/models/block/blueberry_crate.json.

### create_oxidized-0.1.1-horizon.1.hmod

SHA-256: `b5252f44a7ba4882356d99e435a4bcaec7bb58a7ba71e48b141b982c243a64b5`.

Вкладок нет; записей для проверки нет.


### createdeco-2.1.1_1.20.1_fabric-batch2.1.hmod

SHA-256: `3d86087e69b287dad3284571c15ecfc7da503a41026d443803560d1ec1d7d5b2`.

- `createdeco:pane` — texture_alias: createdeco:item/pane#0; texture_alias: createdeco:item/pane#1; texture_alias: createdeco:item/pane#particle; texture_alias: createdeco:item/pane:#0; texture_alias: createdeco:item/pane:#1.

### darksmelting-1.0.6-horizon.3.hmod

SHA-256: `4e67916898e5a902f7e3a142032fdbce87cdc68c2b76ed96959ed0c50947335c`.

Вкладок нет; записей для проверки нет.


### diagonalwalls-8.0.4-horizon.3.hmod

SHA-256: `84850ac5cce8e673400c150ff1e3b6d6066faad7a40269afb629643555432340`.

Вкладок нет; записей для проверки нет.


### diagonalwindows-8.1.5-horizon.3.hmod

SHA-256: `20a28e039b43bcf97a65e9ffac49edaf38946902f75f36f258bfea0a58950623`.

Вкладок нет; записей для проверки нет.


### dustydecorations-1.1_1.20.1_1.20.2-horizon.3.hmod

SHA-256: `78440e3f148fdbecdfc1042e0e02334a002c9575ec0586e62f581a906b83a5f7`.

Замечаний к записям нет.


### dyedend-1.20.1_1.0.0-horizon.3.hmod

SHA-256: `fd2290e9a9deda038c97788b10c3502d7ae78cb1493f1d5bc8897f4a8ccbf305`.

Замечаний к записям нет.


### echoes_of_the_end__structures_-12.00.13-horizon.3.hmod

SHA-256: `9c044bfddc163b5add7c90436ea948f9b787cb69980537561d98a9274afd47d5`.

Вкладок нет; записей для проверки нет.


### ends_delight-refabricated_1.20.1_2.5-horizon.3.hmod

SHA-256: `b35dc233802a18a85b14d6df04a60a74ccd55f65133321268cb222b97eec41af`.

- `ends_delight:dragon_egg_shell` — missing_texture: assets/ends_delight/textures/item/dragon_egg_shell.png.
- `ends_delight:ender_sauce` — missing_texture: assets/ends_delight/textures/item/ender_sauce.png.
- `ends_delight:liquid_dragon_egg_in_shell` — missing_texture: assets/ends_delight/textures/item/liquid_dragon_egg_in_shell.png.

### expandeddelight-0.3.2-batch1.1.hmod

SHA-256: `b9d87396ceab853c029778744a9b483f61fdaab500d1dfea0b845d634d74375c`.

- `expandeddelight:asparagus_crop` — missing_item: assets/expandeddelight/items/asparagus_crop.json.
- `expandeddelight:chili_pepper_crop` — missing_item: assets/expandeddelight/items/chili_pepper_crop.json.
- `expandeddelight:cinnamon_log` — missing_texture: assets/farmersdelight/textures/block/cinnamon_log_bottom.png.
- `expandeddelight:juicer` — missing_item: assets/expandeddelight/items/juicer.json.
- `expandeddelight:mortar_and_pestle` — missing_item: assets/expandeddelight/items/mortar_and_pestle.json.
- `expandeddelight:peanut_crop` — missing_item: assets/expandeddelight/items/peanut_crop.json.
- `expandeddelight:sweet_potatoes_crop` — missing_item: assets/expandeddelight/items/sweet_potatoes_crop.json.

### fairylights-1.1.5_fabric-horizon.3.hmod

SHA-256: `9c57d573c7bd7bcd90dc125cc9644c0879876675b7a56a02cf78367d4beafe9f`.

- `fairylights:fastener` — missing_item: assets/fairylights/items/fastener.json.

### farmersdelight-2.4.1-horizon.10.hmod

SHA-256: `2ab7037d87fb6b13f6f77dd6e7e3b8bd68926012a4168dd5016eba65904703bc`.

- `farmersdelight:mug` — empty_geometry: farmersdelight:item/mug.
- `farmersdelight:black_canvas_wall_sign` — missing_item: assets/farmersdelight/items/black_canvas_wall_sign.json; wall_pair: farmersdelight:black_canvas_sign.
- `farmersdelight:black_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/black_wall_hanging_canvas_sign.json.
- `farmersdelight:blue_canvas_wall_sign` — missing_item: assets/farmersdelight/items/blue_canvas_wall_sign.json; wall_pair: farmersdelight:blue_canvas_sign.
- `farmersdelight:blue_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/blue_wall_hanging_canvas_sign.json.
- `farmersdelight:brown_canvas_wall_sign` — missing_item: assets/farmersdelight/items/brown_canvas_wall_sign.json; wall_pair: farmersdelight:brown_canvas_sign.
- `farmersdelight:brown_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/brown_wall_hanging_canvas_sign.json.
- `farmersdelight:budding_tomatoes` — missing_item: assets/farmersdelight/items/budding_tomatoes.json.
- `farmersdelight:cabbages` — missing_item: assets/farmersdelight/items/cabbages.json.
- `farmersdelight:canvas_wall_sign` — missing_item: assets/farmersdelight/items/canvas_wall_sign.json; wall_pair: farmersdelight:canvas_sign.
- `farmersdelight:cyan_canvas_wall_sign` — missing_item: assets/farmersdelight/items/cyan_canvas_wall_sign.json; wall_pair: farmersdelight:cyan_canvas_sign.
- `farmersdelight:cyan_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/cyan_wall_hanging_canvas_sign.json.
- `farmersdelight:gray_canvas_wall_sign` — missing_item: assets/farmersdelight/items/gray_canvas_wall_sign.json; wall_pair: farmersdelight:gray_canvas_sign.
- `farmersdelight:gray_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/gray_wall_hanging_canvas_sign.json.
- `farmersdelight:green_canvas_wall_sign` — missing_item: assets/farmersdelight/items/green_canvas_wall_sign.json; wall_pair: farmersdelight:green_canvas_sign.
- `farmersdelight:green_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/green_wall_hanging_canvas_sign.json.
- `farmersdelight:light_blue_canvas_wall_sign` — missing_item: assets/farmersdelight/items/light_blue_canvas_wall_sign.json; wall_pair: farmersdelight:light_blue_canvas_sign.
- `farmersdelight:light_blue_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/light_blue_wall_hanging_canvas_sign.json.
- `farmersdelight:light_gray_canvas_wall_sign` — missing_item: assets/farmersdelight/items/light_gray_canvas_wall_sign.json; wall_pair: farmersdelight:light_gray_canvas_sign.
- `farmersdelight:light_gray_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/light_gray_wall_hanging_canvas_sign.json.
- `farmersdelight:lime_canvas_wall_sign` — missing_item: assets/farmersdelight/items/lime_canvas_wall_sign.json; wall_pair: farmersdelight:lime_canvas_sign.
- `farmersdelight:lime_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/lime_wall_hanging_canvas_sign.json.
- `farmersdelight:magenta_canvas_wall_sign` — missing_item: assets/farmersdelight/items/magenta_canvas_wall_sign.json; wall_pair: farmersdelight:magenta_canvas_sign.
- `farmersdelight:magenta_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/magenta_wall_hanging_canvas_sign.json.
- `farmersdelight:onions` — missing_item: assets/farmersdelight/items/onions.json.
- `farmersdelight:orange_canvas_wall_sign` — missing_item: assets/farmersdelight/items/orange_canvas_wall_sign.json; wall_pair: farmersdelight:orange_canvas_sign.
- `farmersdelight:orange_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/orange_wall_hanging_canvas_sign.json.
- `farmersdelight:pink_canvas_wall_sign` — missing_item: assets/farmersdelight/items/pink_canvas_wall_sign.json; wall_pair: farmersdelight:pink_canvas_sign.
- `farmersdelight:pink_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/pink_wall_hanging_canvas_sign.json.
- `farmersdelight:purple_canvas_wall_sign` — missing_item: assets/farmersdelight/items/purple_canvas_wall_sign.json; wall_pair: farmersdelight:purple_canvas_sign.
- `farmersdelight:purple_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/purple_wall_hanging_canvas_sign.json.
- `farmersdelight:red_canvas_wall_sign` — missing_item: assets/farmersdelight/items/red_canvas_wall_sign.json; wall_pair: farmersdelight:red_canvas_sign.
- `farmersdelight:red_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/red_wall_hanging_canvas_sign.json.
- `farmersdelight:rice_panicles` — missing_item: assets/farmersdelight/items/rice_panicles.json.
- `farmersdelight:tomatoes` — missing_item: assets/farmersdelight/items/tomatoes.json.
- `farmersdelight:wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/wall_hanging_canvas_sign.json; wall_pair: farmersdelight:hanging_canvas_sign.
- `farmersdelight:white_canvas_wall_sign` — missing_item: assets/farmersdelight/items/white_canvas_wall_sign.json; wall_pair: farmersdelight:white_canvas_sign.
- `farmersdelight:white_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/white_wall_hanging_canvas_sign.json.
- `farmersdelight:yellow_canvas_wall_sign` — missing_item: assets/farmersdelight/items/yellow_canvas_wall_sign.json; wall_pair: farmersdelight:yellow_canvas_sign.
- `farmersdelight:yellow_wall_hanging_canvas_sign` — missing_item: assets/farmersdelight/items/yellow_wall_hanging_canvas_sign.json.

### farmersknives-3.2-batch1.1.hmod

SHA-256: `8f776761b20880384f2b30953e5c69e193ef3640529e9b0a28669da5376032aa`.

Замечаний к записям нет.


### froglightsreimagined-1.0-horizon.3.hmod

SHA-256: `cbd2bf261c8dbd63167430ef908a1b303f32daed7244e4e0d130821aae17186e`.

Замечаний к записям нет.


### geophilic-3.7-horizon.3.hmod

SHA-256: `436d4a7a9be38ed3eb8dfcaa2daa79fc5b7982ad366b2ab261efb3ea5314b971`.

Вкладок нет; записей для проверки нет.


### homestead_sound-0.1.0-homestead.1.hmod

SHA-256: `c46d274544aab414af47f393da622543077483e964042211be0e19a3c76b9a30`.

Вкладок нет; записей для проверки нет.


### incendium-5.3.5-horizon.3.hmod

SHA-256: `ef7c1e7601c44930ae28b56a98cba14e7f36683fe71359fe8c4fcca73598117e`.

Вкладок нет; записей для проверки нет.


### justoutdoorstuffs-1.0.2-horizon.3.hmod

SHA-256: `fcf92a17e9bb6bcc743ad4f00063adc876bdb5b6076dec7483737bf930362c13`.

Замечаний к записям нет.


### letsdoaddonstructures-1.7.2-horizon.3.hmod

SHA-256: `6f15149213e55429c1f1ceda86c8a9e5631d80ab0a61ffea1a55b3183c41daba`.

Вкладок нет; записей для проверки нет.


### luphieclutteredmod-2.1.0_1.20.1-horizon.3.hmod

SHA-256: `dc1a06f0e02a2b371ec5b60a937839275c75af73cc161c1b52c5241aa4f73d52`.

Замечаний к записям нет.


### mcaccel-1.0.0-batch2.1.hmod

SHA-256: `091247b0e412f39d6ea34309cd76d96caf7d6063d49e5d36cb57dd9eb6b9e402`.

Замечаний к записям нет.


### mcwholidays-1.1.2-horizon.3.hmod

SHA-256: `12b3d445c625f60d4eec758dd95c493d335428c186dd77fdf2b6781436a4df08`.

- `mcwholidays:blue_ornament_wall_deco_1` — wall_pair: mcwholidays:blue_ornament.
- `mcwholidays:blue_ornament_wall_deco_2` — wall_pair: mcwholidays:blue_ornament.
- `mcwholidays:garland_wall_deco` — wall_pair: mcwholidays:garland.
- `mcwholidays:ghost_wall_deco_1` — wall_pair: mcwholidays:ghost.
- `mcwholidays:ghost_wall_deco_2` — wall_pair: mcwholidays:ghost.
- `mcwholidays:green_ornament_wall_deco_1` — wall_pair: mcwholidays:green_ornament.
- `mcwholidays:green_ornament_wall_deco_2` — wall_pair: mcwholidays:green_ornament.
- `mcwholidays:purple_ornament_wall_deco_1` — wall_pair: mcwholidays:purple_ornament.
- `mcwholidays:purple_ornament_wall_deco_2` — wall_pair: mcwholidays:purple_ornament.
- `mcwholidays:red_ornament_wall_deco_1` — wall_pair: mcwholidays:red_ornament.
- `mcwholidays:red_ornament_wall_deco_2` — wall_pair: mcwholidays:red_ornament.
- `mcwholidays:witch_hat_wall_deco_1` — wall_pair: mcwholidays:witch_hat.
- `mcwholidays:witch_hat_wall_deco_2` — wall_pair: mcwholidays:witch_hat.
- `mcwholidays:yellow_ornament_wall_deco_1` — wall_pair: mcwholidays:yellow_ornament.
- `mcwholidays:yellow_ornament_wall_deco_2` — wall_pair: mcwholidays:yellow_ornament.

### mcwpaintings-1.1.0-horizon.3.hmod

SHA-256: `543681ef17d449edb5a74faa1b24fccbde3227956a4498d90b85e01c72e8df3c`.

Вкладок нет; записей для проверки нет.


### mcwpaths-1.1.2-horizon.3.hmod

SHA-256: `b74de4e8bd2e685622c1c34a86d8ddc5b3be810b2c2352c17876a345fb95d633`.

Замечаний к записям нет.


### mo_glass-1.7_MC1.20.1-batch1.1.hmod

SHA-256: `6b276855d9beade4da7b9b9fac86d4b764a9101b404daf813d599326718ed247`.

Замечаний к записям нет.


### modrinth_logo_mod-1.3.4-horizon.3.hmod

SHA-256: `b33633500897b159043e574d49f0146f1f2e886186128c5f4f48636ec61c9867`.

Замечаний к записям нет.


### mr_endercon-1.0.2-horizon.3.hmod

SHA-256: `ebbd3b0f09433c2edd464f3e855c0397b3783d6dfdbe4fc05072f83965b029fa`.

Вкладок нет; записей для проверки нет.


### mr_flower_tweaks-1-horizon.3.hmod

SHA-256: `cd665f3f0e57bc5810a06c51677ef6c31522be8e7caa4e614580db4bb0a7c85b`.

Вкладок нет; записей для проверки нет.


### mr_mastercutter-1.1-horizon.3.hmod

SHA-256: `65a8d404ead6d635602bc02f4966c64b5b8812f9487378a0d9f5b6dcb4ac4db2`.

Вкладок нет; записей для проверки нет.


### mr_tidal_towns-2.0-horizon.3.hmod

SHA-256: `2972b9a14e89d753f2e3316867119220d77a532f29749c07214cc4aa4cb631ae`.

Вкладок нет; записей для проверки нет.


### mr_typewriter_daycounter-1.0-horizon.3.hmod

SHA-256: `b4e8594f9506146616d4e7a7d96b860382838fd1220fcafad0db92eaaafb9edb`.

Вкладок нет; записей для проверки нет.


### natures_delight-1.12_1.20.1-horizon.3.hmod

SHA-256: `85668d78ccdc80dffdee458657b0fc5547188ca3663518fbb929abbd410ddd48`.

Замечаний к записям нет.


### nethersdelight-1.20.1_4.1.1-batch1.1.hmod

SHA-256: `a171020e67a56a23a2ad0a7fab0cbe758808f534c9f9876cd9e6eb5e700e0e62`.

- `nethersdelight:potted_mimicarnation` — missing_item: assets/nethersdelight/items/potted_mimicarnation.json.
- `nethersdelight:propelplant_wall_torch` — missing_item: assets/nethersdelight/items/propelplant_wall_torch.json.

### obsidianequipmentrework-1.1-batch2.1.hmod

SHA-256: `0280378b00e7e5bba80e61a93b98b619e7c66156e6454ed347c77b5706a6bd26`.

- `obsidianequipmentrework:obsidian_spear` — branch_missing_model: assets/obsidianequipmentrework/models/item/obsidian_spear_in_hand.json.

### oceansdelight-fdrf_fabric_1.0.2_1.20-batch2.1.hmod

SHA-256: `c75fe557ea5988ab89bf7b6801852f636488e4a13a35483fea8999c7e6f13ac3`.

Замечаний к записям нет.


### perfectplushies-1.13.3-horizon.3.hmod

SHA-256: `dee82898ab60547b263658f5688bc8c6ab3dbff040024b7f58afd81c3d3ff69c`.

- `perfectplushies:player_plushie` — entity_without_geometry: perfectplushies:item/player_plushie.
- `perfectplushies:daniel_plushie` — entity_without_geometry: perfectplushies:item/daniel_plushie.
- `perfectplushies:dumbo_blob_plushie` — entity_without_geometry: perfectplushies:item/dumbo_blob_plushie.
- `perfectplushies:gamerpotion_plushie` — entity_without_geometry: perfectplushies:item/gamerpotion_plushie.
- `perfectplushies:gamerpotion_plushie_rare` — entity_without_geometry: perfectplushies:item/gamerpotion_plushie_rare.
- `perfectplushies:geode_plushie` — entity_without_geometry: perfectplushies:item/geode_plushie.
- `perfectplushies:hedgehog_plushie` — missing_texture: assets/minecraft/textures/1.png.
- `perfectplushies:joosh_plushie` — entity_without_geometry: perfectplushies:item/joosh_plushie.
- `perfectplushies:june_plushie` — entity_without_geometry: perfectplushies:item/june_plushie.
- `perfectplushies:nyf_plushie` — entity_without_geometry: perfectplushies:item/nyf_plushie.
- `perfectplushies:rocris_plushie` — entity_without_geometry: perfectplushies:item/rocris_plushie.
- `perfectplushies:silk_plushie` — entity_without_geometry: perfectplushies:item/silk_plushie.
- `perfectplushies:sirjain_plushie` — entity_without_geometry: perfectplushies:item/sirjain_plushie.
- `perfectplushies:sirjain_plushie_rare` — entity_without_geometry: perfectplushies:item/sirjain_plushie_rare.
- `perfectplushies:sizableshrimp_plushie` — entity_without_geometry: perfectplushies:item/sizableshrimp_plushie.
- `perfectplushies:tslat_plushie` — entity_without_geometry: perfectplushies:item/tslat_plushie.

Значки вкладок (отдельно от числа записей):
- `perfectplushies:player_plushie`: entity_without_geometry: perfectplushies:item/player_plushie.

### phomesteadplushies-1.1.0-batch1.1.hmod

SHA-256: `3a62c7937344ef962cf9c9e41d3c548b2744808623feba907571ca5081ea7cd5`.

- `phomesteadplushies:fernling_plushie` — entity_without_geometry: phomesteadplushies:item/fernling_plushie; missing_texture: assets/homestead/textures/item/mushling.png.
- `phomesteadplushies:homestead_player_1` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_10` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_11` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_12` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_13` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_14` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_15` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_16` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_17` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_18` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_19` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_2` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_20` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_21` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_22` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_23` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_24` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_25` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_26` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_27` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_28` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_29` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_3` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_30` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_31` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_32` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_33` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_34` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_35` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_36` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_37` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_38` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_39` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_4` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_40` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_41` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_42` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_43` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_44` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_45` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_46` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_47` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_48` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_49` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_5` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_50` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_6` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_7` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_8` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:homestead_player_9` — missing_model: assets/perfectplushieapi/models/item/player_plushie.json.
- `phomesteadplushies:mushling_plushie` — entity_without_geometry: phomesteadplushies:item/mushling_plushie; missing_texture: assets/homestead/textures/item/mushling.png.
- `phomesteadplushies:mystical_elk_plushie` — entity_without_geometry: phomesteadplushies:item/mystical_elk_plushie; missing_texture: assets/homestead/textures/item/mushling.png.

Значки вкладок (отдельно от числа записей):
- `phomesteadplushies:fernling_plushie`: entity_without_geometry: phomesteadplushies:item/fernling_plushie; missing_texture: assets/homestead/textures/item/mushling.png.

### regions_unexplored-0.5.6_1.20.1-horizon.3.hmod

SHA-256: `0c66d53564b6dfc3157f769d4fb9f2c6ae90351128b03b5b56888a759ffe7f6c`.

- `regions_unexplored:cooked_meat` — missing_texture: assets/regions_unexplored/textures/item/cooked_meat.png.
- `regions_unexplored:raw_meat` — missing_texture: assets/regions_unexplored/textures/item/raw_meat.png.
- `regions_unexplored:ruby` — missing_texture: assets/regions_unexplored/textures/item/ruby.png.
- `regions_unexplored:seeded_grass` — missing_texture: assets/regions_unexplored/textures/block/seeded_grass.png.
- `regions_unexplored:seeded_tall_grass` — missing_texture: assets/regions_unexplored/textures/block/seeded_tall_grass_top.png.
- `regions_unexplored:bamboo_log_leaves` — missing_item: assets/regions_unexplored/items/bamboo_log_leaves.json.
- `regions_unexplored:baobab_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/baobab_wall_hanging_sign.json; wall_pair: regions_unexplored:baobab_hanging_sign.
- `regions_unexplored:baobab_wall_sign` — missing_item: assets/regions_unexplored/items/baobab_wall_sign.json; wall_pair: regions_unexplored:baobab_sign.
- `regions_unexplored:blackwood_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/blackwood_wall_hanging_sign.json; wall_pair: regions_unexplored:blackwood_hanging_sign.
- `regions_unexplored:blackwood_wall_sign` — missing_item: assets/regions_unexplored/items/blackwood_wall_sign.json; wall_pair: regions_unexplored:blackwood_sign.
- `regions_unexplored:blue_bioshroom_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/blue_bioshroom_wall_hanging_sign.json; wall_pair: regions_unexplored:blue_bioshroom_hanging_sign.
- `regions_unexplored:blue_bioshroom_wall_sign` — missing_item: assets/regions_unexplored/items/blue_bioshroom_wall_sign.json; wall_pair: regions_unexplored:blue_bioshroom_sign.
- `regions_unexplored:blue_crystalite` — missing_item: assets/regions_unexplored/items/blue_crystalite.json.
- `regions_unexplored:brimwood_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/brimwood_wall_hanging_sign.json; wall_pair: regions_unexplored:brimwood_hanging_sign.
- `regions_unexplored:brimwood_wall_sign` — missing_item: assets/regions_unexplored/items/brimwood_wall_sign.json; wall_pair: regions_unexplored:brimwood_sign.
- `regions_unexplored:brown_wall_mushroom` — missing_texture: assets/regions_unexplored/textures/item/brown_wall_mushroom.png.
- `regions_unexplored:cobalt_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/cobalt_wall_hanging_sign.json; wall_pair: regions_unexplored:cobalt_hanging_sign.
- `regions_unexplored:cobalt_wall_sign` — missing_item: assets/regions_unexplored/items/cobalt_wall_sign.json; wall_pair: regions_unexplored:cobalt_sign.
- `regions_unexplored:cypress_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/cypress_wall_hanging_sign.json; wall_pair: regions_unexplored:cypress_hanging_sign.
- `regions_unexplored:cypress_wall_sign` — missing_item: assets/regions_unexplored/items/cypress_wall_sign.json; wall_pair: regions_unexplored:cypress_sign.
- `regions_unexplored:dead_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/dead_wall_hanging_sign.json; wall_pair: regions_unexplored:dead_hanging_sign.
- `regions_unexplored:dead_wall_sign` — missing_item: assets/regions_unexplored/items/dead_wall_sign.json; wall_pair: regions_unexplored:dead_sign.
- `regions_unexplored:dirt_placement` — missing_item: assets/regions_unexplored/items/dirt_placement.json.
- `regions_unexplored:dropleaf_plant` — missing_item: assets/regions_unexplored/items/dropleaf_plant.json.
- `regions_unexplored:duskmelon` — missing_item: assets/regions_unexplored/items/duskmelon.json.
- `regions_unexplored:eucalyptus_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/eucalyptus_wall_hanging_sign.json; wall_pair: regions_unexplored:eucalyptus_hanging_sign.
- `regions_unexplored:eucalyptus_wall_sign` — missing_item: assets/regions_unexplored/items/eucalyptus_wall_sign.json; wall_pair: regions_unexplored:eucalyptus_sign.
- `regions_unexplored:giant_lily_pad` — missing_item: assets/regions_unexplored/items/giant_lily_pad.json.
- `regions_unexplored:glistering_ivy_plant` — missing_item: assets/regions_unexplored/items/glistering_ivy_plant.json.
- `regions_unexplored:grass_placement` — missing_item: assets/regions_unexplored/items/grass_placement.json.
- `regions_unexplored:green_bioshroom_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/green_bioshroom_wall_hanging_sign.json; wall_pair: regions_unexplored:green_bioshroom_hanging_sign.
- `regions_unexplored:green_bioshroom_wall_sign` — missing_item: assets/regions_unexplored/items/green_bioshroom_wall_sign.json; wall_pair: regions_unexplored:green_bioshroom_sign.
- `regions_unexplored:hanging_earlight` — missing_item: assets/regions_unexplored/items/hanging_earlight.json.
- `regions_unexplored:hanging_earlight_plant` — missing_item: assets/regions_unexplored/items/hanging_earlight_plant.json.
- `regions_unexplored:joshua_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/joshua_wall_hanging_sign.json; wall_pair: regions_unexplored:joshua_hanging_sign.
- `regions_unexplored:joshua_wall_sign` — missing_item: assets/regions_unexplored/items/joshua_wall_sign.json; wall_pair: regions_unexplored:joshua_sign.
- `regions_unexplored:kapok_vines_plant` — missing_item: assets/regions_unexplored/items/kapok_vines_plant.json.
- `regions_unexplored:kapok_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/kapok_wall_hanging_sign.json; wall_pair: regions_unexplored:kapok_hanging_sign.
- `regions_unexplored:kapok_wall_sign` — missing_item: assets/regions_unexplored/items/kapok_wall_sign.json; wall_pair: regions_unexplored:kapok_sign.
- `regions_unexplored:larch_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/larch_wall_hanging_sign.json; wall_pair: regions_unexplored:larch_hanging_sign.
- `regions_unexplored:larch_wall_sign` — missing_item: assets/regions_unexplored/items/larch_wall_sign.json; wall_pair: regions_unexplored:larch_sign.
- `regions_unexplored:magnolia_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/magnolia_wall_hanging_sign.json; wall_pair: regions_unexplored:magnolia_hanging_sign.
- `regions_unexplored:magnolia_wall_sign` — missing_item: assets/regions_unexplored/items/magnolia_wall_sign.json; wall_pair: regions_unexplored:magnolia_sign.
- `regions_unexplored:maple_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/maple_wall_hanging_sign.json; wall_pair: regions_unexplored:maple_hanging_sign.
- `regions_unexplored:maple_wall_sign` — missing_item: assets/regions_unexplored/items/maple_wall_sign.json; wall_pair: regions_unexplored:maple_sign.
- `regions_unexplored:mauve_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/mauve_wall_hanging_sign.json; wall_pair: regions_unexplored:mauve_hanging_sign.
- `regions_unexplored:mauve_wall_sign` — missing_item: assets/regions_unexplored/items/mauve_wall_sign.json; wall_pair: regions_unexplored:mauve_sign.
- `regions_unexplored:mud_placement` — missing_item: assets/regions_unexplored/items/mud_placement.json.
- `regions_unexplored:palm_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/palm_wall_hanging_sign.json; wall_pair: regions_unexplored:palm_hanging_sign.
- `regions_unexplored:palm_wall_sign` — missing_item: assets/regions_unexplored/items/palm_wall_sign.json; wall_pair: regions_unexplored:palm_sign.
- `regions_unexplored:pine_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/pine_wall_hanging_sign.json; wall_pair: regions_unexplored:pine_hanging_sign.
- `regions_unexplored:pine_wall_sign` — missing_item: assets/regions_unexplored/items/pine_wall_sign.json; wall_pair: regions_unexplored:pine_sign.
- `regions_unexplored:pink_bioshroom_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/pink_bioshroom_wall_hanging_sign.json; wall_pair: regions_unexplored:pink_bioshroom_hanging_sign.
- `regions_unexplored:pink_bioshroom_wall_sign` — missing_item: assets/regions_unexplored/items/pink_bioshroom_wall_sign.json; wall_pair: regions_unexplored:pink_bioshroom_sign.
- `regions_unexplored:potted_alpha_dandelion` — missing_item: assets/regions_unexplored/items/potted_alpha_dandelion.json.
- `regions_unexplored:potted_alpha_rose` — missing_item: assets/regions_unexplored/items/potted_alpha_rose.json.
- `regions_unexplored:potted_alpha_sapling` — missing_item: assets/regions_unexplored/items/potted_alpha_sapling.json.
- `regions_unexplored:potted_apple_oak_sapling` — missing_item: assets/regions_unexplored/items/potted_apple_oak_sapling.json.
- `regions_unexplored:potted_ashen_sapling` — missing_item: assets/regions_unexplored/items/potted_ashen_sapling.json.
- `regions_unexplored:potted_aster` — missing_item: assets/regions_unexplored/items/potted_aster.json.
- `regions_unexplored:potted_bamboo_sapling` — missing_item: assets/regions_unexplored/items/potted_bamboo_sapling.json.
- `regions_unexplored:potted_baobab_sapling` — missing_item: assets/regions_unexplored/items/potted_baobab_sapling.json.
- `regions_unexplored:potted_barrel_cactus` — missing_item: assets/regions_unexplored/items/potted_barrel_cactus.json.
- `regions_unexplored:potted_black_snowbelle` — missing_item: assets/regions_unexplored/items/potted_black_snowbelle.json.
- `regions_unexplored:potted_blackwood_sapling` — missing_item: assets/regions_unexplored/items/potted_blackwood_sapling.json.
- `regions_unexplored:potted_bleeding_heart` — missing_item: assets/regions_unexplored/items/potted_bleeding_heart.json.
- `regions_unexplored:potted_blue_bioshroom` — missing_item: assets/regions_unexplored/items/potted_blue_bioshroom.json.
- `regions_unexplored:potted_blue_lupine` — missing_item: assets/regions_unexplored/items/potted_blue_lupine.json.
- `regions_unexplored:potted_blue_magnolia_sapling` — missing_item: assets/regions_unexplored/items/potted_blue_magnolia_sapling.json.
- `regions_unexplored:potted_blue_snowbelle` — missing_item: assets/regions_unexplored/items/potted_blue_snowbelle.json.
- `regions_unexplored:potted_brimwood_sapling` — missing_item: assets/regions_unexplored/items/potted_brimwood_sapling.json.
- `regions_unexplored:potted_brown_snowbelle` — missing_item: assets/regions_unexplored/items/potted_brown_snowbelle.json.
- `regions_unexplored:potted_cactus_flower` — missing_item: assets/regions_unexplored/items/potted_cactus_flower.json.
- `regions_unexplored:potted_cave_hyssop` — missing_item: assets/regions_unexplored/items/potted_cave_hyssop.json.
- `regions_unexplored:potted_cobalt_earlight` — missing_item: assets/regions_unexplored/items/potted_cobalt_earlight.json.
- `regions_unexplored:potted_cobalt_sapling` — missing_item: assets/regions_unexplored/items/potted_cobalt_sapling.json.
- `regions_unexplored:potted_corpse_flower` — missing_item: assets/regions_unexplored/items/potted_corpse_flower.json.
- `regions_unexplored:potted_cyan_snowbelle` — missing_item: assets/regions_unexplored/items/potted_cyan_snowbelle.json.
- `regions_unexplored:potted_cypress_sapling` — missing_item: assets/regions_unexplored/items/potted_cypress_sapling.json.
- `regions_unexplored:potted_daisy` — missing_item: assets/regions_unexplored/items/potted_daisy.json.
- `regions_unexplored:potted_day_lily` — missing_item: assets/regions_unexplored/items/potted_day_lily.json.
- `regions_unexplored:potted_dead_pine_sapling` — missing_item: assets/regions_unexplored/items/potted_dead_pine_sapling.json.
- `regions_unexplored:potted_dead_sapling` — missing_item: assets/regions_unexplored/items/potted_dead_sapling.json.
- `regions_unexplored:potted_dorcel` — missing_item: assets/regions_unexplored/items/potted_dorcel.json.
- `regions_unexplored:potted_dusktrap` — missing_item: assets/regions_unexplored/items/potted_dusktrap.json.
- `regions_unexplored:potted_enchanted_birch_sapling` — missing_item: assets/regions_unexplored/items/potted_enchanted_birch_sapling.json.
- `regions_unexplored:potted_eucalyptus_sapling` — missing_item: assets/regions_unexplored/items/potted_eucalyptus_sapling.json.
- `regions_unexplored:potted_felicia_daisy` — missing_item: assets/regions_unexplored/items/potted_felicia_daisy.json.
- `regions_unexplored:potted_fireweed` — missing_item: assets/regions_unexplored/items/potted_fireweed.json.
- `regions_unexplored:potted_flowering_sapling` — missing_item: assets/regions_unexplored/items/potted_flowering_sapling.json.
- `regions_unexplored:potted_glister_spire` — missing_item: assets/regions_unexplored/items/potted_glister_spire.json.
- `regions_unexplored:potted_glistering_bloom` — missing_item: assets/regions_unexplored/items/potted_glistering_bloom.json.
- `regions_unexplored:potted_golden_larch_sapling` — missing_item: assets/regions_unexplored/items/potted_golden_larch_sapling.json.
- `regions_unexplored:potted_gray_snowbelle` — missing_item: assets/regions_unexplored/items/potted_gray_snowbelle.json.
- `regions_unexplored:potted_green_bioshroom` — missing_item: assets/regions_unexplored/items/potted_green_bioshroom.json.
- `regions_unexplored:potted_green_snowbelle` — missing_item: assets/regions_unexplored/items/potted_green_snowbelle.json.
- `regions_unexplored:potted_hibiscus` — missing_item: assets/regions_unexplored/items/potted_hibiscus.json.
- `regions_unexplored:potted_hyssop` — missing_item: assets/regions_unexplored/items/potted_hyssop.json.
- `regions_unexplored:potted_joshua_sapling` — missing_item: assets/regions_unexplored/items/potted_joshua_sapling.json.
- `regions_unexplored:potted_kapok_sapling` — missing_item: assets/regions_unexplored/items/potted_kapok_sapling.json.
- `regions_unexplored:potted_larch_sapling` — missing_item: assets/regions_unexplored/items/potted_larch_sapling.json.
- `regions_unexplored:potted_light_blue_snowbelle` — missing_item: assets/regions_unexplored/items/potted_light_blue_snowbelle.json.
- `regions_unexplored:potted_light_gray_snowbelle` — missing_item: assets/regions_unexplored/items/potted_light_gray_snowbelle.json.
- `regions_unexplored:potted_lime_snowbelle` — missing_item: assets/regions_unexplored/items/potted_lime_snowbelle.json.
- `regions_unexplored:potted_magenta_snowbelle` — missing_item: assets/regions_unexplored/items/potted_magenta_snowbelle.json.
- `regions_unexplored:potted_magnolia_sapling` — missing_item: assets/regions_unexplored/items/potted_magnolia_sapling.json.
- `regions_unexplored:potted_mallow` — missing_item: assets/regions_unexplored/items/potted_mallow.json.
- `regions_unexplored:potted_maple_sapling` — missing_item: assets/regions_unexplored/items/potted_maple_sapling.json.
- `regions_unexplored:potted_mauve_sapling` — missing_item: assets/regions_unexplored/items/potted_mauve_sapling.json.
- `regions_unexplored:potted_meadow_sage` — missing_item: assets/regions_unexplored/items/potted_meadow_sage.json.
- `regions_unexplored:potted_mycotoxic_daisy` — missing_item: assets/regions_unexplored/items/potted_mycotoxic_daisy.json.
- `regions_unexplored:potted_orange_coneflower` — missing_item: assets/regions_unexplored/items/potted_orange_coneflower.json.
- `regions_unexplored:potted_orange_maple_sapling` — missing_item: assets/regions_unexplored/items/potted_orange_maple_sapling.json.
- `regions_unexplored:potted_orange_snowbelle` — missing_item: assets/regions_unexplored/items/potted_orange_snowbelle.json.
- `regions_unexplored:potted_palm_sapling` — missing_item: assets/regions_unexplored/items/potted_palm_sapling.json.
- `regions_unexplored:potted_pine_sapling` — missing_item: assets/regions_unexplored/items/potted_pine_sapling.json.
- `regions_unexplored:potted_pink_bioshroom` — missing_item: assets/regions_unexplored/items/potted_pink_bioshroom.json.
- `regions_unexplored:potted_pink_lupine` — missing_item: assets/regions_unexplored/items/potted_pink_lupine.json.
- `regions_unexplored:potted_pink_magnolia_sapling` — missing_item: assets/regions_unexplored/items/potted_pink_magnolia_sapling.json.
- `regions_unexplored:potted_pink_snowbelle` — missing_item: assets/regions_unexplored/items/potted_pink_snowbelle.json.
- `regions_unexplored:potted_poppy_bush` — missing_item: assets/regions_unexplored/items/potted_poppy_bush.json.
- `regions_unexplored:potted_purple_coneflower` — missing_item: assets/regions_unexplored/items/potted_purple_coneflower.json.
- `regions_unexplored:potted_purple_lupine` — missing_item: assets/regions_unexplored/items/potted_purple_lupine.json.
- `regions_unexplored:potted_purple_snowbelle` — missing_item: assets/regions_unexplored/items/potted_purple_snowbelle.json.
- `regions_unexplored:potted_red_lupine` — missing_item: assets/regions_unexplored/items/potted_red_lupine.json.
- `regions_unexplored:potted_red_maple_sapling` — missing_item: assets/regions_unexplored/items/potted_red_maple_sapling.json.
- `regions_unexplored:potted_red_snowbelle` — missing_item: assets/regions_unexplored/items/potted_red_snowbelle.json.
- `regions_unexplored:potted_redwood_sapling` — missing_item: assets/regions_unexplored/items/potted_redwood_sapling.json.
- `regions_unexplored:potted_salmon_poppy_bush` — missing_item: assets/regions_unexplored/items/potted_salmon_poppy_bush.json.
- `regions_unexplored:potted_silver_birch_sapling` — missing_item: assets/regions_unexplored/items/potted_silver_birch_sapling.json.
- `regions_unexplored:potted_small_oak_sapling` — missing_item: assets/regions_unexplored/items/potted_small_oak_sapling.json.
- `regions_unexplored:potted_socotra_sapling` — missing_item: assets/regions_unexplored/items/potted_socotra_sapling.json.
- `regions_unexplored:potted_tall_blue_bioshroom` — missing_item: assets/regions_unexplored/items/potted_tall_blue_bioshroom.json.
- `regions_unexplored:potted_tall_cobalt_earlight` — missing_item: assets/regions_unexplored/items/potted_tall_cobalt_earlight.json.
- `regions_unexplored:potted_tall_green_bioshroom` — missing_item: assets/regions_unexplored/items/potted_tall_green_bioshroom.json.
- `regions_unexplored:potted_tall_pink_bioshroom` — missing_item: assets/regions_unexplored/items/potted_tall_pink_bioshroom.json.
- `regions_unexplored:potted_tall_yellow_bioshroom` — missing_item: assets/regions_unexplored/items/potted_tall_yellow_bioshroom.json.
- `regions_unexplored:potted_tsubaki` — missing_item: assets/regions_unexplored/items/potted_tsubaki.json.
- `regions_unexplored:potted_waratah` — missing_item: assets/regions_unexplored/items/potted_waratah.json.
- `regions_unexplored:potted_white_magnolia_sapling` — missing_item: assets/regions_unexplored/items/potted_white_magnolia_sapling.json.
- `regions_unexplored:potted_white_snowbelle` — missing_item: assets/regions_unexplored/items/potted_white_snowbelle.json.
- `regions_unexplored:potted_white_trillium` — missing_item: assets/regions_unexplored/items/potted_white_trillium.json.
- `regions_unexplored:potted_willow_sapling` — missing_item: assets/regions_unexplored/items/potted_willow_sapling.json.
- `regions_unexplored:potted_wilting_trillium` — missing_item: assets/regions_unexplored/items/potted_wilting_trillium.json.
- `regions_unexplored:potted_yellow_bioshroom` — missing_item: assets/regions_unexplored/items/potted_yellow_bioshroom.json.
- `regions_unexplored:potted_yellow_lupine` — missing_item: assets/regions_unexplored/items/potted_yellow_lupine.json.
- `regions_unexplored:potted_yellow_snowbelle` — missing_item: assets/regions_unexplored/items/potted_yellow_snowbelle.json.
- `regions_unexplored:redwood_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/redwood_wall_hanging_sign.json; wall_pair: regions_unexplored:redwood_hanging_sign.
- `regions_unexplored:redwood_wall_sign` — missing_item: assets/regions_unexplored/items/redwood_wall_sign.json; wall_pair: regions_unexplored:redwood_sign.
- `regions_unexplored:salmonberry_bush` — missing_item: assets/regions_unexplored/items/salmonberry_bush.json.
- `regions_unexplored:sand_placement` — missing_item: assets/regions_unexplored/items/sand_placement.json.
- `regions_unexplored:socotra_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/socotra_wall_hanging_sign.json; wall_pair: regions_unexplored:socotra_hanging_sign.
- `regions_unexplored:socotra_wall_sign` — missing_item: assets/regions_unexplored/items/socotra_wall_sign.json; wall_pair: regions_unexplored:socotra_sign.
- `regions_unexplored:spanish_moss_plant` — missing_item: assets/regions_unexplored/items/spanish_moss_plant.json.
- `regions_unexplored:willow_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/willow_wall_hanging_sign.json; wall_pair: regions_unexplored:willow_hanging_sign.
- `regions_unexplored:willow_wall_sign` — missing_item: assets/regions_unexplored/items/willow_wall_sign.json; wall_pair: regions_unexplored:willow_sign.
- `regions_unexplored:yellow_bioshroom_wall_hanging_sign` — missing_item: assets/regions_unexplored/items/yellow_bioshroom_wall_hanging_sign.json; wall_pair: regions_unexplored:yellow_bioshroom_hanging_sign.
- `regions_unexplored:yellow_bioshroom_wall_sign` — missing_item: assets/regions_unexplored/items/yellow_bioshroom_wall_sign.json; wall_pair: regions_unexplored:yellow_bioshroom_sign.
- `regions_unexplored:yellow_wall_bioshroom` — missing_texture: assets/regions_unexplored/textures/item/yellow_wall_bioshroom.png.

Значки вкладок (отдельно от числа записей):
- `regions_unexplored:baobab_boat`: policy_hidden: regions_unexplored:baobab_boat.

### repurposed_structures-7.1.25_1.20.1_fabric-horizon.3.hmod

SHA-256: `7cb9dcdd61ef342824131362f80f3fcb6cf131e56f85460ebacc911945a85cfe`.

Вкладок нет; записей для проверки нет.


### sootychimneys-1.3.4-batch2.1.hmod

SHA-256: `792775fbde6e7dc105c8893b52455fa48ad6d7e43a7feb1f40f556bfc6447e3e`.

- `sootychimneys:dirty_cobblestone_chimney` — texture_alias: sootychimneys:item/dirty_cobblestone_chimney#down; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#east; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#north; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#south; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#up; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#west.

### spuds_shops-1.9.0-batch1.1.hmod

SHA-256: `62f935e02eec0f880d92e0a131ba1ed702fb4de9c22b64a71a6d7f2ad11987ef`.

Вкладок нет; записей для проверки нет.


### suppsquared-1.20_1.1.29-horizon.3.hmod

SHA-256: `2b738c907d7015d9beed81de73fe6421203c8b5538b6f03fddb40245f6a71b67`.

- `suppsquared:heavy_key` — missing_model: assets/supplementaries/models/item/key.json.
- `suppsquared:daub_frame_slab` — missing_texture: assets/supplementaries/textures/block/daub_frame.png.
- `suppsquared:daub_frame_stairs` — missing_texture: assets/supplementaries/textures/block/daub_frame.png.
- `suppsquared:daub_frame_wall` — missing_model: assets/suppsquared/models/block/daub_frame_wall_inventory.json.
- `suppsquared:daub_slab` — missing_texture: assets/supplementaries/textures/block/daub.png.
- `suppsquared:daub_stairs` — missing_texture: assets/supplementaries/textures/block/daub.png.
- `suppsquared:daub_wall` — missing_model: assets/suppsquared/models/block/daub_wall_inventory.json.
- `suppsquared:item_shelf_suppsquared_shelf` — missing_texture: assets/suppsquared/textures/item/item_shelves/suppsquared_shelf.png.
- `suppsquared:sack_black` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_blue` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_brown` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_cyan` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_gray` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_green` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_light_blue` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_light_gray` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_lime` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_magenta` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_orange` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_pink` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_purple` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_red` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_white` — missing_model: assets/supplementaries/models/block/sack_closed.json.
- `suppsquared:sack_yellow` — missing_model: assets/supplementaries/models/block/sack_closed.json.

Значки вкладок (отдельно от числа записей):
- `suppsquared:heavy_key`: missing_model: assets/supplementaries/models/item/key.json.

### t_and_t-1.10.2-horizon.3.hmod

SHA-256: `1928849a86c1e894a91f9fce19880d268aa2322da20e2465a67a153efb023113`.

Вкладок нет; записей для проверки нет.


### universal_graves-3.0.3-homestead.1.hmod

SHA-256: `cab12f5fa2f19bbc245df7db454c577f7096bec773497b5a9a230f2984b1ba98`.

Вкладок нет; записей для проверки нет.


### universalbonemeal-8.0.1-horizon.3.hmod

SHA-256: `82c5f537c3a186f62796253ec5f5c34032a6f17245451b3b739348735cfd9b68`.

Вкладок нет; записей для проверки нет.


### villagesandpillages-2.0.0-horizon.3.hmod

SHA-256: `fd11d2f10028012cc5849bbcdc690edbec73e1fbc57788758a0ea9a547c4112c`.

Вкладок нет; записей для проверки нет.


### yungsextras-1.20_Fabric_4.0.3-horizon.3.hmod

SHA-256: `eefadcab097ff01426911aa11aa253d0eecf206b698e1a11442b7452b44e4a9f`.

Вкладок нет; записей для проверки нет.


## Источники политик

- `batch1-policy.json` — SHA-256 `5fc9667b835592755c82825064819cf15525d87734bc56a8368fa7b376223585`.
- `pack-batch1/overlay-farmersknives.json` — SHA-256 `e174945a5fee8c0517d620d7b6e30319e97ee349c09f58bc8735fb1c618fabe2`.
- `batch2-policy.json` — SHA-256 `d8b28e594391f0850cbf7201e03021dac7d125a46ac17021b4a78fe4f50efc76`.
- `pack-batch2/overlay-cratedelight.json` — SHA-256 `2d2230773e4e11812a8b64894b8130af28912b5f10de46a999c86bf0ff75ed7e`.
- `tools/unplaceable_items.json` — SHA-256 `93413e6d60b73de9b7b75fff9c8de85f2d9c314b1e1894d7f76c5da3d57793ba`.

## Отличается от оригинала

Проверяется порт Horizon и его политики, а не Java-клиент. Наличие модели/текстур не подтверждает точность переноса.

## Не проверено

Фактическое изображение каждого предмета в игре, прозрачность PNG, визуальная корректность transforms, выбор условных веток, поведение специальных рендереров, производительность перелистывания и применимость модели после кэша. Ресурсы из импортируемого Create JAR и подключённых ресурспаков вне `.hmod` не индексируются: такая внешняя ссылка отражается как отсутствующая в данном наборе. Отдельный мод Create, вплавленный в бинарник, не является `.hmod` этой выборки.

## Повторный запуск

Из корня проекта, PowerShell:

```powershell
$cs = 'work/homestead-horizon/create-source/create-horizon-4ae7340c06da5e77828e12494509fe9a70b8eaa4'
$python = 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
& $python -I "$cs/tools/tab_audit.py" --self-test
& $python -I "$cs/tools/tab_audit.py" --format markdown
# Для машинного разбора: --format json. Код 0 = аудит выполнен; 1 = --fail-on-issues; 2 = ошибка ввода.
```
