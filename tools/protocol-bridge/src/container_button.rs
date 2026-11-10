//! ContainerButton: имя enchant_item в pinned JSON не ограничивает кнопки зачарованием.
//! WindowId всегда VarInt; только774 имеет signed i8 для номера кнопки.
//! Решение о допустимой кнопке принимает действительное окно ядра.
use crate::{JavaVersion,wire::{Cursor,Error,uint}};

fn signed_byte(version:JavaVersion)->Result<bool,Error>{
    match version {
        JavaVersion::V1_21_11=>Ok(true),
        JavaVersion::V1_21_4|JavaVersion::V1_21_5|JavaVersion::V1_21_6|JavaVersion::V1_21_8|JavaVersion::V1_21_9|
        JavaVersion::V26_1_2|JavaVersion::V26_2|JavaVersion::V26_3=>Ok(false),
        _=>Err(Error::UnsupportedVersion(version)),
    }
}

pub(crate) fn translate(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion)->Result<(),Error>{
    let source_byte=signed_byte(from)?;let target_byte=signed_byte(to)?;
    let window=input.uint()?;
    let selected=if source_byte {
        u32::try_from(i8::from_be_bytes([input.take(1)?[0]])).map_err(|_|Error::Invalid)?
    }else{input.uint()?};
    if input.remaining()!=0{return Err(Error::Invalid);}
    // Проверяем представимость ДО записи:128 нельзя превратить в отрицательный i8.
    let byte=if target_byte {Some(i8::try_from(selected).map_err(|_|Error::Invalid)? as u8)}else{None};
    uint(out,window);
    if let Some(byte)=byte{out.push(byte);}else{uint(out,selected);}
    Ok(())
}

#[cfg(test)]
#[path="container_button_tests.rs"]
mod tests;
