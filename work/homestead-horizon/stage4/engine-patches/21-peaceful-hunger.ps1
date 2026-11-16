# Local engine patch applied to the staged engine after `horizon-mixins apply` (stage 27, "engine20").
#
# Peaceful Hunger: in Peaceful the engine's `peaceful_regeneration` (vanilla's ServerPlayer.tickRegeneration
# branch) also refills the food bar (+1 every 10 ticks) and the saturation (+1 every second), so the hunger
# bar never matters. The mod keeps hunger real in Peaceful. The engine gets one switch,
# `horizon_client::hunger::PEACEFUL_HUNGER` (default false = vanilla): when it is true the food and
# saturation refill are skipped (the health regeneration stays). The Homestead bundle sets it from
# `config/homestead-hunger.txt` (module homestead_peaceful_hunger); the default there is on, as in the pack.
#
# Idempotent; refuses to run if an anchor is missing.
param([Parameter(Mandatory)][string] $Stage)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
function Patch([string] $Relative, [string] $Anchor, [string] $Replacement, [string] $Marker) {
    $path = Join-Path $Stage $Relative
    $text = [IO.File]::ReadAllText($path)
    if ($text.Contains($Marker)) { return }
    if (-not $text.Contains($Anchor)) { throw "Engine patch anchor missing in $Relative (stage 27 peaceful hunger patch, marker: $Marker)" }
    [IO.File]::WriteAllText($path, $text.Replace($Anchor, $Replacement), $utf8)
}
$file = 'crates/horizon-client/src/hunger.rs'

Patch $file `
    '/// A living entity is about to heal. Mods may change the amount or cancel.' `
    ('/// homestead: true keeps hunger real in Peaceful (no food or saturation refill from regeneration).' + "`n" +
     'pub static PEACEFUL_HUNGER: std::sync::atomic::AtomicBool =' + "`n" +
     '    std::sync::atomic::AtomicBool::new(false);' + "`n" + "`n" +
     '/// A living entity is about to heal. Mods may change the amount or cancel.') `
    'pub static PEACEFUL_HUNGER'

Patch $file `
    ('            let saturation = self.health.food.saturation;' + "`n" +
     '            if saturation < 20.0 {' + "`n" +
     '                self.health.food.saturation = saturation + 1.0;' + "`n" +
     '            }' + "`n" +
     '        }' + "`n" +
     '        if self.health.tick_count % 10 == 0 && self.health.food.needs_food() {') `
    ('            let saturation = self.health.food.saturation;' + "`n" +
     '            let refill = !PEACEFUL_HUNGER.load(std::sync::atomic::Ordering::Relaxed);' + "`n" +
     '            if saturation < 20.0 && refill {' + "`n" +
     '                self.health.food.saturation = saturation + 1.0;' + "`n" +
     '            }' + "`n" +
     '        }' + "`n" +
     '        if self.health.tick_count % 10 == 0' + "`n" +
     '            && self.health.food.needs_food()' + "`n" +
     '            && !PEACEFUL_HUNGER.load(std::sync::atomic::Ordering::Relaxed)' + "`n" +
     '        {') `
    '&& !PEACEFUL_HUNGER.load'

# Vanilla FoodData.tick only takes a food point away when the difficulty is not Peaceful, so with the refill
# skipped the bar would still never go down. With the switch on, a Peaceful world reports Easy to the food
# tick: exhaustion drains saturation and then food as in the other difficulties (starvation then follows
# Easy's rules: it stops at five hearts).
Patch $file `
    ('    fn difficulty(&mut self, _: (), _: &mut FoodData) -> Result<Difficulty> {' + "`n" +
     '        Ok(self.rules.difficulty)' + "`n" +
     '    }') `
    ('    fn difficulty(&mut self, _: (), _: &mut FoodData) -> Result<Difficulty> {' + "`n" +
     '        // homestead: with Peaceful Hunger a Peaceful world drains hunger like Easy.' + "`n" +
     '        if PEACEFUL_HUNGER.load(std::sync::atomic::Ordering::Relaxed)' + "`n" +
     '            && self.rules.difficulty == Difficulty::Peaceful' + "`n" +
     '        {' + "`n" +
     '            return Ok(Difficulty::Easy);' + "`n" +
     '        }' + "`n" +
     '        Ok(self.rules.difficulty)' + "`n" +
     '    }') `
    'drains hunger like Easy'