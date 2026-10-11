//! Перевод событий, а не пересылка Java status как номера Bedrock.
//! Mojang r/26_u1 enum_ActorEvent.json; вики, Entity status «entity event».
use crate::mob::{Event, Kind};

pub fn event(kind: Kind, event: &Event) -> Option<u8> {
    Some(match event {
        Event::Death => 3,
        Event::Swing => 4,
        Event::Hearts => 21,
        Event::Eating => 10,
        Event::Poof => 61,
        Event::Status(6) => 6,
        Event::Status(7) => 7,
        Event::Status(8) if kind == Kind::Wolf => 8,
        Event::Status(4) => 4,
        Event::Status(11) if kind == Kind::IronGolem => 19,
        Event::Status(34) if kind == Kind::IronGolem => 20,
        Event::Status(14) if matches!(kind, Kind::Villager | Kind::WanderingTrader) => 23,
        Event::Status(13) if matches!(kind, Kind::Villager | Kind::WanderingTrader) => 22,
        Event::Status(35) => 65,
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_status_numbers_are_not_bedrock_events() {
        assert_eq!(event(Kind::Wolf, &Event::Status(7)), Some(7));
        assert_eq!(event(Kind::Sheep, &Event::Eating), Some(10));
        assert_eq!(event(Kind::Cow, &Event::Hearts), Some(21));
        assert_eq!(event(Kind::Cow, &Event::Status(61)), None);
        assert_eq!(event(Kind::Warden, &Event::Status(62)), None);
        assert_eq!(event(Kind::Cow, &Event::Poof), Some(61));
        assert_eq!(super::super::health::actor_event(129, 61), vec![0x1b, 0x81, 1, 61, 0]);
    }
}
