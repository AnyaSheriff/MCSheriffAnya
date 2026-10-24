//! Действия и эффекты, содержащие числовые ссылки на игровые реестры.
use std::collections::BTreeSet;
use crate::{JavaVersion,ids::{self,RegistryKind},wire::{Cursor,Error,Context,uint}};
fn string(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{let value=input.string()?;uint(out,value.len() as u32);out.extend(value.as_bytes());Ok(())}

pub(crate) fn teams(input: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion) -> Result<(), Error> {
    string(input,out)?;
    let mode=input.take(1)?[0];
    if mode>4 {return Err(Error::Invalid);}
    out.push(mode);
    if matches!(mode,0|2) {
        let text=|input: &mut Cursor<'_>| -> Result<Vec<u8>,Error> {
            let n=crate::registry::nbt_length(input.rest).map_err(|_|Error::Invalid)?;
            Ok(input.take(n)?.to_vec())
        };
        let name=text(input)?;
        // team-basic777: name/prefix/suffix, visibility/collision,
        // optional color, flags. Pinned JSON ошибочно оставил порядок775.
        let (prefix,suffix,visibility,collision,color,flags)=if from==JavaVersion::V26_3 {
            let prefix=text(input)?;let suffix=text(input)?;
            let visibility=input.count(3)?;let collision=input.count(3)?;
            let color=if input.boolean()? {input.count(15)? as u32}else{21};
            (prefix,suffix,visibility,collision,color,input.take(1)?[0])
        } else {
            let flags=input.take(1)?[0];let visibility=input.count(3)?;let collision=input.count(3)?;
            let color=input.count(21)? as u32;
            if color>15 && color!=21 {return Err(Error::Invalid);}
            (text(input)?,text(input)?,visibility,collision,color,flags)
        };
        if flags&!3!=0 {return Err(Error::Invalid);}
        out.extend(name);
        if from==JavaVersion::V26_3 {
            out.push(flags);uint(out,visibility as u32);uint(out,collision as u32);uint(out,color);
            out.extend(prefix);out.extend(suffix);
        } else {
            out.extend(prefix);out.extend(suffix);uint(out,visibility as u32);uint(out,collision as u32);
            out.push(u8::from(color!=21));if color!=21 {uint(out,color);}
            out.push(flags);
        }
    }
    if matches!(mode,0|3|4) {
        let count=input.count(65536)?;uint(out,count as u32);
        for _ in 0..count {string(input,out)?;}
    }
    Ok(())
}
// Равенство графов775/776 проверяет dispatcher; здесь границы и семантика полей.
// ID эффектов относятся к общему динамическому реестру этого соединения.
pub(crate) fn gui(name: &str, input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    match name {
        "set_beacon_effect" => {
            for _ in 0..2 {
                let present = input.boolean()?; out.push(u8::from(present));
                if present { uint(out, input.count(39)? as u32); }
            }
        }
        "name_item" => string(input, out)?,
        "enchant_item" => { uint(out, input.uint()?); uint(out, input.count(2)? as u32); }
        "select_trade" => uint(out, input.uint()?),
        "set_slot_state" => {
            uint(out, input.uint()?); uint(out, input.uint()?);
            out.push(u8::from(input.boolean()?));
        }
        "update_sign" => {
            out.extend(input.take(8)?); out.push(u8::from(input.boolean()?));
            for _ in 0..4 { string(input, out)?; }
        }
        "select_bundle_item" => {
            uint(out, input.uint()?);
            let selected = input.varint()?;
            if selected < -1 { return Err(Error::Invalid); }
            uint(out, selected as u32);
        }
        "edit_book" => {
            uint(out, input.uint()?);
            let count = input.count(100)?; uint(out, count as u32);
            for _ in 0..count { string(input, out)?; }
            let title = input.boolean()?; out.push(u8::from(title));
            if title { string(input, out)?; }
        }
        "pick_item_from_block" | "pick_item_from_entity" => {
            if name == "pick_item_from_block" { out.extend(input.take(8)?); }
            else { uint(out, input.uint()?); }
            out.push(u8::from(input.boolean()?));
        }
        _ => return Err(Error::Invalid),
    }
    Ok(())
}
fn floats(input:&mut Cursor<'_>,out:&mut Vec<u8>,n:usize,double:bool)->Result<(),Error>{
    for _ in 0..n {let value=input.take(if double{8}else{4})?;
        let finite=if double{f64::from_be_bytes(value.try_into().map_err(|_|Error::Invalid)?).is_finite()}else{f32::from_be_bytes(value.try_into().map_err(|_|Error::Invalid)?).is_finite()};
        if !finite{return Err(Error::Invalid);}out.extend(value);
    }Ok(())
}

