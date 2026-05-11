/// Canonical stat identifiers — exactly 17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum StatId {
    PvPDmgBonus      = 0,
    PvPDmgReduction  = 1,
    IgnorePDEF       = 2,
    IgnoreMDEF       = 3,
    PDMG             = 4,
    MDMG             = 5,
    PATK             = 6,
    MATK             = 7,
    PvEDmgBonus      = 8,
    PvEDmgReduction  = 9,
    PDEF             = 10,
    MDEF             = 11,
    HP               = 12,
    PDMGReduction    = 13,
    MDMGReduction    = 14,
    IntDexStr        = 15,
    VIT              = 16,
}

pub const STAT_COUNT: usize = 17;

pub type StatVec = [f64; STAT_COUNT];

pub trait StatVecExt {
    fn zero() -> Self;
    fn add_assign(&mut self, other: &Self);
    fn dot(&self, other: &Self) -> f64;
    fn scale_pct(&self, pct: &Self) -> Self;
}

impl StatVecExt for StatVec {
    fn zero() -> Self {
        [0.0; STAT_COUNT]
    }

    fn add_assign(&mut self, other: &Self) {
        for i in 0..STAT_COUNT {
            self[i] += other[i];
        }
    }

    fn dot(&self, other: &Self) -> f64 {
        let mut s = 0.0;
        for i in 0..STAT_COUNT {
            s += self[i] * other[i];
        }
        s
    }

    /// Returns self[i] * (1.0 + pct[i] / 100.0) for each i
    fn scale_pct(&self, pct: &Self) -> Self {
        let mut out = *self;
        for i in 0..STAT_COUNT {
            out[i] *= 1.0 + pct[i] / 100.0;
        }
        out
    }
}

/// Parse a stat-name token from a CSV header / preset file into a StatId.
pub fn parse_stat_id(s: &str) -> Option<StatId> {
    let s = s.trim();
    match s {
        "PvPDmgBonus"   | "PvP DMG Bonus"              => Some(StatId::PvPDmgBonus),
        "PvPDmgReduction"| "PvP DMG Reduction"         => Some(StatId::PvPDmgReduction),
        "IgnorePDEF"    | "Ignore PDEF"                => Some(StatId::IgnorePDEF),
        "IgnoreMDEF"    | "Ignore MDEF"                => Some(StatId::IgnoreMDEF),
        "PDMG"                                         => Some(StatId::PDMG),
        "MDMG"                                         => Some(StatId::MDMG),
        "PATK"                                         => Some(StatId::PATK),
        "MATK"                                         => Some(StatId::MATK),
        "PvEDmgBonus"   | "PvE DMG Bonus"              => Some(StatId::PvEDmgBonus),
        "PvEDmgReduction"| "PvE DMG Reduction"         => Some(StatId::PvEDmgReduction),
        "PDEF"                                         => Some(StatId::PDEF),
        "MDEF"                                         => Some(StatId::MDEF),
        "HP"                                           => Some(StatId::HP),
        "PDMGReduction" | "PDMG Reduction"             => Some(StatId::PDMGReduction),
        "MDMGReduction" | "MDMG Reduction"             => Some(StatId::MDMGReduction),
        "IntDexStr"     | "INT/DEX/STR" | "INTDEXSTR"  => Some(StatId::IntDexStr),
        "VIT"                                          => Some(StatId::VIT),
        _                                              => None,
    }
}
