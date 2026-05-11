use crate::domain::stats::{StatVec, StatVecExt};

/// The 5 inventory sets (conversion pools).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Set {
    STDN   = 0,
    DN     = 1,
    ST     = 2,
    LD     = 3,
    Purple = 4,
}

pub const SET_COUNT: usize = 5;

impl Set {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "STDN"   => Some(Set::STDN),
            "DN"     => Some(Set::DN),
            "ST"     => Some(Set::ST),
            "LD"     => Some(Set::LD),
            "Purple" => Some(Set::Purple),
            _        => None,
        }
    }
}

/// Feather type (Orange = Attack/Defense/Hybrid, Purple = separate rarity)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatherType {
    Attack,
    Defense,
    Hybrid,
}

impl FeatherType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Attack"  => Some(FeatherType::Attack),
            "Defense" => Some(FeatherType::Defense),
            "Hybrid"  => Some(FeatherType::Hybrid),
            _         => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rarity {
    Orange,
    Purple,
}

impl Rarity {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Orange" => Some(Rarity::Orange),
            "Purple" => Some(Rarity::Purple),
            _        => None,
        }
    }
}

/// Tier 1..=20, enforced at construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tier(u8);

impl Tier {
    pub const MIN: Tier = Tier(1);
    pub const MAX: Tier = Tier(20);

    pub fn new(v: u8) -> Self {
        assert!(v >= 1 && v <= 20, "Tier must be 1..=20, got {}", v);
        Tier(v)
    }

    pub fn get(self) -> u8 { self.0 }

    pub fn try_new(v: u8) -> Option<Self> {
        if v >= 1 && v <= 20 { Some(Tier(v)) } else { None }
    }

    pub fn next(self) -> Option<Self> {
        if self.0 < 20 { Some(Tier(self.0 + 1)) } else { None }
    }

    pub fn prev(self) -> Option<Self> {
        if self.0 > 1 { Some(Tier(self.0 - 1)) } else { None }
    }
}

impl Default for Tier {
    fn default() -> Self { Tier::MIN }
}

/// Unique feather name enum — 18 feather types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatherId {
    // Attack orange (STDN)
    Space,
    Time,
    // Attack orange (DN)
    Day,
    // Attack orange (ST)
    Sky,
    // Defense orange (STDN)
    Divine,
    Nature,
    // Defense orange (DN)
    Night,
    // Defense orange (ST)
    Terra,
    // Hybrid orange (LD)
    Light,
    Dark,
    // Hybrid purple
    Justice,
    Grace,
    // Attack purple
    Stats,
    // Defense purple
    Soul,
    Virtue,
    Mercy,
}

impl FeatherId {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Space"   => Some(FeatherId::Space),
            "Time"    => Some(FeatherId::Time),
            "Day"     => Some(FeatherId::Day),
            "Sky"     => Some(FeatherId::Sky),
            "Divine"  => Some(FeatherId::Divine),
            "Nature"  => Some(FeatherId::Nature),
            "Night"   => Some(FeatherId::Night),
            "Terra"   => Some(FeatherId::Terra),
            "Light"   => Some(FeatherId::Light),
            "Dark"    => Some(FeatherId::Dark),
            "Justice" => Some(FeatherId::Justice),
            "Grace"   => Some(FeatherId::Grace),
            "Stats"   => Some(FeatherId::Stats),
            "Soul"    => Some(FeatherId::Soul),
            "Virtue"  => Some(FeatherId::Virtue),
            "Mercy"   => Some(FeatherId::Mercy),
            _         => None,
        }
    }

    pub fn all() -> &'static [FeatherId] {
        &[
            FeatherId::Space, FeatherId::Time,
            FeatherId::Day, FeatherId::Sky,
            FeatherId::Divine, FeatherId::Nature,
            FeatherId::Night, FeatherId::Terra,
            FeatherId::Light, FeatherId::Dark,
            FeatherId::Justice, FeatherId::Grace,
            FeatherId::Stats,
            FeatherId::Soul, FeatherId::Virtue, FeatherId::Mercy,
        ]
    }
}

/// Static definition of a feather (type, set, rarity).
#[derive(Debug, Clone)]
pub struct FeatherDef {
    pub id:      FeatherId,
    pub ftype:   FeatherType,
    pub set:     Set,
    pub rarity:  Rarity,
    /// Stats at each tier (index 0 = T1, index 19 = T20).
    pub stats:   [StatVec; 20],
    /// T1-equivalent cost to occupy a slot at tier t (= 1 + total_cost[t]).
    pub t1_cost: [u64; 20],
    /// Cost to upgrade from tier t to t+1 (index 0 = T1→T2 … 18 = T19→T20; 19 = 0 for T20).
    pub upgrade_cost: [u64; 20],
}

impl FeatherDef {
    pub fn stats_at(&self, tier: Tier) -> &StatVec {
        &self.stats[(tier.get() - 1) as usize]
    }

    /// T1-equivalent units consumed by occupying this slot at `tier`.
    pub fn t1_cost_at(&self, tier: Tier) -> u64 {
        self.t1_cost[(tier.get() - 1) as usize]
    }

    /// Cost to upgrade from `tier` to `tier+1`. Returns 0 if at T20.
    pub fn upgrade_cost_from(&self, tier: Tier) -> u64 {
        self.upgrade_cost[(tier.get() - 1) as usize]
    }
}

/// Which kind of statue (Attack or Defense).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatueKind {
    Attack,
    Defense,
}

/// Returns the list of feather IDs eligible for the orange slots of a statue.
pub fn eligible_orange(kind: StatueKind) -> &'static [FeatherId] {
    match kind {
        StatueKind::Attack => &[
            FeatherId::Space, FeatherId::Time,
            FeatherId::Day, FeatherId::Sky,
            FeatherId::Light, FeatherId::Dark,
        ],
        StatueKind::Defense => &[
            FeatherId::Divine, FeatherId::Nature,
            FeatherId::Night, FeatherId::Terra,
            FeatherId::Light, FeatherId::Dark,
        ],
    }
}

/// Returns the list of feather IDs eligible for the purple slot of a statue.
pub fn eligible_purple(kind: StatueKind) -> &'static [FeatherId] {
    match kind {
        StatueKind::Attack  => &[FeatherId::Justice, FeatherId::Grace, FeatherId::Stats],
        StatueKind::Defense => &[FeatherId::Justice, FeatherId::Grace, FeatherId::Soul, FeatherId::Virtue, FeatherId::Mercy],
    }
}
