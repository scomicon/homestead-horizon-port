#!/usr/bin/env python3
# Copyright (c) 2026 create-horizon contributors
# SPDX-License-Identifier: MIT
"""Batch 1 data mods (stage 27): Expanded Delight, Farmer's Knives, Mo Glass, Perfect Homestead
Plushies, Spud's Shops, Go Fish, Nether's Delight (data half) and SparseStructures.

Nothing here copies third-party assets. The mods' own data and assets come from the player's own jars
through `horizon-port-pack carry` (see stage4/Build-Batch1.ps1); this file only

  * `overlay`  prunes a carried mod (recipes that cannot load, items the port cannot honour) and adds the
               item components the Java mods set in code (food values, knife stats). Every number added
               here is an ESTIMATE written from the public descriptions, not read from the mods' code.
  * `sparse`   writes the SparseStructures data pack: random_spread spacing/separation of every
               structure set multiplied by a spread factor, from the vanilla files of the player's own
               prepared resources (never shipped, never committed).

Usage:
  batch1_content.py overlay --carry DIR --out DIR --key KEY --policy policy.json --available NS[,NS...] [--report FILE]
  batch1_content.py sparse --resources DIR --out DIR --mcmeta FILE [--config FILE] [--carry-root DIR] [--factor N]
"""
import argparse
import json
import re
import shutil
import sys
from pathlib import Path

# --------------------------------------------------------------------------- estimates (documented as such)

# Food: name -> (nutrition, saturation modifier). 26.3 stores absolute saturation = nutrition * modifier * 2.
# Values follow the shape of Farmer's Delight's own table (meals 10-14, snacks 2-9, drinks 4-6).
FOODS = {
    'expandeddelight': {
        'asparagus': (2, 0.3), 'chili_pepper': (1, 0.3), 'peanut': (1, 0.3), 'sweet_potato': (3, 0.4),
        'baked_sweet_potato': (6, 0.6), 'cheese_slice': (3, 0.5),
        'cheese_sandwich': (9, 0.7), 'grilled_cheese': (9, 0.75),
        'glow_berry_jelly_sandwich': (8, 0.6), 'sweet_berry_jelly_sandwich': (8, 0.6),
        'peanut_butter_sandwich': (8, 0.6), 'peanut_butter_honey_sandwich': (9, 0.65),
        'chocolate_cookie': (2, 0.1), 'snickerdoodle': (2, 0.1), 'sugar_cookie': (2, 0.1),
        'sweet_roll': (6, 0.5), 'berry_sweet_roll': (6, 0.5), 'glow_berry_sweet_roll': (6, 0.5),
        'cinnamon_apples': (6, 0.5),
        'asparagus_soup': (12, 0.65), 'asparagus_soup_creamy': (12, 0.7), 'peanut_honey_soup': (11, 0.65),
        'asparagus_and_bacon_cheesy': (11, 0.75), 'mac_and_cheese': (11, 0.75), 'peperonata': (10, 0.7),
        'cinnamon_rice': (10, 0.7), 'peanut_salad': (8, 0.6), 'sweet_potato_salad': (8, 0.6),
        'apple_juice': (4, 0.3), 'glow_berry_juice': (4, 0.3), 'sweet_berry_juice': (4, 0.3),
        'glow_berry_jelly': (2, 0.3), 'sweet_berry_jelly': (2, 0.3),
    },
    'nethersdelight': {
        'hoglin_loin': (4, 0.3), 'hoglin_sirloin': (8, 0.8), 'hoglin_ear': (3, 0.3),
        'strider_slice': (3, 0.3), 'ground_strider': (3, 0.3), 'grilled_strider': (10, 0.85),
        'nether_skewer': (10, 0.85), 'strider_moss_stew': (14, 0.8), 'stuffed_hoglin': (12, 0.8),
        'raw_stuffed_hoglin': (6, 0.4), 'plate_of_stuffed_hoglin_snout': (16, 0.85),
        'plate_of_stuffed_hoglin_ham': (18, 0.85), 'plate_of_stuffed_hoglin_roast': (20, 0.85),
        'warped_moldy_meat': (4, 0.2),
    },
}
BOWL = {'asparagus_soup', 'asparagus_soup_creamy', 'peanut_honey_soup', 'asparagus_and_bacon_cheesy',
        'mac_and_cheese', 'peperonata', 'cinnamon_rice', 'peanut_salad', 'sweet_potato_salad',
        'strider_moss_stew'}
