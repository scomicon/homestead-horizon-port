# Сторонние проекты и моды

Ни один из перечисленных ниже модов **не включён** в репозиторий: ни JAR, ни `.hmod`, ни извлечённые из них файлы. Ставьте их сами со страниц авторов (поиск по названию на Modrinth или CurseForge).

Лицензии взяты из меток Modrinth и `fabric.mod.json`. **Это метка, а не проверенный текст лицензии**: для многих модов она относится к коду, а ресурсы (текстуры, звуки, модели) могут иметь другую лицензию. Перед любым распространением проверяйте файлы `LICENSE` в самих модах.

## Проекты, из которых мы собираем

| Проект | Лицензия | Что нужно |
| --- | --- | --- |
| Horizon (клиент) | MIT, © 2026 Ludovic Marie и Horizon contributors | Сохранить уведомление в любой сборке с бинарником |
| horizon-mixins | MIT, © 2026 levzzz | То же |
| create-horizon | MIT, © The Create Team / The Creators of Create, © 2026 create-horizon contributors | Сохранить `LICENSE` и `THIRD_PARTY_NOTICES.md` |
| Create (код) | MIT | Уведомление об авторстве |
| Create (оригинальные ресурсы) | отдельная, «All Rights Reserved» по заявлению upstream | Не распространять; читать из своего JAR |
| Rust-крейты внутри клиента | разные (MIT, Apache-2.0, MPL и др.) | Если распространять бинарник, приложить их уведомления (`cargo about`). Не проверялось |

## Create и Farmer's Delight

| Мод | Метка лицензии | Замечание |
| --- | --- | --- |
| Create | MIT (код) | ресурсы отдельно, см. выше |
| Farmer's Delight (Refabricated) | MIT | лицензия ресурсов не проверялась |
| create_oxidized | MIT | не проверялась |

## Моды, которые можно упаковать как «данные» (оболочки без поведения)

| Метка | Моды |
| --- | --- |
| CC0-1.0 / Unlicense | betterendrod, froglightsreimagined, perfectplushies, blahaj |
| MIT | beautify, dustydecorations, dyedend, fairylights, justoutdoorstuffs, letsdoaddonstructures, mcwpaths, modrinth_logo_mod, ends_delight |
| MPL-2.0 (копилефт по файлам) | diagonalwalls, diagonalwindows, universalbonemeal |
| LGPL-3.0 | repurposed_structures, connectiblechains, betterdungeons, betterendisland, betterfortresses, bettermineshafts, betterstrongholds, yungsextras |
| Academic Free License 3.0 | luphieclutteredmod |
| CC BY-NC-SA 4.0 | mr_endercon, t_and_t |
| CC BY-NC-ND 4.0 (производные запрещены) | villagesandpillages |
| All Rights Reserved | alcocraftplus, darksmelting, echoes_of_the_end__structures_, mcwholidays, mcwpaintings, natures_delight, geophilic, regions_unexplored, mr_flower_tweaks, mr_mastercutter, mr_tidal_towns |
| Нестандартная / неизвестная | mr_typewriter_daycounter, bakery, candlelight, chipped (Terrarium Licence), incendium (Stardust Labs License), suppsquared (Supplementaries Team License) |

Если мод помечен «All Rights Reserved» или «ND», не собирайте из него `.hmod` для передачи другим и не публикуйте результат.

## Моды логики, которые мы переписали на Rust

Наш код реализует **поведение** этих модов, а не копирует их исходники. Исходники при этом могли не читаться: что именно делает мод, часто взято из описания и конфигов.

| Мод | Метка |
| --- | --- |
| rightclickharvest, securecrops, too-cheap, preferred-gamerules, crops_love_rain | MIT |
| betterdays | LGPL-3.0-or-later |

Если вы автор мода и считаете, что наш код слишком близок к вашему, откройте issue.
