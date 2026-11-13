"""Farmer's Delight (MIT, Refabricated 2.4.1 / vectorwing 1.20 values) item overlay.

`horizon-port-pack carry` turns every plain FD item into a `minecraft:stick` shell.
This script rewrites those shells with the data components the original Java sets in
`FoodValues` and `ModItems`: food, the eating/drinking animation and speed, effects,
bowl and bottle remainders, stack sizes and the knives' tool data. It also declares the
`farmersdelight:nourishment` mob effect, whose tick lives in the Rust bundle.

    python overlay.py <carry dir> <out dir>
"""
import json
import shutil
import sys
from pathlib import Path

# FoodValues: nutrition, saturation modifier, can always eat, fast, effects.
# Effect tuples are (id, duration ticks, amplifier, probability).
NOURISH = 'farmersdelight:nourishment'


def nourish(duration):
    return [(NOURISH, duration, 0, 1.0)]


FOODS = {
    'cabbage': (2, 0.4, False, False, []),
    'tomato': (1, 0.3, False, False, []),
    'onion': (2, 0.4, False, False, []),
    'apple_cider': (0, 0.0, True, False, [('minecraft:absorption', 1200, 0, 1.0)]),
    'fried_egg': (4, 0.4, False, False, []),
    'tomato_sauce': (4, 0.4, False, False, []),
    'wheat_dough': (2, 0.3, False, False, [('minecraft:hunger', 600, 0, 0.3)]),
    'raw_pasta': (2, 0.3, False, False, [('minecraft:hunger', 600, 0, 0.3)]),
    'pie_crust': (2, 0.2, False, False, []),
    'pumpkin_slice': (3, 0.3, False, False, []),
    'cabbage_leaf': (1, 0.4, False, True, []),
    'minced_beef': (2, 0.3, False, True, []),
    'beef_patty': (4, 0.8, False, True, []),
    'chicken_cuts': (1, 0.3, False, True, [('minecraft:hunger', 600, 0, 0.3)]),
    'cooked_chicken_cuts': (3, 0.6, False, True, []),
    'bacon': (2, 0.3, False, True, []),
    'cooked_bacon': (4, 0.8, False, True, []),
    'cod_slice': (1, 0.1, False, True, []),
    'cooked_cod_slice': (3, 0.5, False, True, []),
    'salmon_slice': (1, 0.1, False, True, []),
    'cooked_salmon_slice': (3, 0.8, False, True, []),
    'mutton_chops': (1, 0.3, False, True, []),
    'cooked_mutton_chops': (3, 0.8, False, True, []),
    'ham': (5, 0.3, False, False, []),
    'smoked_ham': (10, 0.8, False, False, []),
    'melon_popsicle': (3, 0.2, True, True, []),
    'sweet_berry_cookie': (2, 0.1, False, True, []),
    'honey_cookie': (2, 0.1, False, True, []),
    'cake_slice': (2, 0.1, False, True, [('minecraft:speed', 400, 0, 1.0)]),
    'apple_pie_slice': (3, 0.3, False, True, [('minecraft:speed', 600, 0, 1.0)]),
    'sweet_berry_cheesecake_slice': (3, 0.3, False, True, [('minecraft:speed', 600, 0, 1.0)]),
    'chocolate_pie_slice': (3, 0.3, False, True, [('minecraft:speed', 600, 0, 1.0)]),
    'pumpkin_pie_slice': (3, 0.3, False, True, [('minecraft:speed', 600, 0, 1.0)]),
    'fruit_salad': (6, 0.6, False, False, [('minecraft:regeneration', 100, 0, 1.0)]),
    'glow_berry_custard': (7, 0.6, True, False, [('minecraft:glowing', 100, 0, 1.0)]),
    'mixed_salad': (6, 0.6, False, False, [('minecraft:regeneration', 100, 0, 1.0)]),
    'nether_salad': (5, 0.4, False, False, [('minecraft:nausea', 240, 0, 0.3)]),
    'barbecue_stick': (8, 0.9, False, False, []),
    'egg_sandwich': (8, 0.8, False, False, []),
    'chicken_sandwich': (10, 0.8, False, False, []),
    'hamburger': (11, 0.8, False, False, []),
    'bacon_sandwich': (10, 0.8, False, False, []),
    'mutton_wrap': (10, 0.8, False, False, []),
    'dumplings': (8, 0.8, False, False, []),
    'stuffed_potato': (10, 0.7, False, False, []),
    'cabbage_rolls': (5, 0.5, False, False, []),
    'salmon_roll': (7, 0.6, False, False, []),
    'cod_roll': (7, 0.6, False, False, []),
    'kelp_roll': (12, 0.6, False, False, []),
    'kelp_roll_slice': (6, 0.5, False, True, []),
    'cooked_rice': (6, 0.4, False, False, nourish(600)),
    'bone_broth': (8, 0.7, False, False, nourish(1200)),
    'beef_stew': (12, 0.8, False, False, nourish(3600)),
    'vegetable_soup': (12, 0.8, False, False, nourish(3600)),
    'fish_stew': (12, 0.8, False, False, nourish(3600)),
    'onion_soup': (12, 0.8, False, False, nourish(3600)),
    'chicken_soup': (12, 0.8, False, False, nourish(3600)),
    'fried_rice': (12, 0.8, False, False, nourish(3600)),
    'pumpkin_soup': (14, 0.75, False, False, nourish(6000)),
    'baked_cod_stew': (14, 0.75, False, False, nourish(6000)),
    'noodle_soup': (14, 0.75, False, False, nourish(6000)),
    'bacon_and_eggs': (10, 0.6, False, False, nourish(1200)),
    'ratatouille': (10, 0.6, False, False, nourish(1200)),
    'steak_and_potatoes': (12, 0.8, False, False, nourish(3600)),
    'pasta_with_meatballs': (12, 0.8, False, False, nourish(3600)),
    'pasta_with_mutton_chop': (12, 0.8, False, False, nourish(3600)),
    'mushroom_rice': (12, 0.8, False, False, nourish(3600)),
    'roasted_mutton_chops': (14, 0.75, False, False, nourish(6000)),
    'vegetable_noodles': (14, 0.75, False, False, nourish(6000)),
    'squid_ink_pasta': (14, 0.75, False, False, nourish(6000)),
    'grilled_salmon': (14, 0.75, False, False, nourish(3600)),
    'roast_chicken': (14, 0.75, False, False, nourish(6000)),
    'stuffed_pumpkin': (14, 0.75, False, False, nourish(6000)),
    'honey_glazed_ham': (14, 0.75, False, False, nourish(6000)),
    'shepherds_pie': (14, 0.75, False, False, nourish(6000)),
    'gleaming_salad': (14, 0.75, False, False, nourish(6000)),
}

