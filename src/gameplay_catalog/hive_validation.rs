//! Улей в чанке и переносимый улей проверяются тем же каталогом, что предметы.
use super::{Catalog, StackError, StackLocation, StackProblem};

impl Catalog {
    pub fn validate_hive(self, hive: &crate::hive::Data) -> Result<(), StackError> {
        self.validate_hive_at(hive, &mut Vec::new(), 0)
    }

    pub(super) fn validate_hive_at(self, hive: &crate::hive::Data, path: &mut Vec<StackLocation>, depth: usize) -> Result<(), StackError> {
        if depth >= 16 { return Err(StackError { path: path.clone(), problem: StackProblem::NestingLimit }); }
        if hive.bees.len() > 3 { return Err(StackError { path: path.clone(), problem: StackProblem::InvalidHive }); }
        for (index, occupant) in hive.bees.iter().enumerate() {
            path.push(StackLocation::HiveOccupant(index));
            let result = crate::hive::decode_occupant(occupant)
                .ok_or_else(|| StackError { path: path.clone(), problem: StackProblem::InvalidHive })
                .and_then(|bee| self.validate_mob_at(&bee, path, depth + 1));
            path.pop();
            result?;
        }
        Ok(())
    }
}
