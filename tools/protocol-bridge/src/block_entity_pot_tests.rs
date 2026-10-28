use super::*;
use crate::content::ContentCodec;
use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
fn preview()->ContentCodec{ContentCodec::white_slab_preview()}
fn hex(s:&str)->Vec<u8>{s.as_bytes().chunks_exact(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect()}
fn fixture()->serde_json::Value{serde_json::from_str(include_str!("../testdata/decorated-pot-wire-777.json")).unwrap()}
fn observed(case:&str,packet:&str)->Vec<u8>{let f=fixture();hex(f["observations"].as_array().unwrap().iter().find(|r|r["case"]==case && r["packet"]==packet).unwrap()["nbt_hex"].as_str().unwrap())}
fn text(s:&str)->Vec<u8>{let mut b=(s.len() as u16).to_be_bytes().to_vec();b.extend(s.as_bytes());b}
fn field(kind:u8,name:&str,value:Vec<u8>)->Vec<u8>{let mut b=vec![kind];b.extend(text(name));b.extend(value);b}
fn root(kind:u8,value:Vec<u8>)->Vec<u8>{let mut b=vec![10];b.extend(field(kind,"sherds",value));b.push(0);b}
fn old_list(names:[&str;4])->Vec<u8>{let mut v=vec![8,0,0,0,4];for n in names{v.extend(text(n));}root(9,v)}
fn old_distinct()->Vec<u8>{old_list(["minecraft:angler_pottery_sherd","minecraft:archer_pottery_sherd","minecraft:arms_up_pottery_sherd","minecraft:blade_pottery_sherd"])}
fn translated(from:JavaVersion,to:JavaVersion,kind:u32,data:&[u8],c:ContentCodec)->Result<(u32,Vec<u8>),Error>{translate_with_content(from,to,kind,data,c)?.ok_or(Error::UnsupportedData)}
#[test]
fn six_live_observations_retain_the_exact_supported_777_nbt(){
    let f=fixture();let rows=f["observations"].as_array().unwrap();assert_eq!(rows.len(),6);
    for r in rows {
        let data=hex(r["nbt_hex"].as_str().unwrap());assert_eq!(r["kind"],41);
        assert_eq!(translated(NEW,NEW,41,&data,Default::default()).unwrap(),(41,data.clone()));
        let canonical=translated(NEW,OLD,41,&data,preview()).unwrap();assert_eq!(canonical.0,42);
        assert_eq!(translated(OLD,NEW,42,&canonical.1,preview()).unwrap(),(41,data));
    }
}
#[test]
fn legacy_string_list_converts_to_the_observed_named_side_templates(){
    let old=old_distinct();let new=observed("distinct","map_chunk");
    assert_eq!(translate(OLD,NEW,42,&old).unwrap(),Some((41,new.clone())));
    assert_eq!(translate(NEW,OLD,41,&new).unwrap(),Some((42,old)));
    // 777 input with the obsolete schema is rejected even in preview.
    assert_eq!(translated(NEW,OLD,41,&old_distinct(),preview()),Err(Error::UnsupportedData));
}
#[test]
fn canonical_map_requires_preview_when_builder_endpoint_is_775(){
    let raw=observed("distinct","map_chunk");
    assert_eq!(translate(OLD,NEW,42,&raw),Err(Error::UnsupportedData));
    assert_eq!(translated(OLD,NEW,42,&raw,preview()).unwrap(),(41,raw));
}
#[test]
fn named_brick_components_survive_canonical_roundtrip_and_explicit_projection_preserves_source(){
    let raw=observed("named_front_brick","map_chunk");let before=raw.clone();
    assert_eq!(translate(NEW,OLD,41,&raw),Err(Error::UnsupportedData));
    assert_eq!(translated(NEW,OLD,41,&raw,preview()).unwrap(),(42,raw.clone()));
    assert_eq!(translated(OLD,NEW,42,&raw,preview()).unwrap(),(41,raw.clone()));
    let brick=old_list(["minecraft:brick";4]);
    assert_eq!(translated(OLD,OLD,42,&raw,preview().native_view()).unwrap(),(42,brick));
    assert_eq!(raw,before);
}
#[test]
fn canonical_preview_projects_only_visual_sides_to_each_supported_legacy_table(){
    let old=old_distinct();let raw=observed("distinct","map_chunk");
    for (version,id) in [(JavaVersion::V1_21_4,41),(JavaVersion::V1_21_5,41),(JavaVersion::V1_21_6,41),(JavaVersion::V1_21_8,41),(JavaVersion::V1_21_9,42),(JavaVersion::V1_21_11,42)] {
        assert_eq!(translated(OLD,version,42,&raw,preview()).unwrap(),(id,old.clone()));
        assert_eq!(translated(version,OLD,id,&old,Default::default()).unwrap(),(42,old.clone()));
        assert_eq!(translated(NEW,version,41,&observed("named_front_brick","map_chunk"),Default::default()),Err(Error::UnsupportedData));
        assert_eq!(translated(OLD,version,42,&observed("named_front_brick","map_chunk"),preview()).unwrap(),(id,old_list(["minecraft:brick";4])));
    }
}
#[test]
fn invalid_truncated_and_trailing_bytes_fail_closed_for_every_live_case(){
    for r in fixture()["observations"].as_array().unwrap(){
        let data=hex(r["nbt_hex"].as_str().unwrap());
        for n in 0..data.len(){assert!(translated(NEW,OLD,41,&data[..n],preview()).is_err(),"{}/{}",r["case"],n);}
        let mut extra=data.clone();extra.push(0);assert!(translated(NEW,OLD,41,&extra,preview()).is_err());
    }
}
#[test]
fn map_unknown_sides_items_fields_components_and_duplicate_names_are_rejected(){
    let stack=|name:&str|{let mut b=field(8,"id",text(name));b.push(0);b};
    let map=|side:&str,value:Vec<u8>|{let mut b=field(10,side,value);b.push(0);root(10,b)};
    let good=stack("minecraft:brick");
    let mut duplicate=good[..good.len()-1].to_vec();duplicate.extend(&good);
    let mut unknown=good[..good.len()-1].to_vec();unknown.extend(field(3,"count",1i32.to_be_bytes().to_vec()));unknown.push(0);
    let mut unknown_component=field(10,"components",{let mut b=field(8,"minecraft:unknown",text("x"));b.push(0);b});unknown_component.extend(&good);
    for raw in [map("unknown",good.clone()),map("front",stack("minecraft:unknown_pottery_sherd")),map("front",duplicate),map("front",unknown),map("front",unknown_component),root(10,{let mut b=field(8,"front",text("minecraft:brick"));b.push(0);b})] {
        assert!(translated(NEW,OLD,41,&raw,preview()).is_err());
    }
    let raw=observed("distinct","map_chunk");let mut extra=raw[..raw.len()-1].to_vec();extra.extend(field(1,"unknown",vec![1]));extra.push(0);
    assert_eq!(translated(NEW,OLD,41,&extra,preview()),Err(Error::UnsupportedData));
}
#[test]
fn legacy_default_and_malformed_side_lists_are_checked(){
    assert_eq!(translate(OLD,NEW,42,&old_list(["minecraft:brick";4])).unwrap(),Some((41,vec![10,0])));
    for count in [0i32,3,5,-1] {let mut b=vec![8];b.extend(count.to_be_bytes());assert!(translate(OLD,NEW,42,&root(9,b)).is_err());}
    assert!(translate(OLD,NEW,42,&old_list(["minecraft:stone";4])).is_err());
    assert!(translate(OLD,NEW,42,&root(8,text("minecraft:brick"))).is_err());
}
#[test]
fn fresh_776_wire_still_uses_legacy_string_list_and_777_map_is_not_assumed(){
    let f:serde_json::Value=serde_json::from_str(include_str!("../testdata/decorated-pot-wire-776.json")).unwrap();
    let mid=JavaVersion::V26_2;let rows=f["observations"].as_array().unwrap();assert_eq!(rows.len(),6);
    for r in rows {
        let raw=hex(r["nbt_hex"].as_str().unwrap());assert_eq!(r["kind"],41);
        assert_eq!(translated(mid,mid,41,&raw,Default::default()).unwrap(),(41,raw.clone()));
        if r["case"]=="distinct_legacy_list" {
            assert_eq!(raw,old_distinct());
            assert_eq!(translate(mid,NEW,41,&raw).unwrap(),Some((41,observed("distinct","map_chunk"))));
            assert_eq!(translate(NEW,mid,41,&observed("distinct","map_chunk")).unwrap(),Some((41,raw)));
        }else{
            assert!(matches!(raw.as_slice(),[0]|[10,0]));
        }
    }
    assert_eq!(translate(NEW,mid,41,&observed("named_front_brick","map_chunk")),Err(Error::UnsupportedData));
    assert_eq!(translated(OLD,mid,42,&observed("named_front_brick","map_chunk"),preview()).unwrap(),(41,old_list(["minecraft:brick";4])));
    assert_eq!(translate(mid,NEW,41,&observed("distinct","map_chunk")),Err(Error::UnsupportedData));
}
#[test]
fn duplicate_side_and_structured_custom_name_fail_closed(){
    let stack=field(8,"id",text("minecraft:brick"));
    let mut value=stack.clone();value.push(0);
    let side=field(10,"front",value);
    let mut map=side.clone();map.extend(&side);map.push(0);
    assert_eq!(translated(NEW,OLD,41,&root(10,map),preview()),Err(Error::Invalid));
    let mut components=field(10,"minecraft:custom_name",vec![0]);components.push(0);
    let mut value=field(10,"components",components);value.extend(stack);value.push(0);
    let mut map=field(10,"front",value);map.push(0);
    assert_eq!(translated(NEW,OLD,41,&root(10,map),preview()),Err(Error::UnsupportedData));
}
#[test]
fn known_stone_template_is_observed_on_777_and_only_explicit_projection_becomes_brick(){
    let f:serde_json::Value=serde_json::from_str(include_str!("../testdata/decorated-pot-wire-stone-777.json")).unwrap();
    let rows=f["observations"].as_array().unwrap();assert_eq!(rows.len(),4);
    for r in rows {
        let raw=hex(r["nbt_hex"].as_str().unwrap());let before=raw.clone();
        assert_eq!(translated(NEW,NEW,41,&raw,Default::default()).unwrap(),(41,raw.clone()));
        assert_eq!(translated(OLD,NEW,42,&raw,preview()).unwrap(),(41,raw.clone()));
        assert_eq!(translated(NEW,OLD,41,&raw,preview()).unwrap(),(42,raw.clone()));
        assert_eq!(translate(NEW,OLD,41,&raw),Err(Error::UnsupportedData));
        assert_eq!(translated(OLD,OLD,42,&raw,preview().native_view()).unwrap(),(42,old_list(["minecraft:brick";4])));
        assert_eq!(raw,before);
        for end in 0..raw.len(){assert!(translated(OLD,NEW,42,&raw[..end],preview()).is_err());}
    }
    // Имя, похожее на разрешённое, не заменяет проверку реестра.
    let mut stack=field(8,"id",text("minecraft:unregistered_stone"));stack.push(0);
    let mut map=field(10,"front",stack);map.push(0);
    assert_eq!(translated(OLD,NEW,42,&root(10,map),preview()),Err(Error::UnsupportedData));
}
#[test]
fn all_registered_preview_item_templates_keep_777_names_and_project_legacy_sides(){
    let mut checked=0;
    for row in crate::content::entries().filter(|row|row.has_item()) {
        let mut stack=field(8,"id",text(&format!("minecraft:{}",row.name)));stack.push(0);
        let mut map=field(10,"front",stack);map.push(0);let raw=root(10,map);
        assert_eq!(translated(OLD,NEW,42,&raw,preview()).unwrap(),(41,raw.clone()),"{}",row.name);
        assert_eq!(translated(NEW,OLD,41,&raw,preview()).unwrap(),(42,raw.clone()),"{}",row.name);
        assert_eq!(translated(OLD,OLD,42,&raw,preview().native_view()).unwrap(),(42,old_list(["minecraft:brick";4])),"{}",row.name);
        assert_eq!(translate(OLD,NEW,42,&raw),Err(Error::UnsupportedData),"{}",row.name);
        checked+=1;
    }
    assert!(checked>100);
}
#[test]
fn explicitly_present_plain_brick_is_not_confused_with_absent_side(){
    let plain=|components:bool|{
        let mut stack=Vec::new();
        if components{stack.extend(field(10,"components",vec![0]));}
        stack.extend(field(8,"id",text("minecraft:brick")));stack.push(0);
        let mut map=field(10,"front",stack);map.push(0);root(10,map)
    };
    let raw=plain(false);
    for input in [raw.clone(),plain(true)] {
        assert_eq!(translated(NEW,NEW,41,&input,Default::default()).unwrap(),(41,raw.clone()));
        assert_eq!(translated(NEW,OLD,41,&input,preview()).unwrap(),(42,raw.clone()));
        assert_eq!(translated(OLD,NEW,42,&input,preview()).unwrap(),(41,raw.clone()));
        assert_eq!(translate(NEW,OLD,41,&input).unwrap(),Some((42,old_list(["minecraft:brick";4]))));
    }
    assert_ne!(raw,vec![10,0]);
}