# What the item leaves (craftRemainder in ModItems; ConsumableItem gives it back after eating).
BOWL = {'tomato_sauce', 'fruit_salad', 'mixed_salad', 'nether_salad', 'cooked_rice', 'bone_broth',
        'beef_stew', 'chicken_soup', 'vegetable_soup', 'fish_stew', 'fried_rice', 'pumpkin_soup',
        'baked_cod_stew', 'noodle_soup', 'onion_soup', 'bacon_and_eggs', 'pasta_with_meatballs',
        'pasta_with_mutton_chop', 'mushroom_rice', 'roasted_mutton_chops', 'vegetable_noodles',
        'steak_and_potatoes', 'ratatouille', 'squid_ink_pasta', 'grilled_salmon', 'roast_chicken',
        'stuffed_pumpkin', 'honey_glazed_ham', 'shepherds_pie', 'gleaming_salad', 'dog_food'}
BOTTLE = {'milk_bottle', 'hot_cocoa', 'apple_cider', 'melon_juice', 'glow_berry_custard'}
DRINKS = BOTTLE | {'bone_broth'}

STACK = {name: 16 for name in BOWL | BOTTLE}
STACK.update({'rotten_tomato': 16, 'horse_feed': 16, 'roast_chicken_block': 1,
              'stuffed_pumpkin_block': 1, 'honey_glazed_ham_block': 1, 'shepherds_pie_block': 1,
              'gleaming_salad_block': 1})
# The feast items that carry a portion also stack to 16 in the original (listed above).

# Knives: name -> (tier attack bonus, durability, mining speed, enchantability, repair item).
# KnifeItem(tier, 0.5, -2.0): attack damage 0.5 + tier bonus, attack speed -2.0.
KNIVES = {
    'flint_knife': (1.0, 131, 4.0, 5, 'minecraft:flint'),
    'iron_knife': (2.0, 250, 6.0, 14, 'minecraft:iron_ingot'),
    'golden_knife': (0.0, 32, 12.0, 22, 'minecraft:gold_ingot'),
    'diamond_knife': (3.0, 1561, 8.0, 10, 'minecraft:diamond'),
    'netherite_knife': (4.0, 2031, 9.0, 15, 'minecraft:netherite_ingot'),
}


def consumable(name, fast, effects):
    drink = name in DRINKS
    entry = {'consume_seconds': 0.8 if fast else 1.6,
             'animation': 'drink' if drink else 'eat',
             'sound': 'minecraft:entity.generic.drink' if drink else 'minecraft:entity.generic.eat',
             'has_consume_particles': not drink}
    on = []
    for effect, duration, amplifier, probability in effects:
        item = {'type': 'minecraft:apply_effects',
                'effects': [{'id': effect, 'duration': duration, 'amplifier': amplifier,
                             'show_particles': True, 'show_icon': True}]}
        if probability < 1.0:
            item['probability'] = probability
        on.append(item)
    if on:
        entry['on_consume_effects'] = on
    return entry


