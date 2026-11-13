"""Packager, Package items and Factory Gauge: blocks, items, recipes and fallback artwork.

The block entities, packing rules and gauge monitor belong to the runtime
(`mixins/create/packager_runtime.rs`, `package_runtime.rs`, `gauge_runtime.rs`; rules in
`engine/src/core/packager.rs`, `package.rs`, `factory_gauge.rs`). Models are boxes authored
for this port; nothing is copied from the original artwork. Shape numbers and the two
recipes are estimates, not read from the pinned source (see docs/logistics.md).

Everything is emitted by `logistics(write, model, content)`, which content.py calls once
after `foundation()` (the recipe clean-up and the pickaxe tag are written inside it), and
`logistics_lang(content)`, which merges the labels into the lang files foundation wrote.
Nothing here needs catalog.BLOCKS; add the optional catalog lines from
mixins/create/fragments/logistics.md only to let the JAR importer replace the models.
"""
import json

from fallback_textures import material

PACKAGER = 'packager'
GAUGE = 'factory_gauge'
PACKAGES = ('cardboard_package_12x12', 'cardboard_package_10x12',
            'cardboard_package_10x8', 'cardboard_package_12x10')
FACES = ('down', 'up', 'north', 'south', 'west', 'east')
# Same rotation table as the other directional blocks (models drawn for `up`).
TURNS = {'down': {'x': 180}, 'up': {}, 'north': {'x': 90}, 'south': {'x': 90, 'y': 180},
         'west': {'x': 90, 'y': 270}, 'east': {'x': 90, 'y': 90}}
# Gauge plate (3 px thick) hugging the face opposite to `facing`, in blocks.
PLATES = {
    'up': (2, 0, 2, 14, 3, 14), 'down': (2, 13, 2, 14, 16, 14),
    'north': (2, 2, 13, 14, 14, 16), 'south': (2, 2, 0, 14, 14, 3),
    'west': (13, 2, 2, 16, 14, 14), 'east': (0, 2, 2, 3, 14, 14),
}
BODY = (2 / 16, 2 / 16, 2 / 16, 14 / 16, 14 / 16, 14 / 16)

LANG_EN = {
    'block.create.' + PACKAGER: 'Packager',
    'block.create.' + GAUGE: 'Factory Gauge',
    'itemGroup.create.logistics': 'Create: Logistics',
    **{'item.create.' + name: 'Package' for name in PACKAGES},
}
LANG_RU = {
    'block.create.' + PACKAGER: 'Упаковщик',
    'block.create.' + GAUGE: 'Фабричный датчик',
    'itemGroup.create.logistics': 'Create: Логистика',
    **{'item.create.' + name: 'Посылка' for name in PACKAGES},
}


def _loot(write, name):
    write(f'data/create/loot_table/blocks/{name}.json', {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'conditions': [{'condition': 'minecraft:survives_explosion'}],
            'entries': [{'type': 'minecraft:item', 'name': f'create:{name}'}],
            'rolls': 1.0,
        }],
        'random_sequence': f'create:blocks/{name}',
    })


def _item_files(write, name):
    write(f'assets/create/models/item/{name}.json', {'parent': f'create:block/{name}/item'})
    write(f'assets/create/items/{name}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{name}'},
    })


def packager(write, model):
    shapes = {'default': [list(BODY)]}
    write(f'data/create/horizon/block/{PACKAGER}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': 2.0,
            'explosion_resistance': 3.0,
            'requires_tool': True,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:copper_block',
        },
        'state_properties': {'facing': {'type': 'enum', 'values': list(FACES)}},
        'default_state': {'facing': 'up'},
        'shapes': shapes,
        'collision_shapes': shapes,
    })
    brass = material('brass_casing')
    parts = [([2, 2, 2], [14, 14, 14]), ([4, 14, 4], [12, 16, 12], 'tray')]
    model(f'{PACKAGER}/block', parts, brass, parts={'tray': material('pump_rim')})
    model(f'{PACKAGER}/item', parts, brass, parts={'tray': material('pump_rim')})
    write(f'assets/create/blockstates/{PACKAGER}.json', {'variants': {
        f'facing={face}': {'model': f'create:block/{PACKAGER}/block', **turn}
        for face, turn in TURNS.items()
    }})
    _item_files(write, PACKAGER)
    _loot(write, PACKAGER)


