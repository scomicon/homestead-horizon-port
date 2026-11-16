"""Placeholder sound events for the client sound mods of the native port (stage 20).

The mods rebuilt in mixins/homestead (sound_muffler.rs, sound_ambient.rs, sound_footsteps.rs) ship no
sounds of the originals (AmbientSounds, Presence Footsteps are not redistributable). To try and
test the engine this writes a very small content mod `homestead_sound` with three sound events
that point at vanilla sound events (type "event" in sounds.json), so no audio file of anyone is
shipped or copied:

* homestead_sound:tone.low  -> minecraft:block.note_block.bass
* homestead_sound:tone.mid  -> minecraft:block.note_block.harp
* homestead_sound:tone.high -> minecraft:block.note_block.chime

Real ambience and footstep sounds come from the player's own resource pack: put its events in
config/homestead-ambient.txt and config/homestead-footsteps.txt.

    python sound_content.py <out dir>

The out dir is zipped flat into stage4/extra-mods by stage4/Build-SoundMod.ps1.
"""
import json
import sys
from pathlib import Path

NS = 'homestead_sound'
TONES = {
    'tone.low': 'minecraft:block.note_block.bass',
    'tone.mid': 'minecraft:block.note_block.harp',
    'tone.high': 'minecraft:block.note_block.chime',
}


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, str):
        path.write_text(value, encoding='utf-8')
    else:
        path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')


def build(out):
    out = Path(out)
    _write(out / 'horizon-mod.toml', (
        '# Written by tools/sound_content.py: placeholder sound events for the client sound mods.\n'
        f'id = "{NS}"\n'
        'name = "Homestead Sound"\n'
        'version = "0.1.0"\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n'
    ))
    assets = out / 'assets' / NS
    _write(assets / 'sounds.json', {
        event: {'sounds': [{'name': target, 'type': 'event'}]}
        for event, target in TONES.items()
    })
    _write(assets / 'lang/en_us.json', {})
    print('homestead sound: %d placeholder events' % len(TONES))


if __name__ == '__main__':
    build(sys.argv[1])
