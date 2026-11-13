"""Farmer's Delight (MIT, Refabricated 2.4.1) Cooking Pot declarations.

`horizon-port-pack carry` keeps the Cooking Pot's blockstate, models and loot table but skips
its block declaration and its recipe (the recipe sits outside the carried files). `patch(out)`
adds them to the overlay directory:

* the block `farmersdelight:cooking_pot` (`facing`, `support` none/tray/handle, `waterlogged`)
  with a block item,
* a plain loot table (the carried one names `farmersdelight:copy_meal`, an unknown function),
* the shaped recipe,
* the menu type farmersdelight:cooking_pot (horizon/menu/cooking_pot.json).

The block entity, cooking, serving and placement state are native (`mixins/homestead/cooking_pot.rs`).
"""
import json
from pathlib import Path

BLOCK = 'farmersdelight:cooking_pot'

# CookingPotBlock.SHAPE = Block.box(2, 0, 2, 14, 10, 14); the tray and handle variants add parts
# outside that box which a single box cannot express, so every state uses the pot body.
BODY = [[2 / 16, 0.0, 2 / 16, 14 / 16, 10 / 16, 14 / 16]]


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def patch(out):
    out = Path(out)
    data = out / 'data/farmersdelight'
    _write(data / 'horizon/block/cooking_pot.json', {
        # A barrel template: its use opens the block's mod menu (a glass template's use does nothing).
        'template': 'minecraft:barrel',
        'item': True,
        'properties': {
            'hardness': 2.0,
            'explosion_resistance': 6.0,
            'requires_tool': False,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:lantern',
        },
        'state_properties': {
            'support': {'type': 'enum', 'values': ['none', 'tray', 'handle']},
            'waterlogged': {'type': 'boolean'},
        },
        # acing and open come from the barrel template.
        'default_state': {'support': 'none', 'waterlogged': 'false'},
        'shapes': {'default': BODY},
        'collision_shapes': {'default': BODY},
    })
    # The menu type: nine slots (0-5 ingredients, 6 meal display (hidden by the screen), 7 container,
    # 8 output, take-only), three data slots (cook time, cook time total, heated).
    _write(data / 'horizon/menu/cooking_pot.json', {'slots': 9, 'data_slots': 3, 'results': [8]})
    _write(data / 'loot_table/blocks/cooking_pot.json', {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'condition': {'type': 'minecraft:survives_explosion'},
            'entries': [{'type': 'minecraft:item', 'name': BLOCK}],
            'rolls': 1.0,
        }],
        'random_sequence': 'farmersdelight:blocks/cooking_pot',
    })
    recipe = data / 'recipe/cooking_pot.json'
    if not recipe.exists():
        _write(recipe, {
            'type': 'minecraft:crafting_shaped',
            'category': 'misc',
            'key': {'S': {'item': 'minecraft:wooden_shovel'}, 'W': {'tag': 'c:buckets/water'},
                    'b': {'item': 'minecraft:brick'}, 'i': {'tag': 'c:iron_ingots'}},
            'pattern': ['bSb', 'iWi', 'iii'],
            'result': {'item': BLOCK},
            'show_notification': True,
        })
    print('cooking pot: block, menu type, loot table, recipe')