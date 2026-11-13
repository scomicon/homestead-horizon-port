"""Farmer's Delight (MIT, Refabricated 2.4.1 / vectorwing 1.20 values) Skillet declarations.

`horizon-port-pack carry` keeps the Skillet's blockstate, models, sounds and loot table but
skips its block and item declarations. `patch(out)` adds them to the overlay directory:

* the block `farmersdelight:skillet` (`facing`, `support`; no block item of its own),
* the item `farmersdelight:skillet`, an iron-tier weapon (`SkilletItem`: durability 250, attack
  5.0 + 2.0, speed -3.1, knockback +1) that places the block (`place_block`),
* a plain loot table (the carried one names the `farmersdelight:copy_skillet` function),
* the shaped recipe, and the item's name.

The block entity, cooking, right click and placement state are native (`mixins/homestead/skillet.rs`).
"""
import json
from pathlib import Path

BLOCK = 'farmersdelight:skillet'

# SkilletBlock.SHAPE = Block.box(1, 0, 1, 15, 4, 15). SHAPE_WITH_TRAY adds a slab at y -1..0, below
# the block; a box cannot be declared there, so both states use the pan.
PAN = [[1 / 16, 0.0, 1 / 16, 15 / 16, 4 / 16, 15 / 16]]

# SkilletItem: Tiers.IRON (250 uses, enchantability 14, iron ingot), 5.0F + attack bonus 2.0,
# attack speed -3.1, knockback +1. The item's tool component is dropped (not a tool).
ITEM = {
    'template': 'minecraft:iron_sword',
    'max_stack_size': 1,
    'place_block': BLOCK,
    'components': {
        'minecraft:max_damage': 250,
        'minecraft:damage': 0,
        'minecraft:enchantable': {'value': 14},
        'minecraft:repairable': {'items': 'minecraft:iron_ingot'},
        '!minecraft:tool': {},
        'minecraft:attribute_modifiers': [
            {'type': 'minecraft:attack_damage', 'id': 'minecraft:base_attack_damage',
             'amount': 7.0, 'operation': 'add_value', 'slot': 'mainhand'},
            {'type': 'minecraft:attack_speed', 'id': 'minecraft:base_attack_speed',
             'amount': -3.1, 'operation': 'add_value', 'slot': 'mainhand'},
            {'type': 'minecraft:attack_knockback', 'id': 'farmersdelight:skillet_knockback',
             'amount': 1.0, 'operation': 'add_value', 'slot': 'mainhand'}],
    },
}


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def patch(out):
    out = Path(out)
    data = out / 'data/farmersdelight'
    _write(data / 'horizon/block/skillet.json', {
        'template': 'minecraft:glass',
        'item': False,
        'properties': {
            'hardness': 0.5,
            'explosion_resistance': 6.0,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:lantern',
        },
        'state_properties': {
            'facing': {'type': 'enum', 'values': ['north', 'south', 'west', 'east']},
            'support': {'type': 'boolean'},
        },
        'default_state': {'facing': 'north', 'support': 'false'},
        'shapes': {'default': PAN},
        'collision_shapes': {'default': PAN},
    })
    _write(data / 'horizon/item/skillet.json', ITEM)
    _write(data / 'loot_table/blocks/skillet.json', {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'condition': {'type': 'minecraft:survives_explosion'},
            'entries': [{'type': 'minecraft:item', 'name': BLOCK}],
            'rolls': 1.0,
        }],
        'random_sequence': 'farmersdelight:blocks/skillet',
    })
    recipe = data / 'recipe/skillet.json'
    if not recipe.exists():
        # The carry leaves it outside; same shape as the carried recipes (the adapter converts it).
        _write(recipe, {
            'type': 'minecraft:crafting_shaped',
            'category': 'misc',
            'key': {'#': {'tag': 'c:iron_ingots'}, '/': {'item': 'minecraft:brick'}},
            'pattern': [' ##', ' ##', '/  '],
            'result': {'item': BLOCK},
            'show_notification': True,
        })
    lang = out / 'assets/farmersdelight/lang/en_us.json'
    if lang.exists():
        text = json.loads(lang.read_text(encoding='utf-8'))
        # The item is a standalone item (it has its own components), not a block item.
        text.setdefault('item.farmersdelight.skillet', text.get('block.farmersdelight.skillet', 'Skillet'))
        lang.write_text(json.dumps(text, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print('skillet: block, item, loot table, recipe')
