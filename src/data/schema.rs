use serde::Deserialize;

/// Raw row from feather-stats.csv (T0 rows should be filtered out).
#[derive(Debug, Deserialize)]
pub struct FeatherRow {
    #[serde(rename = "Feather")]
    pub feather: String,
    #[serde(rename = "Type")]
    pub type_: String,
    #[serde(rename = "Set")]
    pub set: String,
    #[serde(rename = "Tier")]
    pub tier: u8,
    #[serde(rename = "Rarity")]
    pub rarity: String,
    #[serde(rename = "Cost to Next Tier")]
    pub cost_to_next: Option<u64>,
    #[serde(rename = "PvP DMG Bonus")]
    pub pvp_dmg_bonus: Option<f64>,
    #[serde(rename = "PvP DMG Reduction")]
    pub pvp_dmg_reduction: Option<f64>,
    #[serde(rename = "Ignore PDEF")]
    pub ignore_pdef: Option<f64>,
    #[serde(rename = "Ignore MDEF")]
    pub ignore_mdef: Option<f64>,
    #[serde(rename = "PDMG")]
    pub pdmg: Option<f64>,
    #[serde(rename = "MDMG")]
    pub mdmg: Option<f64>,
    #[serde(rename = "PATK")]
    pub patk: Option<f64>,
    #[serde(rename = "MATK")]
    pub matk: Option<f64>,
    #[serde(rename = "PvE DMG Bonus")]
    pub pve_dmg_bonus: Option<f64>,
    #[serde(rename = "PvE DMG Reduction")]
    pub pve_dmg_reduction: Option<f64>,
    #[serde(rename = "PDEF")]
    pub pdef: Option<f64>,
    #[serde(rename = "MDEF")]
    pub mdef: Option<f64>,
    #[serde(rename = "HP")]
    pub hp: Option<f64>,
    #[serde(rename = "PDMG Reduction")]
    pub pdmg_reduction: Option<f64>,
    #[serde(rename = "MDMG Reduction")]
    pub mdmg_reduction: Option<f64>,
    #[serde(rename = "INT/DEX/STR")]
    pub int_dex_str: Option<f64>,
    #[serde(rename = "VIT")]
    pub vit: Option<f64>,
    #[serde(rename = "Total Cost")]
    pub total_cost: Option<u64>,
}

/// Raw row from attack-set-bonuses.csv
#[derive(Debug, Clone, Deserialize)]
pub struct AttackBonusRow {
    #[serde(rename = "Tier")]
    pub tier: u8,
    #[serde(rename = "PATK")]
    pub patk: f64,
    #[serde(rename = "MATK")]
    pub matk: f64,
    #[serde(rename = "PvP DMG Bonus")]
    pub pvp_dmg_bonus: f64,
    #[serde(rename = "Attack Stats Percentage Bonus")]
    pub attack_pct: f64,
    #[serde(rename = "PvE Stats Percentage Bonus")]
    pub pve_pct: f64,
    #[serde(rename = "PvP Stats Percentage Bonus")]
    pub pvp_pct: f64,
}

/// Raw row from defense-set-bonuses.csv
#[derive(Debug, Clone, Deserialize)]
pub struct DefenseBonusRow {
    #[serde(rename = "Tier")]
    pub tier: u8,
    #[serde(rename = "HP")]
    pub hp: f64,
    #[serde(rename = "PvP DMG Reduction")]
    pub pvp_dmg_reduction: f64,
    #[serde(rename = "Defense Stats Percentage Bonus")]
    pub defense_pct: f64,
    #[serde(rename = "PvE Stats Percentage Bonus")]
    pub pve_pct: f64,
    #[serde(rename = "PvP Stats Percentage Bonus")]
    pub pvp_pct: f64,
}

/// Raw row from presets.csv
#[derive(Debug, Deserialize)]
pub struct PresetRow {
    pub preset: String,
    pub stat:   String,
    pub weight: f64,
}

/// Raw row from input.csv
#[derive(Debug, Deserialize)]
pub struct InventoryRow {
    #[serde(rename = "Feather")]
    pub feather: String,
    #[serde(rename = "Tier")]
    pub tier: u8,
    #[serde(rename = "Count")]
    pub count: u64,
}
