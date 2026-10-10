// Разрушенные порталы (ruined portal): где стоят и чем сложены.
//
// Где — по вики (Structure set «ruined_portals»: spacing 40, separation 15;
// Ruined Portal, «Generation»): мир поделён на области 40×40 чанков, в каждой
// одна попытка — чанк со сдвигом 0…24 по каждой оси. Вид портала решает биом
// середины чанка (таблица вики: обычный, пустыня, джунгли, болото, горы,
// океан, Незер; в глубокой тьме портала нет), а вид — его высоту и
// украшения:
//
//   вид         высота                         мох   пустота    заросли  лозы
//   обычный     под землёй 50 % / на земле     0,2   да / 50 %  —        —
//   пустыня     полузакопан                    0     —          —        —
//   джунгли     на земле                       0,8   50 %       да       да
//   болото      на дне                         0,5   —          —        да
//   горы        в горе 50 % / на земле         0,2   да / 50 %  —        —
//   океан       на дне                         0,8   —          —        —
//   Незер       в Незере                       0     50 %       —        —
//
// Это совпало с записями о строении в мире оригинала (tools/measure/
// template_structs.py records: поля VerticalPlacement и Properties у 113
// порталов). Гигантский портал — 5 % (вики), три вида; обычных — десять.
//
// Чем — замер мира оригинала 26.1.2: образцы сведены по большинству
// экземпляров (tools/measure/template_merge.py, 212 порталов обычного мира и
// порталы Незера; src/world/ruined_portal.txt). Замены при постановке — по
// вики (таблица «Replacement»): золото → воздух 30 %, лава → магма (дно
// океана — всегда, иначе 20 %, в холоде — незеррак), незеррак → магма 7 %
// (не в холоде), обсидиан → плачущий 15 %, камень и каменные кирпичи
// стареют (половина: трещины или ступени, с долей мха — замшелые), плиты,
// ступени и стены мшеют; в Незере камень — чёрный камень, решётки — цепи.
//
// Высота (замер TPY против земли у углов рамки, 212 порталов): на земле —
// наименьший верх у четырёх углов (вода считается: над морем портал лежит
// на воде, y 62); на дне — наименьший верх дна у углов; под землёй — от
// y −49 до верха минус высота портала; в горе — от 70 до того же (если
// ниже 70 — ровно верх минус высота); полузакопанный — верх минус высота
// плюс 2…8 (вики).
//
// Разлив незеррака (замер вокруг рамок): столбцы вокруг рамки задеты с
// долей по удалению 0,87 · 0,70 · 0,54 · 0,35 · 0,24 · 0,14 · 0,06 (1…7
// блоков), в задетом — верх земли и блок под ним, ниже — каждый следующий
// с половинной долей; под самой рамкой — слой y−1 почти всюду (0,95), ниже
// — 0,6 на шаг. Заросли: на открытом верху своих блоков и разлива —
// листва джунглей в 38 %; лозы — на боках своих блоков в 16 %.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::template::{self, Cell, Orient, Template};
use crate::blocks;

/// Область попыток, наименьший зазор и соль (вики, Structure set).
pub const SPACING: i32 = 40;
pub const SEPARATION: i32 = 15;
const SALT: u64 = 34_222_645;

/// Вид портала по биому (вики, «Generation»).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Variant {
    Standard,
    Desert,
    Jungle,
    Swamp,
    Mountain,
    Ocean,
    Nether,
}

impl Variant {
    pub const ALL: [Variant; 7] = [Variant::Standard, Variant::Desert, Variant::Jungle, Variant::Swamp, Variant::Mountain, Variant::Ocean, Variant::Nether];

    /// Имя строения (`locate structure`).
    pub fn name(self) -> &'static str {
        match self {
            Variant::Standard => "ruined_portal",
            Variant::Desert => "ruined_portal_desert",
            Variant::Jungle => "ruined_portal_jungle",
            Variant::Swamp => "ruined_portal_swamp",
            Variant::Mountain => "ruined_portal_mountain",
            Variant::Ocean => "ruined_portal_ocean",
            Variant::Nether => "ruined_portal_nether",
        }
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        Variant::ALL.into_iter().find(|variant| variant.name() == name)
    }

    /// Вид по биому (вики, таблица «Ruined Portals»).
    pub fn of_biome(biome: super::terrain::Biome) -> Option<Variant> {
        use super::terrain::Biome;

        Some(match biome {
            Biome::DeepDark => return None,
            Biome::Desert => Variant::Desert,
            Biome::Jungle | Biome::SparseJungle | Biome::BambooJungle => Variant::Jungle,
            Biome::Swamp | Biome::MangroveSwamp => Variant::Swamp,
            Biome::WindsweptHills
            | Biome::WindsweptForest
            | Biome::WindsweptGravellyHills
            | Biome::StonyShore
            | Biome::SavannaPlateau
            | Biome::WindsweptSavanna
            | Biome::ErodedBadlands
            | Biome::WoodedBadlands
            | Biome::CherryGrove
            | Biome::FrozenPeaks
            | Biome::JaggedPeaks
            | Biome::Meadow
            | Biome::SnowySlopes
            | Biome::StonyPeaks => Variant::Mountain,
            Biome::NetherWastes | Biome::CrimsonForest | Biome::WarpedForest | Biome::SoulSandValley | Biome::BasaltDeltas => Variant::Nether,
            biome if biome.is_ocean() => Variant::Ocean,
            Biome::TheEnd | Biome::SmallEndIslands | Biome::EndMidlands | Biome::EndHighlands | Biome::EndBarrens => return None,
            _ => Variant::Standard,
        })
    }
}

