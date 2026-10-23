//! Предметы775↔777 и ограниченные Slot/ItemStackTemplate776: только разобранные компоненты.
//! Источник: закреплённые Slot/UntrustedSlotComponent в protocol.json.
use std::collections::BTreeSet;
use crate::{JavaVersion, ids::{self, RegistryKind}, registry, content::{ContentCodec, IdSpace}};

pub(crate) const LIMIT: usize = 1024 * 1024;
#[derive(Clone, Copy, Default)]
pub struct Context {
    /// Одинаковые порядок и содержимое реестра зачарований ОБЯЗАТЕЛЬНЫ.
    pub shared_enchantment_count: Option<u32>,
    pub content: ContentCodec,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedVersion(JavaVersion), Invalid, Truncated, TooLarge, UnsupportedComponent(u32), MissingEnchantmentContext,
    Id(ids::IdTranslationError), Content(crate::content::Error), Nbt(registry::Error),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Component { pub kind: u32, pub data: Vec<u8> }
/// ID предмета принадлежит canonical catalog; ID компонентов остаются775.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stack { pub item: u32, pub count: u32, pub added: Vec<Component>, pub removed: Vec<u32> }

pub(crate) struct Reader<'a>(pub &'a [u8]);
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let (value, rest) = self.0.split_at_checked(n).ok_or(Error::Truncated)?; self.0 = rest; Ok(value)
    }
    pub fn varint(&mut self) -> Result<i32, Error> {
        let mut value = 0;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf0 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return Ok(value as i32); }
        }
        Err(Error::Invalid)
    }
    pub fn uint(&mut self) -> Result<u32, Error> {
        self.varint()?.try_into().map_err(|_|Error::Invalid)
    }
    pub fn count(&mut self, max: usize) -> Result<usize, Error> {
        let n = self.uint()? as usize; if n > max { Err(Error::TooLarge) } else { Ok(n) }
    }
    pub fn boolean(&mut self) -> Result<bool, Error> {
        match self.take(1)? { [0] => Ok(false), [1] => Ok(true), _ => Err(Error::Invalid) }
    }
    pub(crate) fn nbt(&mut self) -> Result<&'a [u8], Error> {
        let size = registry::nbt_length(self.0).map_err(Error::Nbt)?; self.take(size)
    }
}
pub(crate) fn uint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; } out.push(value as u8);
}
fn id(kind: RegistryKind, from: JavaVersion, to: JavaVersion, value: u32) -> Result<u32, Error> {
    ids::translate(kind, from, to, value).map_err(Error::Id)
}

pub(crate) fn content_id(context: Context, kind: RegistryKind, from: IdSpace, to: IdSpace, value: u32) -> Result<u32, Error> {
    context.content.map(kind, from, to, value).map_err(|error| match error {
        crate::content::Error::Id(error) => Error::Id(error),
        error => Error::Content(error),
    })
}

// Таблица ID сама по себе не разрешает payload этой версии.
fn supported_payload(version: JavaVersion) -> Result<(), Error> {
    if matches!(version, JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 | JavaVersion::V26_3) { Ok(()) }
    else { Err(Error::UnsupportedVersion(version)) }
}

// Slot776 и component-free ItemStackTemplate камнереза наблюдены отдельно
// (stonecutter-26.2-observation.json). Для компонентов остаётся прежний gate:
// композиция проверяется синтетически, не объявляется отдельным live наблюдением.
// UntrustedSlot776: та же envelope с длиной каждого компонента, проверенная
// generate_payload_776.py; вложенные ID всё равно проходят typed remapper.
fn supported_form(version: JavaVersion, untrusted: bool) -> Result<(), Error> {
    let _ = untrusted;
    supported_payload(version)
}

/// Разбирает ровно один слот; возвращает число прочитанных байтов для массива слотов.
/// untrusted=true только для serverbound Creative Slot: компоненты имеют длину.
pub fn decode(version: JavaVersion, bytes: &[u8], untrusted: bool, context: Context) -> Result<(Option<Stack>, usize), Error> {
    decode_stack(version, bytes, untrusted, context, false, IdSpace::Wire(version))
}

