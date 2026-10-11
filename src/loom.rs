// Ткацкий станок: minecraft.wiki/w/Loom — «pattern is not consumed».
// Порядок — алфавитный, как у реестра оригинала (чёрный ящик loom-banner, shield-banner-craft: creeper — 4, border — 1).
use crate::{blocks, inventory::Stack};

pub const PATTERNS: [&str; 43] = [
    "base", "border", "bricks", "circle", "creeper", "cross",
    "curly_border", "diagonal_left", "diagonal_right", "diagonal_up_left", "diagonal_up_right", "flow",
    "flower", "globe", "gradient", "gradient_up", "guster", "half_horizontal",
    "half_horizontal_bottom", "half_vertical", "half_vertical_right", "mojang", "piglin", "rhombus",
    "skull", "small_stripes", "square_bottom_left", "square_bottom_right", "square_top_left", "square_top_right",
    "straight_cross", "stripe_bottom", "stripe_center", "stripe_downleft", "stripe_downright", "stripe_left",
    "stripe_middle", "stripe_right", "stripe_top", "triangle_bottom", "triangle_top", "triangles_bottom",
    "triangles_top",
];
pub const BEDROCK: [&str; 43] = ["b","bo","bri","mc","cre","cr","cbo","ld","rd","lud","rud","flw","flo","glb","gra","gru","gus","hh","hhb","vh","vhr","moj","pig","mr","sku","ss","bl","br","tl","tr","sc","bs","cs","dls","drs","ls","ms","rs","ts","bt","tt","bts","tts"];
pub const SPECIAL: [(&str, u8); 10] = [("bordure_indented",6),("field_masoned",2),("creeper",4),("skull",24),("flower",12),("mojang",21),("globe",13),("piglin",22),("flow",11),("guster",16)];
pub fn pattern_item(item: i32) -> Option<u8> {
    let name = blocks::item_name(item)?.strip_suffix("_banner_pattern")?;
    SPECIAL.iter().find(|(n,_)| *n == name).map(|(_,p)| *p)
}
pub fn choices(template: Option<Stack>) -> Vec<u8> {
    match template {
        Some(s) => pattern_item(s.item).into_iter().collect(),
        None => (1..43).filter(|p| !SPECIAL.iter().any(|(_,s)| s == p)).collect(),
    }
}
pub fn may_place(slot: usize, stack: &Stack) -> bool {
    match slot {
        0 => blocks::item_name(stack.item).is_some_and(|n| n.ends_with("_banner")),
        1 => blocks::item_name(stack.item).and_then(crate::sign::dye_color).is_some(),
        2 => pattern_item(stack.item).is_some(),
        _ => false,
    }
}
pub fn result(banner: Option<Stack>, dye: Option<Stack>, template: Option<Stack>, selected: i32) -> Option<Stack> {
    let (banner, dye) = (banner?, dye?);
    if !may_place(0, &banner) { return None; }
    let color = crate::sign::dye_color(blocks::item_name(dye.item)?)?;
    let choices = choices(template);
    let pattern = if template.is_some() { *choices.first()? } else { *choices.get(usize::try_from(selected).ok()?)? };
    let mut data = banner.extras();
    if data.ominous_banner || data.banner_patterns.len() >= 6 { return None; }
    data.banner_patterns.push((pattern, color));
    Some(banner.with_count(1).with_extras(data))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item(n: &str) -> Stack { Stack::new(blocks::item_named(n).unwrap(), 1) }
    #[test]
    fn six_layers_and_reusable_template() {
        let dye = Some(item("red_dye")); let pattern = Some(item("creeper_banner_pattern"));
        let mut banner = item("white_banner");
        for _ in 0..6 { banner = result(Some(banner), dye, pattern, -1).unwrap(); }
        assert_eq!(banner.extras().banner_patterns, vec![(4,14);6]);
        assert!(result(Some(banner), dye, pattern, -1).is_none());
        assert_eq!(choices(None).len(),32);
        assert!(result(Some(item("shield")), dye, pattern, 0).is_none());
    }
    #[test] fn patterns_round_trip_and_templates_do_not_become_ordinary_choices() {
        let (_,data)=crate::stackdata::parse_item_spec("shield[banner_patterns=[{pattern:\"minecraft:creeper\",color:\"red\"}],base_color=\"white\"]").unwrap();
        assert_eq!(data.banner_patterns,vec![(4,14)]);
        assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)),Some(data.clone()));
        let (count,wire)=crate::stackdata::encode_components(&data);assert_eq!(count,2);
        assert_eq!(wire,vec![73,0,72,1,5,14]);
        let mut parsed=crate::stackdata::Data::default();
        crate::stackdata::read_component(73,&[0],&mut parsed);
        crate::stackdata::read_component(72,&[1,5,14],&mut parsed);
        assert_eq!(data,parsed);
        for (name,id) in SPECIAL {
            let template=item(&format!("{name}_banner_pattern"));
            assert_eq!(choices(Some(template)),vec![id]);assert!(!choices(None).contains(&id));
        }
    }
}
