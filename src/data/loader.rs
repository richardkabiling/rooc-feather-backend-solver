use std::collections::HashMap;
use std::path::Path;
use anyhow::{Context, Result};

use crate::data::schema::*;
use crate::domain::feather::{FeatherId, FeatherDef, FeatherType, Rarity, Set, Tier};
use crate::domain::stats::{StatId, StatVec, StatVecExt};
use crate::domain::inventory::Inventory;
use crate::domain::preset::Preset;

fn read_csv<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(path)
        .with_context(|| format!("opening {:?}", path))?;
    let mut rows = Vec::new();
    for result in rdr.deserialize() {
        let row: T = result.with_context(|| format!("parsing row in {:?}", path))?;
        rows.push(row);
    }
    Ok(rows)
}

fn feather_row_to_stat_vec(r: &FeatherRow) -> StatVec {
    let mut v = StatVec::zero();
    v[StatId::PvPDmgBonus    as usize] = r.pvp_dmg_bonus.unwrap_or(0.0);
    v[StatId::PvPDmgReduction as usize] = r.pvp_dmg_reduction.unwrap_or(0.0);
    v[StatId::IgnorePDEF      as usize] = r.ignore_pdef.unwrap_or(0.0);
    v[StatId::IgnoreMDEF      as usize] = r.ignore_mdef.unwrap_or(0.0);
    v[StatId::PDMG            as usize] = r.pdmg.unwrap_or(0.0);
    v[StatId::MDMG            as usize] = r.mdmg.unwrap_or(0.0);
    v[StatId::PATK            as usize] = r.patk.unwrap_or(0.0);
    v[StatId::MATK            as usize] = r.matk.unwrap_or(0.0);
    v[StatId::PvEDmgBonus     as usize] = r.pve_dmg_bonus.unwrap_or(0.0);
    v[StatId::PvEDmgReduction as usize] = r.pve_dmg_reduction.unwrap_or(0.0);
    v[StatId::PDEF            as usize] = r.pdef.unwrap_or(0.0);
    v[StatId::MDEF            as usize] = r.mdef.unwrap_or(0.0);
    v[StatId::HP              as usize] = r.hp.unwrap_or(0.0);
    v[StatId::PDMGReduction   as usize] = r.pdmg_reduction.unwrap_or(0.0);
    v[StatId::MDMGReduction   as usize] = r.mdmg_reduction.unwrap_or(0.0);
    v[StatId::IntDexStr       as usize] = r.int_dex_str.unwrap_or(0.0);
    v[StatId::VIT             as usize] = r.vit.unwrap_or(0.0);
    v
}

pub fn load_feathers(path: &Path) -> Result<Vec<FeatherDef>> {
    let rows: Vec<FeatherRow> = read_csv(path)?;

    // group by feather name
    let mut by_feather: HashMap<String, Vec<FeatherRow>> = HashMap::new();
    for row in rows {
        if row.tier == 0 { continue; }
        by_feather.entry(row.feather.clone()).or_default().push(row);
    }

    let mut defs = Vec::new();
    for (name, mut trows) in by_feather {
        trows.sort_by_key(|r| r.tier);
        let first = &trows[0];
        let ftype  = FeatherType::from_str(&first.type_)
            .with_context(|| format!("unknown feather type '{}' for '{}'", first.type_, name))?;
        let set    = Set::from_str(&first.set)
            .with_context(|| format!("unknown set '{}' for '{}'", first.set, name))?;
        let rarity = Rarity::from_str(&first.rarity)
            .with_context(|| format!("unknown rarity '{}' for '{}'", first.rarity, name))?;
        let id     = FeatherId::from_str(&name)
            .with_context(|| format!("unknown feather name '{}'", name))?;

        let mut stats: [StatVec; 20] = [StatVec::zero(); 20];
        let mut t1_cost: [u64; 20] = [0; 20];
        let mut upgrade_cost: [u64; 20] = [0; 20];

        for row in &trows {
            let idx = (row.tier - 1) as usize;
            stats[idx] = feather_row_to_stat_vec(row);
            let tc = row.total_cost.unwrap_or(0);
            t1_cost[idx] = 1 + tc;
            upgrade_cost[idx] = row.cost_to_next.unwrap_or(0);
        }

        defs.push(FeatherDef { id, ftype, set, rarity, stats, t1_cost, upgrade_cost });
    }
    Ok(defs)
}

pub fn load_attack_bonuses(path: &Path) -> Result<Vec<AttackBonusRow>> {
    read_csv(path)
}

pub fn load_defense_bonuses(path: &Path) -> Result<Vec<DefenseBonusRow>> {
    read_csv(path)
}

pub fn load_presets(path: &Path) -> Result<HashMap<String, Preset>> {
    let rows: Vec<PresetRow> = read_csv(path)?;
    let mut grouped: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for row in rows {
        grouped.entry(row.preset.clone()).or_default().push((row.stat, row.weight));
    }
    let mut map = HashMap::new();
    for (name, pairs) in grouped {
        map.insert(name.clone(), Preset::from_rows(name, &pairs));
    }
    Ok(map)
}

pub fn load_inventory(path: &Path, feathers: &[FeatherDef]) -> Result<Inventory> {
    // Build lookup: feather name → FeatherDef
    let def_map: HashMap<String, &FeatherDef> = feathers.iter()
        .map(|d| (format!("{:?}", d.id), d))
        .collect();

    let rows: Vec<InventoryRow> = read_csv(path)?;
    let mut inv = Inventory::zero();

    for row in &rows {
        let name = row.feather.trim();
        let def = def_map.get(name)
            .with_context(|| format!("unknown feather '{}' in input.csv", name))?;
        let tier = Tier::new(row.tier);
        let t1_equiv = row.count * def.t1_cost_at(tier);
        inv.add(def.set, t1_equiv);
    }
    Ok(inv)
}