/// ItemStackTemplate (в том числе recipe display): ID перед количеством, пустого слота нет.
pub fn decode_template(version: JavaVersion, bytes: &[u8], context: Context) -> Result<(Stack, usize), Error> {
    let (stack, size) = decode_stack(version, bytes, false, context, true, IdSpace::Wire(version))?;
    Ok((stack.ok_or(Error::Invalid)?, size))
}

/// Внутренний payload использует раскладку775, но canonical ID предметов.
/// Его нельзя читать как настоящий Slot клиента775 при включённом extension.
pub fn decode_canonical(bytes: &[u8], untrusted: bool, context: Context) -> Result<(Option<Stack>, usize), Error> {
    decode_stack(JavaVersion::V26_1_2, bytes, untrusted, context, false, IdSpace::Canonical)
}
pub fn decode_canonical_template(bytes: &[u8], context: Context) -> Result<(Stack, usize), Error> {
    let (stack, size) = decode_stack(JavaVersion::V26_1_2, bytes, false, context, true, IdSpace::Canonical)?;
    Ok((stack.ok_or(Error::Invalid)?, size))
}

// Эти функции вызываются только translator-ом пакетов:775 endpoint —
// внутренний builder, не настоящий клиент775 при private preview.
pub(crate) fn decode_packet_slot(version: JavaVersion, bytes: &[u8], untrusted: bool, context: Context) -> Result<(Option<Stack>, usize), Error> {
    decode_stack(version,bytes,untrusted,context,false,context.content.packet_space(version))
}
pub(crate) fn encode_packet_slot(version: JavaVersion, stack: Option<&Stack>, untrusted: bool, context: Context) -> Result<Vec<u8>, Error> {
    encode_stack(version,stack,untrusted,context,false,context.content.output_space(version))
}
pub(crate) fn decode_packet_template(version: JavaVersion, bytes: &[u8], context: Context) -> Result<(Stack, usize), Error> {
    let (stack,size)=decode_stack(version,bytes,false,context,true,context.content.packet_space(version))?;
    Ok((stack.ok_or(Error::Invalid)?,size))
}
pub(crate) fn encode_packet_template(version: JavaVersion, stack: &Stack, context: Context) -> Result<Vec<u8>, Error> {
    encode_stack(version,Some(stack),false,context,true,context.content.output_space(version))
}

fn decode_stack(version: JavaVersion, bytes: &[u8], untrusted: bool, context: Context, template: bool, source: IdSpace) -> Result<(Option<Stack>, usize), Error> {
    let template = template && !matches!(version, JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11);
    supported_form(version, untrusted)?;
    if bytes.len() > LIMIT { return Err(Error::TooLarge); }
    let mut input = Reader(bytes);
    let template_id = if template { Some(input.uint()?) } else { None };
    let count = input.count(127)? as u32;
    if count == 0 {
        return if template { Err(Error::Invalid) } else { Ok((None, bytes.len() - input.0.len())) };
    }
    let item = content_id(context, RegistryKind::Item, source, IdSpace::Canonical, match template_id { Some(id) => id, None => input.uint()? })?;
    if item == 0 { return Err(Error::Invalid); }
    let added_count = input.count(256)?; let removed_count = input.count(256)?;
    let mut added = Vec::new(); let mut removed = Vec::new(); let mut seen = BTreeSet::new();
    for _ in 0..added_count {
        let kind = id(RegistryKind::Component, version, JavaVersion::V26_1_2, input.uint()?)?;
        if !seen.insert(kind) { return Err(Error::Invalid); }
        let data = if untrusted {
            let size = input.count(LIMIT)?; let data = input.take(size)?;
            let mut component = Reader(data); let data=component_bytes_at_spaces(kind,&mut component,version,JavaVersion::V26_1_2,context,0,(source,IdSpace::Canonical))?;
            if !component.0.is_empty() { return Err(Error::Invalid); }
            data
        } else {
            component_bytes_at_spaces(kind,&mut input,version,JavaVersion::V26_1_2,context,0,(source,IdSpace::Canonical))?
        };
        if data.len()>LIMIT{return Err(Error::TooLarge);}
        added.push(Component { kind, data });
    }
    for _ in 0..removed_count {
        let kind = id(RegistryKind::Component, version, JavaVersion::V26_1_2, input.uint()?)?;
        if !seen.insert(kind) { return Err(Error::Invalid); }
        removed.push(kind);
    }
    Ok((Some(Stack { item, count, added, removed }), bytes.len() - input.0.len()))
}

