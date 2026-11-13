"""Farmer's Delight (MIT, Refabricated 2.4.1 / vectorwing 1.20 values) feasts, pies and drinks.

`horizon-port-pack carry` keeps the blockstates, models and (for the feasts) loot tables of the
feast blocks and pies but skips their block declarations, their items and the crafting recipes.
`patch(out)` adds them to the overlay directory:

* five feast blocks (`roast_chicken_block`, `stuffed_pumpkin_block`, `honey_glazed_ham_block`,
  `shepherds_pie_block`, `rice_roll_medley_block`) with `facing` and `servings`, each with an item
  that places it (stack size 1: `PlaceableItem` with `basicItem().stacksTo(1)`),
* three pie blocks (`apple_pie`, `chocolate_pie`, `sweet_berry_cheesecake`) with `facing` and
  `bites` and a plain loot table (the block drops itself),
* the shapeless/shaped crafting recipes of the feasts and pies,
* the `farmersdelight:comfort` mob effect (its tick is native, `mixins/homestead/comfort.rs`),
* the food and consumable components of the three drinks the first overlay left as shells:
  `milk_bottle`, `hot_cocoa`, `melon_juice`,
* the language entries for the new texts.

Serving, cutting and eating are native (`mixins/homestead/feast.rs`).
"""
import json
from pathlib import Path

FACING = {'type': 'enum', 'values': ['north', 'south', 'west', 'east']}

# FeastBlock / PieBlock shapes are per-state box unions; a single box per block is declared.
FEAST_BOX = [[1 / 16, 0.0, 1 / 16, 15 / 16, 7 / 16, 15 / 16]]
PIE_BOX = [[2 / 16, 0.0, 2 / 16, 14 / 16, 4 / 16, 14 / 16]]

# name -> (max servings, container-less)
FEASTS = {
    'roast_chicken_block': 4,
    'stuffed_pumpkin_block': 4,
    'honey_glazed_ham_block': 4,
    'shepherds_pie_block': 4,
    'rice_roll_medley_block': 8,
}
PIES = ('apple_pie', 'chocolate_pie', 'sweet_berry_cheesecake')

# Harmful effects a Hot Cocoa drink clears (HotCocoaItem: one random harmful, milk-curable effect;
# a consume effect removes all listed ones, so this is an approximation).
HARMFUL = ['minecraft:poison', 'minecraft:nausea', 'minecraft:blindness', 'minecraft:hunger',
           'minecraft:slowness', 'minecraft:weakness', 'minecraft:mining_fatigue',
           'minecraft:wither', 'minecraft:darkness']

# Crafting recipes (generated 1.20 data, Forge tags replaced by the items they hold here:
# onion -> farmersdelight:onion, carrot -> minecraft:carrot, eggs -> minecraft:egg,
# cooked mutton -> minecraft:cooked_mutton, milk -> minecraft:milk_bucket, wheat -> minecraft:wheat).
ONION, EGG, CARROT = 'farmersdelight:onion', 'minecraft:egg', 'minecraft:carrot'
SHAPELESS = {
    'roast_chicken_block': [ONION, EGG, 'minecraft:bread', CARROT, 'minecraft:cooked_chicken',
                            'minecraft:baked_potato', CARROT, 'minecraft:bowl', 'minecraft:baked_potato'],
    'honey_glazed_ham_block': ['minecraft:sweet_berries', 'minecraft:honey_bottle', 'minecraft:sweet_berries',
                               'minecraft:sweet_berries', 'farmersdelight:smoked_ham', 'minecraft:sweet_berries',
                               'farmersdelight:cooked_rice', 'minecraft:bowl', 'farmersdelight:cooked_rice'],
    'shepherds_pie_block': ['minecraft:baked_potato', 'minecraft:milk_bucket', 'minecraft:baked_potato',
                            'minecraft:cooked_mutton', 'minecraft:cooked_mutton', 'minecraft:cooked_mutton',
                            ONION, 'minecraft:bowl', ONION],
    'rice_roll_medley_block': ['farmersdelight:kelp_roll_slice', 'farmersdelight:kelp_roll_slice',
                               'farmersdelight:kelp_roll_slice', 'farmersdelight:salmon_roll',
                               'farmersdelight:salmon_roll', 'farmersdelight:salmon_roll',
                               'farmersdelight:cod_roll', 'minecraft:bowl', 'farmersdelight:cod_roll'],
    # The 1.20 recipe could not be fetched (no file of that name); an estimate with the same shape
    # (eight ingredients and the bowl around a pumpkin).
    'stuffed_pumpkin_block': ['minecraft:pumpkin', 'farmersdelight:cooked_rice', 'minecraft:brown_mushroom',
                              'farmersdelight:cooked_rice', 'farmersdelight:cooked_bacon', 'minecraft:bowl',
                              'minecraft:brown_mushroom', 'farmersdelight:cooked_bacon', ONION],
}
SHAPED = {
    'apple_pie': (['###', 'aaa', 'xOx'], {'#': 'minecraft:wheat', 'O': 'farmersdelight:pie_crust',
                                           'a': 'minecraft:apple', 'x': 'minecraft:sugar'}),
    'chocolate_pie': (['ccc', 'mmm', 'xOx'], {'c': 'minecraft:cocoa_beans', 'm': 'minecraft:milk_bucket',
                                               'x': 'minecraft:sugar', 'O': 'farmersdelight:pie_crust'}),
    'sweet_berry_cheesecake': (['sss', 'sss', 'mOm'], {'O': 'farmersdelight:pie_crust',
                                                        'm': 'minecraft:milk_bucket',
                                                        's': 'minecraft:sweet_berries'}),
}

