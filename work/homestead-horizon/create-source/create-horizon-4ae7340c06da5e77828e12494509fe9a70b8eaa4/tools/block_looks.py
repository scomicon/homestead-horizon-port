"""Sets occlusion and collision of carried blocks whose model does not match their guessed template.

`horizon-port-pack carry` has no oracle for these mods, so it guesses a vanilla template from the blockstate
properties (`facing` -> carved_pumpkin, no property -> stone, ...). The template decides three things the
model does not: whether the block hides its neighbours' faces (occlusion), how it blocks movement
(collision) and the targeting outline. For a small model on a full cube template that is wrong:

  * the neighbours' faces are culled, so a plush leaves a see-through hole in the floor under it
    (`properties.occludes: false` fixes it, see horizon docs/mods.md);
  * the collision is a full cube, so a flower or a carpet is a wall;
  * on templates without collision (pressure plate, button, vine, wheat) a model that looks like a solid
    block has no collision at all, so the player walks into it.

    python block_looks.py [--out <dir>] [--no-collision] [--shapes] [--dry-run] [--report <file>] <mod dir> ...
    python block_looks.py --check [--json] <built .hmod or mod dir> ...

Overlay mode reads `data/<ns>/horizon/block/*.json`, the blockstate and the models of the mod dir and writes the
changed block declarations under --out (same relative paths; the mod dir stays untouched; without --out the
files are changed in place; --dry-run only counts). The block declaration gets `properties.occludes: false`
and, unless --no-collision, `collision_shapes: {"default": [...]}` (and with --shapes the same box as
`shapes`, the targeting outline). Check mode reads built archives and exits 1 when a block still has the wrong
occlusion or collision.

The model is classified from its elements (not rotated ones cover a 16x16x16 grid) and from the vanilla parents
it names. Anything that cannot be resolved is left alone and counted as `unknown`.
"""
import json
import math
import sys
import zipfile
from pathlib import Path

# Templates that are a full opaque cube with full collision.
FULL_TEMPLATES = {
    'minecraft:stone', 'minecraft:carved_pumpkin', 'minecraft:oak_log', 'minecraft:redstone_lamp',
    'minecraft:furnace',
}
# Templates with no collision at all.
NO_COLLISION_TEMPLATES = {
    'minecraft:oak_pressure_plate', 'minecraft:oak_button', 'minecraft:vine', 'minecraft:wheat',
}
# Templates whose shape depends on their state (left alone, only counted).
STATE_TEMPLATES = {
    'minecraft:oak_trapdoor', 'minecraft:oak_stairs', 'minecraft:oak_door', 'minecraft:oak_fence_gate',
    'minecraft:oak_slab', 'minecraft:glass_pane', 'minecraft:oak_fence', 'minecraft:cobblestone_wall',
}
TRANSPARENT = {'cutout', 'cutout_mipped', 'translucent'}
CUBE_PARENTS = {
    'block/cube', 'block/cube_all', 'block/cube_column', 'block/cube_column_horizontal',
    'block/cube_bottom_top', 'block/cube_directional', 'block/cube_mirrored', 'block/cube_mirrored_all',
    'block/orientable', 'block/orientable_vertical', 'block/orientable_with_bottom', 'block/leaves',
    'block/cube_top', 'block/template_glass_pane_noside',
}
PLANT_PARENTS = {
    'block/cross', 'block/tinted_cross', 'block/crop', 'block/flower_pot_cross', 'block/tinted_flower_pot_cross',
    'block/template_seagrass', 'block/template_azalea',
}
THIN_PARENTS = {  # name -> synthetic elements (16 units)
    'block/carpet': [([0, 0, 0], [16, 1, 16])],
    'block/thin_block': [([0, 0, 0], [16, 1, 16])],
    'block/slab': [([0, 0, 0], [16, 8, 16])],
    'block/slab_top': [([0, 8, 0], [16, 16, 16])],
}
EPS = 1e-6


# ---- readers ---------------------------------------------------------------------------------