pub fn encode(version: JavaVersion, stack: Option<&Stack>, untrusted: bool, context: Context) -> Result<Vec<u8>, Error> {
    encode_stack(version, stack, untrusted, context, false, IdSpace::Wire(version))
}

pub fn encode_template(version: JavaVersion, stack: &Stack, context: Context) -> Result<Vec<u8>, Error> {
    encode_stack(version, Some(stack), false, context, true, IdSpace::Wire(version))
}

pub fn encode_canonical(stack: Option<&Stack>, untrusted: bool, context: Context) -> Result<Vec<u8>, Error> {
    encode_stack(JavaVersion::V26_1_2, stack, untrusted, context, false, IdSpace::Canonical)
}
pub fn encode_canonical_template(stack: &Stack, context: Context) -> Result<Vec<u8>, Error> {
    encode_stack(JavaVersion::V26_1_2, Some(stack), false, context, true, IdSpace::Canonical)
}

fn encode_stack(version: JavaVersion, stack: Option<&Stack>, untrusted: bool, context: Context, template: bool, target: IdSpace) -> Result<Vec<u8>, Error> {
    let template = template && !matches!(version, JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11);
    supported_form(version, untrusted)?;
    let Some(stack) = stack else { return Ok(vec![0]); };
    if !(1..=127).contains(&stack.count) || stack.item == 0 || stack.added.len() > 256 || stack.removed.len() > 256 { return Err(Error::Invalid); }
    let mut out = Vec::new();
    let item = content_id(context, RegistryKind::Item, IdSpace::Canonical, target, stack.item)?;
    for value in if template { [item, stack.count] } else { [stack.count, item] } { uint(&mut out, value); }
    let visible=|kind:u32| !matches!(version,JavaVersion::V1_21_6|JavaVersion::V1_21_8|JavaVersion::V1_21_9) || !matches!(id(RegistryKind::Component,JavaVersion::V26_1_2,version,kind),Err(Error::Id(crate::ids::IdTranslationError::MissingTarget(_))));
    uint(&mut out, stack.added.iter().filter(|c|visible(c.kind)).count() as u32); uint(&mut out, stack.removed.iter().filter(|&&k|visible(k)).count() as u32);
    let mut seen = BTreeSet::new();
    for component in &stack.added {
        if !seen.insert(component.kind) { return Err(Error::Invalid); }
        if component.data.len() > LIMIT { return Err(Error::TooLarge); }
        if !visible(component.kind) {
            // Отсутствующий компонент не переносит новые правила на старый клиент.
            let mut source=Reader(&component.data);
            component_bytes_at_spaces(component.kind,&mut source,JavaVersion::V26_1_2,JavaVersion::V26_1_2,context,0,(IdSpace::Canonical,IdSpace::Canonical))?;
            if !source.0.is_empty(){return Err(Error::Invalid);}continue;
        }
        let mut input = Reader(&component.data); let data=component_bytes_at_spaces(component.kind,&mut input,JavaVersion::V26_1_2,version,context,0,(IdSpace::Canonical,target))?;
        if !input.0.is_empty() { return Err(Error::Invalid); }
        uint(&mut out, id(RegistryKind::Component, JavaVersion::V26_1_2, version, component.kind)?);
        if untrusted { uint(&mut out, data.len() as u32); }
        out.extend(data);
        if out.len() > LIMIT { return Err(Error::TooLarge); }
    }
    for &kind in &stack.removed {
        if !seen.insert(kind) { return Err(Error::Invalid); }
        if visible(kind) {uint(&mut out, id(RegistryKind::Component, JavaVersion::V26_1_2, version, kind)?);}
    }
    // Removed IDs тоже занимают место: весь Slot должен укладываться в LIMIT.
    if out.len() > LIMIT { return Err(Error::TooLarge); }
    Ok(out)
}

pub(crate) fn component_bytes(kind:u32,input:&mut Reader<'_>,from:JavaVersion,to:JavaVersion,context:Context)->Result<Vec<u8>,Error>{
    component_bytes_at_spaces(kind,input,from,to,context,0,(context.content.packet_space(from),context.content.packet_space(to)))
}

