use super::*;
use crate::{Direction,State,wire};

const VALUES:&[(u32,&[u8])]=&[
    (0,&[0]),(1,&[1]),(2,&[2]),(3,&[3]),(7,&[7]),(100,&[100]),(127,&[127]),
    (128,&[128,1]),(255,&[255,1]),(256,&[128,2]),(i32::MAX as u32,&[255,255,255,255,7]),
];
fn versions()->Vec<JavaVersion>{(769..=777).map(|v|JavaVersion::from_protocol(v).unwrap()).collect()}
fn payload(version:JavaVersion,window:&[u8],value:u32,varint:&[u8])->Vec<u8>{
    let mut body=window.to_vec();
    if version==JavaVersion::V1_21_11{assert!(value<=127);body.push(value as u8);}else{body.extend(varint);}
    body
}
fn fixture()->serde_json::Value{serde_json::from_str(include_str!("../testdata/container-button-fields.json")).unwrap()}
fn id(version:JavaVersion)->u8{
    let rows=fixture();let row=rows.as_array().unwrap().iter().find(|r|r["version"]==version.protocol()).unwrap();
    u8::from_str_radix(row["packet_id"].as_str().unwrap().trim_start_matches("0x"),16).unwrap()
}
fn packet(version:JavaVersion,payload:&[u8])->Vec<u8>{let mut b=vec![id(version)];b.extend(payload);b}
fn contexts()->[wire::Context;2]{
    [wire::Context::default(),wire::Context{items:crate::items::Context{content:crate::content::ContentCodec::white_slab_preview(),..Default::default()},..Default::default()}]
}
fn actual_public_inbound(version:JavaVersion,body:&[u8],context:wire::Context)->Result<Vec<u8>,Error>{
    // Native775 в runtime packet reader возвращается до wire codec, в обоих каталогах.
    if version==wire::INTERNAL{return Ok(body.to_vec());}
    Ok(match version {
        JavaVersion::V1_21_4=>crate::client_769::serverbound(body,context)?,
        JavaVersion::V1_21_5=>crate::client_770::serverbound(body,context)?,
        JavaVersion::V1_21_6=>crate::client_771::serverbound(body,context)?,
        JavaVersion::V1_21_8=>crate::client_772::serverbound(body,context)?,
        JavaVersion::V1_21_9=>crate::client_773::serverbound(body,context)?,
        JavaVersion::V1_21_11=>crate::client_774::serverbound(body,context)?,
        JavaVersion::V26_2=>crate::client_776::serverbound(body,context)?,
        _=>wire::translate_with_context(State::Play,Direction::Serverbound,version,wire::INTERNAL,body,context)?,
    }.into_owned())
}
#[test]
fn pinned_current_fields_are_varint_window_and_only774_signed_byte_button(){
    let rows=fixture();assert_eq!(rows.as_array().unwrap().len(),10);
    for row in rows.as_array().unwrap(){
        let protocol=row["version"].as_i64().unwrap() as i32;
        assert_eq!(row["referenced_root_types"]["ContainerID"],"varint");
        assert_eq!(row["schema"][0],"container");assert_eq!(row["schema"][1].as_array().unwrap().len(),2);
        assert_eq!(row["schema"][1][0]["name"],"windowId");assert_eq!(row["schema"][1][0]["type"],"ContainerID");
        assert_eq!(row["schema"][1][1]["name"],"enchantment");
        assert_eq!(row["schema"][1][1]["type"],if protocol==774{"i8"}else{"varint"});
        if protocol==768{continue;} // Схема — данные; новый private-допуск этим не задаётся.
        let version=JavaVersion::from_protocol(protocol).unwrap();assert_eq!(signed_byte(version).unwrap(),protocol==774);
        let target=if version==wire::INTERNAL{JavaVersion::V26_2}else{wire::INTERNAL};
        assert_eq!(crate::packet_layouts::packet_info(crate::ids::RegistryKind::PlayServerbound,version,target,id(version) as usize).unwrap().name,"enchant_item");
    }
}
#[test]
fn varint_buttons_and_windows_preserve_full_nonnegative_field_range(){
    for version in versions().into_iter().filter(|v|*v!=JavaVersion::V1_21_11){
        for &(value,encoded) in VALUES{for window in [&[1][..],&[128,1],&[255,255,255,255,7]]{
            let input=payload(version,window,value,encoded);let mut output=Vec::new();
            translate(&mut Cursor::new(&input),&mut output,version,wire::INTERNAL).unwrap();
            let mut expected=window.to_vec();expected.extend(encoded);assert_eq!(output,expected,"{} {value}",version.protocol());
        }}
    }
}
#[test]
fn signed774_positive_range_roundtrips_but128_and255_are_not_positive_bytes(){
    for &(value,encoded) in VALUES{
        let input=payload(wire::INTERNAL,&[128,1],value,encoded);let mut output=Vec::new();
        let result=translate(&mut Cursor::new(&input),&mut output,wire::INTERNAL,JavaVersion::V1_21_11);
        if value>127{assert!(result.is_err());assert!(output.is_empty());continue;}
        result.unwrap();assert_eq!(output,[128,1,value as u8]);
        let mut restored=Vec::new();translate(&mut Cursor::new(&output),&mut restored,JavaVersion::V1_21_11,wire::INTERNAL).unwrap();assert_eq!(restored,input);
    }
    for byte in [128,255]{assert!(translate(&mut Cursor::new(&[1,byte]),&mut Vec::new(),JavaVersion::V1_21_11,wire::INTERNAL).is_err());}
}
#[test]
fn admitted_public_adapters_preserve_button_values_without_enchantment_business_cap(){
    for version in versions(){for context in contexts(){for &(value,encoded) in VALUES{
        if version==JavaVersion::V1_21_11&&value>127{continue;}
        let input=packet(version,&payload(version,&[128,1],value,encoded));
        let output=actual_public_inbound(version,&input,context).unwrap();
        let mut expected=vec![0x11,128,1];expected.extend(encoded);assert_eq!(output,expected,"{} {value} preview={}",version.protocol(),context.items.content.is_preview());
    }}}
}
#[test]
fn adjacent_edges_use_each_versions_field_width_in_both_directions(){
    for pair in versions().windows(2){for (from,to) in [(pair[0],pair[1]),(pair[1],pair[0])]{for &(value,encoded) in VALUES{
        if from==JavaVersion::V1_21_11&&value>127{continue;}
        let body=packet(from,&payload(from,&[255,1],value,encoded));
        let result=crate::client_773::translate_selected(Direction::Serverbound,from,to,&body,Default::default());
        if to==JavaVersion::V1_21_11&&value>127{assert!(result.is_err());continue;}
        assert_eq!(result.unwrap(),packet(to,&payload(to,&[255,1],value,encoded)),"{}→{} {value}",from.protocol(),to.protocol());
    }}}
}
#[test]
fn negative_overflow_truncated_and_trailing_fields_publish_nothing(){
    let invalid=[vec![],vec![1],vec![128],vec![1,128],vec![1,3,0],
        vec![255,255,255,255,15,3],vec![1,255,255,255,255,15],
        vec![1,128,128,128,128,8],vec![1,128,128,128,128,128,0]];
    for bytes in invalid{let mut out=vec![99];assert!(translate(&mut Cursor::new(&bytes),&mut out,wire::INTERNAL,JavaVersion::V26_2).is_err());assert_eq!(out,[99]);}
    for bytes in [&[1][..],&[1,128],&[1,255],&[1,3,0]]{
        let mut out=vec![99];assert!(translate(&mut Cursor::new(bytes),&mut out,JavaVersion::V1_21_11,wire::INTERNAL).is_err());assert_eq!(out,[99]);
    }
    assert!(translate(&mut Cursor::new(&[1,3]),&mut Vec::new(),JavaVersion::V1_12_2,wire::INTERNAL).is_err());
}
#[test]
fn native777_and_public_probe_reject_malformed_generic_fields_before_gameplay(){
    for version in versions().into_iter().filter(|v|*v!=wire::INTERNAL){for context in contexts(){
        let valid=packet(version,&[1,7]);assert!(actual_public_inbound(version,&valid,context).is_ok());
        for end in 0..valid.len(){assert!(actual_public_inbound(version,&valid[..end],context).is_err());}
        let mut trailing=valid;trailing.push(0);assert!(actual_public_inbound(version,&trailing,context).is_err());
        let mut negative=vec![1];negative.extend([255,255,255,255,15]);
        if version==JavaVersion::V1_21_11{negative=vec![1,255];}
        assert!(actual_public_inbound(version,&packet(version,&negative),context).is_err());
    }}
}
#[test]
fn canonical_actions_gui_accepts_generic_buttons_while_menu_policy_is_not_in_codec(){
    for &(value,encoded) in VALUES{let mut input=vec![1];input.extend(encoded);let mut output=Vec::new();
        crate::actions::gui("enchant_item",&mut Cursor::new(&input),&mut output).unwrap();assert_eq!(output,input,"{value}");}
}
#[cfg(feature="private-768")]
#[test]
fn private768_container_interaction_and_public_admission_remain_closed(){
    assert!(crate::private768::play_edge(Direction::Serverbound,768,&[15,1,3]).is_err());
    assert!(crate::Negotiated::native_only(768,JavaVersion::V26_3).is_err());
}
