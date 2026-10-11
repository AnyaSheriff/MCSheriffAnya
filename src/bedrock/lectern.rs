// Кафедра Bedrock: ContainerOpen 25, LecternUpdate 125 (protocol.json).
// [проверить] Block Actor Data и taking через page=255 в этой версии.
use super::codec::Out;
use crate::{containers::Pos,world::World};
pub fn actor(world:&World,(x,y,z):Pos)->Option<Vec<u8>> {
    let c=world.container(x,y,z).filter(|c|c.kind==crate::containers::Kind::Lectern)?;
    let mut out=Out::packet(super::session::BLOCK_ACTOR_DATA);out.block_position(x,y,z).u8(10).string("");
    out.u8(8).string("id").string("Lectern");
    for (key,v) in [("x",x),("y",y),("z",z),("page",c.page),("totalPages",c.pages)] {out.u8(3).string(key).zigzag32(v);}
    out.u8(1).string("hasBook").bool(c.slots[0].is_some());
    if let Some(stack)=c.slots[0] {
        out.u8(10).string("book").u8(8).string("Name").string(&format!("minecraft:{}",crate::blocks::item_name(stack.item)?));
        out.u8(1).string("Count").u8(1).u8(2).string("Damage").li16(0).u8(10).string("tag");
        if let Some(book)=stack.extras().book {
            if let Some(title)=book.title {out.u8(8).string("title").string(&title).u8(8).string("author").string(&book.author);}
            out.u8(9).string("pages").u8(10).zigzag32(book.pages.len() as i32);
            for page in book.pages {out.u8(8).string("text").string(&page).u8(8).string("photoname").string("").u8(0);}
        }
        out.u8(0).u8(0);
    }
    out.u8(0);Some(out.bytes)
}
