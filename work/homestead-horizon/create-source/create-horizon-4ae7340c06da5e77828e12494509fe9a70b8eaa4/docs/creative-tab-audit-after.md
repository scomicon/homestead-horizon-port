# Аудит вкладок креатива

## Сделано

Статический аудит текущих runtime `.hmod`, 2026-11-16. Архивы распакованы во временную папку вне репозитория и удалены после чтения. Сборка и игра не запускались. Исходные архивы проверены SHA-256 до и после аудита.

Архивов: **61**; вкладок: **33**; записей: **10020**. Кандидатов на пустую иконку: **0**. Нарушений политик: **0**.

Это диагностика ресурсов, а не подтверждение игрового рендера. Отсутствующая текстура может дать missing-texture, а не пустую ячейку; настенный дубликат может иметь исправную иконку. Категории пересекаются.

Ресурсы Minecraft разрешались по локальному подготовленному снимку `runtime-01/resources`; доступных дополнительных корней: 1. Ссылки между `.hmod` проверяются по общему индексу. Конфликтующих путей assets с разным содержимым: 0. Общие data-теги и отдельные манифесты архивов не считаются конфликтами ресурсов.

| Архив | Вкладки | Записи | Нет items JSON | Нет модели/родителя | Нет текстуры | Пустая геометрия | builtin/entity без геометрии | Настенная пара | Скрыто политикой | Кандидаты пустых |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alcocraftplus-2.1.1-horizon.3.hmod | 1 | 16 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| bakery-2.0.6-horizon.3.hmod | 1 | 58 | 0 | 0 | 7 | 0 | 0 | 0 | 0 | 0 |
| beautify-3.5.5-horizon.3.hmod | 1 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterdungeons-1.20_Fabric_4.0.4-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterendisland-1.20_Fabric_2.0.6-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterendrod-1.1.0-horizon.3.hmod | 1 | 9 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterfortresses-1.20_Fabric_2.0.6-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| bettermineshafts-1.20_Fabric_4.0.4-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| betterstrongholds-1.20_Fabric_4.0.3-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| blahaj-0.3.2-horizon.3.hmod | 1 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| candlelight-2.0.5-horizon.3.hmod | 1 | 121 | 0 | 0 | 4 | 0 | 0 | 0 | 0 | 0 |
| chipped-3.0.7-horizon.3.hmod | 1 | 6936 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| comforts-0.1.0-homestead.1.hmod | 1 | 32 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| connectiblechains-2.5.5_1.20.1-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| cratedelight-26.07.01_1.20_fabric-batch2.1.hmod | 1 | 26 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| createdeco-2.1.1_1.20.1_fabric-batch2.1.hmod | 1 | 326 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| darksmelting-1.0.6-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| diagonalwalls-8.0.4-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| diagonalwindows-8.1.5-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| dustydecorations-1.1_1.20.1_1.20.2-horizon.3.hmod | 1 | 79 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| dyedend-1.20.1_1.0.0-horizon.3.hmod | 1 | 64 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| echoes_of_the_end__structures_-12.00.13-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| ends_delight-refabricated_1.20.1_2.5-horizon.3.hmod | 1 | 60 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 |
| expandeddelight-0.3.2-batch1.1.hmod | 1 | 53 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| fairylights-1.1.5_fabric-horizon.3.hmod | 1 | 30 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| farmersdelight-2.4.1-horizon.10.hmod | 1 | 175 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| farmersknives-3.2-batch1.1.hmod | 1 | 9 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| froglightsreimagined-1.0-horizon.3.hmod | 1 | 48 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| geophilic-3.7-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| homestead_sound-0.1.0-homestead.1.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| incendium-5.3.5-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| justoutdoorstuffs-1.0.2-horizon.3.hmod | 1 | 52 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| letsdoaddonstructures-1.7.2-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| luphieclutteredmod-2.1.0_1.20.1-horizon.3.hmod | 1 | 509 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mcaccel-1.0.0-batch2.1.hmod | 1 | 28 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| mcwholidays-1.1.2-horizon.3.hmod | 1 | 294 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
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
| nethersdelight-1.20.1_4.1.1-batch1.1.hmod | 1 | 21 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| obsidianequipmentrework-1.1-batch2.1.hmod | 1 | 14 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| oceansdelight-fdrf_fabric_1.0.2_1.20-batch2.1.hmod | 1 | 22 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| perfectplushies-1.13.3-horizon.3.hmod | 1 | 33 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| phomesteadplushies-1.1.0-batch1.1.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| regions_unexplored-0.5.6_1.20.1-horizon.3.hmod | 1 | 662 | 0 | 0 | 7 | 0 | 0 | 0 | 0 | 0 |
| repurposed_structures-7.1.25_1.20.1_fabric-horizon.3.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| sootychimneys-1.3.4-batch2.1.hmod | 1 | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| spuds_shops-1.9.0-batch1.1.hmod | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| suppsquared-1.20_1.1.29-horizon.3.hmod | 1 | 8 | 0 | 0 | 5 | 0 | 0 | 0 | 0 | 0 |
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

