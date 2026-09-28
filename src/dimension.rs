// Измерения: обычный мир и Незер.
//
// У каждого измерения свой мир (свои чанки, такты, выпавшие предметы) —
// в своей папке, как у оригинала 26.x: обычный мир в корне папки мира,
// Незер — в `dimensions/minecraft/the_nether`. Клиенту измерения объявлены
// в реестре `minecraft:dimension_type` в этом же порядке.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Dimension {
    #[default]
    Overworld,
    Nether,
}

impl Dimension {
    /// Все измерения по порядку записи в реестре.
    pub const ALL: [Dimension; 2] = [Dimension::Overworld, Dimension::Nether];

    /// Имя измерения, как его знает клиент.
    pub fn name(self) -> &'static str {
        match self {
            Dimension::Overworld => "minecraft:overworld",
            Dimension::Nether => "minecraft:the_nether",
        }
    }

    /// Имя без приставки — для реестра и файлов.
    pub fn short(self) -> &'static str {
        match self {
            Dimension::Overworld => "overworld",
            Dimension::Nether => "the_nether",
        }
    }

    /// Номер типа измерения в реестре `minecraft:dimension_type`.
    pub fn type_id(self) -> i32 {
        match self {
            Dimension::Overworld => 0,
            Dimension::Nether => 1,
        }
    }

    /// Номер в общем списке (для массивов по измерениям).
    pub fn index(self) -> usize {
        self.type_id() as usize
    }

    /// Разбирает имя: с приставкой или без.
    pub fn from_name(name: &str) -> Option<Dimension> {
        let name = name.trim();
        let short = name.strip_prefix("minecraft:").unwrap_or(name);

        Dimension::ALL.into_iter().find(|dimension| dimension.short() == short)
    }

    /// Во сколько раз расстояния здесь короче, чем в обычном мире.
    #[allow(dead_code)] // для порталов
    pub fn scale(self) -> f64 {
        match self {
            Dimension::Overworld => 1.0,
            Dimension::Nether => 8.0,
        }
    }

    /// Папка мира этого измерения внутри папки мира.
    pub fn folder(self, world: &Path) -> PathBuf {
        match self {
            Dimension::Overworld => world.to_path_buf(),
            Dimension::Nether => world.join("dimensions").join("minecraft").join("the_nether"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for dimension in Dimension::ALL {
            assert_eq!(Dimension::from_name(dimension.name()), Some(dimension));
            assert_eq!(Dimension::from_name(dimension.short()), Some(dimension));
        }
        assert_eq!(Dimension::from_name("minecraft:the_end"), None);
    }
}