/// Как портал встаёт по высоте (поле VerticalPlacement записи о строении).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vertical {
    Underground,
    OnLandSurface,
    PartlyBuried,
    OnOceanFloor,
    InMountain,
    InNether,
}

/// Портал области: образец, положение, наименьший угол рамки и поля.
#[derive(Clone, Copy, Debug)]
pub struct Portal {
    pub template: usize,
    pub orient: Orient,
    pub origin: (i32, i32, i32),
    pub variant: Variant,
    pub vertical: Vertical,
    pub air_pocket: bool,
    pub mossiness: f64,
    pub overgrown: bool,
    pub vines: bool,
    pub cold: bool,
    pub blackstone: bool,
    /// Чанк старта.
    pub chunk: (i32, i32),
}

impl Portal {
    /// Сторона рамки в мире по x и z.
    pub fn footprint(&self) -> (i32, i32) {
        library()[self.template].footprint(self.orient)
    }

    pub fn height(&self) -> i32 {
        library()[self.template].height
    }
}

pub fn library() -> &'static [Template] {
    static LIBRARY: OnceLock<Vec<Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| template::parse(include_str!("ruined_portal.txt")))
}

/// Чанк попытки в области.
pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

/// Свойства портала по виду (вики, таблица «Ruined Portals»): как встаёт,
/// пустота вокруг, доля мха, заросли, лозы.
fn properties(variant: Variant, dice: u64) -> (Vertical, bool, f64, bool, bool) {
    let coin = |shift: u32| (dice >> shift) & 1 == 1;

    match variant {
        Variant::Standard if coin(40) => (Vertical::Underground, true, 0.2, false, false),
        Variant::Standard => (Vertical::OnLandSurface, coin(41), 0.2, false, false),
        Variant::Desert => (Vertical::PartlyBuried, false, 0.0, false, false),
        Variant::Jungle => (Vertical::OnLandSurface, coin(41), 0.8, true, true),
        Variant::Swamp => (Vertical::OnOceanFloor, false, 0.5, false, true),
        Variant::Mountain if coin(40) => (Vertical::InMountain, true, 0.2, false, false),
        Variant::Mountain => (Vertical::OnLandSurface, coin(41), 0.2, false, false),
        Variant::Ocean => (Vertical::OnOceanFloor, false, 0.8, false, false),
        Variant::Nether => (Vertical::InNether, coin(41), 0.0, false, false),
    }
}

/// Образец: гигантский в 5 %, иначе один из десяти (вики).
fn pick_template(dice: u64) -> usize {
    let library = library();
    let giants: Vec<usize> = (0..library.len()).filter(|&at| library[at].name.contains("giant")).collect();
    let normal: Vec<usize> = (0..library.len()).filter(|&at| !library[at].name.contains("giant")).collect();
    let pool = if dice % 100 < 5 && !giants.is_empty() { giants } else { normal };

    pool[((dice >> 8) % pool.len().max(1) as u64) as usize]
}

/// Высота по правилу вида: `top` — наименьший верх у четырёх углов рамки.
fn height_for(vertical: Vertical, top: i32, height: i32, dice: u64) -> i32 {
    let between = |low: i32, high: i32| if high <= low { low } else { low + ((dice >> 20) % (high - low + 1) as u64) as i32 };

    match vertical {
        Vertical::OnLandSurface | Vertical::OnOceanFloor => top,
        Vertical::Underground => between(super::MIN_Y + 15, top - height),
        Vertical::InMountain if top - height < 70 => top - height,
        Vertical::InMountain => between(70, top - height),
        Vertical::PartlyBuried => top - height + 2 + ((dice >> 20) % 7) as i32,
        Vertical::InNether => top,
    }
}

type Starts = Mutex<HashMap<(i64, i32, i32, bool), Option<Portal>>>;

fn cache() -> &'static Starts {
    static CACHE: OnceLock<Starts> = OnceLock::new();

    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Портал области обычного мира.
