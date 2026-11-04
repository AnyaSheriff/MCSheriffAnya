// Странствующий торговец: minecraft.wiki/w/Traveling_merchant и
// Java_Edition_1.21.5, таблица сделок. [проверить] Расстановка и таймер питья.
use super::{Event, Mob, Rng};
use crate::{blocks, inventory::Stack, villager::Offer};

#[derive(Debug)]
pub struct Spawner { pub wait: i32, pub chance: u32 }
impl Default for Spawner { fn default() -> Self { Self { wait: 24000, chance: 25 } } }
impl Spawner {
    pub fn tick(&mut self, enabled: bool, spawning: bool, players: bool, rng: &mut Rng) -> bool {
        if !enabled { return false; }
        self.wait -= 1;
        if self.wait > 0 { return false; }
        self.wait = 24000;
        if !spawning { return false; }
        let chance = self.chance;
        self.chance = (self.chance + 25).min(75);
        if rng.below(100) >= chance { return false; }
        if !players { self.chance=25; return false; }
        // Вики: после проверки шанса ещё 1/10 на выбранного игрока.
        rng.below(10) == 0
    }
    pub fn spawned(&mut self) { self.chance = 25; }
}
pub fn think(mob: &mut Mob, day: bool) {
    if mob.land.drinking > 0 {
        mob.land.drinking -= 1;
        if mob.land.drinking == 0 {
            mob.land.invisible = !day;
            mob.attr_changes=mob.attr_changes.wrapping_add(1);
            mob.events.push(Event::Sound { name: if day { "minecraft:entity.wandering_trader.drink_milk" } else { "minecraft:entity.wandering_trader.drink_potion" }, volume: 1.0, pitch: 1.0 });
        }
    } else if mob.land.invisible == day { mob.land.drinking = 32; mob.attr_changes=mob.attr_changes.wrapping_add(1); }
}
fn offer(buy: &str, count: i32, sell: &str, amount: i32, uses: i32) -> Option<Offer> {
    offer_with_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2), buy, count, sell, amount, uses)
}
fn offer_with_catalog(catalog: crate::gameplay_catalog::Catalog, buy: &str, count: i32, sell: &str, amount: i32, uses: i32) -> Option<Offer> {
    Some(Offer { buy: Stack::new(catalog.item_named(buy).ok()??, count), buy_b: None,
        sell: Stack::new(catalog.item_named(sell).ok()??, amount), uses: 0, max_uses: uses,
        xp: 0, reward_exp: true, multiplier: 0.0, special: 0, demand: 0 })
}
fn choose(pool: &mut Vec<Offer>, n: usize, out: &mut Vec<Offer>, rng: &mut Rng) {
    for _ in 0..n.min(pool.len()) { let i = rng.below(pool.len() as u32) as usize; out.push(pool.swap_remove(i)); }
}
#[cfg(test)]
pub fn offers(rng: &mut Rng) -> Vec<Offer> {
    offers_for_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2), rng)
}