- `regions_unexplored`: записей 662, кандидатов пустых 0, настенных пар 0, нарушений политик 0; объединение рекомендуемого фильтра 0.
- `perfectplushies`: записей 33, кандидатов пустых 0, настенных пар 0, нарушений политик 0; объединение рекомендуемого фильтра 0.
- `phomesteadplushies`: записей 0, кандидатов пустых 0, настенных пар 0, нарушений политик 0; объединение рекомендуемого фильтра 0.
- `mcwholidays`: записей 294, кандидатов пустых 0, настенных пар 0, нарушений политик 0; объединение рекомендуемого фильтра 0.

## Полные списки по модам

`missing_item` — нет нового items JSON; `missing_model` — отсутствует модель или родитель; `missing_texture` — отсутствует PNG; `empty_geometry` — нет faces и генерируемых слоёв; `entity_without_geometry` — builtin/entity без собственной геометрии; `wall_pair` — найден парный не-настенный ID; `policy_hidden` — нарушение политики. Прочие типы/ветки/рендереры и ошибки выводятся явно. Все ID с проблемами перечислены без усечения; ID без замечаний не перечисляются.

### alcocraftplus-2.1.1-horizon.3.hmod

SHA-256: `2cbb44dbb4ab09048313af01fffbb0ead1ada5ae9b75f4fb1402a6c61525fb4f`.

Замечаний к записям нет.


### bakery-2.0.6-horizon.3.hmod

SHA-256: `8301b74614d064b47e195879230fd27bac7c727498d072549aa04bc6ea78eb0b`.

- `bakery:cake_tower` — missing_texture: assets/bakery/textures/item/cake_tower.png.
- `bakery:chocolate_bread` — missing_texture: assets/bakery/textures/item/chocolate_bread.png.
- `bakery:donut` — missing_texture: assets/bakery/textures/item/donut.png.
- `bakery:faucet` — missing_texture: assets/bakery/textures/item/faucet.png.
- `bakery:jam_sandwich` — missing_texture: assets/bakery/textures/item/jam_sandwich.png.
- `bakery:pancake` — missing_texture: assets/bakery/textures/item/food/pancake.png.
- `bakery:shelf` — missing_texture: assets/bakery/textures/item/shelf.png.

### beautify-3.5.5-horizon.3.hmod

SHA-256: `a45ae22ea30919d30fea8532978b7e680b1aa8d7098325aa5cfcc77d284e2beb`.

Замечаний к записям нет.


### betterdungeons-1.20_Fabric_4.0.4-horizon.3.hmod

SHA-256: `ac6e7bb3b39bbbdb9d6be79d7c9eb025a7608b1b2544bc13dd5a32c16ca60a19`.

Вкладок нет; записей для проверки нет.


### betterendisland-1.20_Fabric_2.0.6-horizon.3.hmod

SHA-256: `0400c360fa7380bb2db1fc68c26e9b525bf15fab3fa7656d598eb14ee4573530`.

