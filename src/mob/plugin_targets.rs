// Единый доступ к нативным целям, включая независимые головы иссушителя.
use super::{Kind,Mob};
impl Mob {
    pub fn plugin_targets(&self) -> [Option<i32>;3] {
        let target=match self.kind {
            Kind::IronGolem|Kind::SnowGolem => self.golem.target.map(|t|match t {super::tame::Target::Player(id)|super::tame::Target::Mob(id)=>id}),
            Kind::Wolf => self.tame.target.map(|t|match t {super::tame::Target::Player(id)|super::tame::Target::Mob(id)=>id}),
            Kind::Goat|Kind::Panda|Kind::PolarBear|Kind::Llama|Kind::TraderLlama => self.land.target,
            Kind::Dolphin|Kind::Axolotl|Kind::Frog => self.water.target,
            Kind::EnderDragon => self.dragon.prey,
            Kind::Guardian|Kind::ElderGuardian => self.guardian.target,
            Kind::Phantom => self.phantom.target,
            Kind::Warden => self.warden.target,
            Kind::Wither => return self.wither.targets.map(|id|(id!=0).then_some(id)),
            _ => self.h.target,
        };[target,None,None]
    }
    pub fn set_plugin_target(&mut self,slot:usize,target:Option<i32>) {self.set_plugin_target_kind(slot,target,true);}
    pub fn set_plugin_target_kind(&mut self,slot:usize,target:Option<i32>,player:bool) {
        let typed=target.map(|id|if player {super::tame::Target::Player(id)}else {super::tame::Target::Mob(id)});
        if self.kind==Kind::Wither {if let Some(id)=self.wither.targets.get_mut(slot) {*id=target.unwrap_or(0);}return;}
        if slot!=0 {return;}
        match self.kind {
            Kind::IronGolem|Kind::SnowGolem => self.golem.target=typed,
            Kind::Wolf => self.tame.target=typed,
            Kind::Goat|Kind::Panda|Kind::PolarBear|Kind::Llama|Kind::TraderLlama => {self.land.target=target;if target.is_none() {self.land.ram_ticks=0;}},
            Kind::Dolphin|Kind::Axolotl|Kind::Frog => {self.water.target=target;if target.is_none() {self.water.tongue=0;self.water.swim_to=None;}},
            Kind::EnderDragon => self.dragon.prey=target,
            Kind::Guardian|Kind::ElderGuardian => {self.guardian.target=target; if target.is_none() {self.beam_target=None;}},
            Kind::Phantom => self.phantom.target=target,
            Kind::Warden => self.warden.target=target,
            _ => self.h.target=target,
        }
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn species_targets_are_read_and_cleared_in_native_state() {
        for kind in [Kind::Zombie,Kind::Guardian,Kind::ElderGuardian,Kind::Phantom,Kind::Warden,Kind::Wither,Kind::IronGolem,Kind::SnowGolem,Kind::Wolf,Kind::Goat,Kind::Panda,Kind::PolarBear,Kind::Llama,Kind::TraderLlama,Kind::Dolphin,Kind::Axolotl,Kind::Frog,Kind::EnderDragon] {
            let mut mob=Mob::new(1,kind,(0.0,64.0,0.0));mob.set_plugin_target(0,Some(42));assert_eq!(mob.plugin_targets()[0],Some(42));
            mob.set_plugin_target(0,None);assert_eq!(mob.plugin_targets()[0],None);
            match kind {Kind::Guardian|Kind::ElderGuardian=>assert_eq!(mob.guardian.target,None),Kind::Phantom=>assert_eq!(mob.phantom.target,None),Kind::Warden=>assert_eq!(mob.warden.target,None),Kind::Wither=>assert_eq!(mob.wither.targets[0],0),Kind::IronGolem|Kind::SnowGolem=>assert_eq!(mob.golem.target,None),Kind::Wolf=>assert_eq!(mob.tame.target,None),Kind::Goat|Kind::Panda|Kind::PolarBear|Kind::Llama|Kind::TraderLlama=>assert_eq!(mob.land.target,None),Kind::Dolphin|Kind::Axolotl|Kind::Frog=>assert_eq!(mob.water.target,None),Kind::EnderDragon=>assert_eq!(mob.dragon.prey,None),_=>assert_eq!(mob.h.target,None)}
        }
    }
    #[test] fn restoring_a_native_mob_target_keeps_its_entity_class() {
        for kind in [Kind::Wolf,Kind::IronGolem,Kind::SnowGolem] {
            let mut mob=Mob::new(1,kind,(0.0,64.0,0.0));mob.set_plugin_target_kind(0,Some(42),false);
            let target=if kind==Kind::Wolf {mob.tame.target}else {mob.golem.target};assert_eq!(target,Some(super::super::tame::Target::Mob(42)));
            assert_eq!(mob.plugin_targets()[0],Some(42));mob.set_plugin_target(0,None);assert_eq!(mob.plugin_targets()[0],None);
        }
    }
    #[test] fn wither_heads_keep_independent_targets() {let mut mob=Mob::new(1,Kind::Wither,(0.0,64.0,0.0));for i in 0..3 {mob.set_plugin_target(i,Some(42+i as i32));}mob.set_plugin_target(1,None);assert_eq!(mob.plugin_targets(),[Some(42),None,Some(44)]);}
}