pub fn start_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Portal> {
    let key = (terrain.seed(), region_x, region_z, false);

    if let Some(found) = cache().lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return *found;
    }

    let found = decide(terrain.seed(), region_x, region_z, &|x, z| terrain.column_in(x, z, None).biome, &|x, z, floor| {
        let height = terrain.column_in(x, z, None).height;

        if floor { height } else { height.max(super::terrain::SEA) }
    });

    cache().lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// Портал области Незера.
pub fn start_in_nether(nether: &super::nether::Nether, region_x: i32, region_z: i32) -> Option<Portal> {
    let key = (nether.seed(), region_x, region_z, true);

    if let Some(found) = cache().lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return *found;
    }

    let found = decide(nether.seed(), region_x, region_z, &|x, z| nether.biome_at(x, z), &|_, _, _| 0).map(|mut portal| {
        portal.origin.1 = nether_height(nether, &portal);
        portal
    });

    cache().lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// Высота портала Незера (вики): с пустотой — от 32 до 100, без неё —
/// поровну 27…29 или 29…100; оттуда — вниз по пустоте до первой породы
/// или моря лавы под серединой рамки (замер 50 порталов Незера: без пустоты
/// TPY 27…31 у половины, с пустотой — 31…98, 31 — верх моря лавы).
fn nether_height(nether: &super::nether::Nether, portal: &Portal) -> i32 {
    let dice = template::roll(nether.seed(), portal.chunk.0, portal.chunk.1, SALT ^ 0x4E7E);
    let pick = |low: i32, high: i32, shift: u32| low + ((dice >> shift) % (high - low + 1) as u64) as i32;
    let start = if portal.air_pocket {
        pick(32, 100, 8)
    } else if dice & 1 == 0 {
        pick(27, 29, 8)
    } else {
        pick(29, 100, 8)
    };
    let (sx, sz) = portal.footprint();
    let (x, z) = (portal.origin.0 + sx / 2, portal.origin.2 + sz / 2);
    let mut y = start;

    while y > super::nether::LAVA_SEA && !nether.solid_rock(x, y, z) {
        y -= 1;
    }

    y
}

/// Решение области: вид по биому, образец, положение, поля и высота.
fn decide(seed: i64, region_x: i32, region_z: i32, biome: &dyn Fn(i32, i32) -> super::terrain::Biome, top: &dyn Fn(i32, i32, bool) -> i32) -> Option<Portal> {
    let (chunk_x, chunk_z) = attempt(seed, region_x, region_z);
    let (x, z) = (chunk_x * 16, chunk_z * 16);
    let here = biome(x + 8, z + 8);
    let variant = Variant::of_biome(here)?;
    let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0x02F0_B7A1);
    let (vertical, air_pocket, mossiness, overgrown, vines) = properties(variant, dice);
    let template = pick_template(dice >> 16);
    let orient = Orient::from_roll(dice >> 48);
    let (sx, sz) = library()[template].footprint(orient);
    let floor = vertical == Vertical::OnOceanFloor;
    let corners = [(x, z), (x + sx - 1, z), (x, z + sz - 1), (x + sx - 1, z + sz - 1)];
    let lowest = corners.iter().map(|&(cx, cz)| top(cx, cz, floor)).min().unwrap_or(super::terrain::SEA);
    let y = height_for(vertical, lowest, library()[template].height, template::roll(seed, chunk_x, chunk_z, SALT ^ 0x51DE));

    Some(Portal {
        template,
        orient,
        origin: (x, y, z),
        variant,
        vertical,
        air_pocket,
        mossiness,
        overgrown,
        vines,
        // Вики: «cold» — температура биома ниже 0,15.
        cold: here.climate().0 < 0.15,
        blackstone: variant == Variant::Nether,
        chunk: (chunk_x, chunk_z),
    })
}

/// Порталы, чьи рамки (с разливом в 8 блоков) могут задеть этот чанк.
fn near(chunk_x: i32, chunk_z: i32, start: &dyn Fn(i32, i32) -> Option<Portal>) -> Vec<Portal> {
    let mut out = Vec::new();

    for region_x in (chunk_x - 3).div_euclid(SPACING)..=(chunk_x + 3).div_euclid(SPACING) {
        for region_z in (chunk_z - 3).div_euclid(SPACING)..=(chunk_z + 3).div_euclid(SPACING) {
            if let Some(portal) = start(region_x, region_z) {
                let (sx, sz) = portal.footprint();
                let (x0, _, z0) = portal.origin;

                if x0 - SPREAD > chunk_x * 16 + 15 || x0 + sx + SPREAD <= chunk_x * 16 || z0 - SPREAD > chunk_z * 16 + 15 || z0 + sz + SPREAD <= chunk_z * 16 {
                    continue;
                }

                out.push(portal);
            }
        }
    }

    out
}

/// Дальше этого от рамки незеррак не разливается (замер: 7 блоков).
const SPREAD: i32 = 8;