Вкладок нет; записей для проверки нет.


### betterendrod-1.1.0-horizon.3.hmod

SHA-256: `42b61fd9d420c573d8d3e59da66aed00eded71a0993d224448237fe7dd627cd0`.

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

SHA-256: `1849964bfb27437900899a2fae4bab039f0813b73e5384e19a6fa1dd1ee50ee4`.

Замечаний к записям нет.


### candlelight-2.0.5-horizon.3.hmod

SHA-256: `385e9105c40f81e629d77e3d66834663e79d17f8a8246fad7bf6ea884146a823`.

- `candlelight:dough` — missing_texture: assets/candlelight/textures/item/dough.png.
- `candlelight:faucet` — missing_texture: assets/candlelight/textures/item/faucet.png.
- `candlelight:toast` — missing_texture: assets/candlelight/textures/item/toast.png.
- `candlelight:tomato_sauce` — missing_texture: assets/candlelight/textures/item/tomato_sauce.png.
- `candlelight:bamboo_stove` — texture_alias: candlelight:item/bamboo_stove:#missing.

### chipped-3.0.7-horizon.3.hmod

SHA-256: `9c0caa6b2e9dcdf98b69d30a520a4bc036e09e0455f5d098aa3b2cbc1586a99c`.

Замечаний к записям нет.


### comforts-0.1.0-homestead.1.hmod

SHA-256: `588a94bb42fed1a7ebabc6d77102061051b6b8314864249790f2d1b89137adff`.

Замечаний к записям нет.


### connectiblechains-2.5.5_1.20.1-horizon.3.hmod

SHA-256: `2a6a839b7ae4abfededd28abb07919af8ef8a203504620a9b2315a741810e661`.

Вкладок нет; записей для проверки нет.


### cratedelight-26.07.01_1.20_fabric-batch2.1.hmod

SHA-256: `55de2924b63be5faacc6e3e94b2ce5bdd81d52a605c8fe1dd4589d85339397ff`.

Замечаний к записям нет.


### createdeco-2.1.1_1.20.1_fabric-batch2.1.hmod

SHA-256: `32e4698bb5bce4e494c5ac1fa10c380ba4a9269cc2b12f2f22ea9b6b4410000b`.

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

SHA-256: `80189ebf2c993dd64a78c6cff8f5a30abece26227f8cba26e19957fd5a734342`.

Замечаний к записям нет.


### dyedend-1.20.1_1.0.0-horizon.3.hmod

SHA-256: `a52be0d35f52c87ce5133f9aeb049cdc604ad1d6f1be35924c5b9aba25b91ec8`.

Замечаний к записям нет.


### echoes_of_the_end__structures_-12.00.13-horizon.3.hmod

SHA-256: `9c044bfddc163b5add7c90436ea948f9b787cb69980537561d98a9274afd47d5`.

Вкладок нет; записей для проверки нет.


### ends_delight-refabricated_1.20.1_2.5-horizon.3.hmod

SHA-256: `3921e2f34f9d2dcd86d98f862617b338fd1d72d0c1af8e523fb11d03ca0200a5`.

- `ends_delight:dragon_egg_shell` — missing_texture: assets/ends_delight/textures/item/dragon_egg_shell.png.
- `ends_delight:ender_sauce` — missing_texture: assets/ends_delight/textures/item/ender_sauce.png.
- `ends_delight:liquid_dragon_egg_in_shell` — missing_texture: assets/ends_delight/textures/item/liquid_dragon_egg_in_shell.png.

### expandeddelight-0.3.2-batch1.1.hmod

SHA-256: `1badee7d2ec7002e3f2648ab7ba2a6dc40083e4d16e8f887b9adee8ebdc9d5cd`.

- `expandeddelight:cinnamon_log` — missing_texture: assets/farmersdelight/textures/block/cinnamon_log_bottom.png.

### fairylights-1.1.5_fabric-horizon.3.hmod

