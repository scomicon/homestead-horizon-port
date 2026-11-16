#!/usr/bin/env python3
# Copyright (c) 2026 create-horizon contributors
# SPDX-License-Identifier: MIT
"""Batch 2 data mods (stage 23): Ocean's Delight, Crate Delight, Obsidian Equipment Reworked,
Minecraft Accelerated, Create Deco and Sooty Chimneys.

Nothing here copies third-party assets. Each mod's own data and assets come from the player's own
jars through `horizon-port-pack carry` (see stage4/Build-DataMods-batch2.ps1); this script only

  * `overlay`       prunes recipes/items the port cannot honour (blocked recipe types, unavailable
                    namespaces, unsatisfied fabric/neoforge mod-loaded conditions, uncraftable
                    items when enabled) and adds the item/block metadata that the Java mods set in
                    code (food values, obsidian tool/armor stats, jack o'lantern light levels, and
                    the Guardian Soup feast block declaration). Every stat added here is an ESTIMATE
                    written from public descriptions, not read from the mods' Java code.
  * `check-policy`  validates batch2-policy.json (and checks for overlap with batch1-policy.json).

Usage:
  batch2_content.py overlay --carry DIR --out DIR --key KEY --policy FILE --available NS[,NS...] [--report FILE]
  batch2_content.py check-policy --policy FILE [--batch1 FILE]
"""
import argparse
import json
import shutil
import sys
from pathlib import Path

# --------------------------------------------------------------------------- estimates (documented as such)

# Ocean's Delight food values: name -> (nutrition, saturation_modifier).
# 26.3 stores absolute saturation = nutrition * modifier * 2.
# Estimates follow Farmer's Delight's scale (raw seafood 2-4, snacks 5-8, meals 10-14).
FOODS = {
    'oceansdelight': {
        'guardian_tail': (3, 0.3),
        'cooked_guardian_tail': (8, 0.7),
        'tentacles': (3, 0.3),
        'cut_tentacles': (2, 0.3),
        'tentacle_on_a_stick': (4, 0.3),
        'baked_tentacle_on_a_stick': (7, 0.6),
        'squid_rings': (9, 0.7),
        'stuffed_cod': (5, 0.4),
        'cooked_stuffed_cod': (12, 0.8),
        'seagrass_salad': (6, 0.6),
        'honey_fried_kelp': (6, 0.5),
        'fugu_slice': (2, 0.2),
        'fugu_roll': (8, 0.6),
        'elder_guardian_slice': (4, 0.3),
        'cooked_elder_guardian_slice': (9, 0.75),
        'elder_guardian_slab': (6, 0.4),
        'elder_guardian_roll': (10, 0.75),
        'cabbage_wrapped_elder_guardian': (14, 0.8),
        'braised_sea_pickle': (8, 0.65),
        'bowl_of_guardian_soup': (14, 0.8),
    },
}

BOWL_FOODS = {
    'squid_rings',
    'seagrass_salad',
    'braised_sea_pickle',
    'bowl_of_guardian_soup',
}

# Ocean's Delight guardian_soup feast block (mirrors fd/feast.py shell layout; serving behaviour is not ported).
FEAST_FACING = {'type': 'enum', 'values': ['north', 'south', 'west', 'east']}
FEAST_BOX = [[1 / 16, 0.0, 1 / 16, 15 / 16, 7 / 16, 15 / 16]]

# Obsidian Equipment Reworked estimates (placed between diamond and netherite).
OBSIDIAN_ARMOR = {
    'obsidian_helmet': ('head', 480, 3.0, 2.5, 0.05),
    'obsidian_chestplate': ('chest', 700, 8.0, 2.5, 0.05),
    'obsidian_leggings': ('legs', 650, 6.0, 2.5, 0.05),
    'obsidian_boots': ('feet', 560, 3.0, 2.5, 0.05),
}

