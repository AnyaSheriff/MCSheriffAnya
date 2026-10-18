//! Книга рецептов/камнерез и достижения: ID предметов вложены глубже заголовка.
use crate::{JavaVersion,ids::{self,RegistryKind},items,registry,wire::{Cursor,Error,Context,uint}};
fn string(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{let s=input.string()?;uint(out,s.len() as u32);out.extend(s.as_bytes());Ok(())}
fn nbt(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{let n=registry::nbt_length(input.rest).map_err(|_|Error::Invalid)?;out.extend(input.take(n)?);Ok(())}
fn boolean(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<bool,Error>{let value=input.boolean()?;out.push(u8::from(value));Ok(value)}
fn floats(input:&mut Cursor<'_>,out:&mut Vec<u8>,count:usize)->Result<(),Error>{for _ in 0..count {let v=input.take(4)?;if !f32::from_be_bytes(v.try_into().map_err(|_|Error::Invalid)?).is_finite(){return Err(Error::Invalid);}out.extend(v);}Ok(())}
fn template(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context)->Result<(),Error>{
    let (stack,n)=items::decode_template(from,input.rest,context.items).map_err(Error::Item)?;input.take(n)?;
    out.extend(items::encode_template(to,&stack,context.items).map_err(Error::Item)?);Ok(())
}
fn item(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion)->Result<(),Error>{uint(out,ids::translate(RegistryKind::Item,from,to,input.uint()?)?);Ok(())}
fn id_set(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion)->Result<(),Error>{
    let n=input.count(65537)?;uint(out,n as u32);
    if n==0 {string(input,out)?;} else {for _ in 1..n {item(input,out,from,to)?;}}Ok(())
}
fn display(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context,depth:usize,budget:&mut usize)->Result<(),Error>{
    if depth>32 || *budget==0 {return Err(Error::TooLarge);}*budget-=1;
    let kind=input.count(10)?;uint(out,kind as u32);
    match kind {
        0|1=>(),
        2=>display(input,out,from,to,context,depth+1,budget)?,
        3=>{display(input,out,from,to,context,depth+1,budget)?;uint(out,ids::translate(RegistryKind::Component,from,to,input.uint()?)?);}
        4=>item(input,out,from,to)?,
        5=>template(input,out,from,to,context)?,
        6=>id_set(input,out,from,to)?,
        7|9=>{for _ in 0..2 {display(input,out,from,to,context,depth+1,budget)?;}}
        8=>{
            for _ in 0..2 {display(input,out,from,to,context,depth+1,budget)?;}
            let holder=input.uint()?;uint(out,holder);
            if holder==0 {string(input,out)?;nbt(input,out)?;boolean(input,out)?;}
            else {let count=context.metadata_registry_counts.iter().find(|(n,_)|*n=="minecraft:trim_pattern").ok_or(Error::RegistryUnavailable("minecraft:trim_pattern".into()))?.1;if holder>count{return Err(Error::Invalid);}}
        }
        10=>{let n=input.count(4096)?;uint(out,n as u32);for _ in 0..n {display(input,out,from,to,context,depth+1,budget)?;}}
        _=>return Err(Error::Invalid),
    }Ok(())
}
fn recipe(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context,budget:&mut usize)->Result<(),Error>{
    let kind=input.count(4)?;uint(out,kind as u32);
    let count=match kind {
        0|1=>{
            let dimensions=if kind==1{let w=input.count(3)?;let h=input.count(3)?;if w==0||h==0{return Err(Error::Invalid);}uint(out,w as u32);uint(out,h as u32);Some(w*h)}else{None};
            let n=input.count(9)?;if dimensions.is_some_and(|size|size!=n){return Err(Error::Invalid);}uint(out,n as u32);
            for _ in 0..n{display(input,out,from,to,context,0,budget)?;}2
        }
        2=>4,3=>3,4=>5,_=>return Err(Error::Invalid),
    };
    for _ in 0..count{display(input,out,from,to,context,0,budget)?;}
    if kind==2{uint(out,input.uint()?);floats(input,out,1)?;}Ok(())
}
pub(crate) fn translate(name:&str,input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context)->Result<(),Error>{
    let mut budget=65536;
    match name {
        "declare_recipes"=>{
            let n=input.count(4096)?;uint(out,n as u32);
            for _ in 0..n{string(input,out)?;let n=input.count(65536)?;uint(out,n as u32);for _ in 0..n{item(input,out,from,to)?;}}
            let n=input.count(65536)?;uint(out,n as u32);
            for _ in 0..n{id_set(input,out,from,to)?;display(input,out,from,to,context,0,&mut budget)?;}
        }
        "recipe_book_add"=>{
            let n=input.count(65536)?;uint(out,n as u32);
            for _ in 0..n{
                uint(out,input.uint()?);recipe(input,out,from,to,context,&mut budget)?;uint(out,input.uint()?);uint(out,input.count(12)? as u32);
                if boolean(input,out)?{let n=input.count(1024)?;uint(out,n as u32);for _ in 0..n{id_set(input,out,from,to)?;}}
                let flags=input.take(1)?[0];if flags&!3!=0{return Err(Error::Invalid);}out.push(flags);
            }boolean(input,out)?;
        }
        "craft_recipe_response"=>{uint(out,input.uint()?);recipe(input,out,from,to,context,&mut budget)?;}
        "advancements"=>advancements(input,out,from,to,context)?,
        _=>return Err(Error::Invalid),
    }Ok(())
}
fn advancements(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context)->Result<(),Error>{
    boolean(input,out)?;let n=input.count(65536)?;uint(out,n as u32);
    for _ in 0..n {
        string(input,out)?;if boolean(input,out)?{string(input,out)?;}
        let shown=input.boolean()?;let mut shown_data=Vec::new();let mut coords=Vec::new();
        if shown {
            nbt(input,&mut shown_data)?;nbt(input,&mut shown_data)?;template(input,&mut shown_data,from,to,context)?;
            uint(&mut shown_data,input.count(2)? as u32);
            let flags=input.take(4)?;let flag_value=u32::from_be_bytes(flags.try_into().map_err(|_|Error::Invalid)?);
            if flag_value&!7!=0{return Err(Error::Invalid);}shown_data.extend(flags);
            if flag_value&1!=0{string(input,&mut shown_data)?;}
            if from==JavaVersion::V26_1_2{floats(input,&mut coords,2)?;}
        }
        let mut requirements=Vec::new();let n=input.count(4096)?;uint(&mut requirements,n as u32);
        for _ in 0..n{let n=input.count(4096)?;uint(&mut requirements,n as u32);for _ in 0..n{string(input,&mut requirements)?;}}
        boolean(input,&mut requirements)?;
        if from==JavaVersion::V26_3{floats(input,&mut coords,2)?;}
        else if !shown {coords.extend([0;8]);}
        out.push(u8::from(shown));out.extend(shown_data);
        if to==JavaVersion::V26_1_2 {
            if shown{out.extend(&coords);}else if coords!=[0;8]{return Err(Error::PayloadUnavailable("coordinates of hidden advancement"));}
            out.extend(requirements);
        }else{out.extend(requirements);out.extend(coords);}
    }
    let removed=input.count(65536)?;uint(out,removed as u32);for _ in 0..removed{string(input,out)?;}
    let progress=input.count(65536)?;uint(out,progress as u32);
    for _ in 0..progress{string(input,out)?;let n=input.count(4096)?;uint(out,n as u32);for _ in 0..n{string(input,out)?;if boolean(input,out)?{out.extend(input.take(8)?);}}}
    boolean(input,out)?;Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
    fn convert(name:&str,bytes:&[u8],from:JavaVersion,to:JavaVersion)->Result<Vec<u8>,Error>{
        let mut input=Cursor::new(bytes);let mut output=Vec::new();
        translate(name,&mut input,&mut output,from,to,Context::default())?;
        if !input.rest.is_empty(){return Err(Error::Invalid);}Ok(output)
    }
    #[test]
    fn stonecutter_and_recipe_display_roundtrip_with_remapped_items() {
        // Без property sets, одна запись: вход IDSet и выход ItemStackTemplate.
        let mut body=vec![0,1,2];uint(&mut body,100);body.push(5);uint(&mut body,200);body.extend([3,0,0]);
        let changed=convert("declare_recipes",&body,OLD,NEW).unwrap();
        assert_eq!(convert("declare_recipes",&changed,NEW,OLD).unwrap(),body);
        for n in 0..body.len(){assert!(convert("declare_recipes",&body[..n],OLD,NEW).is_err());}
        // Книга: shapeless с одним входом, результат и верстак — display ItemID.
        let mut book=vec![1,7,0,1,4];uint(&mut book,100);book.push(4);uint(&mut book,200);book.push(4);uint(&mut book,300);book.extend([0,0,0,3,1]);
        let changed=convert("recipe_book_add",&book,OLD,NEW).unwrap();
        assert_eq!(convert("recipe_book_add",&changed,NEW,OLD).unwrap(),book);
    }
    #[test]
    fn hidden_advancement_gets_explicit_coordinates_and_nonzero_downgrade_fails() {
        // reset, одно достижение a, без parent/display, без requirements,
        // telemetry=false, removed/progress пусты, show=false.
        let old=[0,1,1,b'a',0,0,0,0,0,0,0];
        let new=convert("advancements",&old,OLD,NEW).unwrap();
        assert_eq!(new.len(),old.len()+8);
        assert_eq!(convert("advancements",&new,NEW,OLD).unwrap(),old);
        let mut moved=new;moved[8..12].copy_from_slice(&1f32.to_be_bytes());
        assert!(convert("advancements",&moved,NEW,OLD).is_err());
    }
    #[test]
    fn recursive_displays_and_bad_dimensions_are_bounded() {
        let mut deep=vec![0,1,0,1,b'a'];deep.extend([2;34]);deep.push(0);
        assert_eq!(convert("declare_recipes",&deep,OLD,NEW),Err(Error::TooLarge));
        // craft_recipe_response: окно0, shaped0×1 недопустим.
        assert!(convert("craft_recipe_response",&[0,1,0,1,0,0,0],OLD,NEW).is_err());
    }
}