BOTTLE = {'apple_juice', 'glow_berry_juice', 'sweet_berry_juice'}
DRINKS = BOTTLE
# Raw meats and the always-edible ingredients: nothing special. The spicy chili is not "can always eat".

# Knife tiers: material keyword -> (attack bonus, durability, mining speed, enchantability, repair item or None).
# Farmer's Knives are KnifeItem(tier, 0.5, -2.0) like Farmer's Delight's; tiers are matched to the closest
# vanilla material by name. ESTIMATES: the mod's tier table was not read.
KNIFE_TIERS = [
    ('netherite_diamond', (4.0, 2300, 9.0, 15, None)),
    ('netherite_emerald', (4.0, 2300, 9.0, 15, None)),
    ('netherite_gold', (4.0, 2100, 9.0, 18, None)),
    ('netherite_iron', (4.0, 2150, 9.0, 15, None)),
    ('gilded_netherite', (4.0, 2100, 9.5, 20, None)),
    ('obsidian', (3.0, 1800, 7.0, 8, None)),
    ('amethyst', (1.0, 190, 5.0, 12, 'minecraft:amethyst_shard')),
    ('emerald', (2.5, 900, 8.0, 14, 'minecraft:emerald')),
    ('nether_brick', (1.0, 200, 5.0, 8, 'minecraft:nether_brick')),
    ('copper', (1.0, 190, 5.0, 10, 'minecraft:copper_ingot')),
    ('bronze', (2.0, 300, 6.0, 12, None)),
    ('steel', (2.0, 450, 6.5, 10, None)),
    ('rose_gold', (1.0, 80, 10.0, 22, None)),
]
KNIFE_DEFAULT = (2.0, 250, 6.0, 12, None)
# Nether's Delight machetes: SwordItem-like (attack bonus on top of 3, speed -2.4 in the original). Estimates.
MACHETES = {
    'iron_machete': (2.0, 250, 14, 'minecraft:iron_ingot'),
    'golden_machete': (1.0, 32, 22, 'minecraft:gold_ingot'),
    'diamond_machete': (3.0, 1561, 10, 'minecraft:diamond'),
    'netherite_machete': (4.0, 2031, 15, 'minecraft:netherite_ingot'),
}

# Directories (relative to the carry root, with {ns}) that hold one file per item/block name.
NAME_DIRS = [
    'assets/{ns}/models/item', 'assets/{ns}/models/block', 'assets/{ns}/textures/item',
    'assets/{ns}/textures/block', 'assets/{ns}/blockstates', 'data/{ns}/horizon/item',
    'data/{ns}/horizon/block', 'data/{ns}/loot_tables/blocks', 'data/{ns}/loot_table/blocks',
]