pub(crate) fn component_bytes_at_spaces(kind:u32,input:&mut Reader<'_>,from:JavaVersion,to:JavaVersion,context:Context,depth:usize,spaces:(IdSpace,IdSpace))->Result<Vec<u8>,Error>{
    supported_payload(from)?; supported_payload(to)?;
    // JSON773/774 не описывает реальный префикс bee NBT.
    // Оба непрозрачных клиента приняли типизированный префикс пчелы.

    if (from == JavaVersion::V26_2 || to == JavaVersion::V26_2)
        && !crate::generated_payload_776::COMPONENTS.contains(&kind)
        && !matches!(kind,25|49|50|75) {
        return Err(Error::UnsupportedComponent(kind));
    }
    if matches!(kind,53|56|57|59|61|66|68|69|72|74|76|77|86|88|89|90|104) {
        if !crate::component_776::supported_pair(from,to) {return Err(Error::UnsupportedComponent(kind));}
        return crate::component_776::remap(kind,input,context,spaces);
    }
    // В776 эти два компонента наблюдены отдельно. В777 swing_animation
    // удалён, а collar payload здесь ещё не открываем по одному совпавшему ID.
    if matches!(kind,40|84) && matches!(from,JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_1_2|JavaVersion::V26_2)
        && matches!(to,JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_1_2|JavaVersion::V26_2) {
        let start=input.0;
        if kind==40 { input.count(2)?; input.uint()?; } else { input.count(15)?; }
        return Ok(start[..start.len()-input.0.len()].to_vec());
    }
    if kind == 70 && matches!(from,JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_1_2|JavaVersion::V26_2)
        && matches!(to,JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_1_2|JavaVersion::V26_2) {
        let start = input.0;
        let mut cursor = crate::wire::Cursor::new(start);
        let mut out=Vec::new();
        if matches!(from,JavaVersion::V1_21_6|JavaVersion::V1_21_8) {
            let begin=cursor.rest;
            if cursor.boolean().map_err(|_|Error::Invalid)? {cursor.string().map_err(|_|Error::Invalid)?;}
            if cursor.boolean().map_err(|_|Error::Invalid)? {cursor.take(16).map_err(|_|Error::Invalid)?;}
            for _ in 0..cursor.count(256).map_err(|_|Error::Invalid)? {cursor.string().map_err(|_|Error::Invalid)?;cursor.string().map_err(|_|Error::Invalid)?;if cursor.boolean().map_err(|_|Error::Invalid)? {cursor.string().map_err(|_|Error::Invalid)?;}}
            out.push(0);out.extend(&begin[..begin.len()-cursor.rest.len()]);out.extend([0;4]);
        } else {
            crate::profile::read(&mut cursor).map_err(|_| Error::Invalid)?;
            if matches!(to,JavaVersion::V1_21_6|JavaVersion::V1_21_8) {
                let mut source=crate::wire::Cursor::new(start);
                let complete=source.count(1).map_err(|_|Error::Invalid)?;
                if complete==0 {
                    let begin=source.rest;
                    if source.boolean().map_err(|_|Error::Invalid)? {source.string().map_err(|_|Error::Invalid)?;}
                    if source.boolean().map_err(|_|Error::Invalid)? {source.take(16).map_err(|_|Error::Invalid)?;}
                    out.extend(&begin[..begin.len()-source.rest.len()]);
                }else{
                    let uuid=source.take(16).map_err(|_|Error::Invalid)?;let name=source.string().map_err(|_|Error::Invalid)?;
                    out.push(1);uint(&mut out,name.len() as u32);out.extend(name.as_bytes());out.push(1);out.extend(uuid);
                }
                let begin=source.rest;
                for _ in 0..source.count(256).map_err(|_|Error::Invalid)? {source.string().map_err(|_|Error::Invalid)?;source.string().map_err(|_|Error::Invalid)?;if source.boolean().map_err(|_|Error::Invalid)? {source.string().map_err(|_|Error::Invalid)?;}}
                out.extend(&begin[..begin.len()-source.rest.len()]);
            }else{out.extend(&start[..start.len()-cursor.rest.len()]);}
        }
        input.take(start.len()-cursor.rest.len())?;
        return Ok(out);
    }
    // Для776 вложенные формы25/49/50/75 разрешены только по равенству pinned
    // схем775/776 (fixture); live776 nested ещё НЕ проверены. Общие32 уровня,
    // 256 записей и LIMIT сохраняются, внутри действует тот же component gate.
    if matches!(kind,25|49|50|75){return crate::nested_items::remap_with_spaces(kind,input,from,to,context,depth,spaces);}
    if matches!(kind,16|18){return crate::item_components::remap(kind,input,from,to);}
    let start=input.0;component_data(kind,input,context)?;Ok(start[..start.len()-input.0.len()].to_vec())
}

