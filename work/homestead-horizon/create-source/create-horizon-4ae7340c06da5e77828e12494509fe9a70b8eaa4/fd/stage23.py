"""Farmer's Delight (MIT, Refabricated 2.4.1) stage 23: baskets, cabinets, rugs, ropes, mushroom colonies.

`horizon-port-pack carry` keeps the blockstates, models, loot tables, recipes and tags of these
blocks but writes the blocks themselves as shells (a pumpkin for the basket, a fence gate for the
cabinets, stone for the canvas rug, a wheat crop for the mushroom colonies) and leaves out `rope`,
`tatami` and `full_tatami_mat` because no template has their properties. `patch(out)` redeclares
them so the carried blockstates and models fit:

* `basket`: a barrel template (its use opens the block's mod menu; see cooking_pot.py), `facing` over
  six directions as the carried blockstate wants; the 27 slots, the hopper access and the
  placement (the clicked face) are native (`mixins/homestead/storage.rs`). The shape is the open
  box of the model, turned to every facing.
* the eleven `*_cabinet` blocks: the same barrel template (`facing`, `open`); the placement
  (horizontal, towards the placer) and the 27 slots are native. The carried blockstates only
  know the four horizontal facings, so `facing=up|down` variants (the north model) are added.
* `canvas_rug`: a carpet template (the model is a 1 px sheet with decorative extrusions).
* `half_tatami_mat`: the carried pumpkin shell with its 2 px high shape.
* `rope`: a connecting block (chorus plant template, `connects_to` the `farmersdelight:ropes` tag,
  2 px thick as the carried `rope_post` model) with the `tied_to_bell` property the carried
  blockstate tests. Climbing comes from the carried `minecraft:climbable` tag.
* `brown_mushroom_colony`, `red_mushroom_colony`: the vanilla mushroom of their colour as a
  template (survival on `minecraft:mushroom_grow_block`, which the carried data fills with rich
  soil and organic compost) with the colony `age` 0-3 of the carried blockstate. The template's
  own random tick (spreading plain mushrooms) is switched off; the growth is native
  (`mixins/homestead/colony.rs`, driven by the rich soil's random tick).

Not declared here (see docs/fd-stage23.md): `tatami` (facing x paired) and `full_tatami_mat` (two
parts, bed-like) need a block shape this engine has no template for.

    python stage23.py <overlay dir>
"""
import json
import sys
from pathlib import Path

NAMESPACE = 'farmersdelight'
WOODS = ['oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak', 'mangrove', 'cherry', 'bamboo',
         'crimson', 'warped']
COLONIES = ['brown', 'red']
FACINGS = ['down', 'up', 'north', 'south', 'west', 'east']

# `BasketBlockEntity` and the cabinets: 27 slots.
SLOTS = 27

# The basket model opening upwards: a 2 px floor and four 2 px walls (pixels, from / to).
BASKET_UP = [
    (0, 0, 0, 16, 2, 16),
    (0, 2, 0, 16, 16, 2),
    (0, 2, 14, 16, 16, 16),
    (0, 2, 2, 2, 16, 14),
    (14, 2, 2, 16, 16, 14),
]
TATAMI_HALF = [(0, 0, 0, 16, 2, 16)]


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def _read(path, default):
    if path.exists():
        return json.loads(path.read_text(encoding='utf-8'))
    return default


def _blocks(boxes):
    return [[round(v / 16, 6) for v in box] for box in boxes]


def _turn(point, facing):
    """A point of the upward model (block units) turned so that the opening faces `facing`.

    Each is a proper rotation about the block centre, as the carried blockstate's x / y turns:
    the model's +y goes to `facing`."""
    x, y, z = point
    return {
        'up': (x, y, z),
        'down': (x, 1 - y, 1 - z),
        'north': (x, z, 1 - y),
        'south': (x, 1 - z, y),
        'east': (y, 1 - x, z),
        'west': (1 - y, x, z),
    }[facing]


def turned_boxes(boxes, facing):
    """`boxes` (pixels, opening up) as block-unit boxes for a block that opens towards `facing`."""
    out = []
    for x0, y0, z0, x1, y1, z1 in boxes:
        corners = [_turn((cx / 16, cy / 16, cz / 16), facing)
                   for cx in (x0, x1) for cy in (y0, y1) for cz in (z0, z1)]
        lows = [round(min(c[i] for c in corners), 6) for i in range(3)]
        highs = [round(max(c[i] for c in corners), 6) for i in range(3)]
        out.append(lows + highs)
    return out