def read_json(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')


def namespaces_of(value):
    """Every namespace used by item/id fields inside a recipe (tags are ignored on purpose)."""
    found = set()
    if isinstance(value, dict):
        for key, inner in value.items():
            if key in ('item', 'id') and isinstance(inner, str) and ':' in inner and not inner.startswith('#'):
                found.add(inner.split(':', 1)[0])
            else:
                found |= namespaces_of(inner)
    elif isinstance(value, list):
        for inner in value:
            found |= namespaces_of(inner)
    return found


def result_names(recipe):
    out = []
    res = recipe.get('result')
    if isinstance(res, dict):
        name = res.get('item') or res.get('id')
        if isinstance(name, str):
            out.append(name)
    elif isinstance(res, str):
        out.append(res)
    return out


def drop_name(root, ns, name, dropped):
    for pattern in NAME_DIRS:
        base = root / pattern.format(ns=ns)
        if not base.is_dir():
            continue
        for path in base.rglob('*'):
            if path.is_file() and path.stem == name:
                path.unlink()
                dropped.append(str(path.relative_to(root)).replace('\\', '/'))


def food_components(ns, name):
    nutrition, modifier = FOODS[ns][name]
    drink = name in DRINKS
    components = {
        'minecraft:food': {'nutrition': nutrition, 'saturation': round(nutrition * modifier * 2.0, 4),
                           'can_always_eat': False},
        'minecraft:consumable': {
            'consume_seconds': 1.6, 'animation': 'drink' if drink else 'eat',
            'sound': 'minecraft:entity.generic.drink' if drink else 'minecraft:entity.generic.eat',
            'has_consume_particles': not drink},
    }
    if name in BOWL:
        components['minecraft:use_remainder'] = {'id': 'minecraft:bowl', 'count': 1}
        components['minecraft:max_stack_size'] = 16
    if name in BOTTLE:
        components['minecraft:use_remainder'] = {'id': 'minecraft:glass_bottle', 'count': 1}
        components['minecraft:max_stack_size'] = 16
    return components


def knife_stats(name):
    for key, stats in KNIFE_TIERS:
        if key in name:
            return stats
    return KNIFE_DEFAULT


def tool_components(bonus, uses, speed, enchant, repair, knife):
    rules = [{'blocks': '#farmersdelight:mineable/knife', 'speed': speed, 'correct_for_drops': True},
             {'blocks': 'minecraft:cobweb', 'speed': 15.0, 'correct_for_drops': True}]
    if not knife:
        rules = [{'blocks': 'minecraft:cobweb', 'speed': 15.0, 'correct_for_drops': True},
                 {'blocks': '#minecraft:sword_efficient', 'speed': 1.5}]
    components = {
        'minecraft:max_damage': uses, 'minecraft:damage': 0,
        'minecraft:enchantable': {'value': enchant},
        'minecraft:tool': {'rules': rules, 'default_mining_speed': 1.0, 'damage_per_block': 1},
        'minecraft:attribute_modifiers': [
            {'type': 'minecraft:attack_damage', 'id': 'minecraft:base_attack_damage',
             'amount': round((0.5 if knife else 3.0) + bonus, 2), 'operation': 'add_value', 'slot': 'mainhand'},
            {'type': 'minecraft:attack_speed', 'id': 'minecraft:base_attack_speed',
             'amount': -2.0 if knife else -2.4, 'operation': 'add_value', 'slot': 'mainhand'}],
    }
    if repair:
        components['minecraft:repairable'] = {'items': repair}
    return components


def apply_item_components(out, ns, name, components, template=None):
    path = out / f'data/{ns}/horizon/item/{name}.json'
    if not path.exists():
        return False
    declaration = read_json(path)
    components = dict(components)
    if 'minecraft:max_stack_size' in components:
        declaration['max_stack_size'] = components.pop('minecraft:max_stack_size')
    if template:
        declaration['template'] = template
    declaration['components'] = components
    write_json(path, declaration)
    return True


def cmd_overlay(args):
    carry, out = Path(args.carry), Path(args.out)
    policy = read_json(args.policy)
    entry = next((m for m in policy['mods'] if m['key'] == args.key), None)
    if entry is None:
        sys.exit(f'{args.key}: not in the batch 1 policy')
    available = set(filter(None, args.available.split(',')))
    ns = entry['namespace']
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(carry, out)
    report = {'key': args.key, 'namespace': ns, 'dropped_files': [], 'dropped_recipes': [],
              'foods': [], 'knives': [], 'machetes': [], 'notes': []}

    # 1. Recipes the port cannot honour: an unavailable namespace in an item/id, or a blocked recipe type.
    blocked_types = set(entry.get('blocked_recipe_types', []))
    recipe_dirs = [out / f'data/{ns}/recipes', out / f'data/{ns}/recipe']
    dropped_items = set(entry.get('drop_items', []))
    for base in recipe_dirs:
        if not base.is_dir():
            continue
        for path in sorted(base.rglob('*.json')):
            try:
                recipe = read_json(path)
            except ValueError:
                continue
            reason = None
            kind = recipe.get('type', '')
            if kind in blocked_types:
                reason = f'recipe type {kind} has no consumer in this port'
            else:
                missing = sorted(namespaces_of(recipe) - available - {'minecraft', ns})
                if missing:
                    reason = 'needs items of ' + ','.join(missing) + ' (not in the port)'
                elif any(r.split(':', 1)[-1] in dropped_items for r in result_names(recipe)):
                    reason = 'result item is dropped'
            if reason:
                report['dropped_recipes'].append({'recipe': str(path.relative_to(out)).replace('\\', '/'),
                                                  'reason': reason})
                # Items of a Farmer's Knives recipe that cannot be crafted are dropped with it.
                if entry.get('drop_uncraftable_items'):
                    for r in result_names(recipe):
                        if r.startswith(ns + ':'):
                            dropped_items.add(r.split(':', 1)[1])
                path.unlink()

    # 2. Items and blocks that need behaviour this port does not have.
    for name in sorted(dropped_items):
        drop_name(out, ns, name, report['dropped_files'])

    # 3. Components the Java mods set in code.
    if entry.get('foods'):
        for name in FOODS.get(ns, {}):
            if name in dropped_items:
                continue
            if apply_item_components(out, ns, name, food_components(ns, name)):
                report['foods'].append(name)
    if entry.get('knives'):
        items = out / f'data/{ns}/horizon/item'
        for path in sorted(items.glob('*_knife*.json')) if items.is_dir() else []:
            name = path.stem
            if name in dropped_items:
                continue
            bonus, uses, speed, enchant, repair = knife_stats(name)
            components = tool_components(bonus, uses, speed, enchant, repair, knife=True)
            components['minecraft:max_stack_size'] = 1
            apply_item_components(out, ns, name, components, template='minecraft:iron_sword')
            report['knives'].append(name)
    if entry.get('machetes'):
        for name, (bonus, uses, enchant, repair) in MACHETES.items():
            components = tool_components(bonus, uses, 6.0, enchant, repair, knife=False)
            components['minecraft:max_stack_size'] = 1
            if name == 'netherite_machete':
                components['minecraft:damage_resistant'] = {'types': '#minecraft:is_fire'}
            if name not in dropped_items and apply_item_components(out, ns, name, components,
                                                                  template='minecraft:iron_sword'):
                report['machetes'].append(name)

    report['estimated'] = ('food values, knife and machete stats are estimates from public descriptions; '
                           'the mods code was not read')
    if args.report:
        write_json(args.report, report)
    print(json.dumps({k: (len(v) if isinstance(v, list) else v) for k, v in report.items()}))


# --------------------------------------------------------------------------- SparseStructures

def strip_json5(text):
    text = re.sub(r'(?m)^\s*//.*$', '', text)
    text = re.sub(r'(?m)\s//[^"\n]*$', '', text)
    text = re.sub(r',(\s*[}\]])', r'\1', text)
    return text


def load_config(path, factor_default):
    config = {'spreadFactor': factor_default, 'customSpreadFactors': [{'structure': 'minecraft:mansion', 'factor': 2}]}
    if path and Path(path).is_file():
        try:
            config.update(json.loads(strip_json5(Path(path).read_text(encoding='utf-8'))))
        except ValueError as error:
            print(f'warning: config not read ({error}); using defaults', file=sys.stderr)
    return config


def collect_sets(resources, carry_root):
    sets = {}
    vanilla = Path(resources) / 'data/minecraft/worldgen/structure_set'
    if vanilla.is_dir():
        for path in sorted(vanilla.glob('*.json')):
            sets[f'minecraft:{path.stem}'] = read_json(path)
    if carry_root and Path(carry_root).is_dir():
        for path in sorted(Path(carry_root).glob('*/carry/data/*/worldgen/structure_set/*.json')):
            ns = path.parts[-4]
            sets[f'{ns}:{path.stem}'] = read_json(path)
    return sets


def structures_of(structure_set):
    names = []
    for entry in structure_set.get('structures', []):
        name = entry.get('structure') if isinstance(entry, dict) else entry
        if isinstance(name, str):
            names.append(name if ':' in name else 'minecraft:' + name)
    return names


def cmd_sparse(args):
    config = load_config(args.config, args.factor)
    sets = collect_sets(args.resources, args.carry_root)
    if not sets:
        sys.exit('no structure sets found: pass the prepared resources folder (data/minecraft/worldgen/structure_set)')
    custom = {}
    for item in config.get('customSpreadFactors', []):
        custom[item['structure']] = float(item['factor'])
    out = Path(args.out)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    # Same pack_format/min_max_format as the shipped homestead-create data pack, own description.
    mcmeta = read_json(args.mcmeta)
    mcmeta['pack']['description'] = (f"Homestead SparseStructures: structure spacing x{config['spreadFactor']} "
                                     f"(custom: {', '.join(i['structure'] + ' x' + str(i['factor']) for i in config.get('customSpreadFactors', [])) or 'none'})")
    write_json(out / 'pack.mcmeta', mcmeta)
    changed, kept = [], []
    for set_id, body in sorted(sets.items()):
        placement = body.get('placement', {})
        if placement.get('type', '').split(':')[-1] != 'random_spread':
            kept.append(set_id)
            continue
        factor = float(config['spreadFactor'])
        # A custom factor names a set or one of its structures (the whole set is then changed).
        for key in [set_id] + structures_of(body):
            if key in custom:
                factor = custom[key]
        spacing, separation = int(placement['spacing']), int(placement['separation'])
        new_spacing = max(2, int(round(spacing * factor)))
        new_separation = min(max(0, int(round(separation * factor))), new_spacing - 1)
        if (new_spacing, new_separation) == (spacing, separation):
            kept.append(set_id)
            continue
        placement = dict(placement, spacing=new_spacing, separation=new_separation)
        ns, name = set_id.split(':', 1)
        write_json(out / f'data/{ns}/worldgen/structure_set/{name}.json', dict(body, placement=placement))
        changed.append({'set': set_id, 'factor': factor, 'spacing': [spacing, new_spacing],
                        'separation': [separation, new_separation]})
    write_json(out / 'sparsestructures-report.json', {'config': config, 'changed': changed, 'unchanged': kept})
    print(json.dumps({'changed': len(changed), 'unchanged': len(kept), 'sets': len(sets)}))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='command', required=True)
    o = sub.add_parser('overlay')
    o.add_argument('--carry', required=True)
    o.add_argument('--out', required=True)
    o.add_argument('--key', required=True)
    o.add_argument('--policy', required=True)
    o.add_argument('--available', default='')
    o.add_argument('--report')
    o.set_defaults(func=cmd_overlay)
    s = sub.add_parser('sparse')
    s.add_argument('--resources', required=True)
    s.add_argument('--out', required=True)
    s.add_argument('--mcmeta', required=True)
    s.add_argument('--config')
    s.add_argument('--carry-root')
    s.add_argument('--factor', type=float, default=1.2)
    s.set_defaults(func=cmd_sparse)
    args = parser.parse_args()
    args.func(args)


if __name__ == '__main__':
    main()
