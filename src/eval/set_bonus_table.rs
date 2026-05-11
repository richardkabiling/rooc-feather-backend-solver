use crate::domain::stats::{StatId, StatVec, StatVecExt};
use crate::data::schema::{AttackBonusRow, DefenseBonusRow};

/// Precomputed bonus for one tier of an Attack statue.
#[derive(Debug, Clone, Copy)]
pub struct AttackSetBonus {
    pub flat: StatVec,
    pub pct:  StatVec,
}

/// Precomputed bonus for one tier of a Defense statue.
#[derive(Debug, Clone, Copy)]
pub struct DefenseSetBonus {
    pub flat: StatVec,
    pub pct:  StatVec,
}

/// Build the [AttackSetBonus; 20] table (index 0 = T1).
pub fn build_attack_table(rows: &[AttackBonusRow]) -> [AttackSetBonus; 20] {
    let mut table = [AttackSetBonus { flat: StatVec::zero(), pct: StatVec::zero() }; 20];
    for row in rows {
        let idx = (row.tier - 1) as usize;
        let mut flat = StatVec::zero();
        let mut pct  = StatVec::zero();

        // Flat bonuses
        flat[StatId::PATK          as usize] = row.patk;
        flat[StatId::MATK          as usize] = row.matk;
        flat[StatId::PvPDmgBonus   as usize] = row.pvp_dmg_bonus;

        // Attack Stats % → IgnorePDEF, IgnoreMDEF, PATK, MATK, PDMG, MDMG
        pct[StatId::IgnorePDEF    as usize] = row.attack_pct;
        pct[StatId::IgnoreMDEF    as usize] = row.attack_pct;
        pct[StatId::PATK          as usize] = row.attack_pct;
        pct[StatId::MATK          as usize] = row.attack_pct;
        pct[StatId::PDMG          as usize] = row.attack_pct;
        pct[StatId::MDMG          as usize] = row.attack_pct;

        // PvE Stats % → PvEDmgReduction, PvEDmgBonus
        pct[StatId::PvEDmgReduction as usize] = row.pve_pct;
        pct[StatId::PvEDmgBonus     as usize] = row.pve_pct;

        // PvP Stats % → PvPDmgReduction, PvPDmgBonus
        pct[StatId::PvPDmgReduction as usize] = row.pvp_pct;
        pct[StatId::PvPDmgBonus     as usize] = row.pvp_pct;

        table[idx] = AttackSetBonus { flat, pct };
    }
    table
}

/// Build the [DefenseSetBonus; 20] table (index 0 = T1).
pub fn build_defense_table(rows: &[DefenseBonusRow]) -> [DefenseSetBonus; 20] {
    let mut table = [DefenseSetBonus { flat: StatVec::zero(), pct: StatVec::zero() }; 20];
    for row in rows {
        let idx = (row.tier - 1) as usize;
        let mut flat = StatVec::zero();
        let mut pct  = StatVec::zero();

        // Flat bonuses
        flat[StatId::HP             as usize] = row.hp;
        flat[StatId::PvPDmgReduction as usize] = row.pvp_dmg_reduction;

        // Defense Stats % → PDEF, MDEF, HP, PDMGReduction, MDMGReduction
        pct[StatId::PDEF          as usize] = row.defense_pct;
        pct[StatId::MDEF          as usize] = row.defense_pct;
        pct[StatId::HP            as usize] = row.defense_pct;
        pct[StatId::PDMGReduction as usize] = row.defense_pct;
        pct[StatId::MDMGReduction as usize] = row.defense_pct;

        // PvE Stats %
        pct[StatId::PvEDmgReduction as usize] = row.pve_pct;
        pct[StatId::PvEDmgBonus     as usize] = row.pve_pct;

        // PvP Stats %
        pct[StatId::PvPDmgReduction as usize] = row.pvp_pct;
        pct[StatId::PvPDmgBonus     as usize] = row.pvp_pct;

        table[idx] = DefenseSetBonus { flat, pct };
    }
    table
}