class DirReader:
    def __init__(self, root):
        self.root = Path(root)
        self.cache = {}

    def read_json(self, rel):
        if rel not in self.cache:
            path = self.root / rel
            try:
                self.cache[rel] = json.loads(path.read_text(encoding='utf-8')) if path.is_file() else None
            except (OSError, ValueError):
                self.cache[rel] = None
        return self.cache[rel]

    def list(self, prefix, suffix='.json'):
        base = self.root / prefix
        if not base.is_dir():
            return []
        return sorted(str(p.relative_to(self.root)).replace('\\', '/') for p in base.rglob('*') if p.is_file() and p.name.endswith(suffix))


class ZipReader:
    def __init__(self, path):
        self.zip = zipfile.ZipFile(path)
        self.names = sorted(n for n in self.zip.namelist() if not n.endswith('/'))
        self.cache = {}

    def read_json(self, rel):
        if rel not in self.cache:
            try:
                self.cache[rel] = json.loads(self.zip.read(rel).decode('utf-8')) if rel in self.zip.NameToInfo else None
            except (KeyError, ValueError):
                self.cache[rel] = None
        return self.cache[rel]

    def list(self, prefix, suffix='.json'):
        return [n for n in self.names if n.startswith(prefix) and n.endswith(suffix)]

    def close(self):
        self.zip.close()


def open_reader(path):
    path = Path(path)
    return ZipReader(path) if path.is_file() else DirReader(path)


# ---- models ----------------------------------------------------------------------------------

class Model:
    def __init__(self, kind, elements=None, render=None):
        self.kind = kind          # 'cube', 'plant', 'elements', 'geo' (no geometry), 'unknown'
        self.elements = elements or []   # list of (from, to, rotated)
        self.render = render


def split_id(model_id, default_ns):
    if ':' in model_id:
        ns, path = model_id.split(':', 1)
    else:
        ns, path = default_ns, model_id
    return ns, path


def parse_elements(raw):
    out = []
    for el in raw or []:
        try:
            lo = [float(v) for v in el['from']]
            hi = [float(v) for v in el['to']]
            if len(lo) != 3 or len(hi) != 3:
                continue
            rot = el.get('rotation') or {}
            rotated = abs(float(rot.get('angle', 0))) > EPS if isinstance(rot, dict) else False
            out.append((lo, hi, rotated))
        except (KeyError, TypeError, ValueError):
            continue
    return out


def load_model(reader, model_id, default_ns, cache):
    key = model_id
    if key in cache:
        return cache[key]
    cache[key] = Model('unknown')  # cycle guard
    elements = None
    render = None
    current = model_id
    result = None
    for _ in range(16):
        ns, path = split_id(current, default_ns)
        if ns == 'minecraft':
            if path in CUBE_PARENTS:
                result = Model('cube', None, render)
            elif path in PLANT_PARENTS:
                result = Model('plant', None, render)
            elif path in THIN_PARENTS and elements is None:
                result = Model('elements', [(lo, hi, False) for lo, hi in THIN_PARENTS[path]], render)
            elif elements is not None:
                result = Model('elements', elements, render)
            elif path == 'block/block':
                result = Model('geo', None, render)
            else:
                result = Model('unknown', None, render)
            break
        doc = reader.read_json(f'assets/{ns}/models/{path}.json')
        if doc is None:
            result = Model('elements', elements, render) if elements is not None else Model('unknown', None, render)
            break
        if render is None and isinstance(doc.get('render_type'), str):
            render = doc['render_type'].replace('minecraft:', '')
        if elements is None and 'elements' in doc:
            elements = parse_elements(doc['elements'])
        parent = doc.get('parent')
        if not parent:
            # no elements and no parent: a geometry-less model (drawn by a block entity / geo model)
            result = Model('elements', elements, render) if elements is not None else Model('geo', None, render)
            break
        current = parent
    if result is None:
        result = Model('unknown', None, render)
    if result.render is None:
        result.render = render
    cache[key] = result
    return result


def covers_cube(elements):
    cells = bytearray(4096)
    for lo, hi, rotated in elements:
        if rotated:
            continue
        x0, y0, z0 = (max(0, math.ceil(v - EPS)) for v in lo)
        x1, y1, z1 = (min(16, math.floor(v + EPS)) for v in hi)
        for x in range(x0, x1):
            for y in range(y0, y1):
                base = (x * 16 + y) * 16
                for z in range(z0, z1):
                    cells[base + z] = 1
    return all(cells)


