//! Внешний вид транспорта: порода лодки и качание после удара.
//! Вики, Boat: «wood type»; Mojang r/26_u1 ActorDataIDs HURT_TIME/HURT_DIR.
//! [проверить] Отрисовка плота и новых пород на клиенте 944.
use crate::minecart::{Kind, Minecart};
use super::mobdata::Value;

/// Имя вида для клиента Bedrock (Порода — `variant` в данных; седоки идут через SetActorLink).
pub fn identifier(kind: Kind) -> &'static str {
    match kind {
        Kind::Plain => "minecraft:minecart",
        Kind::Chest => "minecraft:chest_minecart",
        Kind::Hopper => "minecraft:hopper_minecart",
        Kind::Tnt => "minecraft:tnt_minecart",
        // У Bedrock вагонетки с печью нет: показываем обычную.
        Kind::Furnace => "minecraft:minecart",
        Kind::Boat(index) => if crate::minecart::BOATS[index as usize].2 { "minecraft:chest_boat" } else { "minecraft:boat" },
    }
}

pub fn entries(cart: &Minecart) -> Vec<(u32, Value)> {
    let mut data = vec![(11, Value::Int(cart.hurt_time)), (12, Value::Int(cart.hurt_dir))];
    if let Kind::Boat(index) = cart.kind {
        // Флаги лодки (stackable, столкновение, тяжесть) добавляет профиль BDS; управление WASD у неё во flags2.
        data.push((0, Value::Long((1 << 48) | (1 << 49))));
        // Java хранит пары лодка/сундук; Bedrock — старый порядок пород.
        let variant = match index / 2 { 7 => 8, 8 => 9, 9 => 7, other => other };
        data.push((2, Value::Int(i32::from(variant))));
    }
    if cart.kind == Kind::Tnt { data.push((55, Value::Int(cart.fuse))); }
    // Постоянные флаги, размер и значения по умолчанию — как у BDS (профиль, запись v-*).
    super::bds_profile::extend_thing(identifier(cart.kind), &mut data);
    data
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boat_pairs_share_variant_and_bamboo_is_seven() {
        for index in 0..20 {
            let cart = Minecart::new(4, Kind::Boat(index), (0.0, 0.0, 0.0));
            let expected = [0, 1, 2, 3, 4, 5, 6, 8, 9, 7][usize::from(index / 2)];
            assert!(entries(&cart).contains(&(2, Value::Int(expected))));
        }
    }
}