pub fn offers_for_catalog(catalog: crate::gameplay_catalog::Catalog, rng: &mut Rng) -> Vec<Offer> {
    let mut out = Vec::new();
    let mut buying: Vec<_> = [("baked_potato",4,1),("fermented_spider_eye",1,3),("hay_block",1,1),("milk_bucket",1,2),("potion",1,1),("water_bucket",1,2)]
        .into_iter().filter_map(|(item,n,emerald)| offer(item,n,"emerald",emerald,2)).collect();
    // Вода — potion id 0; существующая таблица зелий используется без дампов.
    for o in &mut buying { if blocks::item_name(o.buy.item)==Some("potion") { let mut d=o.buy.extras(); d.potion=Some(0); o.buy=o.buy.with_extras(d); } }
    choose(&mut buying,2,&mut out,rng);
    let mut special = Vec::new();
    for name in ["acacia_log","birch_log","cherry_log","dark_oak_log","jungle_log","oak_log","spruce_log","pale_oak_log"] {
        if let Some(o)=offer("emerald",1,name,8,4) { special.push(o); }
    }
    if catalog.changed_rules() && let Some(o) = offer_with_catalog(catalog,"emerald",1,"poplar_log",8,4) { special.push(o); }
    for (name,cost,count,uses) in [("gunpowder",1,4,2),("packed_ice",1,1,6),("blue_ice",6,1,6),("podzol",3,3,6),("potion",5,1,1)] {
        if let Some(mut o)=offer("emerald",cost,name,count,uses) {
            if name=="potion" { let mut d=o.sell.extras(); d.potion=Some(7); o.sell=o.sell.with_extras(d); }
            special.push(o);
        }
    }
    // [проверить] Зачарованная кирка: пул зачарований требует отдельной сверки.
    if let Some(mut o)=offer("emerald",6+rng.below(15) as i32,"iron_pickaxe",1,1) {
        o.sell=crate::enchanting::merchant_enchant(o.sell,o.buy.count-1,rng.below(u32::MAX) as i64); special.push(o);
    }
    choose(&mut special,2,&mut out,rng);
    // [проверить] Полная таблица 26.1, включая запас новых бледных растений.
    let mut ordinary = Vec::new();
    for (name,cost,count,uses) in [
        ("fern",1,1,12),("dandelion",1,1,12),("poppy",1,1,12),("blue_orchid",1,1,8),("allium",1,1,12),
        ("azure_bluet",1,1,12),("red_tulip",1,1,12),("orange_tulip",1,1,12),("white_tulip",1,1,12),("pink_tulip",1,1,12),
        ("oxeye_daisy",1,1,12),("cornflower",1,1,12),("lily_of_the_valley",1,1,7),("wheat_seeds",1,1,12),
        ("beetroot_seeds",1,1,12),("pumpkin_seeds",1,1,12),("melon_seeds",1,1,12),("sugar_cane",1,1,8),
        ("pumpkin",1,1,4),("kelp",3,1,12),("cactus",3,1,8),("vine",1,3,4),("brown_mushroom",1,3,4),
        ("red_mushroom",1,3,4),("lily_pad",1,5,2),("sand",1,8,8),("red_sand",1,4,6),
        ("open_eyeblossom",1,1,12),("pale_moss_block",1,2,5),("pale_hanging_moss",1,2,5),
        ("ink_sac",1,3,12),("lapis_lazuli",1,3,12),("cocoa_beans",1,3,12),("bone_meal",1,3,12),
        ("pointed_dripstone",1,2,5),("rooted_dirt",1,2,5),("moss_block",1,2,5),("small_dripleaf",1,2,5),
        ("slime_ball",4,1,5),("glowstone",2,1,5),("nautilus_shell",5,1,5),("pufferfish_bucket",3,1,4),("tropical_fish_bucket",3,1,4),
        ("sea_pickle",2,1,5),("oak_sapling",5,1,8),("spruce_sapling",5,1,8),("birch_sapling",5,1,8),
        ("jungle_sapling",5,1,8),("acacia_sapling",5,1,8),("dark_oak_sapling",5,1,8),("cherry_sapling",5,1,8),
        ("pale_oak_sapling",5,1,8),("mangrove_propagule",5,1,8),
        ("tube_coral_block",3,1,8),("brain_coral_block",3,1,8),("bubble_coral_block",3,1,8),("fire_coral_block",3,1,8),("horn_coral_block",3,1,8),
    ] { if let Some(o)=offer("emerald",cost,name,count,uses) { ordinary.push(o); } }
    for color in ["orange","magenta","light_blue","yellow","lime","pink","gray","light_gray","cyan","purple","green","red"] {
        if let Some(o)=offer("emerald",1,&format!("{color}_dye"),3,12) { ordinary.push(o); }
    }
    // Вики, Trading: стоимость, число и запас новых предложений 26.2/26.3.
    if catalog.changed_rules() {
        for (name,cost,count,uses) in [("sulfur_spike",1,2,5),("shelf_mushroom",1,3,12),("poplar_sapling",5,1,8)] {
            if let Some(o)=offer_with_catalog(catalog,"emerald",cost,name,count,uses) { ordinary.push(o); }
        }
    }
    choose(&mut ordinary,5,&mut out,rng); out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_trader_pool_contains_new_offers_without_changing_native_pool() {
        use crate::gameplay_catalog::{Catalog, Profile};
        let names = ["sulfur_spike", "shelf_mushroom", "poplar_log", "poplar_sapling"];
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let mut found = [false;4];
            for seed in 0..400 {
                let pool = offers_for_catalog(Catalog::new(profile), &mut Rng::new(seed));
                assert_eq!(pool.len(), 9);
                for offer in pool {
                    for (i, name) in names.iter().enumerate() {
                        if Catalog::new(profile).name(crate::gameplay_catalog::Kind::Item,offer.sell.item) == Ok(*name) { found[i] = true; }
                    }
                }
            }
            assert_eq!(found, [profile == Profile::Preview26_3;4]);
        }
    }
    #[test] fn timer_freezes_and_chance_is_bounded() {
        let mut s=Spawner::default(); let mut rng=Rng::new(8);
        for _ in 0..24000 { assert!(!s.tick(false,true,true,&mut rng)); }
        assert_eq!(s.wait,24000);
        for _ in 0..24000*5 { s.tick(true,true,true,&mut rng); }
        assert_eq!(s.chance,75); s.spawned(); assert_eq!(s.chance,25);
    }
    #[test] fn nine_offers_in_three_groups_with_finite_stock() {
        let mut rng=Rng::new(42);
        for _ in 0..100 { let o=offers(&mut rng); assert_eq!(o.len(),9); assert!(o.iter().all(|o| o.max_uses>0)); assert!(o[..2].iter().all(|o| blocks::item_name(o.sell.item)==Some("emerald"))); }
    }
}