def model_bbox(model):
    """Bounding box in block units [minX, minY, minZ, maxX, maxY, maxZ], or None (no collision at all)."""
    if model.kind == 'cube':
        return [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]
    if model.kind == 'plant':
        return None
    if model.kind != 'elements' or not model.elements:
        return None
    lo = [min(e[0][i] for e in model.elements) for i in range(3)]
    hi = [max(e[1][i] for e in model.elements) for i in range(3)]
    box = [min(1.0, max(0.0, v / 16.0)) for v in lo] + [min(1.0, max(0.0, v / 16.0)) for v in hi]
    box = [min(box[i], box[i + 3]) for i in range(3)] + [max(box[i], box[i + 3]) for i in range(3)]
    return [round(v, 4) for v in box]


def rotate_union(box):
    """The box united with its three 90 degree turns around the block centre (a model that faces any way)."""
    pts = []
    for angle in range(4):
        corners = [(box[0], box[2]), (box[3], box[2]), (box[0], box[5]), (box[3], box[5])]
        for x, z in corners:
            x, z = x - 0.5, z - 0.5
            for _ in range(angle):
                x, z = -z, x
            pts.append((x + 0.5, z + 0.5))
    xs = [p[0] for p in pts]
    zs = [p[1] for p in pts]
    return [round(min(xs), 4), box[1], round(min(zs), 4), round(max(xs), 4), box[4], round(max(zs), 4)]


def blockstate_models(reader, ns, name):
    """(list of model ids, rotated?) from the blockstate, or (None, False) when there is none."""
    doc = reader.read_json(f'assets/{ns}/blockstates/{name}.json')
    if not isinstance(doc, dict):
        return None, False
    models = []
    rotated = False

    def take(entry):
        nonlocal rotated
        if isinstance(entry, list):
            for sub in entry:
                take(sub)
        elif isinstance(entry, dict) and isinstance(entry.get('model'), str):
            models.append(entry['model'])
            if entry.get('y') not in (None, 0):
                rotated = True

    for entry in (doc.get('variants') or {}).values():
        take(entry)
    for part in doc.get('multipart') or []:
        if isinstance(part, dict):
            take(part.get('apply'))
    return (sorted(set(models)) or None), rotated


# ---- classification ----------------------------------------------------------------------------

class Look:
    def __init__(self):
        self.status = 'unknown'   # 'ok' or 'unknown'
        self.full = False         # every model is a full opaque-shaped cube
        self.transparent = False  # some model uses a cutout/translucent render type
        self.plant = False
        self.geo = False          # some model has no geometry of its own (drawn by a block entity)
        self.bbox = None          # union bbox (all turns when the blockstate rotates)
        self.boxes = []           # bbox of each model on its own (a button-like block has one per face)
        self.solid_models = False  # every model is a full cube or a substantial box on its own


def substantial(box):
    """A box that reads as a solid block: nearly a block tall and at least a quarter block of footprint."""
    return box is not None and (box[4] - box[1]) >= 0.9 and (box[3] - box[0]) * (box[5] - box[2]) >= 0.25


def analyze_models(reader, ns, name):
    look = Look()
    models, rotated = blockstate_models(reader, ns, name)
    if not models:
        return look
    cache = {}
    loaded = [load_model(reader, m, ns, cache) for m in models]
    # a model without elements (drawn by a block entity, geo models) tells nothing about its shape
    if any(m.kind == 'unknown' or (m.kind == 'elements' and not m.elements) for m in loaded):
        return look
    look.status = 'ok'
    look.plant = all(m.kind == 'plant' for m in loaded)
    look.geo = any(m.kind == 'geo' for m in loaded)
    look.transparent = any((m.render or '') in TRANSPARENT for m in loaded)
    look.full = all(m.kind == 'cube' or (m.kind == 'elements' and covers_cube(m.elements)) for m in loaded)
    per_model = [model_bbox(m) for m in loaded]
    look.boxes = [b for b in per_model if b is not None]
    look.solid_models = all(
        m.kind == 'cube' or (m.kind == 'elements' and (covers_cube(m.elements) or substantial(model_bbox(m))))
        for m in loaded)
    boxes = look.boxes
    if boxes:
        union = [min(b[i] for b in boxes) for i in range(3)] + [max(b[i] for b in boxes) for i in range(3, 6)]
        look.bbox = rotate_union(union) if rotated else [round(v, 4) for v in union]
    return look


