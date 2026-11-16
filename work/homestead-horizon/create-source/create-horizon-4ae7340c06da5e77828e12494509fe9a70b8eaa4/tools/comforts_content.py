# Copyright (c) 2026 create-horizon contributors
# SPDX-License-Identifier: MIT
"""Writes the Comforts content mod (sleeping bags and hammocks) as a `.hmod` tree.

Comforts (copyleft) adds portable beds: sleeping bags and hammocks in 16 colours that never set the respawn
point. The content here is rebuilt from the public description only: 16 sleeping bags
(`comforts:sleeping_bag_<colour>`) and 16 hammocks (`comforts:hammock_<colour>`) as blocks on the vanilla bed
template (two blocks, head and foot, right click to sleep), each with an item that places it, a placeholder
shaped recipe, a loot table that drops the item from the head, vanilla wool textures for the look (a flat slab
for the bag, a thin carpet for the hammock) and English and Russian names. The respawn rule is native
(`mixins/homestead/comforts.rs`, engine patch 20-sleep-host.ps1). Nothing is copied from the original mod.

    python comforts_content.py <out_dir>

The out dir is zipped flat into stage4/extra-mods by stage4/Build-Comforts.ps1.
"""
import json
import sys
from pathlib import Path

NS = 'comforts'
COLOURS = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray',
           'light_gray', 'cyan', 'purple', 'blue', 'brown', 'green', 'red', 'black']
EN_COLOUR = {c: c.replace('_', ' ').title() for c in COLOURS}
RU_COLOUR = {
    'white': 'белый', 'orange': 'оранжевый', 'magenta': 'пурпурный', 'light_blue': 'голубой',
    'yellow': 'жёлтый', 'lime': 'лаймовый', 'pink': 'розовый', 'gray': 'серый',
    'light_gray': 'светло-серый', 'cyan': 'бирюзовый', 'purple': 'фиолетовый', 'blue': 'синий',
    'brown': 'коричневый', 'green': 'зелёный', 'red': 'красный', 'black': 'чёрный',
}
KINDS = {
    'sleeping_bag': ('Sleeping Bag', 'Спальный мешок'),
    'hammock': ('Hammock', 'Гамак'),
}
FACINGS = ['north', 'south', 'east', 'west']


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, str):
        path.write_text(value, encoding='utf-8', newline='\n')
    else:
        path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')


def _recipe(kind, colour):
    wool = f'minecraft:{colour}_wool'
    if kind == 'sleeping_bag':
        # Placeholder: three wool over a leather; the original recipe was not read.
        return {'type': 'minecraft:crafting_shaped', 'category': 'misc',
                'key': {'W': {'item': wool}, 'L': {'item': 'minecraft:leather'}},
                'pattern': ['WWW', ' L '], 'result': {'item': f'{NS}:{kind}_{colour}'},
                'show_notification': True}
    # Placeholder: a row of three wool hung on two strings.
    return {'type': 'minecraft:crafting_shaped', 'category': 'misc',
            'key': {'W': {'item': wool}, 'S': {'item': 'minecraft:string'}},
            'pattern': ['S S', 'WWW'], 'result': {'item': f'{NS}:{kind}_{colour}'},
            'show_notification': True}


def build(out):
    out = Path(out)
    _write(out / 'horizon-mod.toml', (
        '# Written by tools/comforts_content.py for the native port of Comforts (copyleft; rebuilt from the\n'
        '# public description only). Local use.\n'
        f'id = "{NS}"\nname = "Comforts"\nversion = "0.1.0"\napi = "0.2"\nside = "both"\n'
        'capabilities = ["registries"]\n'
    ))
    data = out / 'data' / NS
    assets = out / 'assets' / NS
    lang_en = {f'itemGroup.{NS}': 'Comforts'}
    lang_ru = {f'itemGroup.{NS}': 'Comforts'}
    items = []
    for kind, (en_kind, ru_kind) in KINDS.items():
        for colour in COLOURS:
            ident = f'{kind}_{colour}'
            full = f'{NS}:{ident}'
            items.append(full)
            # The bed template gives the block its part/facing/occupied states, the two-block placement and
            # the sleeping behaviour (Class AbstractBed). No auto item: the item below places it.
            _write(data / f'horizon/block/{ident}.json', {
                'template': f'minecraft:{colour}_bed',
                'item': False,
                'properties': {
                    'hardness': 0.2,
                    'explosion_resistance': 0.2,
                    'requires_tool': False,
                    'occludes': False,
                    'render_shape': 'MODEL',
                    'sounds': f'minecraft:{colour}_wool',
                },
            })
            _write(data / f'horizon/item/{ident}.json', {
                'template': f'minecraft:{colour}_bed',
                'place_block': full,
                'max_stack_size': 1,
            })
            _write(data / f'recipe/{ident}.json', _recipe(kind, colour))
            _write(data / f'loot_table/blocks/{ident}.json', {
                'type': 'minecraft:block',
                'pools': [{
                    'rolls': 1.0, 'bonus_rolls': 0.0,
                    'entries': [{
                        'type': 'minecraft:item', 'name': full,
                        'conditions': [{'condition': 'minecraft:block_state_property', 'block': full,
                                        'properties': {'part': 'head'}}],
                    }],
                    'conditions': [{'condition': 'minecraft:survives_explosion'}],
                }],
                'random_sequence': f'{NS}:blocks/{ident}',
            })
            # Every combination of the bed template's states points at one model per colour.
            variants = {}
            for facing in FACINGS:
                for occupied in ('true', 'false'):
                    for part in ('head', 'foot'):
                        variants[f'facing={facing},occupied={occupied},part={part}'] = {'model': f'{NS}:block/{ident}'}
            _write(assets / f'blockstates/{ident}.json', {'variants': variants})
            wool = f'minecraft:block/{colour}_wool'
            if kind == 'sleeping_bag':
                _write(assets / f'models/block/{ident}.json', {
                    'parent': 'minecraft:block/slab',
                    'textures': {'bottom': wool, 'top': wool, 'side': wool},
                })
            else:
                _write(assets / f'models/block/{ident}.json', {
                    'parent': 'minecraft:block/carpet',
                    'textures': {'wool': wool},
                })
            _write(assets / f'items/{ident}.json', {'model': {'type': 'minecraft:model', 'model': f'{NS}:item/{ident}'}})
            _write(assets / f'models/item/{ident}.json', {
                'parent': 'minecraft:item/generated', 'textures': {'layer0': wool}})
            lang_en[f'block.{NS}.{ident}'] = f'{EN_COLOUR[colour]} {en_kind}'
            lang_en[f'item.{NS}.{ident}'] = f'{EN_COLOUR[colour]} {en_kind}'
            lang_ru[f'block.{NS}.{ident}'] = f'{ru_kind} ({RU_COLOUR[colour]})'
            lang_ru[f'item.{NS}.{ident}'] = f'{ru_kind} ({RU_COLOUR[colour]})'
    _write(data / f'horizon/creative_tab/{NS}.json', {
        'title': f'itemGroup.{NS}', 'icon': items[0], 'items': items})
    _write(assets / 'lang/en_us.json', lang_en)
    _write(assets / 'lang/ru_ru.json', lang_ru)
    print(f'comforts: {len(items)} blocks and items, recipes, loot tables, models, names')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    build(sys.argv[1])
