# Local engine patch applied to the staged engine after `horizon-mixins apply` (stage 27, "engine20").
#
# Grind Enchantments needs the grindstone's result being TAKEN to be changeable: vanilla's GrindstoneMenu
# result `onTake` (menu.rs, `Role::GrindResult`) clears BOTH inputs and hands out experience from them. For the
# mod's result (an enchanted item plus a plain book gives an enchanted book carrying its enchantments) the
# item must stay in the first slot without them, the book is used up, the player pays a level cost and no
# experience is awarded. The result slot itself is answered by an injection into
# `stations::grindstone_result` (bundle module `homestead_grind`), so the preview, the host's menu and a
# LAN guest's menu all show the same result; this patch makes the take match.
#
# Needs the bundle module `homestead_grind` in stations.rs (applied by horizon-mixins before this script). The
# script refuses to run if it is missing or if the anchor changed. Idempotent.
param([Parameter(Mandatory)][string] $Stage)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$stations = [IO.File]::ReadAllText((Join-Path $Stage 'crates/horizon-client/src/stations.rs'))
if (-not $stations.Contains('homestead_grind')) {
    throw 'Engine patch 22-grind-take needs the homestead bundle (module homestead_grind in stations.rs): run horizon-mixins apply with mixins/homestead first.'
}
$path = Join-Path $Stage 'crates/horizon-client/src/menu.rs'
$text = [IO.File]::ReadAllText($path)
$marker = 'homestead: a grindstone result of Grind Enchantments'
if ($text.Contains($marker)) { return }
$anchor = ('            self.put(env, 0, None);' + "`n" +
           '            self.put(env, 1, None);' + "`n" +
           '            return;' + "`n" +
           '        }' + "`n" +
           '        if self.slots[s].role == Role::CombineResult && self.kind == Kind::Anvil {')
if (-not $text.Contains($anchor)) { throw 'Engine patch anchor missing in crates/horizon-client/src/menu.rs (stage 27 grind take patch)' }
$replacement = ('            // ' + $marker + ' leaves the stripped item in the first slot, uses the book up,' + "`n" +
                '            // charges the levels (not in creative) and awards no experience.' + "`n" +
                '            if let Some(plan) = crate::stations::homestead_grind::plan(' + "`n" +
                '                env.enchantments,' + "`n" +
                '                self.containers[MAIN][0].as_ref(),' + "`n" +
                '                self.containers[MAIN][1].as_ref(),' + "`n" +
                '            ) {' + "`n" +
                '                self.ground = Some(0);' + "`n" +
                '                if !self.creative {' + "`n" +
                '                    self.level_change -= plan.cost.clamp(0, self.levels.max(0));' + "`n" +
                '                }' + "`n" +
                '                self.put(env, 0, plan.remaining);' + "`n" +
                '                self.put(env, 1, None);' + "`n" +
                '                return;' + "`n" +
                '            }' + "`n" +
                $anchor)
[IO.File]::WriteAllText($path, $text.Replace($anchor, $replacement), $utf8)