def is_full_box(box):
    return box is not None and all(box[i] <= EPS for i in range(3)) and all(box[i] >= 1 - EPS for i in range(3, 6))


class Verdict:
    def __init__(self):
        self.look = Look()
        self.occludes_false = False
        self.collision = None      # None = leave, else list of boxes ([] = no collision)
        self.category = 'ok'       # ok, oversized, plant_empty, empty_solid, state_template, other_template, unknown


def decide(template, look):
    v = Verdict()
    v.look = look
    if template in STATE_TEMPLATES:
        v.category = 'state_template'
        return v
    if template not in FULL_TEMPLATES and template not in NO_COLLISION_TEMPLATES:
        v.category = 'other_template'
        return v
    if look.status != 'ok':
        v.category = 'unknown'
        return v
    if template in FULL_TEMPLATES:
        if not look.full or look.transparent:
            v.occludes_false = True
        if look.geo:
            v.category = 'geo'
        elif look.plant:
            v.collision = []
            v.category = 'plant_empty'
        elif not look.full and look.bbox is not None and not is_full_box(look.bbox):
            v.collision = [look.bbox]
            v.category = 'oversized'
        elif not look.full and look.bbox is None:
            v.collision = []
            v.category = 'plant_empty'
        return v
    # No-collision templates (button, plate, vine, crop): only a model that looks like a solid block in EVERY
    # state gets a collision. A cutout/translucent model (vines, hanging ornaments) stays walk-through, and a
    # button-like block whose faces use different models is judged model by model, not by their union.
    if look.plant or look.bbox is None or look.geo or look.transparent or not look.solid_models:
        return v
    v.collision = [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]] if look.full else [look.bbox]
    v.category = 'empty_solid'
    return v


def block_ids(reader):
    """(ns, name) of every block declaration in the reader."""
    out = []
    for rel in reader.list('data/', '.json'):
        parts = rel.split('/')
        if len(parts) >= 5 and parts[2] == 'horizon' and parts[3] == 'block':
            out.append((parts[1], '/'.join(parts[4:])[:-5]))
    return out


# ---- overlay ------------------------------------------------------------------------------------

def new_counts():
    return {'blocks': 0, 'occludes_false': 0, 'oversized': 0, 'plant_empty': 0, 'empty_solid': 0,
            'geo': 0, 'state_template': 0, 'slab_mismatch': 0, 'other_template': 0, 'unknown': 0, 'written': 0}


def process(root, out=None, collision=True, shapes=False, dry_run=False):
    """Overlay one mod dir. Returns {ns: counts}."""
    reader = DirReader(root)
    counts = {}
    for ns, name in block_ids(reader):
        rel = f'data/{ns}/horizon/block/{name}.json'
        decl = reader.read_json(rel)
        c = counts.setdefault(ns, new_counts())
        c['blocks'] += 1
        if not isinstance(decl, dict) or 'template' not in decl:
            continue
        look = analyze_models(reader, ns, name)
        verdict = decide(decl['template'], look)
        if verdict.category in ('state_template', 'other_template', 'unknown'):
            c[verdict.category] += 1
        if decl['template'] == 'minecraft:oak_slab' and look.status == 'ok' and not look.geo and look.bbox is not None:
            # diagnostic only (a slab's collision depends on its state): its collision is half a block tall
            if abs((look.bbox[4] - look.bbox[1]) - 0.5) > 0.15:
                c['slab_mismatch'] += 1
        if verdict.category in ('oversized', 'plant_empty', 'empty_solid', 'geo'):
            c[verdict.category] += 1
        if verdict.occludes_false:
            c['occludes_false'] += 1
        want_collision = collision and verdict.collision is not None and 'collision_shapes' not in decl
        want_occ = verdict.occludes_false and (decl.get('properties') or {}).get('occludes') is not False
        if not (want_collision or want_occ):
            continue
        c['written'] += 1
        if dry_run:
            continue
        new = json.loads(json.dumps(decl))
        if want_occ:
            props = new.get('properties')
            if not isinstance(props, dict):
                props = {}
            props['occludes'] = False
            new['properties'] = props
        if want_collision:
            boxes = [list(b) for b in verdict.collision]
            new['collision_shapes'] = {'default': boxes}
            if shapes and boxes and 'shapes' not in new:
                new['shapes'] = {'default': boxes}
        target = (Path(out) / rel) if out else (Path(root) / rel)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(new, indent=2) + '\n', encoding='utf-8')
    return counts