/// Доля задетых столбцов по удалению от рамки 1…7 (замер, см. начало): у
/// закопанных (под землёй, в горе, в песке, в Незере) — где верх и блок под
/// ним свои; у стоящих на земле и на дне — доля незеррака на верху земли.
const SPREAD_BURIED: [f64; 7] = [0.87, 0.70, 0.54, 0.35, 0.24, 0.14, 0.06];
const SPREAD_SURFACE: [f64; 7] = [0.58, 0.43, 0.28, 0.16, 0.09, 0.05, 0.02];
// На дне — замер верха дна (0,63 · 0,48 · …), поднятый на четверть: у нас
// дно вокруг круче, и разлив дна в рамке+6 без этого на 13 % беднее
// оригинала (перепись ruined_portal_world) [проверить].
const SPREAD_FLOOR: [f64; 7] = [0.79, 0.60, 0.40, 0.25, 0.15, 0.08, 0.04];

/// Кладёт в сложенный чанк обычного мира части ближних порталов — до
/// деревьев: у оригинала строения ставятся раньше растительности.
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };

    for portal in near(chunk_x, chunk_z, &|x, z| start_in(terrain, x, z)) {
        paint(terrain.seed(), &portal, chunk, chunk_x, chunk_z);
    }
}

/// То же в Незере.
pub(super) fn build_nether(nether: &super::nether::Nether, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    for portal in near(chunk_x, chunk_z, &|x, z| start_in_nether(nether, x, z)) {
        paint(nether.seed(), &portal, chunk, chunk_x, chunk_z);
    }
}

/// Блок образца после замен вики — запись состояния; None — воздух
/// (пропавшее золото).
fn processed(seed: i64, portal: &Portal, text: &str, (x, y, z): (i32, i32, i32)) -> Option<String> {
    let roll = |salt: u64| template::chance(seed, x, y, z, salt);
    let (name, props) = text.split_once('[').map_or((text, ""), |(name, rest)| (name, rest.trim_end_matches(']')));
    let with = |new: &str, props: &str| if props.is_empty() { new.to_string() } else { format!("{}[{}]", new, props) };
    let random_stairs = |name: &str| {
        let facing = ["north", "east", "south", "west"][(roll(0x51) * 4.0) as usize % 4];
        let half = if roll(0x52) < 0.5 { "bottom" } else { "top" };

        format!("{}[facing={},half={},shape=straight,waterlogged=false]", name, facing, half)
    };
    let moss = portal.mossiness;

    let out = match name {
        "gold_block" if roll(0x60) < 0.3 => return None,
        "lava" if portal.vertical == Vertical::OnOceanFloor => "magma_block".to_string(),
        "lava" if portal.cold => "netherrack".to_string(),
        "lava" if roll(0x61) < 0.2 => "magma_block".to_string(),
        "netherrack" if !portal.cold && roll(0x62) < 0.07 => "magma_block".to_string(),
        "obsidian" if roll(0x63) < 0.15 => "crying_obsidian".to_string(),
        "stone" | "stone_bricks" | "chiseled_stone_bricks" => {
            let r = roll(0x64);

            if r < 0.5 * (1.0 - moss) {
                if roll(0x65) < 0.5 { "cracked_stone_bricks".to_string() } else { random_stairs("stone_brick_stairs") }
            } else if r < 0.5 {
                if roll(0x65) < 0.5 { "mossy_stone_bricks".to_string() } else { random_stairs("mossy_stone_brick_stairs") }
            } else {
                text.to_string()
            }
        }
        slab if slab.ends_with("_slab") && roll(0x66) < moss => with("mossy_stone_brick_slab", props),
        stairs if stairs.ends_with("_stairs") => {
            let r = roll(0x67);
            let half = if props.contains("half=top") { "top" } else { "bottom" };

            if r < 0.5 * (1.0 - moss) {
                let slab = if roll(0x68) < 0.5 { "stone_slab" } else { "stone_brick_slab" };

                format!("{}[type={},waterlogged=false]", slab, half)
            } else if r < 0.5 {
                if roll(0x68) < 0.5 { with("mossy_stone_brick_stairs", props) } else { format!("mossy_stone_brick_slab[type={},waterlogged=false]", half) }
            } else {
                text.to_string()
            }
        }
        wall if wall.ends_with("_wall") && roll(0x69) < moss => with("mossy_stone_brick_wall", props),
        _ => text.to_string(),
    };

    Some(if portal.blackstone { blackened(&out) } else { out })
}