fn sound(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    let id=input.uint()?;
    if id!=0{return Err(Error::PayloadUnavailable("static sound registry"));}
    uint(out,0);string(input,out)?;let fixed=input.boolean()?;out.push(u8::from(fixed));if fixed{floats(input,out,1,false)?;}Ok(())
}
fn prediction(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:crate::items::Context)->Result<(),Error>{
    if from==JavaVersion::V1_21_4 {
        let (_,size)=crate::items::decode_packet_slot(from,input.rest,false,context).map_err(Error::Item)?;
        input.take(size)?;
        // Старый полный Slot не является хэшем; ядро сверяет авторитетный контейнер.
        out.push(0);return Ok(());
    }
    let present=input.boolean()?;out.push(u8::from(present));if !present{return Ok(());}
    let item=context.content.packet_id(RegistryKind::Item,from,to,input.uint()?)?;if item==0{return Err(Error::Invalid);}uint(out,item);
    let count=input.count(127)?;if count==0{return Err(Error::Invalid);}uint(out,count as u32);
    let mut seen=BTreeSet::new();
    let n=input.count(256)?;uint(out,n as u32);
    for _ in 0..n {
        let kind=ids::translate(RegistryKind::Component,from,to,input.uint()?)?;
        if !seen.insert(kind){return Err(Error::Invalid);}uint(out,kind);
        // Хэш непрозрачен и не доказывает равенства компонентов разных версий.
        // Ядро принудительно пересылает настоящий контейнер после такого щелчка.
        out.extend(input.take(4)?);
    }
    let n=input.count(256)?;uint(out,n as u32);
    for _ in 0..n {let kind=ids::translate(RegistryKind::Component,from,to,input.uint()?)?;if !seen.insert(kind){return Err(Error::Invalid);}uint(out,kind);}
    Ok(())
}
#[cfg(test)]
pub(crate) fn click(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion)->Result<(),Error>{
    click_with_context(input,out,from,to,crate::items::Context::default())
}
pub(crate) fn click_with_context(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:crate::items::Context)->Result<(),Error>{
    uint(out,input.uint()?);
    // Официальные775/777 принимают revision=-1 и возвращают полный снимок
    // (probe_inventory_modes.py). Сохраняем signed VarInt без смены смысла.
    uint(out,input.varint()? as u32);out.extend(input.take(3)?);uint(out,input.count(6)? as u32);
    let n=input.count(128)?;uint(out,n as u32);let mut seen=BTreeSet::new();
    for _ in 0..n {
        let slot=input.take(2)?;let number=i16::from_be_bytes(slot.try_into().map_err(|_|Error::Invalid)?);
        if number<0 || !seen.insert(number){return Err(Error::Invalid);}out.extend(slot);prediction(input,out,from,to,context)?;
    }
    prediction(input,out,from,to,context)
}
pub(crate) fn effects(name:&str,input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context)->Result<(),Error>{
    match name {
        "entity_effect"|"remove_entity_effect"=>{
            uint(out,input.uint()?);
            // Обычные /effect give на775/777; записи effect-observations.json.
            // Новые неизвестные эффекты не подменяем эффектом с похожим номером.
            uint(out,input.count(39)? as u32);
            if name=="entity_effect" {
                uint(out,input.count(255)? as u32);
                let duration=input.varint()?;if duration < -1{return Err(Error::Invalid);}uint(out,duration as u32);
                let flags=input.take(1)?[0];if flags&!15!=0{return Err(Error::Invalid);}out.push(flags);
            }
        }
        "damage_event"=>{
            uint(out,input.uint()?);let kind=input.uint()?;
            let count=context.metadata_registry_counts.iter().find(|(name,_)|*name=="minecraft:damage_type")
                .ok_or_else(||Error::RegistryUnavailable("minecraft:damage_type".into()))?.1;
            if kind>=count{return Err(Error::Invalid);}uint(out,kind);
            uint(out,input.uint()?);uint(out,input.uint()?);
            let position=input.boolean()?;out.push(u8::from(position));if position{floats(input,out,3,true)?;}
        }
        "sound_effect"|"entity_sound_effect"=>{
            // Категория ui10 наблюдалась в cmd-playsound777; старые
            // маршруты сохраняют прежнюю границу до отдельной проверки.
            let category_limit=if from==JavaVersion::V26_3 || to==JavaVersion::V26_3 {10}else{9};
            sound(input,out)?;uint(out,input.count(category_limit)? as u32);
            if name=="sound_effect"{out.extend(input.take(12)?);}else{uint(out,input.uint()?);}
            floats(input,out,2,false)?;out.extend(input.take(8)?);
        }
        "explosion"=>{
            floats(input,out,3,true)?;let mut discarded=Vec::new();
            let older_to=matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8);let older_from=matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8);
            if !older_from {floats(input,if older_to{&mut discarded}else{out},1,false)?;}else if !older_to {out.extend(0f32.to_be_bytes());}
            if !older_from {let count=input.take(4)?;if i32::from_be_bytes(count.try_into().map_err(|_|Error::Invalid)?)<0{return Err(Error::Invalid);}
            if !older_to {out.extend(count);}}else if !older_to {out.extend(0i32.to_be_bytes());}
            let knockback=input.boolean()?;out.push(u8::from(knockback));if knockback{floats(input,out,3,true)?;}
            crate::particles::particle(input,out,from,to,context.items)?;sound(input,out)?;
            if !older_from {let n=input.count(4096)?;let target=if older_to{&mut discarded}else{&mut *out};uint(target,n as u32);
            for _ in 0..n{crate::particles::particle(input,target,from,to,context.items)?;floats(input,target,2,false)?;uint(target,input.uint()?);}}else if !older_to {uint(out,0);}
            // Wiki Packets777/Explosion: завершающий Play Sound. В записи
            // Wind Burst777 у обоих получателей true;775 поля ещё не имеет.
            let play_sound = if from == JavaVersion::V26_3 { input.boolean()? } else { true };
            if to == JavaVersion::V26_3 { out.push(u8::from(play_sound)); }
            else if !play_sound { return Err(Error::PayloadUnavailable("silent explosion before777")); }
        }
        "world_event"=>{
            let effect=input.take(4)?;let kind=i32::from_be_bytes(effect.try_into().map_err(|_|Error::Invalid)?);out.extend(effect);out.extend(input.take(8)?);
            let data=i32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
            let data=match kind {
                // Wiki Java Edition protocol/Packets, Level Event:
                // 2001/3008: «Block state ID» (слом и завершение археологии).
                2001|3008=>context.items.content.packet_id(RegistryKind::BlockState,from,to,u32::try_from(data).map_err(|_|Error::Invalid)?)? as i32,
                // mining-feedback-777: data — грань, не состояние блока.
                // Ядро создаёт эти события только для777; старому клиенту
                // они не отправляются (обратного аналога ещё не установлено).
                2019|2020 if from == JavaVersion::V26_1_2 && to == JavaVersion::V26_3 && (0..=5).contains(&data) => data,
                // Обычные звуковые события777 (wide777 + автокрафтер):
                // data0 не является ссылкой на реестр.
                // Другие данные/версии этим наблюдением не разрешаются.
                1000|1011|1026|1035|1038|1043|1044|1049|1050|1053|1054 if data == 0 && matches!((from,to),
                    (JavaVersion::V26_1_2,JavaVersion::V26_3) | (JavaVersion::V26_3,JavaVersion::V26_1_2)) => 0,
                // Jukebox использует общий префикс согласованного реестра
                // песен, не item ID; новая bounce добавлена в его конец.
                1010 if data >= 0 && context.metadata_registry_counts.iter()
                    .any(|(name,count)| *name=="minecraft:jukebox_song" && (data as u32)<*count) => data,
                // Цвета, направления и число частиц, а не ID содержимого.
                1500..=1505|2000|2002..=2004|2006..=2013|3000..=3007|3009=>data,
                _=>return Err(Error::PayloadUnavailable("unverified level event data")),
            };out.extend(data.to_be_bytes());out.push(u8::from(input.boolean()?));
        }
        _=>return Err(Error::Invalid),
    }Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
    #[test]
    fn observed_777_teams_reorder_components_and_use_optional_color() {
        let evidence: serde_json::Value=serde_json::from_str(include_str!("../testdata/team-layout-777.json")).unwrap();
        let bytes=|hex: &str| (0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect::<Vec<_>>();
        for row in evidence["cases"].as_array().unwrap() {
            let old=bytes(row["legacy"].as_str().unwrap());let modern=bytes(row["modern"].as_str().unwrap());
            for (from,input,want) in [(OLD,&old,&modern),(NEW,&modern,&old)] {
                let mut cursor=Cursor::new(input);let mut output=Vec::new();
                teams(&mut cursor,&mut output,from).unwrap();
                assert!(cursor.rest.is_empty());assert_eq!(&output,want);
                for end in 0..input.len() {
                    assert!(teams(&mut Cursor::new(&input[..end]),&mut Vec::new(),from).is_err());
                }
            }
        }
    }
    #[test]
    fn nineteen_official_block_menus_match_in_both_directions() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/menu-observations.json")).unwrap();
        let bytes=|hex:&str|hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect::<Vec<_>>();
        let a=data["old"]["menus"].as_array().unwrap();let b=data["new"]["menus"].as_array().unwrap();assert_eq!(a.len(),19);assert_eq!(b.len(),19);
        for (old,new) in a.iter().zip(b) {
            assert_eq!(old["block"],new["block"]);
            let old=bytes(old["body"].as_str().unwrap());let new=bytes(new["body"].as_str().unwrap());
            for (from,to,a,b) in [(OLD,NEW,&old,&new),(NEW,OLD,&new,&old)] {
                assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,from,to,a).unwrap().as_ref(),b);
                for n in 0..a.len(){assert!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,from,to,&a[..n]).is_err());}
            }
        }
    }
    #[test]
    fn all_forty_official_effects_translate_exactly_both_ways() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/effect-observations.json")).unwrap();
        let bytes=|hex:&str|hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect::<Vec<_>>();
        let old=data["old"]["effects"].as_array().unwrap();let new=data["new"]["effects"].as_array().unwrap();
        assert_eq!(old.len(),40);assert_eq!(new.len(),40);
        for (a,b) in old.iter().zip(new) {
            assert_eq!(a["name"],b["name"]);assert_eq!(a["observed_ids"],b["observed_ids"]);
            let a=bytes(a["packets"][0]["body"].as_str().unwrap());let b=bytes(b["packets"][0]["body"].as_str().unwrap());
            assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,OLD,NEW,&a).unwrap().as_ref(),b);
            assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,NEW,OLD,&b).unwrap().as_ref(),a);
        }
    }
    #[test]
    fn effects_reject_unknown_ids_and_preserve_infinite_duration() {
        let body=[1,0,1,255,255,255,255,15,7];let mut result=Vec::new();
        effects("entity_effect",&mut Cursor::new(&body),&mut result,OLD,NEW,Context::default()).unwrap();assert_eq!(result,body);
        for body in [[1,40,0,1,0],[1,0,0,1,16]] {
            assert!(effects("entity_effect",&mut Cursor::new(&body),&mut Vec::new(),OLD,NEW,Context::default()).is_err());
        }
        let body=[1,0,0,0,0];
        assert!(effects("damage_event",&mut Cursor::new(&body),&mut Vec::new(),OLD,NEW,Context::default()).is_err());
        let context=Context {metadata_registry_counts:&[("minecraft:damage_type",1)],..Default::default()};
        let mut result=Vec::new();effects("damage_event",&mut Cursor::new(&body),&mut result,OLD,NEW,context).unwrap();assert_eq!(result,body);
        assert!(effects("damage_event",&mut Cursor::new(&[1,1,0,0,0]),&mut Vec::new(),OLD,NEW,context).is_err());
    }
    #[test]
    fn observed_wind_burst_explosions_translate_the_final_sound_flag() {
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../testdata/wind-burst-explosion-777.json")).unwrap();
        for row in fixture["packets"].as_array().unwrap() {
            let captured:Vec<u8>=row["hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
                .map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect();
            // Статический sound holder пока не переводится; проверяем форму
            // с тем же звуком inline, не угадывая межверсионный sound ID.
            assert!(matches!(effects("explosion",&mut Cursor::new(&captured),&mut Vec::new(),NEW,OLD,Context::default()),Err(Error::PayloadUnavailable("static sound registry"))));
            assert_eq!(&captured[captured.len()-4..], &[0xf1,0x0d,0,1]);
            let mut raw=captured[..captured.len()-4].to_vec();
            uint(&mut raw,0);let name="minecraft:entity.wind_charge.wind_burst";
            uint(&mut raw,name.len() as u32);raw.extend(name.as_bytes());raw.extend([0,0,1]);
            let mut input=Cursor::new(&raw);let mut old=Vec::new();
            effects("explosion",&mut input,&mut old,NEW,OLD,Context::default()).unwrap();
            assert!(input.rest.is_empty());assert_eq!(old.len()+1,raw.len());
            let mut restored=Vec::new();effects("explosion",&mut Cursor::new(&old),&mut restored,OLD,NEW,Context::default()).unwrap();
            assert_eq!(restored,raw);
            for tail in [None,Some(2)] {
                let mut invalid=raw[..raw.len()-1].to_vec();if let Some(value)=tail {invalid.push(value);}
                assert!(effects("explosion",&mut Cursor::new(&invalid),&mut Vec::new(),NEW,OLD,Context::default()).is_err());
            }
        }
    }
    #[test]
    fn negative_click_revision_reaches_the_authoritative_inventory() {
        let body = [0, 255, 255, 255, 255, 15, 0, 5, 0, 0, 0, 0];
        for (from, to) in [(OLD, NEW), (NEW, OLD)] {
            let mut result = Vec::new();
            let mut input = Cursor::new(&body);
            click(&mut input, &mut result, from, to).unwrap();
            assert!(input.rest.is_empty());
            assert_eq!(result, body);
        }
    }
    #[test]
    fn click_maps_items_and_components_but_does_not_trust_hashes() {
        let mut body=vec![0,7,0,36,0,0,1,0,36,1];
        uint(&mut body,100);body.extend([5,1,3]);body.extend(0x12345678i32.to_be_bytes());body.extend([0,0]);
        let mut result=Vec::new();let mut cursor=Cursor::new(&body);
        click(&mut cursor,&mut result,OLD,NEW).unwrap();assert!(cursor.rest.is_empty());
        let mut restored=Vec::new();click(&mut Cursor::new(&result),&mut restored,NEW,OLD).unwrap();assert_eq!(restored,body);
        for n in 0..body.len(){assert!(click(&mut Cursor::new(&body[..n]),&mut Vec::new(),OLD,NEW).is_err());}
        let mut duplicate=body.clone();duplicate[11]=2; // Две добавленные компоненты вместо одной.
        let added=duplicate[12..17].to_vec();duplicate.splice(17..17,added);
        assert!(click(&mut Cursor::new(&duplicate),&mut Vec::new(),OLD,NEW).is_err());
    }
    #[test]
    fn invalid_click_counts_modes_and_slots_fail_closed() {
        let good=[0,0,0,36,0,0,0,0];
        click(&mut Cursor::new(&good),&mut Vec::new(),OLD,NEW).unwrap();
        for (index,value) in [(5,7),(6,129),(7,2)] {
            let mut bad=good;bad[index]=value;
            assert!(click(&mut Cursor::new(&bad),&mut Vec::new(),OLD,NEW).is_err());
        }
        assert!(click(&mut Cursor::new(&[0,0,0,36,0,0,1,255,255,0,0]),&mut Vec::new(),OLD,NEW).is_err());
    }
    #[test]
    fn inline_sound_is_preserved_but_unknown_static_ids_are_rejected() {
        let name=b"minecraft:entity.item.pickup";let mut body=vec![0];uint(&mut body,name.len() as u32);body.extend(name);body.extend([0,0]);
        body.extend([0;12]);body.extend(1f32.to_be_bytes());body.extend(0.5f32.to_be_bytes());body.extend(42u64.to_be_bytes());
        let mut result=Vec::new();let mut cursor=Cursor::new(&body);
        effects("sound_effect",&mut cursor,&mut result,OLD,NEW,Context::default()).unwrap();assert_eq!(result,body);assert!(cursor.rest.is_empty());
        for n in 0..body.len(){assert!(effects("sound_effect",&mut Cursor::new(&body[..n]),&mut Vec::new(),OLD,NEW,Context::default()).is_err());}
        body[0]=1;assert!(effects("sound_effect",&mut Cursor::new(&body),&mut Vec::new(),OLD,NEW,Context::default()).is_err());
    }
    #[test]
    fn observed_ui_sound_category_777_survives_translation() {
        let hex=include_str!("../testdata/blackbox-ui-sound-777.txt").trim();
        let body:Vec<u8>=hex.as_bytes().as_chunks::<2>().0.iter()
            .map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
        for (from,to) in [(JavaVersion::V26_1_2,JavaVersion::V26_3),(JavaVersion::V26_3,JavaVersion::V26_1_2)] {
            let mut output=Vec::new();
            effects("sound_effect",&mut Cursor::new(&body),&mut output,from,to,Context::default()).unwrap();
            assert_eq!(body,output);
        }
    }
    #[test]
    fn observed_jukebox_events_use_the_negotiated_song_registry() {
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../testdata/blackbox-jukebox-events-777.json")).unwrap();
        let context=Context {metadata_registry_counts:&[("minecraft:jukebox_song",19)],..Default::default()};
        for row in fixture["events"].as_array().unwrap() {
            let body:Vec<u8>=row["payload_hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
                .map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
            for (from,to) in [(OLD,NEW),(NEW,OLD)] {
                let mut output=Vec::new();effects("world_event",&mut Cursor::new(&body),&mut output,from,to,context).unwrap();
                assert_eq!(output,body);
                let mut invalid=body.clone();invalid[12..16].copy_from_slice(&255i32.to_be_bytes());
                assert!(effects("world_event",&mut Cursor::new(&invalid),&mut Vec::new(),from,to,context).is_err());
            }
        }
    }

    #[test]
    fn observed_level_sound_packets_do_not_disconnect_777_clients() {
        let fixture:serde_json::Value = serde_json::from_str(include_str!("../testdata/blackbox-level-sounds-777.json")).unwrap();
        for row in fixture["events"].as_array().unwrap() {
            let hex = row["payload_hex"].as_str().unwrap();
            let body:Vec<u8> = hex.as_bytes().as_chunks::<2>().0.iter()
                .map(|pair|u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect();
            for (from,to) in [(OLD,NEW),(NEW,OLD)] {
                let mut output=Vec::new();let mut input=Cursor::new(&body);
                effects("world_event",&mut input,&mut output,from,to,Context::default()).unwrap();
                assert!(input.rest.is_empty());assert_eq!(output,body);
                let mut invalid=body.clone();invalid[12..16].copy_from_slice(&1i32.to_be_bytes());
                assert!(effects("world_event",&mut Cursor::new(&invalid),&mut Vec::new(),from,to,Context::default()).is_err());
            }
        }
    }

    #[test]
    fn observed_crafter_events_preserve_data_and_reject_unobserved_sound_values() {
        let fixture:serde_json::Value = serde_json::from_str(include_str!("../testdata/white-slab-autocrafting-events-777.json")).unwrap();
        let rows = fixture["events"].as_array().unwrap();
        for required in [1049,1050,2010] { assert!(rows.iter().any(|row| row["effect_id"] == required)); }
        for row in rows {
            let hex = row["payload_hex"].as_str().unwrap(); assert_eq!(hex.len()%2,0);
            let body:Vec<u8> = hex.as_bytes().as_chunks::<2>().0.iter()
                .map(|pair|u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect();
            for (from,to) in [(OLD,NEW),(NEW,OLD)] {
                let mut cursor = Cursor::new(&body); let mut output = Vec::new();
                effects("world_event",&mut cursor,&mut output,from,to,Context::default()).unwrap();
                assert!(cursor.rest.is_empty()); assert_eq!(output,body);
                for end in 0..body.len() {
                    assert!(effects("world_event",&mut Cursor::new(&body[..end]),&mut Vec::new(),from,to,Context::default()).is_err());
                }
                if matches!(row["effect_id"].as_i64(),Some(1049|1050)) {
                    let mut invalid = body.clone(); invalid[12..16].copy_from_slice(&1_i32.to_be_bytes());
                    assert_eq!(effects("world_event",&mut Cursor::new(&invalid),&mut Vec::new(),from,to,Context::default()),
                        Err(Error::PayloadUnavailable("unverified level event data")));
                    assert_eq!(effects("world_event",&mut Cursor::new(&body),&mut Vec::new(),OLD,JavaVersion::V26_2,Context::default()),
                        Err(Error::PayloadUnavailable("unverified level event data")));
                }
            }
        }
    }

    #[test]
    fn observed_infection_sound_preserves_zero_data_only() {
        // Официальный777: неслышимые сущности события не прислали;
        // без Silent превращение прислало1026/0, global=false.
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/infection-event-777.json")).unwrap();
        let rows = fixture["events"].as_array().unwrap();
        assert!(!rows.is_empty());
        for row in rows {
            let hex = row["payload_hex"].as_str().unwrap();
            let body: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect();
            for (from,to) in [(OLD,NEW),(NEW,OLD)] {
                let mut input = Cursor::new(&body);
                let mut output = Vec::new();
                effects("world_event", &mut input, &mut output, from, to, Context::default()).unwrap();
                assert!(input.rest.is_empty());
                assert_eq!(output, body);
                let mut invalid = body.clone(); invalid[15] = 1;
                assert!(effects("world_event", &mut Cursor::new(&invalid), &mut Vec::new(), from, to, Context::default()).is_err());
                for end in 0..body.len() {
                    assert!(effects("world_event", &mut Cursor::new(&body[..end]), &mut Vec::new(), from, to, Context::default()).is_err());
                }
            }
        }
    }

    #[test]
    fn observed_mining_feedback_preserves_face_and_rejects_old_clients() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/mining-feedback-777.json")).unwrap();
        for row in fixture["packets"].as_array().unwrap() {
            let hex = row["hex"].as_str().unwrap();
            let body: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect();
            let mut output = Vec::new();
            effects("world_event", &mut Cursor::new(&body), &mut output, OLD, NEW, Context::default()).unwrap();
            assert_eq!(output, body);
            assert!(effects("world_event", &mut Cursor::new(&body), &mut Vec::new(), NEW, OLD, Context::default()).is_err());
            assert!(effects("world_event", &mut Cursor::new(&body), &mut Vec::new(), OLD, JavaVersion::V26_2, Context::default()).is_err());
            for face in [-1_i32, 6] {
                let mut invalid = body.clone(); invalid[12..16].copy_from_slice(&face.to_be_bytes());
                assert!(effects("world_event", &mut Cursor::new(&invalid), &mut Vec::new(), OLD, NEW, Context::default()).is_err());
            }
        }
    }

    #[test]
    fn block_break_event_maps_state_instead_of_copying_its_number() {
        for event in [2001i32,3008] {
        let mut body=event.to_be_bytes().to_vec();body.extend([0;8]);body.extend(27i32.to_be_bytes());body.push(0);
        let mut result=Vec::new();effects("world_event",&mut Cursor::new(&body),&mut result,OLD,NEW,Context::default()).unwrap();
        assert_eq!(&result[12..16],&28i32.to_be_bytes());
        let mut restored=Vec::new();effects("world_event",&mut Cursor::new(&result),&mut restored,NEW,OLD,Context::default()).unwrap();assert_eq!(restored,body);
        }
    }
}
