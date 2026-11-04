// Особые рецепты по minecraft.wiki: Dye, Firework Star, Map, Written Book,
// Banner, Item repair, Decorated Pot. «5% of the tool's maximum durability».
// [проверить] Порядок довесков и округление цветов — сценариями чёрного ящика.
use crate::{blocks, inventory::Stack, stackdata::{Data, Explosion, Fireworks}};

pub const DYE_COLORS: [i32; 16] = [0xf9fffe,0xf9801d,0xc74ebd,0x3ab3da,0xfed83d,0x80c71f,0xf38baa,0x474f52,0x9d9d97,0x169c9c,0x8932b8,0x3c44aa,0x835432,0x5e7c16,0xb02e26,0x1d1d21];
const FIREWORK_COLORS: [i32; 16] = [15790320,15435844,12801229,6719955,14602026,4312372,14188952,4408131,11250603,2651799,8073150,2437522,5320730,3887386,11743532,1973019];
fn name(s: &Stack) -> &'static str { blocks::item_name(s.item).unwrap_or("") }
fn dye(s: &Stack) -> Option<usize> { let n = name(s).strip_suffix("_dye")?; crate::sign::COLORS.iter().position(|c| *c == n) }
fn output(n: &str, count: i32, data: Data) -> Option<Stack> { Some(Stack::new(blocks::item_named(n)?, count).with_extras(data)) }