# ---- check --------------------------------------------------------------------------------------

def check(path):
    """Checks one built archive or mod dir. Returns (violations list, per-ns counts of blocks checked)."""
    reader = open_reader(path)
    violations = []
    checked = {}
    try:
        for ns, name in block_ids(reader):
            decl = reader.read_json(f'data/{ns}/horizon/block/{name}.json')
            if not isinstance(decl, dict) or 'template' not in decl:
                continue
            template = decl['template']
            if template in STATE_TEMPLATES or (template not in FULL_TEMPLATES and template not in NO_COLLISION_TEMPLATES):
                continue
            look = analyze_models(reader, ns, name)
            if look.status != 'ok':
                continue
            checked[ns] = checked.get(ns, 0) + 1
            verdict = decide(template, look)
            props = decl.get('properties') if isinstance(decl.get('properties'), dict) else {}
            coll = decl.get('collision_shapes')
            default_boxes = coll.get('default') if isinstance(coll, dict) else None
            if verdict.occludes_false and props.get('occludes') is not False:
                violations.append((ns, name, 'occludes', 'non-cube or transparent model on an occluding template'))
            if verdict.collision is not None and coll is None and 'shapes' not in decl:
                violations.append((ns, name, verdict.category, 'collision should be ' + json.dumps(verdict.collision)))
            if look.full and not look.transparent and default_boxes == []:
                violations.append((ns, name, 'solid_empty', 'full cube model but an empty collision'))
            if verdict.category == 'empty_solid' and coll is not None and default_boxes == []:
                violations.append((ns, name, 'solid_empty', 'solid-looking model but an empty collision'))
    finally:
        if hasattr(reader, 'close'):
            reader.close()
    return violations, checked


def main(argv):
    args = list(argv)
    out = None
    collision = True
    shapes = False
    dry = False
    report = None
    do_check = False
    as_json = False
    while args and args[0].startswith('--'):
        flag = args.pop(0)
        if flag == '--out':
            out = Path(args.pop(0))
        elif flag == '--no-collision':
            collision = False
        elif flag == '--shapes':
            shapes = True
        elif flag == '--dry-run':
            dry = True
        elif flag == '--report':
            report = Path(args.pop(0))
        elif flag == '--check':
            do_check = True
        elif flag == '--json':
            as_json = True
        else:
            print(f'unknown option {flag}', file=sys.stderr)
            return 2
    if do_check:
        total = 0
        result = {}
        for path in args:
            try:
                violations, checked = check(path)
            except RuntimeError as error:  # the bundled python can lack zlib and cannot read deflated archives
                print(f'cannot read {path}: {error}. Extract the archive and pass the folder.', file=sys.stderr)
                return 2
            total += len(violations)
            result[str(path)] = {'checked': sum(checked.values()), 'violations': [list(v) for v in violations[:200]],
                                 'violation_count': len(violations)}
            if not as_json:
                print(f'{Path(path).name}: {sum(checked.values())} blocks checked, {len(violations)} violation(s)')
                for ns, name, kind, why in violations[:20]:
                    print(f'  {ns}:{name} [{kind}] {why}')
        if as_json:
            print(json.dumps(result, indent=2))
        return 1 if total else 0
    total_counts = new_counts()
    per_ns = {}
    for arg in args:
        for ns, counts in process(Path(arg), out, collision, shapes, dry).items():
            into = per_ns.setdefault(ns, new_counts())
            for k, v in counts.items():
                into[k] += v
                total_counts[k] += v
    line = ('block_looks: ' + ', '.join(f'{k}={v}' for k, v in total_counts.items()))
    print(line)
    if report:
        lines = [line] + [f'{ns}: ' + ', '.join(f'{k}={v}' for k, v in c.items()) for ns, c in sorted(per_ns.items())]
        report.write_text('\n'.join(lines) + '\n', encoding='utf-8')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