SHA-256: `91daa2f9e7e29db9f5d7603174587d1410acc8d64ed42eb3d42b9ff90f007601`.

Замечаний к записям нет.


### farmersdelight-2.4.1-horizon.10.hmod

SHA-256: `b153b0caf1cb8595d4d528f63edccc2dcbd0023e18a5af91cca9cbd222b4b1ac`.

Замечаний к записям нет.


### farmersknives-3.2-batch1.1.hmod

SHA-256: `53d9bba57511ac52c86a03bfc9fc6ccea08f63cc96454059aae4b0e8192877ad`.

Замечаний к записям нет.


### froglightsreimagined-1.0-horizon.3.hmod

SHA-256: `6d7555269a8469c9a1d988a0d441100dd80f09b25d69c724e4a0fecdfb71b83c`.

Замечаний к записям нет.


### geophilic-3.7-horizon.3.hmod

SHA-256: `436d4a7a9be38ed3eb8dfcaa2daa79fc5b7982ad366b2ab261efb3ea5314b971`.

Вкладок нет; записей для проверки нет.


### homestead_sound-0.1.0-homestead.1.hmod

SHA-256: `c46d274544aab414af47f393da622543077483e964042211be0e19a3c76b9a30`.

Вкладок нет; записей для проверки нет.


### incendium-5.3.5-horizon.3.hmod

SHA-256: `cd4566bae88a5c1be041390924c1ddace5145413344a2d22ed6711369733d2b8`.

Вкладок нет; записей для проверки нет.


### justoutdoorstuffs-1.0.2-horizon.3.hmod

SHA-256: `fde05cc0991e14ae8f77b4dc738376d724a5490e9938c7ef4ac357abc457709e`.

Замечаний к записям нет.


### letsdoaddonstructures-1.7.2-horizon.3.hmod

SHA-256: `6f15149213e55429c1f1ceda86c8a9e5631d80ab0a61ffea1a55b3183c41daba`.

Вкладок нет; записей для проверки нет.


### luphieclutteredmod-2.1.0_1.20.1-horizon.3.hmod

SHA-256: `bf4624fc184ad847830e491a1edce5ad9338b851211f97e5ff02e1f5d0afb1d7`.

Замечаний к записям нет.


### mcaccel-1.0.0-batch2.1.hmod

SHA-256: `ab6bfac87fd2e1cf72359dcbf9832ac783f09d03c5ba3b310c0c4af7eea6dc79`.

Замечаний к записям нет.


### mcwholidays-1.1.2-horizon.3.hmod

SHA-256: `b30f5c291d8162c0e822dd8dce6069bdd63e4a4e4383ddf75e4b754f6e0ce540`.

Замечаний к записям нет.


### mcwpaintings-1.1.0-horizon.3.hmod

SHA-256: `543681ef17d449edb5a74faa1b24fccbde3227956a4498d90b85e01c72e8df3c`.

Вкладок нет; записей для проверки нет.


### mcwpaths-1.1.2-horizon.3.hmod

SHA-256: `78db44f9f64ce600e0d53cfe4604064fa35f2e4b8057fe0f36a7768e0030405d`.

Замечаний к записям нет.


### mo_glass-1.7_MC1.20.1-batch1.1.hmod

SHA-256: `6a4c108249ef5e606d8ca51467ffee3ae1097eff401170e06c929a225b140763`.

Замечаний к записям нет.


### modrinth_logo_mod-1.3.4-horizon.3.hmod

SHA-256: `3c577fe48c50a09d722e2bbb52b0919192d831030ff252084c24fadc950a1802`.

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

SHA-256: `a984a635c9fbdbae49118a967fc5f0e752b00fb6cad7c7b8cf561f3db48a66b2`.

Замечаний к записям нет.


### nethersdelight-1.20.1_4.1.1-batch1.1.hmod

SHA-256: `f1b42f89c4d933cd91c46843351f41b5c9ceed983f535aa37350dabba22914b5`.

Замечаний к записям нет.


### obsidianequipmentrework-1.1-batch2.1.hmod