/// Камень → чёрный камень, решётки → цепи (вики: «stone and stone bricks are
/// replaced with their blackstone equivalents, and … chains replace iron
/// bars»). Соответствия — по порталам Незера мира оригинала.
fn blackened(text: &str) -> String {
    let (name, rest) = text.split_once('[').map_or((text, None), |(name, rest)| (name, Some(rest)));
    let new = match name {
        "stone" => "blackstone",
        "stone_bricks" => "polished_blackstone_bricks",
        "cracked_stone_bricks" => "cracked_polished_blackstone_bricks",
        "chiseled_stone_bricks" => "chiseled_polished_blackstone",
        "stone_brick_stairs" => "polished_blackstone_brick_stairs",
        "stone_brick_slab" => "polished_blackstone_brick_slab",
        "stone_brick_wall" => "polished_blackstone_brick_wall",
        "stone_slab" => "blackstone_slab",
        "stone_stairs" => "blackstone_stairs",
        "smooth_stone_slab" => "polished_blackstone_slab",
        "iron_bars" => "iron_chain",
        other => other,
    };

    match rest {
        // Цепь стоит по оси, у решётки — стороны.
        Some(_) if new == "iron_chain" => "iron_chain[axis=y,waterlogged=false]".to_string(),
        Some(rest) => format!("{}[{}", new, rest),
        None => new.to_string(),
    }
}

/// Состояние по записи (с запасом на блок по умолчанию).
fn state_of(text: &str) -> i32 {
    blocks::state_from_text(text).or_else(|| blocks::state_by_name(text.split('[').next().unwrap_or(text))).unwrap_or(super::AIR)
}

/// Свой ли блок портала стоит в клетке мира (после замен) — для лоз.
fn own_block(seed: i64, portal: &Portal, x: i32, y: i32, z: i32) -> bool {
    let template = &library()[portal.template];
    let (sx, sz) = template.footprint(portal.orient);
    let (x0, y0, z0) = portal.origin;

    if !(x0..x0 + sx).contains(&x) || !(z0..z0 + sz).contains(&z) {
        return false;
    }

    let (a, b) = portal.orient.local(x - x0, z - z0, template.width, template.length);

    match template.cell(a, y - y0, b) {
        Cell::Block(index) => processed(seed, portal, &template.texts[usize::from(index)], (x, y, z)).is_some(),
        _ => false,
    }
}

/// Пусто ли место для разлива и зарослей: воздух.
fn is_air(state: i32) -> bool {
    state == super::AIR || blocks::block_at_state(state).is_some_and(|name| matches!(name, "cave_air" | "void_air"))
}

/// Бедрок ли (его разлив не трогает).
fn is_bedrock(state: i32) -> bool {
    blocks::block_at_state(state) == Some("bedrock")
}

/// Вода ли.
fn is_water(state: i32) -> bool {
    blocks::block_at_state(state) == Some("water")
}

