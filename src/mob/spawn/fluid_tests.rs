use super::*;
use std::collections::HashMap;

struct Pad {
    floor: i32,
    feet: i32,
    head: i32,
    cells: HashMap<(i32, i32, i32), i32>,
}
impl Pad {
    fn new(feet: &str) -> Self {
        Self { floor: state("grass_block"), feet: state(feet), head: 0, cells: HashMap::new() }
    }
}
impl View for Pad {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        self.cells.get(&(x,y,z)).copied().unwrap_or(if y <= 60 { self.floor } else if y == 61 { self.feet } else if y == 62 { self.head } else { 0 })
    }
    fn loaded(&self, _: i32, _: i32) -> bool { true }
    fn open_sky(&self, _: i32, _: i32, _: i32) -> bool { true }
    fn light(&self, _: i32, _: i32, _: i32, _: i32) -> (u8,u8) { (15,0) }
}
fn state(name: &str) -> i32 { blocks::state_from_text(name).unwrap() }

#[test]
fn swamp_chunk_sheep_need_dry_grass_in_both_generators() {
    let dry = Pad::new("air");
    for generate in [for_new_chunk_in, land_for_new_chunk] {
        let mut sheep = 0;
        for seed in 0..600 {
            sheep += generate(&dry,-11,21,"swamp",&mut Rng::new(seed),&mut || 1).iter().filter(|m|m.kind==Kind::Sheep).count();
            for wet in ["water", "water[level=7]", "lava", "seagrass", "oak_slab[type=bottom,waterlogged=true]"] {
                assert!(generate(&Pad::new(wet),-11,21,"swamp",&mut Rng::new(seed),&mut || 1).is_empty(), "{wet}, seed {seed}");
            }
        }
        assert!(sheep > 0, "сухая трава должна давать овец");
    }
}

#[test]
fn wolves_and_small_animals_reject_flooded_surfaces() {
    for (kind, floor) in [(Kind::Rabbit,"sand"),(Kind::Fox,"podzol"),(Kind::Parrot,"oak_leaves"),(Kind::Armadillo,"red_sand")] {
        let mut pad=Pad::new("air"); pad.floor=state(floor);
        assert_eq!(animal_surface(&pad,0,0,kind),Some(60));
        pad.feet=state("water");
        assert_eq!(animal_surface(&pad,0,0,kind),None);
    }
    let mut dry_count=0;
    for seed in 0..2000 {
        dry_count+=wolves_for_new_chunk(&Pad::new("air"),0,0,"taiga",&mut Rng::new(seed),&mut || 1).len();
        assert!(wolves_for_new_chunk(&Pad::new("water"),0,0,"taiga",&mut Rng::new(seed),&mut || 1).is_empty());
    }
    assert!(dry_count>0);
}

#[test]
fn waterlogged_noncolliding_blocks_and_head_fluids_block_land_spawns() {
    for name in ["water", "lava", "bubble_column", "kelp", "tall_seagrass", "oak_sign[waterlogged=true]", "light[waterlogged=true]"] {
        let fluid=state(name);
        assert!(space::has_fluid(fluid),"{name}");
        let mut pad=Pad::new("air"); pad.head=fluid;
        assert!(!spawnable(&pad,0,61,0,2),"{name}");
        assert!(!fits(&pad,&Mob::new(1,Kind::Sheep,(0.5,61.0,0.5))),"{name}");
    }
    assert!(!space::has_fluid(state("oak_sign[waterlogged=false]")));
}

#[test]
fn full_box_checks_width_height_age_and_negative_coordinates() {
    let mut pad=Pad::new("air");
    let mut sheep=Mob::new(1,Kind::Sheep,(-0.1,61.0,-0.1));
    assert!(fits(&pad,&sheep));
    pad.cells.insert((0,61,-1),state("water"));
    assert!(!fits(&pad,&sheep),"край ящика в соседней колонке");
    pad.cells.clear(); pad.head=state("water");
    assert!(!fits(&pad,&sheep));
    sheep.age=super::super::BABY_AGE;
    assert!(fits(&pad,&sheep),"над ящиком детёныша вода не мешает");
    pad.head=0;
    pad.cells.insert((0,63,0),state("lava"));
    assert!(!fits(&pad,&Mob::new(2,Kind::Enderman,(0.5,61.0,0.5))));
    pad.cells.clear(); pad.cells.insert((1,61,0),state("water"));
    assert!(!fits(&pad,&Mob::new(3,Kind::Spider,(0.5,61.0,0.5))));
    assert!(fits(&pad,&Mob::new(4,Kind::Sheep,(0.5,61.0,0.5))));
}

