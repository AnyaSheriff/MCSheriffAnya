//! Типизированный769→768 Play. Общие структуры рецептов получают VERSION768,
//! а Slots читаются отдельным adjacent codec, не parser-ом769 над байтами768.
use super::*;
impl Session {
    pub(super) fn from_769(&mut self,body:&[u8],context:wire::Context)->Result<Vec<u8>,Error>{
        let mut input=Cursor::new(body);let id=input.uint()?;
        let(name,target,_)=packet_info(State::Play,Direction::Clientbound,769,id)?;
        let mut out=Vec::new();uint(&mut out,target);
        match name {
            "login"=>{
                let player=u32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);out.extend(player.to_be_bytes());
                boolean(&mut input,&mut out)?;let n=input.count(64)?;uint(&mut out,n as u32);for _ in 0..n{copy_string(&mut input,&mut out)?;}
                for _ in 0..3{uint(&mut out,input.uint()?);}for _ in 0..3{boolean(&mut input,&mut out)?;}
                self.spawn_info(&mut input,&mut out)?;boolean(&mut input,&mut out)?;finish(&input)?;self.self_entity=Some(player);
            },
            "respawn"=>{self.spawn_info(&mut input,&mut out)?;let flags=input.take(1)?[0];if flags&!3!=0{return Err(Error::Invalid);}out.push(flags);finish(&input)?;},
            "map_chunk"=>{
                let chunk_context=context.chunk.ok_or(Error::MissingChunkContext)?;
                let dimension=self.dimension.and_then(|id|self.dimensions.get(id as usize)).ok_or(Error::Invalid)?;
                let biome_count=self.registries.iter().find(|r|r.name=="minecraft:worldgen/biome").ok_or(Error::Invalid)?.entries.len() as u32;
                if (chunk_context.min_y,chunk_context.sections)!=*dimension||chunk_context.shared_biome_count!=biome_count{return Err(Error::MissingChunkContext);}
                out.extend(chunk(&mut input,chunk_context)?);
            },
            "declare_recipes"|"recipe_book_add"|"craft_recipe_response"|"advancements"=>{
                crate::recipes::translate(name,&mut input,&mut out,JavaVersion::V1_21_4,VERSION,context)?;finish(&input)?;
            },
            "declare_commands"=>{crate::client_770::commands(&mut input,&mut out)?;finish(&input)?;},
            "entity_metadata"=>self.metadata(&mut input,&mut out)?,
            "entity_update_attributes"=>{
                let entity=input.uint()?;if Some(entity)!=self.self_entity{return Err(Error::MissingEntityContext);}uint(&mut out,entity);
                let n=input.count(32)?;uint(&mut out,n as u32);let mut seen=BTreeSet::new();
                for _ in 0..n {
                    let key=input.count(31)?;if !seen.insert(key){return Err(Error::Invalid);}uint(&mut out,key as u32);floats(&mut input,&mut out,1,8)?;
                    let n=input.count(1024)?;uint(&mut out,n as u32);
                    for _ in 0..n{copy_string(&mut input,&mut out)?;floats(&mut input,&mut out,1,8)?;let op=input.take(1)?[0];if op>2{return Err(Error::Invalid);}out.push(op);}
                }finish(&input)?;
            },
            "abilities"=>{let flags=input.take(1)?[0];if flags&!15!=0{return Err(Error::Invalid);}out.push(flags);floats(&mut input,&mut out,2,4)?;finish(&input)?;},
            "difficulty"=>{let level=input.take(1)?[0];if level>3{return Err(Error::Invalid);}out.push(level);boolean(&mut input,&mut out)?;finish(&input)?;},
            "entity_status"=>{out.extend(input.take(4)?);let value=input.take(1)?[0];if !(22..=28).contains(&value){return Err(Error::PayloadUnavailable("private768 entity status"));}out.push(value);finish(&input)?;},
            "game_state_change"=>{let reason=input.take(1)?[0];if reason>13{return Err(Error::Invalid);}out.push(reason);floats(&mut input,&mut out,1,4)?;finish(&input)?;},
            "update_view_position"=>{for _ in 0..2{uint(&mut out,input.varint()? as u32);}finish(&input)?;},
            "update_view_distance"|"simulation_distance"=>{uint(&mut out,input.count(32)? as u32);finish(&input)?;},
            "spawn_position"=>{out.extend(input.take(8)?);floats(&mut input,&mut out,1,4)?;finish(&input)?;},
            "update_time"=>{out.extend(input.take(16)?);boolean(&mut input,&mut out)?;finish(&input)?;},
            "initialize_world_border"=>{floats(&mut input,&mut out,4,8)?;for _ in 0..4{uint(&mut out,input.uint()?);}finish(&input)?;},
            "update_health"=>{floats(&mut input,&mut out,1,4)?;uint(&mut out,input.count(20)? as u32);floats(&mut input,&mut out,1,4)?;finish(&input)?;},
            "experience"=>{floats(&mut input,&mut out,1,4)?;uint(&mut out,input.uint()?);uint(&mut out,input.uint()?);finish(&input)?;},
            "recipe_book_settings"=>{for _ in 0..8{boolean(&mut input,&mut out)?;}finish(&input)?;},
            "set_ticking_state"=>{floats(&mut input,&mut out,1,4)?;boolean(&mut input,&mut out)?;finish(&input)?;},
            "step_tick"=>{uint(&mut out,input.uint()?);finish(&input)?;},
            "system_chat"=>{text_nbt(&mut input,&mut out)?;boolean(&mut input,&mut out)?;finish(&input)?;},
            "kick_disconnect"=>{text_nbt(&mut input,&mut out)?;finish(&input)?;},
            "server_data"=>{
                text_nbt(&mut input,&mut out)?;if boolean(&mut input,&mut out)?{let n=input.count(1024*1024)?;uint(&mut out,n as u32);out.extend(input.take(n)?);}finish(&input)?;
            },
            "unload_chunk"=>{out.extend(input.take(8)?);finish(&input)?;},
            _=>return play_edge(Direction::Clientbound,769,body),
        }Ok(out)
    }
    fn spawn_info(&mut self,input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
        let target=self.mapping.map("minecraft:dimension_type",input.uint()?)?;
        if target as usize>=self.dimensions.len(){return Err(Error::Invalid);}self.dimension=Some(target);
        uint(out,target);copy_string(input,out)?;out.extend(input.take(8)?);
        let mode=input.take(1)?[0];let previous=input.take(1)?[0];if mode>3||!(previous<=3||previous==255){return Err(Error::Invalid);}out.extend([mode,previous]);
        boolean(input,out)?;boolean(input,out)?;if boolean(input,out)?{copy_string(input,out)?;out.extend(input.take(8)?);}
        uint(out,input.uint()?);uint(out,input.varint()? as u32);Ok(())
    }
    fn metadata(&self,input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
        let entity=input.uint()?;if Some(entity)!=self.self_entity{return Err(Error::MissingEntityContext);}uint(out,entity);
        let mut seen=BTreeSet::new();
        loop {
            let key=input.take(1)?[0];out.push(key);if key==255{break;}
            if key>=21||!seen.insert(key){return Err(Error::Invalid);}let kind=input.uint()?;uint(out,kind);
            match kind {
                0=>{let value=input.take(1)?[0];if key==17&&self.skin_parts.is_some_and(|skin|skin!=value){return Err(Error::PayloadUnavailable("private768 skin parts context"));}out.push(value);},
                1|20=>uint(out,input.varint()? as u32),
                2=>{varlong(input,out)?;},
                3=>floats(input,out,1,4)?,
                4=>copy_string(input,out)?,
                5=>text_nbt(input,out)?,
                6=>{if boolean(input,out)?{text_nbt(input,out)?;}},
                7=>{
                    let(stack,n)=crate::items::adjacent_slots::decode(crate::items::adjacent_slots::Layout::V769,input.rest).map_err(Error::Item)?;input.take(n)?;
                    out.extend(crate::items::adjacent_slots::encode(crate::items::adjacent_slots::Layout::V768,stack.as_ref()).map_err(Error::Item)?);
                },
                8=>{boolean(input,out)?;},
                9|29=>floats(input,out,3,4)?,
                10=>out.extend(input.take(8)?),
                11=>{if boolean(input,out)?{out.extend(input.take(8)?);}},
                12=>uint(out,input.count(5)? as u32),
                13=>{if boolean(input,out)?{out.extend(input.take(16)?);}},
                14=>uint(out,map_id(RegistryKind::BlockState,769,input.uint()?)?),
                15=>{let id=input.uint()?;uint(out,if id==0{0}else{map_id(RegistryKind::BlockState,769,id-1)?+1});},
                16=>{if input.take(2)?!=[10,0]{return Err(Error::PayloadUnavailable("private768 nonempty shoulder NBT"));}out.extend([10,0]);},
                18=>{if input.uint()?!=0{return Err(Error::PayloadUnavailable("private768 metadata particles"));}out.push(0);},
                21=>uint(out,input.count(17)? as u32),
                30=>floats(input,out,4,4)?,
                _=>return Err(Error::PayloadUnavailable("private768 metadata serializer")),
            }
        }finish(input)
    }
}
fn floats(input:&mut Cursor<'_>,out:&mut Vec<u8>,n:usize,width:usize)->Result<(),Error>{
    for _ in 0..n{let b=input.take(width)?;let finite=if width==4{f32::from_be_bytes(b.try_into().map_err(|_|Error::Invalid)?).is_finite()}else{f64::from_be_bytes(b.try_into().map_err(|_|Error::Invalid)?).is_finite()};if !finite{return Err(Error::Invalid);}out.extend(b);}Ok(())
}
fn boolean(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<bool,Error>{let v=input.boolean()?;out.push(u8::from(v));Ok(v)}
fn text_nbt(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    let n=crate::registry::nbt_length(input.rest).map_err(|_|Error::Invalid)?;let nbt=input.take(n)?;crate::items::plain_text(nbt).map_err(Error::Item)?;out.extend(nbt);Ok(())
}
fn varlong(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    for i in 0..10{let b=input.take(1)?[0];if i==9&&b&!1!=0{return Err(Error::Invalid);}out.push(b);if b<128{return Ok(());}}Err(Error::Invalid)
}
fn chunk(input:&mut Cursor<'_>,context:crate::chunk::Context)->Result<Vec<u8>,Error>{
    let mut out=input.take(8)?.to_vec();
    let length=crate::registry::nbt_length(input.rest).map_err(|_|Error::Invalid)?;
    let heightmap=input.take(length)?;let mut maps=Cursor::new(heightmap);
    if maps.take(1)?!=[10]{return Err(Error::Invalid);}
    let mut seen=BTreeSet::new();
    loop {
        let tag=maps.take(1)?[0];if tag==0{break;}if tag!=12{return Err(Error::Invalid);}
        let size=u16::from_be_bytes(maps.take(2)?.try_into().map_err(|_|Error::Invalid)?) as usize;
        let key=std::str::from_utf8(maps.take(size)?).map_err(|_|Error::Invalid)?;
        if !["WORLD_SURFACE","OCEAN_FLOOR","MOTION_BLOCKING","MOTION_BLOCKING_NO_LEAVES","WORLD_SURFACE_WG","OCEAN_FLOOR_WG"].contains(&key)||!seen.insert(key){return Err(Error::Invalid);}
        let count=u32::from_be_bytes(maps.take(4)?.try_into().map_err(|_|Error::Invalid)?) as usize;if count>256{return Err(Error::TooLarge);}maps.take(count*8)?;
    }finish(&maps)?;out.extend(heightmap);
    let size=input.count(MAX_BYTES)?;
    let mut sections=crate::chunk_sections::decode(JavaVersion::V1_21_4,input.take(size)?,context.sections,context.shared_biome_count).map_err(|e|Error::Chunk(crate::chunk::Error::Sections(e)))?;
    for section in &mut sections{for id in &mut section.blocks{*id=map_id(RegistryKind::BlockState,769,*id)?;}}
    let bytes=crate::chunk_sections::encode(VERSION,&sections,context.shared_biome_count).map_err(|e|Error::Chunk(crate::chunk::Error::Sections(e)))?;
    uint(&mut out,bytes.len() as u32);out.extend(bytes);
    if input.uint()?!=0{return Err(Error::PayloadUnavailable("private768 chunk block entities require typed target context"));}out.push(0);
    let light=crate::light::Light::decode(JavaVersion::V1_21_4,input.rest).map_err(Error::Light)?;
    out.extend(light.encode(VERSION).map_err(Error::Light)?);Ok(out)
}

pub(super) fn dimension_context(registries:&[crate::registry::Registry])->Result<Vec<(i32,usize)>,Error>{
    let registry=registries.iter().find(|r|r.name=="minecraft:dimension_type").ok_or(Error::Invalid)?;
    let mut result=Vec::new();
    for entry in &registry.entries {
        let mut input=Cursor::new(entry.data.as_ref().ok_or(Error::Invalid)?);
        if input.take(1)?!=[10]{return Err(Error::Invalid);}let mut min_y=None;let mut height=None;
        loop {
            let kind=input.take(1)?[0];if kind==0{break;}
            let len=u16::from_be_bytes(input.take(2)?.try_into().map_err(|_|Error::Invalid)?) as usize;
            let name=std::str::from_utf8(input.take(len)?).map_err(|_|Error::Invalid)?;
            let length=crate::registry::nbt_payload_length(kind,input.rest).map_err(|_|Error::Invalid)?;
            let data=input.take(length)?;
            if matches!(name,"min_y"|"height") {
                if kind!=3{return Err(Error::Invalid);}let value=i32::from_be_bytes(data.try_into().map_err(|_|Error::Invalid)?);
                let target=if name=="min_y"{&mut min_y}else{&mut height};if target.replace(value).is_some(){return Err(Error::Invalid);}
            }
        }finish(&input)?;
        let min_y=min_y.ok_or(Error::Invalid)?;let height=height.ok_or(Error::Invalid)?;
        if min_y%16!=0||height%16!=0||!(16..=4096).contains(&height){return Err(Error::Invalid);}
        result.push((min_y,(height/16) as usize));
    }Ok(result)
}
