use super::*;
use crate::content::ContentCodec;
use JavaVersion::{V26_1_2 as OLD, V26_3 as NEW};
fn preview() -> Context { Context { content: ContentCodec::white_slab_preview(), ..Default::default() } }
fn slab() -> Stack { Stack { item:1506, count:3, added:vec![Component {kind:3,data:vec![7]}], removed:vec![] } }

fn assert_roundtrip(stack: &Stack) {
    for untrusted in [false,true] {
        let bytes=encode(NEW,Some(stack),untrusted,preview()).unwrap();
        let (decoded,n)=decode(NEW,&bytes,untrusted,preview()).unwrap();
        assert_eq!(n,bytes.len()); assert_eq!(decoded.as_ref(),Some(stack));
        assert!(decode(NEW,&bytes,untrusted,Context::default()).is_err());
        for length in 0..bytes.len() { assert!(decode(NEW,&bytes[..length],untrusted,preview()).is_err()); }
    }
    let bytes=encode_template(NEW,stack,preview()).unwrap();
    assert_eq!(decode_template(NEW,&bytes,preview()).unwrap(),(stack.clone(),bytes.len()));
    assert!(encode(OLD,Some(stack),false,preview()).is_ok());
    assert!(encode(NEW,Some(stack),false,Context::default()).is_err());
}

#[test]
fn canonical_slab_item_roundtrips_slots_templates_and_untrusted_forms() {
    let stack=slab(); assert_roundtrip(&stack);
    let bytes=encode(NEW,Some(&stack),false,preview()).unwrap();
    let mut input=Reader(&bytes); assert_eq!(input.uint().unwrap(),3); assert_eq!(input.uint().unwrap(),721);
    // Даже preview не принимает внутренний ID как Wire775 ID.
    let mut bad=vec![3];uint(&mut bad,1506);bad.extend([0,0]);
    assert!(decode(OLD,&bad,false,preview()).is_err());
}

#[test]
fn nested_slab_templates_keep_canonical_ids_components_and_counts() {
    for kind in [25,49,50,75] {
        let mut data=Vec::new();
        if kind!=25 { data.push(1); }
        if kind==75 { data.push(1); }
        uint(&mut data,1506);data.extend([3,1,0,3,7]);
        let stack=Stack {item:1,count:1,added:vec![Component {kind,data}],removed:vec![]};
        assert_roundtrip(&stack);
        // Ещё один уровень: item внутри контейнера, а slab внутри него.
        let mut outer_data=vec![1,1,1,1,0];uint(&mut outer_data,kind);outer_data.extend(&stack.added[0].data);
        let outer=Stack {item:1,count:2,added:vec![Component {kind:50,data:outer_data}],removed:vec![]};
        assert_roundtrip(&outer);
    }
}

#[test]
fn preview_does_not_change_base_slot_bytes_or_allow_unknown_items() {
    let stack=Stack {item:1,count:2,added:vec![Component {kind:3,data:vec![4]}],removed:vec![]};
    for version in [OLD,JavaVersion::V26_2,NEW] {
        for untrusted in [false,true] {
            if version==JavaVersion::V26_2 && untrusted { continue; }
            let bytes=encode(version,Some(&stack),untrusted,Context::default()).unwrap();
            assert_eq!(encode(version,Some(&stack),untrusted,preview()).unwrap(),bytes);
            assert_eq!(decode(version,&bytes,untrusted,preview()).unwrap().0,Some(stack.clone()));
        }
    }
    let mut unknown=slab();unknown.item=u32::MAX;
    assert!(encode(NEW,Some(&unknown),false,preview()).is_err());
    let mut malformed=slab();malformed.added[0].data=vec![255];
    assert!(encode(NEW,Some(&malformed),false,preview()).is_err());
}