#[test]
fn aquatic_placement_remains_wet_but_frogs_and_turtles_need_dry_boxes() {
    let mut pad=Pad::new("water"); pad.floor=state("water"); pad.head=state("water");
    for (kind,biome) in [(Kind::Cod,"ocean"),(Kind::Salmon,"river"),(Kind::Squid,"ocean"),(Kind::Dolphin,"ocean"),(Kind::TropicalFish,"warm_ocean"),(Kind::Pufferfish,"warm_ocean")] {
        assert!(super::super::water::spawn::allowed(&pad,kind,(0,61,0),biome,(15,0)),"{kind:?}");
        assert!(space::fits(&pad,&Mob::new(1,kind,(0.5,61.0,0.5))),"{kind:?}");
    }
    pad.cells.insert((0,60,0),state("clay"));
    assert!(!super::super::water::spawn::allowed(&pad,Kind::Axolotl,(0,59,0),"lush_caves",(0,0)));
    pad.cells.insert((0,57,0),state("clay"));
    assert!(super::super::water::spawn::allowed(&pad,Kind::Axolotl,(0,59,0),"lush_caves",(0,0)));
    assert!(super::super::water::spawn::allowed(&pad,Kind::GlowSquid,(0,20,0),"dripstone_caves",(0,0)));
    for kind in [Kind::Drowned,Kind::Guardian,Kind::ElderGuardian] {
        assert!(space::fits(&pad,&Mob::new(1,kind,(0.5,61.0,0.5))));
    }
    for (kind,biome,floor,y) in [(Kind::Frog,"swamp","mud",61),(Kind::Turtle,"beach","sand",63)] {
        let mut land=Pad::new("air"); land.floor=state(floor);
        if y==63 {land.feet=land.floor;land.head=land.floor;}
        assert!(super::super::water::spawn::allowed(&land,kind,(0,y,0),biome,(15,0)));
        land.cells.insert((0,y,0),state("water"));
        assert!(!super::super::water::spawn::allowed(&land,kind,(0,y,0),biome,(15,0)));
        if kind==Kind::Turtle {
            land.cells.clear(); land.cells.insert((1,y,0),state("water"));
            assert!(!super::super::water::spawn::allowed(&land,kind,(0,y,0),biome,(15,0)),"край широкого ящика");
        }
    }
}

#[test]
fn hostile_attempts_keep_drowned_and_guardians_in_water() {
    let mut pad=Pad::new("water"); pad.head=state("water");
    for kind in [Kind::Drowned,Kind::Guardian,Kind::Zombie,Kind::Sheep] {
        let mut count=0;
        for seed in 0..200 {
            count+=hostile_attempt(&pad,&[(0.5,61.0,0.5)],&|_,_|61,&[(kind,1)],&mut Rng::new(seed),&mut || 1).len();
        }
        assert_eq!(count>0,matches!(kind,Kind::Drowned|Kind::Guardian),"{kind:?}");
    }
}

#[test]
fn spawners_check_full_boxes_and_allow_aquatic_mobs_and_striders() {
    let data=crate::spawner::Data { spawn_range:0,..Default::default() };
    let mut pad=Pad::new("air");
    let spawn=|pad:&Pad,kind| (0..100).filter_map(|seed|crate::spawner::try_spawn(&data,kind,(0,61,0),pad,&mut Rng::new(seed),&mut || 1)).collect::<Vec<_>>();
    assert!(!spawn(&pad,Kind::Spider).is_empty());
    pad.cells.insert((1,61,0),state("water"));
    assert!(spawn(&pad,Kind::Spider).is_empty());
    pad.cells.clear(); pad.feet=state("water"); pad.head=state("water");
    assert!(spawn(&pad,Kind::Sheep).is_empty());
    assert!(!spawn(&pad,Kind::Cod).is_empty());
    pad.feet=state("lava"); pad.head=state("lava");
    assert!(spawn(&pad,Kind::Cod).is_empty());
    assert!(!spawn(&pad,Kind::Strider).is_empty());
}

#[test]
fn phantom_spots_reject_fluids_and_keep_dry_air() {
    struct Sky(i32);
    impl View for Sky {
        fn block(&self,_:i32,_:i32,_:i32)->i32 {self.0}
        fn loaded(&self,_:i32,_:i32)->bool {true}
        fn open_sky(&self,_:i32,_:i32,_:i32)->bool {true}
    }
    let spot=super::super::phantom::Spot {x:0.5,y:70.0,z:0.5,rest:1_000_000};
    let mut dry=0;
    for seed in 0..100 {
        dry+=usize::from(super::super::phantom::try_spot(&Sky(0),&spot,4.0,&mut Rng::new(seed)).is_some());
        for name in ["water","lava","oak_sign[waterlogged=true]"] {
            assert!(super::super::phantom::try_spot(&Sky(state(name)),&spot,4.0,&mut Rng::new(seed)).is_none());
        }
    }
    assert!(dry>0);
}