fn component_data(kind: u32, input: &mut Reader<'_>, context: Context) -> Result<(), Error> {
    match kind {
        0 => { // Пользовательский custom_data не интерпретируется как игровые ID.
            let data = input.nbt()?; if data.first() != Some(&10) { return Err(Error::Invalid); }
        }
        1 => { if !(1..=99).contains(&input.uint()?) { return Err(Error::Invalid); } }
        2 => { if input.uint()? == 0 { return Err(Error::Invalid); } }
        3 | 19 | 46 => { input.uint()?; }
        4 | 20 => (),
        6 | 9 => { plain_text(input.nbt()?)?; }
        11 => { let count = input.count(256)?; for _ in 0..count { plain_text(input.nbt()?)?; } }
        12 => { input.count(3)?; }
        13 | 42 => {
            let limit = context.shared_enchantment_count.ok_or(Error::MissingEnchantmentContext)?;
            let count = input.count(256)?; let mut seen = BTreeSet::new();
            for _ in 0..count {
                let id = input.uint()?;
                if id >= limit || !seen.insert(id) || input.uint()? > 255 { return Err(Error::Invalid); }
            }
        }
        21 => { input.boolean()?; }
        44 | 45 => {
            let color = i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
            if !(0..=0xffffff).contains(&color) { return Err(Error::Invalid); }
        }
        48 => { input.count(1)?; }
        22 => { if input.nbt()? != [10,0] { return Err(Error::Invalid); } } // intangible_projectile: пустой compound.
        63 => { input.count(4)?; }
        73 => { input.count(15)?; }
        _ => return crate::item_components::validate(kind,input),
    }
    Ok(())
}

/// Текст без ссылок на предметы/сущности: wiki Text_component_format.
/// Вложенные списки ограничены; hover с игровыми ID требует remapper.
pub(crate) fn plain_text(data: &[u8]) -> Result<(), Error> {
    if registry::nbt_length(data).map_err(Error::Nbt)? != data.len() { return Err(Error::Invalid); }
    let (&kind, payload) = data.split_first().ok_or(Error::Truncated)?;
    text_node(kind, payload, 0)
}