def gauge(write, model):
    shapes = {'property': 'facing', 'values': {face: [[v / 16 for v in plate]] for face, plate in PLATES.items()}}
    write(f'data/create/horizon/block/{GAUGE}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': 1.0,
            'explosion_resistance': 3.0,
            'requires_tool': False,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:copper_block',
        },
        'state_properties': {
            'facing': {'type': 'enum', 'values': list(FACES)},
            'satisfied': {'type': 'boolean'},
        },
        'default_state': {'facing': 'up', 'satisfied': 'false'},
        'shapes': shapes,
        'collision_shapes': shapes,
    })
    base = [([2, 0, 2], [14, 3, 14]), ([5, 3, 5], [11, 4, 11], 'lamp')]
    model(f'{GAUGE}/block', base, material('brass_casing'), parts={'lamp': material('chain_indicator')})
    model(f'{GAUGE}/lit', base, material('brass_casing'), parts={'lamp': material('chain_indicator_powered')})
    model(f'{GAUGE}/item', base, material('brass_casing'), parts={'lamp': material('chain_indicator')})
    variants = {}
    for face, turn in TURNS.items():
        for lit in ('false', 'true'):
            part = 'lit' if lit == 'true' else 'block'
            variants[f'facing={face},satisfied={lit}'] = {'model': f'create:block/{GAUGE}/{part}', **turn}
    write(f'assets/create/blockstates/{GAUGE}.json', {'variants': variants})
    _item_files(write, GAUGE)
    _loot(write, GAUGE)


def packages(write):
    for name in PACKAGES:
        write(f'data/create/horizon/item/{name}.json', {'template': 'minecraft:iron_ingot', 'max_stack_size': 1})
        write(f'assets/create/models/item/{name}.json',
              {'parent': 'minecraft:item/generated', 'textures': {'layer0': 'minecraft:item/bundle'}})
        write(f'assets/create/items/{name}.json', {
            'model': {'type': 'minecraft:model', 'model': f'create:item/{name}'},
        })


def recipes(write):
    # Placeholders: the pinned recipes were not read. Packager = paper over a brass casing,
    # Factory Gauge = electron tube over a brass casing.
    write(f'data/create/recipe/crafting/logistics/{PACKAGER}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'misc',
        'key': {'P': 'minecraft:paper', 'C': 'create:brass_casing'},
        'pattern': ['P', 'C'],
        'result': {'id': f'create:{PACKAGER}'},
        'show_notification': True,
    })
    write(f'data/create/recipe/crafting/logistics/{GAUGE}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'redstone',
        'key': {'E': 'create:electron_tube', 'C': 'create:brass_casing'},
        'pattern': ['E', 'C'],
        'result': {'id': f'create:{GAUGE}', 'count': 2},
        'show_notification': True,
    })


def logistics(write, model, content):
    packager(write, model)
    gauge(write, model)
    packages(write)
    recipes(write)
    write('data/create/horizon/creative_tab/logistics.json', {
        'title': 'itemGroup.create.logistics',
        'icon': f'create:{PACKAGER}',
        'items': [f'create:{PACKAGER}', f'create:{GAUGE}', f'create:{PACKAGES[0]}'],
    })
    tag = content / 'data/minecraft/tags/block/mineable/pickaxe.json'
    if tag.exists():
        value = json.loads(tag.read_text(encoding='utf-8'))
        values = value.setdefault('values', [])
        for name in (PACKAGER,):
            if f'create:{name}' not in values:
                values.append(f'create:{name}')
        write('data/minecraft/tags/block/mineable/pickaxe.json', value)


def logistics_lang(content):
    for locale, labels in (('en_us', LANG_EN), ('ru_ru', LANG_RU)):
        path = content / f'assets/create/lang/{locale}.json'
        lang = json.loads(path.read_text(encoding='utf-8'))
        lang.update(labels)
        path.write_text(json.dumps(lang, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