pub fn craft(grid: &[Option<Stack>]) -> Option<Stack> {
    let items: Vec<_> = grid.iter().flatten().copied().collect();
    if items.is_empty() { return None; }
    let n = items.len();
    let count = |s: &str| items.iter().filter(|i| name(i) == s).count();
    let dyes: Vec<_> = items.iter().filter_map(dye).collect();
    let others: Vec<_> = items.iter().filter(|s| dye(s).is_none()).copied().collect();
    if !dyes.is_empty() && others.len() == 1 {
        let source = others[0];
        let mut data = source.extras();
        if matches!(name(&source), "leather_helmet" | "leather_chestplate" | "leather_leggings" | "leather_boots" | "leather_horse_armor" | "wolf_armor") {
            let colors: Vec<_> = dyes.iter().map(|i| DYE_COLORS[*i]).chain(data.dyed_color).collect();
            let mut sums = [0; 3]; let mut brightness = 0;
            for c in &colors { let rgb = [(c >> 16) & 255, (c >> 8) & 255, c & 255]; for i in 0..3 { sums[i] += rgb[i]; } brightness += rgb.iter().max().unwrap(); }
            for c in &mut sums { *c /= colors.len() as i32; }
            let gain = brightness as f32 / colors.len() as f32 / *sums.iter().max().unwrap() as f32;
            data.dyed_color = Some(sums.iter().fold(0, |v,c| (v << 8) | (*c as f32 * gain) as i32));
            return Some(source.with_count(1).with_extras(data));
        }
        if name(&source) == "firework_star" {
            let explosion = data.explosion.as_mut()?;
            explosion.fade_colors = dyes.iter().map(|i| FIREWORK_COLORS[*i]).collect();
            return Some(source.with_count(1).with_extras(data));
        }
        if dyes.len() == 1 {
            let color = crate::sign::COLORS[dyes[0]];
            if crate::containers::is_shulker_item(source.item) { return output(&format!("{color}_shulker_box"), 1, data); }
            if crate::bundle::is_bundle(source.item) { return output(&format!("{color}_bundle"), 1, data); }
        }
    }
    if count("paper") == 1 && (1..=3).contains(&count("gunpowder")) && items.iter().all(|s| matches!(name(s), "paper" | "gunpowder" | "firework_star")) {
        let mut data = Data::default();
        data.set_rocket(Fireworks { flight: count("gunpowder") as i8, explosions: items.iter().filter(|s| name(s) == "firework_star").filter_map(|s| s.extras().explosion).collect() });
        return output("firework_rocket", 3, data);
    }
    if count("gunpowder") == 1 && !dyes.is_empty() {
        let mut explosion = Explosion { colors: dyes.iter().map(|i| FIREWORK_COLORS[*i]).collect(), ..Default::default() };
        let mut shape = false;
        for s in &others {
            match name(s) {
                "gunpowder" => {},
                "diamond" if !explosion.trail => explosion.trail = true,
                "glowstone_dust" if !explosion.twinkle => explosion.twinkle = true,
                "fire_charge" | "gold_nugget" | "feather" | "skeleton_skull" | "wither_skeleton_skull" | "creeper_head" | "player_head" | "zombie_head" | "dragon_head" | "piglin_head" if !shape => {
                    shape = true; explosion.shape = match name(s) { "fire_charge" => 1, "gold_nugget" => 2, "feather" => 4, _ => 3 };
                }
                _ => return None,
            }
        }
        return output("firework_star", 1, Data { explosion: Some(explosion), ..Default::default() });
    }
    // Копия карты, в том числе карты строения 26.3 (вики, Explorer Map: «clone by crafting with an Empty Map»).
    let is_map = |s: &Stack| name(s) == "filled_map" || crate::maps::is_structure_map(name(s));
    if items.iter().filter(|s| is_map(s)).count() == 1 && count("map") + 1 == n && n >= 2 {
        return Some(items.iter().find(|s| is_map(s))?.with_count(n as i32));
    }
    if count("written_book") == 1 && count("writable_book") + 1 == n && n >= 2 {
        let book = items.iter().find(|s| name(s) == "written_book")?;
        let mut data = book.extras(); let contents = data.book.as_mut()?;
        if contents.generation >= 2 { return None; }
        contents.generation += 1;
        return Some(book.with_count((n - 1) as i32).with_extras(data));
    }
    if n == 2 && items[0].item == items[1].item {
        let a = items[0].extras(); let b = items[1].extras();
        if name(&items[0]).ends_with("_banner") {
            let patterned = |d: &Data| !d.banner_patterns.is_empty() || d.ominous_banner;
            if patterned(&a) != patterned(&b) {
                let original = if patterned(&a) { items[0] } else { items[1] };
                let data = original.extras();
                let layers = if data.ominous_banner { crate::stackdata::OMINOUS_BANNER_LAYERS.len() } else { data.banner_patterns.len() };
                if layers <= 6 { return Some(original.with_count(1)); }
            }
            return None;
        }
        let max = crate::combat::max_damage(&items[0]);
        if max > 0 && items.iter().all(|s| s.count == 1) {
            let mut data = Data { damage: (a.damage.clamp(0,max) + b.damage.clamp(0,max) - max - max * 5 / 100).max(0), ..Default::default() };
            // Вики, Item repair: «curses are retained».
            for curse in ["binding_curse", "vanishing_curse"] { let level = a.level(curse).max(b.level(curse)); if level > 0 { data.set_enchant(crate::stackdata::enchantment_number(curse)?, level); } }
            return Some(items[0].with_extras(data));
        }
    }
    if grid.len() == 9 && n == 9 && grid[4].is_some_and(|s| name(&s) == "lingering_potion") && grid.iter().enumerate().all(|(i,s)| i == 4 || s.is_some_and(|s| name(&s) == "arrow")) {
        let potion = grid[4]?.extras();
        return output("tipped_arrow", 8, Data { potion: potion.potion, potion_extra: potion.potion_extra, ..Default::default() });
    }
    if grid.len() == 9 && n == 4 && grid.iter().enumerate().all(|(i,s)| if [1,3,5,7].contains(&i) { s.is_some_and(|s| name(&s) == "brick" || name(&s).ends_with("_pottery_sherd")) } else { s.is_none() }) {
        return output("decorated_pot", 1, Data { pot_decorations: Some([grid[1]?.item, grid[3]?.item, grid[5]?.item, grid[7]?.item]), ..Default::default() });
    }
    if grid.len() == 9 && n == 9 && grid[4].is_some_and(|s| name(&s) == "filled_map") && grid.iter().enumerate().all(|(i,s)| i == 4 || s.is_some_and(|s| name(&s) == "paper")) {
        let map = grid[4]?; let mut data = map.extras(); data.map_post = Some(1);
        return Some(map.with_count(1).with_extras(data));
    }
    None
}

/// Копируемый оригинал не расходуется (вики, Written Book/Banner: «not consumed»).
pub fn remainders(grid: &[Option<Stack>]) -> Vec<Option<Stack>> {
    grid.iter().map(|slot| slot.and_then(|s| {
        if name(&s) == "written_book" || (name(&s).ends_with("_banner") && grid.iter().flatten().filter(|i| name(i).ends_with("_banner")).count() == 2 && (!s.extras().banner_patterns.is_empty() || s.extras().ominous_banner)) { Some(s.with_count(1)) }
        else { super::remainder(s.item).map(|item| Stack::new(item, 1)) }
    })).collect()
}