def item_for(name):
    components = {}
    if name in FOODS:
        nutrition, modifier, always, fast, effects = FOODS[name]
        # 26.3 stores absolute saturation: nutrition * modifier * 2.
        components['minecraft:food'] = {'nutrition': nutrition,
                                        'saturation': round(nutrition * modifier * 2.0, 4),
                                        'can_always_eat': always}
        components['minecraft:consumable'] = consumable(name, fast, effects)
    if name in BOWL:
        components['minecraft:use_remainder'] = {'id': 'minecraft:bowl', 'count': 1}
    if name in BOTTLE:
        components['minecraft:use_remainder'] = {'id': 'minecraft:glass_bottle', 'count': 1}
    if name in STACK:
        components['minecraft:max_stack_size'] = STACK[name]
    if name in KNIVES:
        bonus, uses, speed, enchant, repair = KNIVES[name]
        components.update({
            'minecraft:max_stack_size': 1,
            'minecraft:max_damage': uses,
            'minecraft:damage': 0,
            'minecraft:enchantable': {'value': enchant},
            'minecraft:repairable': {'items': repair},
            'minecraft:tool': {'rules': [
                {'blocks': '#farmersdelight:mineable/knife', 'speed': speed, 'correct_for_drops': True},
                {'blocks': 'minecraft:cobweb', 'speed': 15.0, 'correct_for_drops': True}],
                'default_mining_speed': 1.0, 'damage_per_block': 1},
            'minecraft:attribute_modifiers': [
                {'type': 'minecraft:attack_damage', 'id': 'minecraft:base_attack_damage',
                 'amount': round(0.5 + bonus, 2), 'operation': 'add_value', 'slot': 'mainhand'},
                {'type': 'minecraft:attack_speed', 'id': 'minecraft:base_attack_speed',
                 'amount': -2.0, 'operation': 'add_value', 'slot': 'mainhand'}],
        })
        if name == 'netherite_knife':
            components['minecraft:damage_resistant'] = {'types': '#minecraft:is_fire'}
    return components


def main(source, out):
    source, out = Path(source), Path(out)
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(source, out)
    items = out / 'data/farmersdelight/horizon/item'
    changed = 0
    for path in sorted(items.glob('*.json')):
        components = item_for(path.stem)
        if not components:
            continue
        declaration = json.loads(path.read_text(encoding='utf-8'))
        if knife := path.stem in KNIVES:
            declaration['template'] = 'minecraft:iron_sword'
        if 'minecraft:max_stack_size' in components:
            declaration['max_stack_size'] = components.pop('minecraft:max_stack_size')
        declaration['components'] = components
        path.write_text(json.dumps(declaration, indent=2) + '\n', encoding='utf-8')
        changed += 1
    board = out / 'data/farmersdelight/horizon/block/cutting_board.json'
    if board.exists():
        # Thin board (AABB 1..15 x 0..1 x 1..15), hand-axe mineable; behaviour is native (homestead).
        shape = [[1 / 16, 0.0, 1 / 16, 15 / 16, 1 / 16, 15 / 16]]
        board.write_text(json.dumps({
            'template': 'minecraft:carved_pumpkin',
            'properties': {'hardness': 0.5, 'occludes': False, 'render_shape': 'MODEL', 'sounds': 'minecraft:oak_planks'},
            'shapes': {'default': shape}, 'collision_shapes': {'default': shape}},
            indent=2) + '\n', encoding='utf-8')
    # The skillet block and item are declared by a sibling script (the overlay runs isolated).
    import importlib.util
    spec = importlib.util.spec_from_file_location('fd_skillet', Path(__file__).with_name('skillet.py'))
    skillet = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(skillet)
    skillet.patch(out)
    spec = importlib.util.spec_from_file_location('fd_cooking_pot', Path(__file__).with_name('cooking_pot.py'))
    cooking_pot = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(cooking_pot)
    cooking_pot.patch(out)
    spec = importlib.util.spec_from_file_location('fd_crops', Path(__file__).with_name('crops.py'))
    crops = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(crops)
    crops.patch(out)
    spec = importlib.util.spec_from_file_location('fd_feast', Path(__file__).with_name('feast.py'))
    feast = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(feast)
    feast.patch(out)
    effects = out / 'data/farmersdelight/horizon/mob_effect'
    effects.mkdir(parents=True, exist_ok=True)
    # Nourishment is beneficial and light green in the original; its tick is native.
    (effects / 'nourishment.json').write_text(
        json.dumps({'category': 'beneficial', 'color': 0x8DC15E}, indent=2) + '\n', encoding='utf-8')
    lang = out / 'assets/farmersdelight/lang/en_us.json'
    if lang.exists():
        text = json.loads(lang.read_text(encoding='utf-8'))
        text.setdefault('effect.farmersdelight.nourishment', 'Nourishment')
        lang.write_text(json.dumps(text, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(f'{changed} items rewritten; {len(FOODS)} foods, {len(KNIVES)} knives')


if __name__ == '__main__':
    main(*sys.argv[1:3])
