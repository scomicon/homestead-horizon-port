"""Farmer's Delight (MIT, Refabricated 2.4.1) crops, soil and compost declarations.

`horizon-port-pack carry` keeps the crop blockstates, models, loot tables and recipes but writes
the crop blocks as plain shells (wheat or stone templates, no state properties), skips the
`rice`, `tomatoes`, `rich_soil_farmland` and `organic_compost` blocks and the `rice` item, and
leaves the seed items as sticks. `patch(out)` gives them back the shapes the carried blockstates
and loot tables are written for:

* `cabbages` and `onions` stay wheat-template crops (age 0-7, the shells the carry wrote),
* `budding_tomatoes` (age 0-3) and `tomatoes` (age 0-3 plus `ropelogged`) use the beetroot
  template, `rice_panicles` (age 0-3) too; their ground check is the vanilla crop rule,
* `rice` is a seagrass-template water plant (age 0-3, `supporting`); its growth, and the panicles
  it puts on top, are native (`mixins/homestead/crops.rs`),
* `rich_soil_farmland` (moisture 0-7, 15/16 high) and `organic_compost` (composting 0-7) are
  dirt-template blocks that tick randomly; the growth boost, the hoe and the composting are native,
* the wild crops become fern / seagrass plants,
* the seed items, the onion and the new `rice` item place their crop (`place_block`),
* `minecraft:supports_crops` gets rich soil farmland (and rice, the ground of the panicles) and
  `minecraft:mineable/shovel` gets the three soil blocks.

Recipes, advancements and worldgen are the carried ones. See docs/farmersdelight-crops.md for
what differs from the original.
"""
import json
from pathlib import Path


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def _read(path, default):
    if path.exists():
        return json.loads(path.read_text(encoding='utf-8'))
    return default


def _numbers(count):
    return {'type': 'enum', 'values': [str(i) for i in range(count)]}


BOOLEAN = {'type': 'boolean'}

# RichSoilFarmlandBlock: the farmland hitbox (15/16 high), a full block otherwise.
FARMLAND_SHAPE = [[0.0, 0.0, 0.0, 1.0, 15 / 16, 1.0]]

BLOCKS = {
    # BudSimple: the seed's block. The native tick turns the ripe bud into a vine (crops.rs).
    'budding_tomatoes': {'template': 'minecraft:beetroots', 'item': False},
    # The vine without the rope climbing: `ropelogged` exists only so the carried blockstate and
    # loot table (which test it) still match.
    'tomatoes': {
        'template': 'minecraft:beetroots',
        'item': False,
        'state_properties': {'ropelogged': BOOLEAN},
        'default_state': {'ropelogged': 'false'},
    },
    # The upper part of a ripe rice plant; it stands on rice (minecraft:supports_crops below).
    'rice_panicles': {'template': 'minecraft:beetroots', 'item': False},
    'rice': {
        'template': 'minecraft:seagrass',
        'item': False,
        'properties': {'random_ticks': True, 'hardness': 0.0},
        'state_properties': {'age': _numbers(4), 'supporting': BOOLEAN},
        'default_state': {'age': '0', 'supporting': 'false'},
    },
    'rich_soil': {
        'template': 'minecraft:dirt',
        'properties': {'hardness': 0.5, 'sounds': 'minecraft:rooted_dirt', 'random_ticks': True},
    },
    'rich_soil_farmland': {
        'template': 'minecraft:dirt',
        'item': False,
        'properties': {'hardness': 0.6, 'sounds': 'minecraft:rooted_dirt', 'random_ticks': True},
        'state_properties': {'moisture': _numbers(8)},
        'default_state': {'moisture': '7'},
        'shapes': {'default': FARMLAND_SHAPE},
        'collision_shapes': {'default': FARMLAND_SHAPE},
    },
    'organic_compost': {
        'template': 'minecraft:dirt',
        'properties': {'hardness': 0.6, 'sounds': 'minecraft:rooted_dirt', 'random_ticks': True},
        'state_properties': {'composting': _numbers(8)},
        'default_state': {'composting': '0'},
    },
    'wild_cabbages': {'template': 'minecraft:fern', 'properties': {'hardness': 0.0}},
    'wild_onions': {'template': 'minecraft:fern', 'properties': {'hardness': 0.0}},
    'wild_tomatoes': {'template': 'minecraft:fern', 'properties': {'hardness': 0.0}},
    # The original is a two block tall plant; one cell keeps the `lower` half.
    'wild_rice': {
        'template': 'minecraft:seagrass',
        'properties': {'hardness': 0.0},
        'state_properties': {'half': {'type': 'enum', 'values': ['lower', 'upper']}},
        'default_state': {'half': 'lower'},
    },
}

# Items that plant a crop (`ItemNameBlockItem`): item -> block. `rice` is new, the rest exist.
PLANTERS = {
    'cabbage_seeds': 'farmersdelight:cabbages',
    'tomato_seeds': 'farmersdelight:budding_tomatoes',
    'onion': 'farmersdelight:onions',
    'rice': 'farmersdelight:rice',
}

# Block tags the mod adds to (a mod may add to vanilla's tags).
TAGS = {
    'supports_crops': ['farmersdelight:rich_soil_farmland', 'farmersdelight:rice'],
    'mineable/shovel': ['farmersdelight:rich_soil', 'farmersdelight:rich_soil_farmland',
                        'farmersdelight:organic_compost'],
}

# The carried wild rice loot tests `half` of a two block plant; shears keep the plant, anything
# else gets one rice.
WILD_RICE_LOOT = {
    'pools': [{
        'entries': [{
            'children': [
                {'condition': {'predicate': {'tag': 'c:shears'}, 'type': 'minecraft:match_tool'},
                 'name': 'farmersdelight:wild_rice', 'type': 'minecraft:item'},
                {'condition': {'type': 'minecraft:survives_explosion'},
                 'name': 'farmersdelight:rice', 'type': 'minecraft:item'}],
            'type': 'minecraft:alternatives'}],
        'rolls': 1.0}],
    'type': 'minecraft:block',
}


def patch(out):
    out = Path(out)
    data = out / 'data/farmersdelight'
    for name, block in BLOCKS.items():
        _write(data / f'horizon/block/{name}.json', block)
    for name, block in PLANTERS.items():
        path = data / f'horizon/item/{name}.json'
        item = _read(path, {'template': 'minecraft:stick'})
        item['place_block'] = block
        _write(path, item)
    _write(data / 'loot_table/blocks/wild_rice.json', WILD_RICE_LOOT)
    for tag, values in TAGS.items():
        path = out / f'data/minecraft/tags/block/{tag}.json'
        current = _read(path, {'values': []})
        current['values'] = sorted(set(current.get('values', [])) | set(values))
        _write(path, current)
    lang = out / 'assets/farmersdelight/lang/en_us.json'
    if lang.exists():
        text = json.loads(lang.read_text(encoding='utf-8'))
        text.setdefault('item.farmersdelight.rice', 'Rice')
        lang.write_text(json.dumps(text, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(f'crops: {len(BLOCKS)} blocks, {len(PLANTERS)} planting items, {len(TAGS)} tags')
