use super::*;
use mcsa_protocol::{content::ContentCodec, ids::{self, RegistryKind}};

fn event(song: i32) -> Vec<u8> {
    // Собственный Level Event:1010, точка(0,64,0), номер песни, не глобальный.
    let mut body = vec![0x2e];
    body.extend_from_slice(&1010i32.to_be_bytes());
    body.extend_from_slice(&64i64.to_be_bytes());
    body.extend_from_slice(&song.to_be_bytes());
    body.push(0);
    body
}

#[tokio::test]
async fn bounce_event_reaches_777_with_the_song_sent_in_configuration() {
    let version = JavaVersion::V26_3;
    let content = ContentCodec::white_slab_preview();
    let bundle = super::super::configuration::registry_bundle_for(version, true).unwrap();
    let songs = bundle.iter().find(|registry| registry.name == "minecraft:jukebox_song").unwrap();
    let bounce = songs.entries.iter().position(|entry| entry.name == "minecraft:bounce").unwrap() as i32;
    let disc = crate::blocks::item_named("music_disc_bounce").unwrap();
    assert_eq!(crate::redstone::jukebox_song(disc).unwrap().0, bounce);
    let body = event(bounce);
    let mut out = Outgoing::capture_content(version, content);
    out.send(&body).await.unwrap();
    let mut bytes = out.queued();
    let packet = read_packet(&mut bytes).await.unwrap();
    assert_eq!(packet.id as u32, ids::translate(RegistryKind::PlayClientbound, wire::INTERNAL, version, 0x2e).unwrap());
    assert_eq!(packet.payload(), &body[1..]);
    assert!(bytes.is_empty());
    // За последней действительно отправленной записью новых песен нет.
    let mut rejected = Outgoing::capture_content(version, content);
    assert_eq!(rejected.send(&event(songs.entries.len() as i32)).await.unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert!(rejected.queued().is_empty());
}

#[tokio::test]
async fn ordinary_jukebox_song_reaches_774_but_unadvertised_bounce_is_refused() {
    let version = JavaVersion::V1_21_11;
    let bundle = super::super::configuration::registry_bundle_for(version, true).unwrap();
    let songs = bundle.iter().find(|registry| registry.name == "minecraft:jukebox_song").unwrap();
    assert!(songs.entries.iter().all(|entry| entry.name != "minecraft:bounce"));
    let cat = songs.entries.iter().position(|entry| entry.name == "minecraft:cat").unwrap() as i32;
    let cat_disc = crate::blocks::item_named("music_disc_cat").unwrap();
    assert_eq!(crate::redstone::jukebox_song(cat_disc).unwrap().0, cat);
    let bounce = crate::redstone::jukebox_song(crate::blocks::item_named("music_disc_bounce").unwrap()).unwrap().0;
    for content in [ContentCodec::default(), ContentCodec::white_slab_preview()] {
        let mut out = Outgoing::capture_content(version, content);
        out.probe_776 = true;
        let body = event(cat);
        out.send(&body).await.unwrap();
        let mut bytes = out.queued();
        let packet = read_packet(&mut bytes).await.unwrap();
        assert_eq!(packet.id as u32, ids::translate(RegistryKind::PlayClientbound, wire::INTERNAL, version, 0x2e).unwrap());
        assert_eq!(packet.payload(), &body[1..]);
        assert!(bytes.is_empty());
        let mut rejected = Outgoing::capture_content(version, content);
        rejected.probe_776 = true;
        assert_eq!(rejected.send(&event(bounce)).await.unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert!(rejected.queued().is_empty());
    }
}

#[test]
fn outgoing_registry_limits_match_each_advertised_bundle_and_profile() {
    for version in [JavaVersion::V1_21_4, JavaVersion::V1_21_5, JavaVersion::V1_21_6,
        JavaVersion::V1_21_8, JavaVersion::V1_21_9, JavaVersion::V1_21_11,
        JavaVersion::V26_1_2, JavaVersion::V26_2, JavaVersion::V26_3] {
        for preview in [false, true] {
            let counts = negotiated_registry_counts(version, preview).unwrap();
            let bundle = super::super::configuration::registry_bundle_for(version, preview).unwrap();
            assert_eq!(counts.len(), bundle.len());
            for registry in bundle {
                assert_eq!(counts.iter().find(|(name, _)| *name == registry.name).unwrap().1 as usize,
                    registry.entries.len(), "{} / {} / preview={preview}", version.name(), registry.name);
            }
        }
    }
    assert!(negotiated_registry_counts(JavaVersion::V1_12_2, false).unwrap().is_empty());
}