OBSIDIAN_MOUNT_ARMOR = {
    'obsidian_horse_armor': ('body', 12.0),
    'obsidian_nautilus_armor': ('body', 12.0),
}

# Tool stats: name -> (mineable_tag or None, attack_damage_total, attack_speed, durability, speed, enchantability).
# Base player attack is 1.0, so add_value amount = total - 1.0 (e.g. +6.5 -> 7.5 total for sword).
OBSIDIAN_TOOLS = {
    'obsidian_sword': (None, 6.5, -2.4, 1800, 8.5, 12),
    'obsidian_pickaxe': ('#minecraft:mineable/pickaxe', 4.5, -2.8, 1800, 8.5, 12),
    'obsidian_axe': ('#minecraft:mineable/axe', 8.5, -3.0, 1800, 8.5, 12),
    'obsidian_shovel': ('#minecraft:mineable/shovel', 5.0, -3.0, 1800, 8.5, 12),
    'obsidian_hoe': ('#minecraft:mineable/hoe', 0.0, 0.0, 1800, 8.5, 12),
    'obsidian_spear': (None, 6.0, -2.6, 1800, 8.5, 12),
}

# Directories (relative to carry root, with {ns}) that hold one file per item/block name.
NAME_DIRS = [
    'assets/{ns}/models/item', 'assets/{ns}/models/block', 'assets/{ns}/textures/item',
    'assets/{ns}/textures/block', 'assets/{ns}/blockstates', 'assets/{ns}/items',
    'data/{ns}/horizon/item', 'data/{ns}/horizon/block',
    'data/{ns}/loot_tables/blocks', 'data/{ns}/loot_table/blocks',
]