LANG = {
    'farmersdelight.block.feast.use_bowl': 'You need a Bowl to serve this.',
    'effect.farmersdelight.comfort': 'Comfort',
}


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def _block(path, states, box, item):
    _write(path, {
        'template': 'minecraft:glass',
        'item': item,
        'properties': {
            'hardness': 0.5,
            'explosion_resistance': 0.5,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:white_wool',
        },
        'state_properties': states,
        'default_state': {'facing': 'north', **{key: value for key, value in _defaults(states).items()}},
        'shapes': {'default': box},
        'collision_shapes': {'default': box},
    })


def _defaults(states):
    out = {}
    for key, spec in states.items():
        if key == 'facing':
            continue
        out[key] = spec['values'][-1] if key == 'servings' else spec['values'][0]
    return out


def _recipe_shapeless(data, name, ingredients):
    path = data / 'recipe' / f'{name}.json'
    if not path.exists():
        _write(path, {
            'type': 'minecraft:crafting_shapeless',
            'category': 'misc',
            'ingredients': [{'item': item} for item in ingredients],
            'result': {'item': f'farmersdelight:{name}'},
        })


def _recipe_shaped(data, name, pattern, key):
    path = data / 'recipe' / f'{name}.json'
    if not path.exists():
        _write(path, {
            'type': 'minecraft:crafting_shaped',
            'category': 'misc',
            'key': {symbol: {'item': item} for symbol, item in key.items()},
            'pattern': pattern,
            'result': {'item': f'farmersdelight:{name}'},
            'show_notification': True,
        })


def _drink(items, name, food, consume_effects, always=True):
    path = items / f'{name}.json'
    if not path.exists():
        return False
    declaration = json.loads(path.read_text(encoding='utf-8'))
    components = declaration.setdefault('components', {})
    components['minecraft:food'] = {'nutrition': food, 'saturation': 0.0, 'can_always_eat': always}
    consumable = {'consume_seconds': 1.6, 'animation': 'drink',
                  'sound': 'minecraft:entity.generic.drink', 'has_consume_particles': False}
    if consume_effects:
        consumable['on_consume_effects'] = consume_effects
    components['minecraft:consumable'] = consumable
    components.setdefault('minecraft:use_remainder', {'id': 'minecraft:glass_bottle', 'count': 1})
    declaration.setdefault('max_stack_size', 16)
    path.write_text(json.dumps(declaration, indent=2) + '\n', encoding='utf-8')
    return True


def patch(out):
    out = Path(out)
    data = out / 'data/farmersdelight'
    blocks = data / 'horizon/block'
    items = data / 'horizon/item'
    for name, maximum in FEASTS.items():
        states = {'facing': FACING,
                  'servings': {'type': 'enum', 'values': [str(i) for i in range(maximum + 1)]}}
        _block(blocks / f'{name}.json', states, FEAST_BOX, False)
        _write(items / f'{name}.json', {
            'template': 'minecraft:stick',
            'max_stack_size': 1,
            'place_block': f'farmersdelight:{name}',
        })
    for name in PIES:
        states = {'facing': FACING, 'bites': {'type': 'enum', 'values': ['0', '1', '2', '3']}}
        _block(blocks / f'{name}.json', states, PIE_BOX, True)
        _write(data / f'loot_table/blocks/{name}.json', {
            'type': 'minecraft:block',
            'pools': [{
                'bonus_rolls': 0.0,
                'condition': {'type': 'minecraft:survives_explosion'},
                'entries': [{'type': 'minecraft:item', 'name': f'farmersdelight:{name}'}],
                'rolls': 1.0,
            }],
            'random_sequence': f'farmersdelight:blocks/{name}',
        })
    for name, ingredients in SHAPELESS.items():
        _recipe_shapeless(data, name, ingredients)
    for name, (pattern, key) in SHAPED.items():
        _recipe_shaped(data, name, pattern, key)
    # Comfort: beneficial, the colour of the original (a warm tan); the tick is native.
    _write(data / 'horizon/mob_effect/comfort.json', {'category': 'beneficial', 'color': 0xFF9E6D})
    # DrinkableItem: 32 ticks, the drink animation. Milk Bottle clears effects (a random curable one
    # in the original; all of them here), Hot Cocoa clears harmful ones, Melon Juice heals 2 HP (a
    # 100-tick Regeneration here).
    drinks = [
        _drink(items, 'milk_bottle', 0, [{'type': 'minecraft:clear_all_effects'}]),
        _drink(items, 'hot_cocoa', 0, [{'type': 'minecraft:remove_effects', 'effects': HARMFUL}]),
        _drink(items, 'melon_juice', 0, [{
            'type': 'minecraft:apply_effects',
            'effects': [{'id': 'minecraft:regeneration', 'duration': 100, 'amplifier': 0,
                         'show_particles': True, 'show_icon': True}]}]),
    ]
    lang = out / 'assets/farmersdelight/lang/en_us.json'
    if lang.exists():
        text = json.loads(lang.read_text(encoding='utf-8'))
        for key, value in LANG.items():
            text.setdefault(key, value)
        lang.write_text(json.dumps(text, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(f'feasts: {len(FEASTS)} feasts, {len(PIES)} pies, {sum(drinks)} drinks, comfort effect')