/// Мягкое, что разлив не трогает и сквозь что ищет верх: воздух, вода,
/// трава, листва, снег.
fn soft(state: i32) -> bool {
    is_air(state)
        || blocks::block_at_state(state).is_some_and(|name| {
            matches!(name, "water" | "lava" | "short_grass" | "tall_grass" | "fern" | "large_fern" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant" | "snow" | "vine" | "dead_bush" | "bubble_column")
                || name.ends_with("_leaves")
                || name.ends_with("_flower")
                || matches!(name, "dandelion" | "poppy" | "sugar_cane" | "cactus" | "lily_pad")
        })
}

/// Блоки портала в чанке: образец, разлив незеррака, заросли.
fn paint(seed: i64, portal: &Portal, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let template = &library()[portal.template];
    let (sx, sz) = template.footprint(portal.orient);
    let (x0, y0, z0) = portal.origin;
    let (cx0, cz0) = (chunk_x * 16, chunk_z * 16);
    let mut placed: Vec<(i32, i32, i32)> = Vec::new();

    // Образец.
    for (a, y, b, cell) in template.cells() {
        let (dx, dz) = portal.orient.offset(a, b, template.width, template.length);
        let (x, wy, z) = (x0 + dx, y0 + y, z0 + dz);

        if x < cx0 || x > cx0 + 15 || z < cz0 || z > cz0 + 15 || !(super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT).contains(&wy) {
            continue;
        }

        let (lx, lz) = (x - cx0, z - cz0);

        match cell {
            Cell::Keep => {}
            Cell::Air => {
                if portal.air_pocket {
                    chunk.put_generated(lx, wy, lz, super::AIR, true);
                }
            }
            Cell::Block(index) => {
                let Some(text) = processed(seed, portal, &template.texts[usize::from(index)], (x, wy, z)) else {
                    chunk.put_generated(lx, wy, lz, super::AIR, true);
                    continue;
                };
                let turned = template::orient_text(&text, portal.orient);
                let wet = is_water(chunk.block(lx, wy, lz)) && turned.contains("waterlogged=false");
                let state = state_of(&if wet { turned.replace("waterlogged=false", "waterlogged=true") } else { turned.clone() });

                chunk.put_generated(lx, wy, lz, state, true);
                placed.push((lx, wy, lz));

                // Сундук — с добычей таблицы, брошенной по месту сундука.
                if text.starts_with("chest") {
                    let mut dice = crate::loot::Dice::new(template::roll(seed, x, z, 0x000C_4E57 ^ (wy as u64)));
                    let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

                    chest.slots = crate::loot::fill(&crate::loot::RUINED_PORTAL, chest.slots.len(), &mut dice);
                    chunk.put_generated_container(lx, wy, lz, chest);
                }
            }
        }
    }

    let rack = |chunk: &mut super::Chunk, lx: i32, y: i32, lz: i32, x: i32, z: i32, placed: &mut Vec<(i32, i32, i32)>| {
        let magma = !portal.cold && template::chance(seed, x, y, z, 0x77) < 0.07;
        let name = if magma { "magma_block" } else { "netherrack" };

        chunk.put_generated(lx, y, lz, state_of(name), true);
        placed.push((lx, y, lz));
    };

    // Разлив незеррака: вокруг рамки и под ней.
    for lx in 0..16 {
        for lz in 0..16 {
            let (x, z) = (cx0 + lx, cz0 + lz);
            let distance = (x0 - x).max(x - (x0 + sx - 1)).max(z0 - z).max(z - (z0 + sz - 1));

            if distance > SPREAD_BURIED.len() as i32 {
                continue;
            }

            if distance <= 0 {
                // Под рамкой: y0 − 1 почти всюду (и над водой, лавой, пустотой —
                // замер: воздуха там 4 %), ниже — 0,6 на шаг («сосульки»).
                let mut y = y0 - 1;
                let mut share = 0.95;

                while y > super::MIN_Y && !is_bedrock(chunk.block(lx, y, lz)) && template::chance(seed, x, y, z, 0x71) < share {
                    rack(chunk, lx, y, lz, x, z, &mut placed);
                    y -= 1;
                    share = 0.6;
                }

                continue;
            }

            // Верх столбца: на земле и на дне — верх твёрдого не выше y0 + 3
            // (замер: выше разлива нет), в толще — сам y0.
            let buried = matches!(portal.vertical, Vertical::Underground | Vertical::InMountain | Vertical::PartlyBuried | Vertical::InNether);
            let shares = match portal.vertical {
                _ if buried => &SPREAD_BURIED,
                Vertical::OnOceanFloor => &SPREAD_FLOOR,
                _ => &SPREAD_SURFACE,
            };

            if template::chance(seed, x, 0, z, 0x72) >= shares[(distance - 1) as usize] {
                continue;
            }

            let top = if buried {
                y0
            } else {
                let mut y = y0 + 3;

                while y > y0 - 6 && soft(chunk.block(lx, y, lz)) {
                    y -= 1;
                }

                y
            };

            // Вниз: у закопанных верх и блок под ним, дальше — половина на
            // шаг; у стоящих — под верхом 0,9, дальше 0,6 (замер по слоям).
            let mut y = top;
            let mut step = 0;
            let share = |step: i32| match (buried, step) {
                (true, 0 | 1) | (false, 0) => 1.0,
                (true, _) => 0.5,
                (false, 1) => 0.9,
                (false, _) => 0.6,
            };

            while y > super::MIN_Y && !soft(chunk.block(lx, y, lz)) && template::chance(seed, x, y, z, 0x73) < share(step) {
                rack(chunk, lx, y, lz, x, z, &mut placed);
                y -= 1;
                step += 1;
            }
        }
    }

    // Заросли джунглей: листва на открытом верху своих блоков и разлива.
    if portal.overgrown {
        let leaves = state_of("jungle_leaves[distance=7,persistent=true,waterlogged=false]");

        for &(lx, y, lz) in &placed {
            if is_air(chunk.block(lx, y + 1, lz)) && template::chance(seed, cx0 + lx, y, cz0 + lz, 0x74) < 0.38 {
                chunk.put_generated(lx, y + 1, lz, leaves, true);
            }
        }
    }

    // Лозы — на боках своих блоков: решает воздух рядом, по сторонам.
    if portal.vines {
        for lx in 0..16 {
            for lz in 0..16 {
                let (x, z) = (cx0 + lx, cz0 + lz);

                if x < x0 - 1 || x > x0 + sx || z < z0 - 1 || z > z0 + sz {
                    continue;
                }

                for y in y0..y0 + portal.height() {
                    if !is_air(chunk.block(lx, y, lz)) {
                        continue;
                    }

                    let faces: Vec<&str> = [("north", 0, -1), ("east", 1, 0), ("south", 0, 1), ("west", -1, 0)]
                        .into_iter()
                        .filter(|&(_, dx, dz)| own_block(seed, portal, x + dx, y, z + dz) && template::chance(seed, x + dx, y, z + dz, 0x75 ^ (dx + 2 * dz + 3) as u64) < 0.16)
                        .map(|(face, _, _)| face)
                        .collect();

                    if faces.is_empty() {
                        continue;
                    }

                    let text = format!(
                        "vine[east={},north={},south={},up=false,west={}]",
                        faces.contains(&"east"),
                        faces.contains(&"north"),
                        faces.contains(&"south"),
                        faces.contains(&"west")
                    );

                    chunk.put_generated(lx, y, lz, state_of(&text), true);
                }
            }
        }
    }
}

/// Ближайший портал вида (`locate structure`): чанк старта.
pub(super) fn nearest(generator: &super::Generator, variant: Variant, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    let start = |region_x: i32, region_z: i32| -> Option<(i32, i32)> {
        let portal = match generator {
            super::Generator::Normal(terrain) => start_in(terrain, region_x, region_z),
            super::Generator::Nether(nether) => start_in_nether(nether, region_x, region_z),
            _ => None,
        }?;

        (portal.variant == variant).then_some(portal.chunk)
    };

    template::nearest(chunk_x, chunk_z, SPACING, rings, &start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::Biome;

    /// Образцы разобраны: 10 обычных и 3 гигантских, в каждом один сундук и
    /// обсидиан (вики, «Structure»).
    #[test]
    fn templates_parse() {
        let library = library();

        assert_eq!(library.iter().filter(|t| t.name.contains("giant")).count(), 3);
        assert_eq!(library.iter().filter(|t| !t.name.contains("giant")).count(), 10);

        for template in library {
            let count = |name: &str| template.cells().filter(|(_, _, _, cell)| matches!(cell, Cell::Block(i) if template.block_name(*i) == name)).count();

            assert_eq!(count("chest"), 1, "{}", template.name);
            assert!(count("obsidian") >= 10, "{}", template.name);
        }
    }

    /// Вид по биому — таблица вики.
    #[test]
    fn variants_follow_the_biome_table() {
        assert_eq!(Variant::of_biome(Biome::Desert), Some(Variant::Desert));
        assert_eq!(Variant::of_biome(Biome::BambooJungle), Some(Variant::Jungle));
        assert_eq!(Variant::of_biome(Biome::MangroveSwamp), Some(Variant::Swamp));
        assert_eq!(Variant::of_biome(Biome::CherryGrove), Some(Variant::Mountain));
        assert_eq!(Variant::of_biome(Biome::DeepColdOcean), Some(Variant::Ocean));
        assert_eq!(Variant::of_biome(Biome::Plains), Some(Variant::Standard));
        assert_eq!(Variant::of_biome(Biome::SoulSandValley), Some(Variant::Nether));
        assert_eq!(Variant::of_biome(Biome::DeepDark), None);
    }

    /// Попытки: в своей области, сдвиг 0…24 (spacing 40 − separation 15).
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -5..5 {
            for region_z in -5..5 {
                let (x, z) = attempt(7, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
            }
        }
    }

    /// Поля по виду (вики): обычный — половина под землёй с пустотой,
    /// половина на земле; пустыня — полузакопан, без мха; океан — на дне,
    /// мох 0,8; Незер — чёрный камень (по виду), без мха.
    #[test]
    fn properties_follow_the_wiki() {
        let mut underground = 0;

        for dice in 0..2000u64 {
            let roll = template::roll(1, dice as i32, 0, 5);
            let (vertical, air, moss, overgrown, vines) = properties(Variant::Standard, roll);

            assert_eq!(moss, 0.2);
            assert!(!overgrown && !vines);

            if vertical == Vertical::Underground {
                underground += 1;
                assert!(air);
            }
        }

        assert!((850..1150).contains(&underground), "под землёй {underground}");
        assert_eq!(properties(Variant::Desert, 3).0, Vertical::PartlyBuried);
        assert_eq!(properties(Variant::Ocean, 3), (Vertical::OnOceanFloor, false, 0.8, false, false));
        assert_eq!(properties(Variant::Swamp, 3), (Vertical::OnOceanFloor, false, 0.5, false, true));
        assert!(properties(Variant::Jungle, 3).3);
    }

    /// Высота: полузакопанный торчит на 2…8 слоёв, в горе — не ниже 70
    /// (или целиком в горе), под землёй — не выше верха минус высота.
    #[test]
    fn heights_follow_the_rules() {
        for dice in 0..500u64 {
            let roll = template::roll(3, dice as i32, 1, 9);
            let y = height_for(Vertical::PartlyBuried, 80, 10, roll);

            assert!((72..=78).contains(&y), "{y}");

            let y = height_for(Vertical::Underground, 90, 10, roll);

            assert!((super::super::MIN_Y + 15..=80).contains(&y));

            let y = height_for(Vertical::InMountain, 150, 10, roll);

            assert!((70..=140).contains(&y));
            assert_eq!(height_for(Vertical::InMountain, 60, 10, roll), 50);
        }
    }

    /// Замены вики: доля плачущего обсидиана ~15 %, пропавшего золота ~30 %,
    /// старых кирпичей у мха 0,2 — половина, из них замшелых — пятая часть.
    #[test]
    fn replacements_follow_the_wiki() {
        let portal = Portal {
            template: 0,
            orient: Orient::default(),
            origin: (0, 64, 0),
            variant: Variant::Standard,
            vertical: Vertical::OnLandSurface,
            air_pocket: false,
            mossiness: 0.2,
            overgrown: false,
            vines: false,
            cold: false,
            blackstone: false,
            chunk: (0, 0),
        };
        let (mut crying, mut gone, mut aged, mut mossy) = (0, 0, 0, 0);
        let runs = 20_000;

        for at in 0..runs {
            let place = (at % 97, at / 97, 5);

            crying += usize::from(processed(9, &portal, "obsidian", place).as_deref() == Some("crying_obsidian"));
            gone += usize::from(processed(9, &portal, "gold_block", place).is_none());

            let brick = processed(9, &portal, "stone_bricks", place).unwrap_or_default();

            aged += usize::from(brick != "stone_bricks");
            mossy += usize::from(brick.starts_with("mossy"));
        }

        let share = |count: usize| count as f64 / runs as f64;

        assert!((share(crying) - 0.15).abs() < 0.02, "плачущий {}", share(crying));
        assert!((share(gone) - 0.30).abs() < 0.02, "золото {}", share(gone));
        assert!((share(aged) - 0.5).abs() < 0.02, "старые {}", share(aged));
        assert!((share(mossy) - 0.1).abs() < 0.02, "мох {}", share(mossy));
        assert_eq!(blackened("stone_bricks"), "polished_blackstone_bricks");
    }

    /// Перепись в рамке с запасом 6 блоков (как tools/measure/template_merge.py
    /// census ruined_portal): наши порталы против замера оригинала, по видам
    /// (`cargo test --release ruined_portal_world -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn ruined_portal_world() {
        use crate::world::{nether::Nether, terrain, Chunk, Generator};
        use std::collections::BTreeMap;

        const NAMES: [&str; 20] = [
            "netherrack", "magma_block", "obsidian", "crying_obsidian", "stone_bricks", "cracked_stone_bricks", "mossy_stone_bricks", "stone_brick_stairs",
            "mossy_stone_brick_stairs", "stone_brick_slab", "mossy_stone_brick_slab", "stone_slab", "iron_bars", "gold_block", "chest", "lava", "jungle_leaves",
            "vine", "polished_blackstone_bricks", "iron_chain",
        ];
        let normal = Generator::Normal(terrain::Terrain::new(1_234_567_890, terrain::Style::Vanilla));
        let nether = Generator::Nether(Nether::new(1_234_567_890));
        let mut totals: BTreeMap<&str, (u32, [u64; 20])> = BTreeMap::new();

        for (generator, regions) in [(&normal, 12), (&nether, 4)] {
            for region_x in -regions..regions {
                for region_z in -regions..regions {
                    let portal = match generator {
                        Generator::Normal(terrain) => start_in(terrain, region_x, region_z),
                        Generator::Nether(nether) => start_in_nether(nether, region_x, region_z),
                        _ => None,
                    };
                    let Some(portal) = portal else { continue };
                    let entry = totals.entry(portal.variant.name()).or_insert((0, [0; 20]));

                    if entry.0 >= 12 {
                        continue;
                    }

                    entry.0 += 1;

                    let (sx, sz) = portal.footprint();
                    let (x0, y0, z0) = portal.origin;
                    let (lo_x, hi_x, lo_z, hi_z) = (x0 - 6, x0 + sx + 5, z0 - 6, z0 + sz + 5);

                    for chunk_x in lo_x.div_euclid(16)..=hi_x.div_euclid(16) {
                        for chunk_z in lo_z.div_euclid(16)..=hi_z.div_euclid(16) {
                            let chunk = Chunk::generated(generator, chunk_x, chunk_z);

                            for lx in 0..16 {
                                for lz in 0..16 {
                                    let (x, z) = (chunk_x * 16 + lx, chunk_z * 16 + lz);

                                    if !(lo_x..=hi_x).contains(&x) || !(lo_z..=hi_z).contains(&z) {
                                        continue;
                                    }

                                    for y in y0 - 6..y0 + portal.height() + 6 {
                                        let state = chunk.block(lx, y, lz);
                                        let Some(name) = blocks::block_at_state(state) else { continue };

                                        // Листва — только своя (persistent), как в замере.
                                        if name == "jungle_leaves" && Some(state) != blocks::state_from_text("jungle_leaves[distance=7,persistent=true,waterlogged=false]") {
                                            continue;
                                        }

                                        if let Some(at) = NAMES.iter().position(|known| *known == name) {
                                            entry.1[at] += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for (variant, (count, sums)) in &totals {
            let line: Vec<String> = NAMES.iter().zip(sums).filter(|(_, sum)| **sum > 0).map(|(name, sum)| format!("{}={:.1}", name, *sum as f64 / f64::from(*count))).collect();

            println!("{variant} {count} {}", line.join(" "));
        }

        assert!(totals.len() >= 5);
    }

}