def basket_shapes():
    return {'property': 'facing', 'values': {f: turned_boxes(BASKET_UP, f) for f in FACINGS}}


def _planks(wood):
    return f'minecraft:{wood}_planks'


def blocks():
    """Every block declaration this slice writes: name -> declaration."""
    out = {
        'basket': {
            # A barrel template: its use opens the block's mod menu (a carved pumpkin's use does not).
            'template': 'minecraft:barrel',
            'item': True,
            'properties': {'hardness': 1.5, 'explosion_resistance': 1.5, 'requires_tool': False,
                           'occludes': False, 'render_shape': 'MODEL',
                           'sounds': 'minecraft:bamboo_planks'},
            'shapes': basket_shapes(),
            'collision_shapes': basket_shapes(),
        },
        'canvas_rug': {
            'template': 'minecraft:white_carpet',
            'item': True,
            'properties': {'hardness': 0.1, 'sounds': 'minecraft:white_wool'},
        },
        'half_tatami_mat': {
            'template': 'minecraft:carved_pumpkin',
            'item': True,
            'properties': {'hardness': 0.2, 'occludes': False, 'render_shape': 'MODEL',
                           'sounds': 'minecraft:white_wool'},
            'shapes': {'default': _blocks(TATAMI_HALF)},
            'collision_shapes': {'default': _blocks(TATAMI_HALF)},
            'components': {'!minecraft:attribute_modifiers': {}, '!minecraft:equippable': {},
                           '!minecraft:compostable': {}},
        },
        'rope': {
            'template': 'minecraft:chorus_plant',
            'item': True,
            'properties': {'hardness': 0.2, 'sounds': 'minecraft:white_wool'},
            'connects_to': '#farmersdelight:ropes',
            'thickness': 2.0,
            'state_properties': {'tied_to_bell': {'type': 'boolean'}},
            'default_state': {'tied_to_bell': 'false'},
        },
    }
    for wood in WOODS:
        out[f'{wood}_cabinet'] = {
            'template': 'minecraft:barrel',
            'item': True,
            'properties': {'hardness': 2.0, 'explosion_resistance': 3.0, 'requires_tool': False,
                           'sounds': _planks(wood)},
        }
    for colour in COLONIES:
        out[f'{colour}_mushroom_colony'] = {
            'template': f'minecraft:{colour}_mushroom',
            'item': True,
            # The template's own random tick spreads plain mushrooms; the colony grows natively.
            'properties': {'hardness': 0.0, 'random_ticks': False},
            'state_properties': {'age': {'type': 'enum', 'values': ['0', '1', '2', '3']}},
            'default_state': {'age': '0'},
        }
    return out


def widen_cabinet_blockstate(path):
    """Adds `facing=up|down` variants (the north model) to a cabinet blockstate (horizontal only)."""
    state = _read(path, None)
    if not state or 'variants' not in state:
        return False
    variants = state['variants']
    changed = False
    for open_value in ('false', 'true'):
        north = variants.get(f'facing=north,open={open_value}')
        if north is None:
            continue
        for facing in ('up', 'down'):
            key = f'facing={facing},open={open_value}'
            if key not in variants:
                variants[key] = dict(north)
                changed = True
    if changed:
        _write(path, state)
    return changed


def patch(out):
    out = Path(out)
    data = out / f'data/{NAMESPACE}'
    declared = blocks()
    for name, block in declared.items():
        _write(data / f'horizon/block/{name}.json', block)
    # The carried loot tables of the mushroom colonies (and wild crops) test the common tag c:shears,
    # which the 26.3 data does not define; without it a ripe colony never drops itself.
    _write(out / 'data/c/tags/item/shears.json', {'replace': False, 'values': ['minecraft:shears']})
    widened = 0
    for wood in WOODS:
        widened += widen_cabinet_blockstate(out / f'assets/{NAMESPACE}/blockstates/{wood}_cabinet.json')
    print(f'stage23: {len(declared)} blocks ({len(WOODS)} cabinets, {len(COLONIES)} colonies), '
          f'{widened} cabinet blockstates widened')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('usage: stage23.py <overlay dir>')
    patch(sys.argv[1])