fn text_node(kind: u8, data: &[u8], depth: usize) -> Result<(), Error> {
    if depth > 32 { return Err(Error::TooLarge); }
    if kind == 8 { nbt_string(data)?; return Ok(()); }
    if kind == 9 { return text_list(data, depth); }
    if kind != 10 { return Err(Error::Invalid); }
    let fields = fields(data)?;
    if !fields.iter().any(|(name, kind, _)| matches!(name.as_str(), "text" | "translate" | "keybind" | "selector") && *kind == 8) { return Err(Error::Invalid); }
    for (name, kind, payload) in fields {
        match name.as_str() {
            "text" | "font" | "insertion" | "translate" | "fallback" | "keybind" | "selector" if kind == 8 => { nbt_string(payload)?; }
            "extra" | "with" if kind == 9 => text_list(payload, depth + 1)?,
            "separator" => text_node(kind, payload, depth + 1)?,
            "hover_event" if kind == 10 => {
                let event = fields_event(payload)?;
                if event.0 != "show_text" || event.1.len() != 1 || event.1[0].0 != "value" { return Err(Error::UnsupportedComponent(6)); }
                text_node(event.1[0].1, event.1[0].2, depth + 1)?;
            }
            "click_event" if kind == 10 => {
                let (action, values) = fields_event(payload)?;
                if values.len() != 1 { return Err(Error::Invalid); }
                let (key, kind, value) = &values[0];
                match (action.as_str(), key.as_str(), *kind) {
                    ("open_url", "url", 8) => {
                        let value=nbt_string(value)?;
                        if !value.starts_with("https://") && !value.starts_with("http://") { return Err(Error::Invalid); }
                    }
                    ("run_command" | "suggest_command", "command", 8) | ("copy_to_clipboard", "value", 8) => { nbt_string(value)?; }
                    ("change_page", "page", 3) if i32::from_be_bytes((*value).try_into().map_err(|_| Error::Invalid)?) > 0 => (),
                    _ => return Err(Error::UnsupportedComponent(6)),
                }
            }
            "color" if kind == 8 => {
                let color = nbt_string(payload)?;
                let named = matches!(color,"black"|"dark_blue"|"dark_green"|"dark_aqua"|"dark_red"|"dark_purple"|"gold"|"gray"|"dark_gray"|"blue"|"green"|"aqua"|"red"|"light_purple"|"yellow"|"white");
                if !named && !(color.len() == 7 && color.starts_with('#') && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)) { return Err(Error::Invalid); }
            }
            "bold" | "italic" | "underlined" | "strikethrough" | "obfuscated" if kind == 1 && matches!(payload, [0] | [1]) => (),
            _ => return Err(Error::UnsupportedComponent(6)),
        }
    }
    Ok(())
}

fn text_list(mut data: &[u8], depth: usize) -> Result<(), Error> {
    if data.len() < 5 { return Err(Error::Truncated); }
    let kind=data[0];
    let count=i32::from_be_bytes(data[1..5].try_into().map_err(|_| Error::Invalid)?);
    if !(0..=1024).contains(&count) { return Err(Error::TooLarge); }
    data=&data[5..];
    for _ in 0..count {
        let size=registry::nbt_payload_length(kind,data).map_err(Error::Nbt)?;
        text_node(kind,&data[..size],depth+1)?;
        data=&data[size..];
    }
    if !data.is_empty() { return Err(Error::Invalid); }
    Ok(())
}

fn fields_event(data: &[u8]) -> Result<(String, Vec<NbtField<'_>>), Error> {
    let mut fields=fields(data)?;
    let at=fields.iter().position(|(key,kind,_)|key=="action" && *kind==8).ok_or(Error::Invalid)?;
    let (_,_,value)=fields.remove(at);
    Ok((nbt_string(value)?.to_owned(),fields))
}

pub(crate) fn nbt_string(bytes: &[u8]) -> Result<&str, Error> {
    if bytes.len() < 2 { return Err(Error::Truncated); }
    let n = u16::from_be_bytes([bytes[0],bytes[1]]) as usize;
    if bytes.len() != n + 2 { return Err(Error::Invalid); }
    std::str::from_utf8(&bytes[2..]).map_err(|_| Error::Invalid)
}
type NbtField<'a> = (String, u8, &'a [u8]);
pub(crate) fn fields(mut bytes: &[u8]) -> Result<Vec<NbtField<'_>>, Error> {
    let mut out = Vec::new(); let mut seen = BTreeSet::new();
    loop {
        let (&kind, rest) = bytes.split_first().ok_or(Error::Truncated)?;
        if kind == 0 { return if rest.is_empty() { Ok(out) } else { Err(Error::Invalid) }; }
        if rest.len() < 2 { return Err(Error::Truncated); }
        let n = 2 + u16::from_be_bytes([rest[0],rest[1]]) as usize;
        let (name, rest) = rest.split_at_checked(n).ok_or(Error::Truncated)?;
        let name = nbt_string(name)?.to_owned();
        if !seen.insert(name.clone()) { return Err(Error::Invalid); }
        let n = registry::nbt_payload_length(kind, rest).map_err(Error::Nbt)?;
        out.push((name, kind, &rest[..n])); bytes = &rest[n..];
        if out.len() > 256 { return Err(Error::TooLarge); }
    }
}

#[cfg(test)]
#[path = "item_776_tests.rs"]
mod tests_776;

#[cfg(test)]
#[path = "items_catalog_tests.rs"]
mod catalog_tests;