def read_json(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')


def namespaces_of(value):
    """Every namespace used by item/id fields inside a recipe (tags are ignored)."""
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


def mentions_item(value, target_ids):
    """True when any item/id string inside a JSON value matches one of target_ids."""
    if isinstance(value, str):
        return value in target_ids
    if isinstance(value, dict):
        for key, inner in value.items():
            if key in ('item', 'id') and isinstance(inner, str) and inner in target_ids:
                return True
            if mentions_item(inner, target_ids):
                return True
    elif isinstance(value, list):
        return any(mentions_item(inner, target_ids) for inner in value)
    return False


def result_names(recipe):
    out = []
    res = recipe.get('result')
    if isinstance(res, dict):
        name = res.get('item') or res.get('id')
        if isinstance(name, str):
            out.append(name)
    elif isinstance(res, str):
        out.append(res)
    elif isinstance(res, list):
        for entry in res:
            if isinstance(entry, dict):
                name = entry.get('item') or entry.get('id')
                if isinstance(name, str):
                    out.append(name)
    return out


def failed_load_condition(recipe, available):
    """Checks fabric:load_conditions / neoforge:conditions mod-loaded checks."""
    for cond in recipe.get('fabric:load_conditions', []) or []:
        if not isinstance(cond, dict):
            continue
        ctype = cond.get('condition', '')
        vals = [str(v) for v in (cond.get('values') or [])]
        if ctype == 'fabric:all_mods_loaded':
            missing = [v for v in vals if v not in available]
            if missing:
                return 'fabric:all_mods_loaded missing ' + ','.join(missing)
        elif ctype == 'fabric:any_mod_loaded':
            if vals and not any(v in available for v in vals):
                return 'fabric:any_mod_loaded none of ' + ','.join(vals)
        elif ctype == 'fabric:not':
            inner = cond.get('value') or {}
            if isinstance(inner, dict) and inner.get('condition') == 'fabric:all_mods_loaded':
                vals = [str(v) for v in (inner.get('values') or [])]
                if vals and all(v in available for v in vals):
                    return 'fabric:not condition failed'
    for cond in recipe.get('neoforge:conditions', []) or []:
        if not isinstance(cond, dict):
            continue
        if cond.get('type') == 'neoforge:mod_loaded':
            modid = str(cond.get('modid', ''))
            if modid and modid not in available:
                return f'neoforge:mod_loaded missing {modid}'
    return None


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
    components = {
        'minecraft:food': {
            'nutrition': nutrition,
            'saturation': round(nutrition * modifier * 2.0, 4),
            'can_always_eat': False,
        },
        'minecraft:consumable': {
            'consume_seconds': 1.6,
            'animation': 'eat',
            'sound': 'minecraft:entity.generic.eat',
            'has_consume_particles': True,
        },
    }
    if name in BOWL_FOODS:
        components['minecraft:use_remainder'] = {'id': 'minecraft:bowl', 'count': 1}
        components['minecraft:max_stack_size'] = 16
    return components


def armor_components(ns, slot, uses, armor, toughness, kb_res):
    return {
        'minecraft:max_stack_size': 1,
        'minecraft:max_damage': uses,
        'minecraft:damage': 0,
        'minecraft:enchantable': {'value': 12},
        'minecraft:repairable': {'items': f'{ns}:obsidian_ingot'},
        'minecraft:equippable': {'slot': slot, 'asset_id': f'{ns}:obsidian'},
        'minecraft:attribute_modifiers': [
            {'type': 'minecraft:armor', 'id': f'minecraft:armor.{slot}',
             'amount': armor, 'operation': 'add_value', 'slot': slot},
            {'type': 'minecraft:armor_toughness', 'id': f'minecraft:armor_toughness.{slot}',
             'amount': toughness, 'operation': 'add_value', 'slot': slot},
            {'type': 'minecraft:knockback_resistance', 'id': f'minecraft:knockback_resistance.{slot}',
             'amount': kb_res, 'operation': 'add_value', 'slot': slot},
        ],
    }


def mount_armor_components(ns, slot, armor):
    return {
        'minecraft:max_stack_size': 1,
        'minecraft:equippable': {'slot': slot, 'asset_id': f'{ns}:obsidian'},
        'minecraft:attribute_modifiers': [
            {'type': 'minecraft:armor', 'id': 'minecraft:armor.body',
             'amount': armor, 'operation': 'add_value', 'slot': slot},
        ],
    }


def obsidian_tool_components(ns, mineable_tag, attack, attack_speed, uses, speed, enchant):
    if mineable_tag:
        rules = [
            {'blocks': mineable_tag, 'speed': speed, 'correct_for_drops': True},
            {'blocks': '#minecraft:incorrect_for_netherite_tool', 'correct_for_drops': False},
        ]
    else:
        rules = [
            {'blocks': 'minecraft:cobweb', 'speed': 15.0, 'correct_for_drops': True},
            {'blocks': '#minecraft:sword_efficient', 'speed': 1.5},
        ]
    return {
        'minecraft:max_stack_size': 1,
        'minecraft:max_damage': uses,
        'minecraft:damage': 0,
        'minecraft:enchantable': {'value': enchant},
        'minecraft:repairable': {'items': f'{ns}:obsidian_ingot'},
        'minecraft:tool': {'rules': rules, 'default_mining_speed': 1.0, 'damage_per_block': 1},
        'minecraft:attribute_modifiers': [
            {'type': 'minecraft:attack_damage', 'id': 'minecraft:base_attack_damage',
             'amount': attack, 'operation': 'add_value', 'slot': 'mainhand'},
            {'type': 'minecraft:attack_speed', 'id': 'minecraft:base_attack_speed',
             'amount': attack_speed, 'operation': 'add_value', 'slot': 'mainhand'},
        ],
    }


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


def add_guardian_soup_feast(out):
    """Registers oceansdelight:guardian_soup as a FeastBlock shell (no right-click serving code)."""
    ns = 'oceansdelight'
    states = {'facing': FEAST_FACING, 'servings': {'type': 'enum', 'values': ['0', '1', '2', '3', '4']}}
    write_json(out / f'data/{ns}/horizon/block/guardian_soup.json', {
        'template': 'minecraft:glass',
        'item': False,
        'properties': {
            'hardness': 0.5,
            'explosion_resistance': 0.5,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:white_wool',
        },
        'state_properties': states,
        'default_state': {'facing': 'north', 'servings': '4'},
        'shapes': {'default': FEAST_BOX},
        'collision_shapes': {'default': FEAST_BOX},
    })
    write_json(out / f'data/{ns}/horizon/item/guardian_soup.json', {
        'template': 'minecraft:stick',
        'max_stack_size': 1,
        'place_block': f'{ns}:guardian_soup',
    })


def apply_pumpkin_lights(out, ns):
    changed = []
    blocks_dir = out / f'data/{ns}/horizon/block'
    if not blocks_dir.is_dir():
        return changed
    for path in sorted(blocks_dir.glob('*.json')):
        name = path.stem
        if name.startswith('jack_o_lantern_'):
            light = 15
        elif name.startswith('soul_jack_o_lantern_'):
            light = 10
        else:
            continue
        decl = read_json(path)
        props = dict(decl.get('properties') or {})
        props['light'] = light
        decl['properties'] = props
        write_json(path, decl)
        changed.append(name)
    return changed


def all_recipe_files(out):
    data_dir = out / 'data'
    if not data_dir.is_dir():
        return []
    files = []
    for ns_dir in sorted(data_dir.iterdir()):
        if not ns_dir.is_dir():
            continue
        for folder in ('recipes', 'recipe'):
            base = ns_dir / folder
            if base.is_dir():
                files.extend(sorted(base.rglob('*.json')))
    return files


def cmd_overlay(args):
    carry, out = Path(args.carry), Path(args.out)
    policy = read_json(args.policy)
    entry = next((m for m in policy['mods'] if m['key'] == args.key), None)
    if entry is None:
        sys.exit(f'{args.key}: not in the batch 2 policy')
    available = set(filter(None, args.available.split(',')))
    ns = entry['namespace']
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(carry, out)

    report = {
        'key': args.key,
        'namespace': ns,
        'dropped_files': [],
        'dropped_recipes': [],
        'foods': [],
        'armor': [],
        'tools': [],
        'lights': [],
        'feasts': [],
        'notes': [],
    }

    blocked_types = set(entry.get('blocked_recipe_types', []))
    dropped_items = set(entry.get('drop_items', []))

    # Optionally drop non-owned minecraft recipe overrides (e.g. obsidianequipmentrework's netherite override)
    if entry.get('drop_vanilla_recipe_overrides'):
        for folder in ('recipes', 'recipe'):
            mc_rec = out / 'data/minecraft' / folder
            if mc_rec.is_dir():
                for path in sorted(mc_rec.rglob('*.json')):
                    report['dropped_recipes'].append({
                        'recipe': str(path.relative_to(out)).replace('\\', '/'),
                        'reason': 'vanilla recipe override dropped by policy',
                    })
                    path.unlink()

    # Pass 1: prune recipes by blocked type, load conditions, missing namespaces, or explicitly dropped results.
    surviving_results = set()
    failed_results = set()
    kept_recipes = []
    for path in all_recipe_files(out):
        try:
            recipe = read_json(path)
        except ValueError:
            continue
        reason = None
        kind = recipe.get('type', '')
        cond_err = failed_load_condition(recipe, available)
        if kind in blocked_types:
            reason = f'recipe type {kind} has no consumer in this port'
        elif cond_err:
            reason = cond_err
        else:
            missing = sorted(namespaces_of(recipe) - available - {'minecraft', ns})
            if missing:
                reason = 'needs items of ' + ','.join(missing) + ' (not in the port)'
            elif any(r.split(':', 1)[-1] in dropped_items for r in result_names(recipe)):
                reason = 'result item is dropped'
        if reason:
            report['dropped_recipes'].append({
                'recipe': str(path.relative_to(out)).replace('\\', '/'),
                'reason': reason,
            })
            for r in result_names(recipe):
                if r.startswith(ns + ':'):
                    failed_results.add(r.split(':', 1)[1])
            path.unlink()
        else:
            for r in result_names(recipe):
                if r.startswith(ns + ':'):
                    surviving_results.add(r.split(':', 1)[1])
            kept_recipes.append((path, recipe))

    # Drop items that became completely uncraftable (e.g. Crate Delight crates for absent mods)
    # and clean up any reverse-unpacking recipes that still mention those dropped items.
    if entry.get('drop_uncraftable_items'):
        uncraftable = failed_results - surviving_results
        dropped_items |= uncraftable
        if dropped_items:
            target_ids = {f'{ns}:{n}' for n in dropped_items}
            for path, recipe in kept_recipes:
                if path.exists() and mentions_item(recipe, target_ids):
                    report['dropped_recipes'].append({
                        'recipe': str(path.relative_to(out)).replace('\\', '/'),
                        'reason': 'mentions dropped uncraftable item',
                    })
                    path.unlink()

    for name in sorted(dropped_items):
        drop_name(out, ns, name, report['dropped_files'])

    # Item & block component overlays (documented estimates).
    if entry.get('foods'):
        for name in FOODS.get(ns, {}):
            if name in dropped_items:
                continue
            if apply_item_components(out, ns, name, food_components(ns, name)):
                report['foods'].append(name)
        if ns == 'oceansdelight' and 'guardian_soup' not in dropped_items:
            add_guardian_soup_feast(out)
            report['feasts'].append('guardian_soup')

    if entry.get('obsidian_equipment'):
        for name, (slot, uses, armor, toughness, kb) in OBSIDIAN_ARMOR.items():
            if name not in dropped_items and apply_item_components(
                out, ns, name, armor_components(ns, slot, uses, armor, toughness, kb)
            ):
                report['armor'].append(name)
        for name, (slot, armor) in OBSIDIAN_MOUNT_ARMOR.items():
            if name not in dropped_items and apply_item_components(
                out, ns, name, mount_armor_components(ns, slot, armor)
            ):
                report['armor'].append(name)
        for name, (tag, atk, spd, uses, mining_spd, ench) in OBSIDIAN_TOOLS.items():
            if name not in dropped_items and apply_item_components(
                out, ns, name,
                obsidian_tool_components(ns, tag, atk, spd, uses, mining_spd, ench),
                template='minecraft:iron_sword',
            ):
                report['tools'].append(name)

    if entry.get('pumpkin_lights'):
        report['lights'] = apply_pumpkin_lights(out, ns)

    report['estimated'] = (
        'food values, obsidian tool/armor stats and light levels are estimates from public descriptions; '
        'the mods Java code was not read'
    )
    if args.report:
        write_json(args.report, report)
    print(json.dumps({k: (len(v) if isinstance(v, list) else v) for k, v in report.items()}))


def cmd_check_policy(args):
    p2 = read_json(args.policy)
    mods2 = p2.get('mods')
    if not isinstance(mods2, list) or not (5 <= len(mods2) <= 8):
        sys.exit(f'Expected 5-8 mods in batch2 policy, got {len(mods2) if isinstance(mods2, list) else type(mods2)}')
    keys2 = [m['key'] for m in mods2]
    if len(keys2) != len(set(keys2)):
        sys.exit('Duplicate keys in batch2 policy')
    if args.batch1:
        p1 = read_json(args.batch1)
        keys1 = {m['key'] for m in p1.get('mods', [])}
        overlap = sorted(set(keys2) & keys1)
        if overlap:
            sys.exit('Overlap with batch1 policy: ' + ', '.join(overlap))
    print(json.dumps({'ok': True, 'count': len(mods2), 'keys': keys2}))


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

    c = sub.add_parser('check-policy')
    c.add_argument('--policy', required=True)
    c.add_argument('--batch1')
    c.set_defaults(func=cmd_check_policy)

    args = parser.parse_args()
    args.func(args)


if __name__ == '__main__':
    main()
