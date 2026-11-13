"""Stock Link, Stock Ticker, Package Frogport and Package Postbox: blocks, recipes and fallback artwork.

The block entities, the stock network and the port logic belong to the runtime
(`mixins/create/stock_link_runtime.rs`, `stock_ticker_runtime.rs`, `frogport_runtime.rs`; rules
in `engine/src/core/stock_network.rs` and `package_post.rs`). Models are boxes authored for this
port; nothing is copied from the original artwork. Shape numbers and the recipes are estimates,
not read from the pinned source (see docs/stock-network.md).

Everything is emitted by `stock(write, model, content)`, which content.py calls once after
`logistics(...)`, and `stock_lang(content)`, which merges the labels into the lang files.
"""
import json

from fallback_textures import material

LINK = 'stock_link'
TICKER = 'stock_ticker'
FROGPORT = 'package_frogport'
POSTBOX = 'package_postbox'
BLOCKS = (LINK, TICKER, FROGPORT, POSTBOX)
FACES = ('down', 'up', 'north', 'south', 'west', 'east')
# Same rotation table as the other directional blocks (models drawn for `up`).
TURNS = {'down': {'x': 180}, 'up': {}, 'north': {'x': 90}, 'south': {'x': 90, 'y': 180},
         'west': {'x': 90, 'y': 270}, 'east': {'x': 90, 'y': 90}}
# Link plate (3 px thick) hugging the face opposite to `facing`, in pixels.
PLATES = {
    'up': (3, 0, 3, 13, 3, 13), 'down': (3, 13, 3, 13, 16, 13),
    'north': (3, 3, 13, 13, 13, 16), 'south': (3, 3, 0, 13, 13, 3),
    'west': (13, 3, 3, 16, 13, 13), 'east': (0, 3, 3, 3, 13, 13),
}

LANG_EN = {
    'block.create.' + LINK: 'Stock Link',
    'block.create.' + TICKER: 'Stock Ticker',
    'block.create.' + FROGPORT: 'Package Frogport',
    'block.create.' + POSTBOX: 'Package Postbox',
    'itemGroup.create.stock': 'Create: Stock Network',
}
LANG_RU = {
    'block.create.' + LINK: 'Складская связь',
    'block.create.' + TICKER: 'Складской тикер',
    'block.create.' + FROGPORT: 'Посылочный порт «Лягушка»',
    'block.create.' + POSTBOX: 'Почтовый ящик',
    'itemGroup.create.stock': 'Create: Складская сеть',
}

PROPERTIES = {
    'render_shape': 'MODEL',
    'occludes': False,
    'sounds': 'minecraft:copper_block',
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


def link(write, model):
    shapes = {'property': 'facing', 'values': {face: [[v / 16 for v in plate]] for face, plate in PLATES.items()}}
    write(f'data/create/horizon/block/{LINK}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {'hardness': 1.0, 'explosion_resistance': 3.0, 'requires_tool': False, **PROPERTIES},
        'state_properties': {'facing': {'type': 'enum', 'values': list(FACES)}},
        'default_state': {'facing': 'up'},
        'shapes': shapes,
        'collision_shapes': shapes,
    })
    base = [([3, 0, 3], [13, 3, 13]), ([6, 3, 6], [10, 4, 10], 'lamp')]
    model(f'{LINK}/block', base, material('brass_casing'), parts={'lamp': material('chain_indicator')})
    model(f'{LINK}/item', base, material('brass_casing'), parts={'lamp': material('chain_indicator')})
    write(f'assets/create/blockstates/{LINK}.json', {'variants': {
        f'facing={face}': {'model': f'create:block/{LINK}/block', **turn} for face, turn in TURNS.items()
    }})
    _item_files(write, LINK)
    _loot(write, LINK)


def _plain(write, model, name, boxes, shape, hardness, parts):
    shapes = {'default': [[v / 16 for v in shape]]}
    write(f'data/create/horizon/block/{name}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {'hardness': hardness, 'explosion_resistance': 3.0, 'requires_tool': True, **PROPERTIES},
        'shapes': shapes,
        'collision_shapes': shapes,
    })
    model(f'{name}/block', boxes, material('brass_casing'), parts=parts)
    model(f'{name}/item', boxes, material('brass_casing'), parts=parts)
    write(f'assets/create/blockstates/{name}.json', {'variants': {'': {'model': f'create:block/{name}/block'}}})
    _item_files(write, name)
    _loot(write, name)


def ticker(write, model):
    boxes = [([1, 0, 1], [15, 3, 15]), ([3, 3, 3], [13, 11, 13]), ([4, 11, 4], [12, 15, 12], 'screen')]
    _plain(write, model, TICKER, boxes, (1, 0, 1, 15, 15, 15), 2.0, {'screen': material('chain_indicator')})


def frogport(write, model):
    boxes = [([3, 0, 3], [13, 5, 13]), ([4, 5, 4], [12, 11, 12], 'frog')]
    _plain(write, model, FROGPORT, boxes, (3, 0, 3, 13, 11, 13), 1.5, {'frog': material('pump_rim')})


def postbox(write, model):
    boxes = [([2, 0, 2], [14, 12, 14]), ([5, 12, 5], [11, 14, 11], 'slot')]
    _plain(write, model, POSTBOX, boxes, (2, 0, 2, 14, 14, 14), 1.5, {'slot': material('pump_rim')})


def recipes(write):
    # Placeholders: the pinned recipes were not read.
    def shaped(name, key, pattern, count=1):
        write(f'data/create/recipe/crafting/logistics/{name}.json', {
            'type': 'minecraft:crafting_shaped',
            'category': 'misc',
            'key': key,
            'pattern': pattern,
            'result': {'id': f'create:{name}', 'count': count},
            'show_notification': True,
        })
    shaped(LINK, {'E': 'create:electron_tube', 'R': 'minecraft:redstone', 'C': 'create:brass_casing'},
           ['E', 'R', 'C'])
    shaped(TICKER, {'G': 'minecraft:glass', 'L': f'create:{LINK}', 'C': 'create:brass_casing'},
           ['G', 'L', 'C'])
    shaped(FROGPORT, {'S': 'minecraft:slime_ball', 'K': 'create:packager'}, ['S', 'K'])
    shaped(POSTBOX, {'B': 'minecraft:chest', 'K': 'create:packager'}, ['B', 'K'])


def stock(write, model, content):
    link(write, model)
    ticker(write, model)
    frogport(write, model)
    postbox(write, model)
    recipes(write)
    write('data/create/horizon/creative_tab/stock.json', {
        'title': 'itemGroup.create.stock',
        'icon': f'create:{LINK}',
        'items': [f'create:{name}' for name in BLOCKS],
    })
    tag = content / 'data/minecraft/tags/block/mineable/pickaxe.json'
    if tag.exists():
        value = json.loads(tag.read_text(encoding='utf-8'))
        values = value.setdefault('values', [])
        for name in (TICKER, FROGPORT, POSTBOX):
            if f'create:{name}' not in values:
                values.append(f'create:{name}')
        write('data/minecraft/tags/block/mineable/pickaxe.json', value)


def stock_lang(content):
    for locale, labels in (('en_us', LANG_EN), ('ru_ru', LANG_RU)):
        path = content / f'assets/create/lang/{locale}.json'
        lang = json.loads(path.read_text(encoding='utf-8'))
        lang.update(labels)
        path.write_text(json.dumps(lang, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
