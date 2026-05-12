# Commit Conventions

All commits must follow **Conventional Commits** format:

```
<type>[!]: <description>
```

| Type | Semver bump | When to use |
|------|-------------|-------------|
| `fix` | patch | Bug fix |
| `chore` | patch | Maintenance, deps, tooling |
| `refactor` | patch | Code restructure, no behaviour change |
| `perf` | patch | Performance improvement |
| `docs` | patch | Documentation only |
| `test` | patch | Tests only |
| `feat` | minor | New feature, backwards-compatible |
| `feat!` or any `!` suffix | major | Breaking change |

Examples:
```
fix: correct t1_cost double-count in inventory loader
feat: add cross-chain solution sharing
feat!: change JSON output schema to include statue scores
```

The CI auto-tag workflow reads the commits since the last tag and applies the **highest** bump found:
- Any `!` suffix or `BREAKING CHANGE` in the body → major
- Any `feat` → minor
- Anything else → patch

---

# Setup Rules

## Statues
- There are **5 Attack Statues** and **5 Defense Statues** (10 total).
- Each statue holds **5 feathers**: exactly **4 orange** rarity and **1 purple** rarity.
- All 5 feathers in a statue must be **unique** (no repeats within the same statue).
- A feather can be placed in any statue of any kind, but only if it is eligible for that statue type (see Feather Types below).

## Feather Slots per Statue
- **Attack Statue orange slots**: must be filled with Attack or Hybrid orange feathers (Space, Time, Day, Sky, Light, Dark).
- **Attack Statue purple slot**: must be filled with a Hybrid or Attack purple feather (Justice, Grace, Stats).
- **Defense Statue orange slots**: must be filled with Defense or Hybrid orange feathers (Divine, Nature, Night, Terra, Light, Dark).
- **Defense Statue purple slot**: must be filled with a Hybrid or Defense purple feather (Justice, Grace, Soul, Virtue, Mercy).

## Feather Tiers
- Feathers can be tiered up from **tier 1 (minimum)** to **tier 20 (maximum)**.
- Each tier-up has an associated cost (in T1-equivalent units).
- The **set bonus** for a statue is determined by the **lowest feather tier** among the 5 feathers in that statue.

## Stat Calculation per Statue
1. Sum the raw stats from all 5 feathers in the statue.
2. Apply the **percentage bonuses** from the set bonus (multiplier applied to the raw sum).
3. Add the **flat bonuses** from the set bonus on top.

So: `Statue Stats = (Sum of Feather Stats × (1 + Percentage Bonus%)) + Flat Bonus`

## Feather Conversion
- Feathers can be converted to any other feather **within the same conversion set** (STDN, DN, ST, LD, or Purple).
- Conversion sets determine which feathers are interchangeable in the inventory.

---

# Feathers

## Conversion Sets
Feathers are grouped into conversion sets. Within a set, any feather can be converted to any other.

| Set    | Feathers                                   |
|--------|--------------------------------------------|
| STDN   | Space, Time, Divine, Nature (Attack + Defense orange) |
| DN     | Day, Night (Attack + Defense orange)       |
| ST     | Sky, Terra (Attack + Defense orange)       |
| LD     | Light, Dark (Hybrid orange)                |
| Purple | Justice, Grace, Stats, Soul, Virtue, Mercy |

## Feather Types and Rarities

### Orange Feathers — Attack Type
| Feather | Set  | Eligible For  |
|---------|------|---------------|
| Space   | STDN | Attack Statue |
| Time    | STDN | Attack Statue |
| Day     | DN   | Attack Statue |
| Sky     | ST   | Attack Statue |

### Orange Feathers — Defense Type
| Feather | Set  | Eligible For   |
|---------|------|----------------|
| Divine  | STDN | Defense Statue |
| Nature  | STDN | Defense Statue |
| Night   | DN   | Defense Statue |
| Terra   | ST   | Defense Statue |

### Orange Feathers — Hybrid Type
| Feather | Set | Eligible For                    |
|---------|-----|---------------------------------|
| Light   | LD  | Attack Statue or Defense Statue |
| Dark    | LD  | Attack Statue or Defense Statue |

### Purple Feathers — Hybrid Type
| Feather | Eligible For                    |
|---------|---------------------------------|
| Justice | Attack Statue or Defense Statue |
| Grace   | Attack Statue or Defense Statue |

### Purple Feathers — Attack Type
| Feather | Eligible For  |
|---------|---------------|
| Stats   | Attack Statue |

### Purple Feathers — Defense Type
| Feather | Eligible For   |
|---------|----------------|
| Soul    | Defense Statue |
| Virtue  | Defense Statue |
| Mercy   | Defense Statue |

## Feather Stats
Each feather provides a subset of the following stats per tier. Stats are split into categories that correspond to which percentage bonus amplifies them.

