pub mod schema;
pub mod loader;

use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;

use crate::domain::feather::FeatherDef;
use crate::domain::preset::Preset;
use crate::data::schema::{AttackBonusRow, DefenseBonusRow};

/// All game data loaded at startup.
pub struct GameData {
    pub feathers:          Vec<FeatherDef>,
    pub attack_bonuses:    Vec<AttackBonusRow>,
    pub defense_bonuses:   Vec<DefenseBonusRow>,
    pub presets:           HashMap<String, Preset>,
}

impl GameData {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let feathers        = loader::load_feathers(&data_dir.join("feather-stats.csv"))?;
        let attack_bonuses  = loader::load_attack_bonuses(&data_dir.join("attack-set-bonuses.csv"))?;
        let defense_bonuses = loader::load_defense_bonuses(&data_dir.join("defense-set-bonuses.csv"))?;
        let presets         = loader::load_presets(&data_dir.join("presets.csv"))?;
        Ok(GameData { feathers, attack_bonuses, defense_bonuses, presets })
    }
}