SHA-256: `bda0db0741e82cc64d697465785ea43eb56e0234eb9f4d116fb37f54fd222a43`.

- `obsidianequipmentrework:obsidian_spear` — branch_missing_model: assets/obsidianequipmentrework/models/item/obsidian_spear_in_hand.json.

### oceansdelight-fdrf_fabric_1.0.2_1.20-batch2.1.hmod

SHA-256: `89561bed32ec36a2a6554cf187c33830d933cc5ecbd927b61f50e763f4d30e35`.

Замечаний к записям нет.


### perfectplushies-1.13.3-horizon.3.hmod

SHA-256: `1b8ae456b15532df0943f8fbdfad1846ccb8ea97295591670cdfd930b8df570f`.

- `perfectplushies:hedgehog_plushie` — missing_texture: assets/minecraft/textures/1.png.

### phomesteadplushies-1.1.0-batch1.1.hmod

SHA-256: `7b02a472aea1d640cddf6efbcae745d4b6ea86b6239f777387a0e8bd50e74658`.

Вкладок нет; записей для проверки нет.


### regions_unexplored-0.5.6_1.20.1-horizon.3.hmod

SHA-256: `93c9e7a227cd999e949fadee0ce10672bf5551037f835cad42dff37fd7ea5037`.

- `regions_unexplored:cooked_meat` — missing_texture: assets/regions_unexplored/textures/item/cooked_meat.png.
- `regions_unexplored:raw_meat` — missing_texture: assets/regions_unexplored/textures/item/raw_meat.png.
- `regions_unexplored:ruby` — missing_texture: assets/regions_unexplored/textures/item/ruby.png.
- `regions_unexplored:seeded_grass` — missing_texture: assets/regions_unexplored/textures/block/seeded_grass.png.
- `regions_unexplored:seeded_tall_grass` — missing_texture: assets/regions_unexplored/textures/block/seeded_tall_grass_top.png.
- `regions_unexplored:brown_wall_mushroom` — missing_texture: assets/regions_unexplored/textures/item/brown_wall_mushroom.png.
- `regions_unexplored:yellow_wall_bioshroom` — missing_texture: assets/regions_unexplored/textures/item/yellow_wall_bioshroom.png.

### repurposed_structures-7.1.25_1.20.1_fabric-horizon.3.hmod

SHA-256: `1a362672ec0ef99db23ac7555d959b548c9a1542e8616746ca94d859690c3031`.

Вкладок нет; записей для проверки нет.


### sootychimneys-1.3.4-batch2.1.hmod

SHA-256: `d1412717d71a43cf06ae0e06b69c1949d53e5da6c13157c0e8315520e05b388d`.

- `sootychimneys:dirty_cobblestone_chimney` — texture_alias: sootychimneys:item/dirty_cobblestone_chimney#down; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#east; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#north; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#south; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#up; texture_alias: sootychimneys:item/dirty_cobblestone_chimney#west.

### spuds_shops-1.9.0-batch1.1.hmod

SHA-256: `ca4aa60c0e34ca3dc0678ed9eff5bcc3f3cfc6dc0f4e88f086163ce03e9e5a77`.

Вкладок нет; записей для проверки нет.


### suppsquared-1.20_1.1.29-horizon.3.hmod

SHA-256: `d0fd6448fd24376712c0a3655448b6344ce7fa31675f197559558ea8cea091c2`.

- `suppsquared:daub_frame_slab` — missing_texture: assets/supplementaries/textures/block/daub_frame.png.
- `suppsquared:daub_frame_stairs` — missing_texture: assets/supplementaries/textures/block/daub_frame.png.
- `suppsquared:daub_slab` — missing_texture: assets/supplementaries/textures/block/daub.png.
- `suppsquared:daub_stairs` — missing_texture: assets/supplementaries/textures/block/daub.png.
- `suppsquared:item_shelf_suppsquared_shelf` — missing_texture: assets/suppsquared/textures/item/item_shelves/suppsquared_shelf.png.

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