| Stat             | Category  |
|------------------|-----------|
| Ignore PDEF      | Attack    |
| Ignore MDEF      | Attack    |
| PATK             | Attack    |
| MATK             | Attack    |
| PDMG             | Attack    |
| MDMG             | Attack    |
| PDEF             | Defense   |
| MDEF             | Defense   |
| HP               | Defense   |
| PDMG Reduction   | Defense   |
| MDMG Reduction   | Defense   |
| PvE DMG Bonus    | PvE       |
| PvE DMG Reduction| PvE       |
| PvP DMG Bonus    | PvP       |
| PvP DMG Reduction| PvP       |
| INT/DEX/STR      | (misc)    |
| VIT              | (misc)    |

---

# Set Bonuses

Set bonuses are determined by the **lowest tier** feather in the statue's 5-feather set.

## Attack Statue Set Bonuses (by minimum tier)

| Min Tier | PATK (flat) | MATK (flat) | PvP DMG Bonus (flat) | Attack % Bonus | PvE % Bonus | PvP % Bonus |
|----------|-------------|-------------|----------------------|----------------|-------------|-------------|
| 1        | 13          | 13          | 12                   | 10%            | 30%         | 30%         |
| 2        | 18          | 18          | 15                   | 11%            | 33%         | 33%         |
| 3        | 23          | 23          | 18                   | 12%            | 36%         | 36%         |
| 4        | 28          | 28          | 21                   | 13%            | 39%         | 39%         |
| 5        | 33          | 33          | 24                   | 14%            | 42%         | 42%         |
| 6        | 38          | 38          | 27                   | 15%            | 45%         | 45%         |
| 7        | 43          | 43          | 30                   | 16%            | 48%         | 48%         |
| 8        | 48          | 48          | 33                   | 17%            | 51%         | 51%         |
| 9        | 53          | 53          | 36                   | 18%            | 54%         | 54%         |
| 10       | 58          | 58          | 39                   | 20%            | 60%         | 60%         |
| 11       | 60          | 60          | 42                   | 21%            | 62%         | 62%         |
| 12       | 62          | 62          | 45                   | 22%            | 64%         | 64%         |
| 13       | 64          | 64          | 48                   | 23%            | 66%         | 66%         |
| 14       | 66          | 66          | 51                   | 24%            | 68%         | 68%         |
| 15       | 68          | 68          | 54                   | 25%            | 70%         | 70%         |
| 16       | 70          | 70          | 57                   | 26%            | 72%         | 72%         |
| 17       | 72          | 72          | 60                   | 27%            | 74%         | 74%         |
| 18       | 74          | 74          | 63                   | 28%            | 76%         | 76%         |
| 19       | 76          | 76          | 66                   | 29%            | 78%         | 78%         |
| 20       | 78          | 78          | 69                   | 30%            | 80%         | 80%         |

## Defense Statue Set Bonuses (by minimum tier)

| Min Tier | HP (flat) | PvP DMG Reduction (flat) | Defense % Bonus | PvE % Bonus | PvP % Bonus |
|----------|-----------|--------------------------|-----------------|-------------|-------------|
| 1        | 110       | 12                       | 10%             | 30%         | 30%         |
| 2        | 200       | 15                       | 11%             | 33%         | 33%         |
| 3        | 290       | 18                       | 12%             | 36%         | 36%         |
| 4        | 380       | 21                       | 13%             | 39%         | 39%         |
| 5        | 470       | 24                       | 14%             | 42%         | 42%         |
| 6        | 560       | 27                       | 15%             | 45%         | 45%         |
| 7        | 650       | 30                       | 16%             | 48%         | 48%         |
| 8        | 740       | 33                       | 17%             | 51%         | 51%         |
| 9        | 830       | 36                       | 18%             | 54%         | 54%         |
| 10       | 920       | 39                       | 20%             | 60%         | 60%         |
| 11       | 960       | 42                       | 21%             | 62%         | 62%         |
| 12       | 1000      | 45                       | 22%             | 64%         | 64%         |
| 13       | 1040      | 48                       | 23%             | 66%         | 66%         |
| 14       | 1080      | 51                       | 24%             | 68%         | 68%         |
| 15       | 1120      | 54                       | 25%             | 70%         | 70%         |
| 16       | 1160      | 57                       | 26%             | 72%         | 72%         |
| 17       | 1200      | 60                       | 27%             | 74%         | 74%         |
| 18       | 1240      | 63                       | 28%             | 76%         | 76%         |
| 19       | 1280      | 66                       | 29%             | 78%         | 78%         |
| 20       | 1320      | 69                       | 30%             | 80%         | 80%         |

---

# Stat Categories and Percentage Bonus Amplification

## Attack Stats
Amplified by the **Attack Stats Percentage Bonus** from Attack Statue set bonuses:
- Ignore PDEF
- Ignore MDEF
- PATK
- MATK
- PDMG
- MDMG

## Defense Stats
Amplified by the **Defense Stats Percentage Bonus** from Defense Statue set bonuses:
- PDEF
- MDEF
- HP
- PDMG Reduction
- MDMG Reduction

## PvE Stats
Amplified by the **PvE Stats Percentage Bonus** (present on both Attack and Defense statues):
- PvE DMG Reduction
- PvE DMG Bonus

## PvP Stats
Amplified by the **PvP Stats Percentage Bonus** (present on both Attack and Defense statues):
- PvP DMG Reduction
- PvP DMG Bonus