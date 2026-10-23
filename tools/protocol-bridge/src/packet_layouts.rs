//! Явная пара версий для имён, ID и равенства схем пакетов.
//! Это private inventory: открытие таблицы не разрешает payload или admission.
use crate::{JavaVersion, generated_ids, generated_packet_776, ids::RegistryKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PacketInfo {
    pub name: &'static str,
    /// None: исходное имя существует, но отсутствует в целевой версии.
    pub target_id: Option<u32>,
    /// Только полное равенство pinned JSON графов выбранной пары.
    pub schema_equal: bool,
}

fn name(kind: RegistryKind, version: JavaVersion, id: usize) -> Option<&'static str> {
    match version {
        JavaVersion::V26_1_2 => generated_ids::packet_info(kind, false, id).map(|(name, _)| name),
        JavaVersion::V26_2 => generated_packet_776::names(kind)?.get(id).copied(),
        JavaVersion::V26_3 => generated_ids::packet_info(kind, true, id).map(|(name, _)| name),
        JavaVersion::V1_21_11 | JavaVersion::V1_12_2 => None,
    }
}

/// None: неподдерживаемая версия/реестр или отсутствующий исходный ID.
/// Отсутствующая цель представлена отдельно через PacketInfo::target_id.
/// Числовые ID не являются основанием для переименования spectator packets.
pub(crate) fn packet_info(kind: RegistryKind, from: JavaVersion, to: JavaVersion, id: usize) -> Option<PacketInfo> {
    if (from == JavaVersion::V1_21_11 && to == JavaVersion::V26_1_2) || (to == JavaVersion::V1_21_11 && from == JavaVersion::V26_1_2) {
        let &(name, target, schema_equal) = crate::generated_step1::packets(kind, from == JavaVersion::V1_21_11)?.get(id)?;
        return Some(PacketInfo { name, target_id: (target != u16::MAX).then_some(u32::from(target)), schema_equal });
    }
    if !matches!(to, JavaVersion::V26_1_2 | JavaVersion::V26_2 | JavaVersion::V26_3) { return None; }
    let name = name(kind, from, id)?;
    if from == to { return Some(PacketInfo { name, target_id: Some(u32::try_from(id).ok()?), schema_equal: true }); }
    if from == JavaVersion::V26_2 || to == JavaVersion::V26_2 {
        let &(target, schema_equal) = generated_packet_776::route(kind, from, to)?.get(id)?;
        return Some(PacketInfo { name,
            target_id: if target == generated_packet_776::MISSING { None } else { Some(u32::from(target)) },
            schema_equal });
    }
    // Прежние775↔777 таблицы и equality переиспользуются без регенерации.
    let modern = from == JavaVersion::V26_3;
    let (_, schema_equal) = generated_ids::packet_info(kind, modern, id)?;
    let (forward, backward) = generated_ids::tables(kind);
    let target = *(if modern { backward } else { forward }).get(id)?;
    Some(PacketInfo { name, target_id: u32::try_from(target).ok(), schema_equal })
}

#[cfg(test)]
#[path = "packet_layout_tests.rs"]
mod tests;
