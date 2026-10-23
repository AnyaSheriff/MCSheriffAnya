//! ResolvableProfile: закреплённые схемы775/776, без статических игровых ID.
use crate::wire::{Cursor, Error};

pub(crate) fn read(input: &mut Cursor<'_>) -> Result<(), Error> {
    match input.count(1)? {
        0 => {
            if input.boolean()? { input.string()?; }
            if input.boolean()? { input.take(16)?; }
        }
        1 => { input.take(16)?; input.string()?; }
        _ => return Err(Error::Invalid),
    }
    for _ in 0..input.count(256)? {
        input.string()?;
        input.string()?;
        if input.boolean()? { input.string()?; }
    }
    for _ in 0..3 {
        if input.boolean()? { input.string()?; }
    }
    if input.boolean()? { input.count(1)?; }
    Ok(())
}
