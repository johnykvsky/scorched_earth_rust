# Scorched Earth 1.5 Game Mechanics

A reference for the rules, formulas and numbers of **Scorched Earth v1.5**
(Wendell Hicken, 1995, DOS shareware). It is meant for anyone remaking,
modding or studying the game, so nobody has to disassemble anything again.

This document describes the **original game only**. It ignores anything added
by remakes or clones.

## Contents

1. [Sources and confidence](#1-sources-and-confidence)
2. [Game structure](#2-game-structure)
3. [Tanks](#3-tanks)
4. [Physics](#4-physics)
5. [Terrain](#5-terrain)
6. [Damage](#6-damage)
7. [Weapons](#7-weapons)
8. [Accessories](#8-accessories)
9. [Economy](#9-economy)
10. [Scoring](#10-scoring)
11. [Computer players](#11-computer-players)
12. [Environment](#12-environment)
13. [Play modes, order, teams](#13-play-modes-order-teams)
14. [Controls](#14-controls)
15. [Presentation](#15-presentation)
16. [Settings reference (`scorch.cfg`)](#16-settings-reference-scorchcfg)
17. [File formats](#17-file-formats)
18. [Verification notes](#18-verification-notes)

---

## 1. Sources and confidence

Every fact is tagged with where it comes from:

| Tag | Meaning |
|---|---|
| ✅ | **Verified in the binary**: read from the original `SCORCH.EXE` machine code or data, or from its data files, by this project. A code location is given as `seg:off`. |
| DOSBox ✅ | **Observed** in the original running in DOSBox 0.74-3 (fixed 20,000 cycles, default 360×480 mode). |
| 📘 | **Manual**: stated in the original manual `SCORCH.DOC` (v1.5), page given as *p. N*. |
| 🟡 | **Third-party reverse engineering**, not re-checked here. Taken from [gosuwachu/scorched-earth](https://github.com/gosuwachu/scorched-earth) (static analysis + DOSBox runtime checks; preferred) or [DigitalCyberSoft/scorchedearth-python](https://github.com/DigitalCyberSoft/scorchedearth-python) (static analysis only). Many of their values were spot-checked and all matched, but not every one. |

**Reference binary:** `SCORCH.EXE` v1.50, 415,456 bytes, SHA-256
`05e4a11643ec7055b1c811531a73c5b4f5c67c463612f3e4910e557301c3179f`, from the
shareware `SCORCH15.zip` at
<https://archive.org/details/msdos_SCORCH15_shareware>.

**Address convention:** `seg:off` uses the numbering of the projects above
(load segment + 0x1000).
- Code file offset = `0x6a00 + (seg − 0x1000) × 16 + off`.
- The data segment (`DS`, seg `5f38`) starts at file offset `0x55d80`, so
  `DS:xxxx` = file offset `0x55d80 + xxxx`.
- The game uses Borland's floating-point emulator: `INT 34h–3Bh` stand for FPU
  instructions `D8–DF`, `INT 3Ch` for an FPU instruction with a segment
  override, and `INT 3Dh` for `FWAIT`.
- Tools to read the code: `tools/re/dis.py` and `tools/re/refs.py` in this
  repository.

**Built with:** Borland C++ 4.0 and Turbo Assembler 3.0; about 122 C/C++ files
and 3 assembly files (📘 README). Large memory model, 16-bit `int`.

---

## 2. Game structure

- **Players:** 2–10, default 2 📘. Each is human or computer. In the original,
  every player defaults to human, and the computer type selector defaults to
  Tosser 🟡.
- **Rounds:** 1–1000, default 10 📘.
- **A round:**
  1. Shop (only players with cash; computers too if *Computers Buy*).
  2. A new landscape is generated or loaded.
  3. Tanks are spread more or less evenly across the width.
  4. Players take turns until at most one tank (or team) is left.
  5. Score screen, interest is paid, and the next round begins.
- **First shooter:** random. Play then goes **left to right** 📘 *p. 5*, subject
  to *Play Order* (§13).
- **Winning the game:** the ranking after the last round (§10).
- **Max inventory:** **99** of any item. Baby Missiles are always 99 and are never
  used up ✅ (the inventory slot is reset to `0x63` at `39fe:002c`) 📘 *p. 18*.

---

## 3. Tanks

| Property | Value | Source |
|---|---|---|
| Health ("power") | 0–100, starts at 100 | 📘 *p. 10* 🟡 |
| Max firing power | **10 × health** (1000 at full health); `setPower` clamps to 0…max | 📘 *p. 9* ✅ `3ac8:004e` |
| Starting power | **200** each round | ✅ `3a40:0066` (DOSBox ✅) |
| Angle | 0–180 internally (0 = right, 90 = up, 180 = left); **shown as 0–90**, with the direction read from the barrel | 📘 *p. 9* 🟡 |
| Support | a tank falls when **fewer than 3 pixels** of ground are under its footprint | 🟡 |
| Direct hit | an unshielded tank hit directly by a projectile **loses all its health** (then the normal explosion follows) | ✅ `4d1e:007c` |
| Fixed emplacement | tank icons without wheels or treads can't buy or use fuel | 📘 *p. 8* |
| Triple turret | one icon (registered version and computers only) fires 3 shots at once with Missiles or Baby Missiles | 📘 *p. 8* |

Default tank icon: index 3 of 7 🟡.

---

## 4. Physics

### 4.1 Integration

Forward Euler, once per simulation step 🟡 (`2a4a:0b1f`). Per step:

1. Clamp the speed (`speed² > 1,000,000` → rescale to **1000**) ✅ `DS:1d2c`, `DS:1d30`.
2. Move: `x += vx·dt`, `y += vy·dt`.
3. Drag: `v *= 1 − AirViscosity / 10000` 🟡.
4. Gravity, then wind.

**Launch:** `vx = power·cos(angle)`, `vy = power·sin(angle)`. Power is used
**directly as the speed** (no scaling factor), and power 1000 equals the speed
clamp 🟡 (`2a4a:02f2`; π/180 constant ✅ `DS:1d08`).

**Effective accelerations** (independent of the time step):

| Force | Acceleration | Source |
|---|---|---|
| Gravity | **2500 × Gravity** (default Gravity 0.2 → 500) | ✅ constant 50.0 `DS:1cf2`; 50² = 2500 |
| Wind | **1.25 × Wind** | ✅ constants 50.0 / 40.0 `DS:1cf2`, `DS:1cf6` |

**Time step.** The original has **no fixed frame rate**. At startup it measures
the CPU speed (`MIPS`) and sets 🟡 (`2a4a:01c4`):

```
L  = MIPS × FireDelay / (shotsInFlight × 100)
dt = 1 / (50 × L)
```

`dt` is recalculated whenever a projectile is added or removed. Trajectories are
the same on any machine (in the limit of a small step); only the animation
speed changes. Real machines were within about 0.4 px of the small-step limit,
so a remake can use a small fixed step (e.g. 1/1920 s) 🟡.

### 4.2 Wind

| Rule | Value | Source |
|---|---|---|
| Max Wind | 0–500, default **200** (0 = no wind) | 📘 ✅ `DS:515c` |
| **Round start** | `wind = random(Max/2) − Max/4`; then 20 % chance `wind ×= 2`, and if that happened, a further 40 % chance `wind ×= 2` | ✅ `32a3:000a` |
| **Changing Wind** (default off) | each turn `wind += random(11) − 5` (−5…+5), clamped to ±Max | ✅ `3249:0019`; flag `DS:50fa` |
| Sign | negative = blowing left | ✅ |
| Display | "Wind" in light blue, top right of the sky, with an arrow and number, or "No Wind" | 📘 *p. 9* |

`random(n)` means an integer in `0…n−1`.

### 4.3 Walls ("Effect of Walls", config `ELASTIC`)

| Type | Effect | Wall colour |
|---|---|---|
| None *(default)* | no effect; Borders Extend applies | grey |
| Concrete | blocks shots in **every** direction (ceiling too) | white |
| Padded | reflects with velocity factor **−0.5** | green |
| Rubber | reflects with factor **−1.0** | red |
| Spring | reflects with factor **−2.0** (a "kick") | cyan |
| Wrap-around | left and right edges join | yellow |
| Random | one of the above, picked each round | – |
| Erratic | one of the above, picked before each shot | – |

Behaviour and colours: 📘 *p. 45*. Order in the menu (confirmed in DOSBox):
None, Wrap-around, Padded, Rubber, Spring, Concrete, Random, Erratic. Bounce
factors: 🟡 (`DS:1d34/1d3c/1d44`).

**Borders Extend** (`EDGES_EXTEND`): 0–10000, default **75**. This is how many
pixels off-screen a shot is still tracked before it counts as a miss, so wind
can bring it back 📘 *p. 44*.

### 4.4 Tunneling

Config `TUNNELLING` (manual default **ON**; the binary's built-in default with
no config file is OFF 🟡) 📘 *p. 49*. With tunneling on, an ordinary projectile
entering dirt:

- for **each dirt pixel** entered, multiplies its velocity by **0.75** ✅
  `DS:1d54` and erases that pixel;
- explodes at that pixel once `speed² < 2000` ✅ `DS:1d58` (exactly 2000
  continues);
- has no gravity, wind or drag while inside dirt;
- can come out the other side of a thin pile at reduced speed.

These details 🟡 are also confirmed with DOSBox runtime samples by gosuwachu.
Rollers, tracers and Dirt Charge never tunnel. Contact Triggers (§8.5) and
Tunneling OFF make shots explode on first contact.

---

## 5. Terrain

### 5.1 Dirt and settling

- The terrain is a pixel bitmap. Explosions carve circles; dirt weapons add
  circles.
- Settling moves every unsupported run of dirt down until it rests, including
  deeper layers, keeping its colours in order 🟡.
- **Suspend Dirt** (`SUSPEND_DIRT`, 0–100, default 0). Stored internally as
  `F = 100 − SuspendDirt` ✅ (writer `2426:0018`). After a shot ✅ (`264f:0008`):
  - `F = 0` → floating dirt **never** falls;
  - `F = 100` → it **always** falls;
  - otherwise it falls if `random(100) < F`.

  So **Suspend Dirt is the chance that floating dirt stays in the air**. The
  manual's description (*p. 44*) reads the other way round, but the code is
  as described here. The Earth Disrupter forces a collapse anyway.

### 5.2 Falling tanks

| Rule | Value | Source |
|---|---|---|
| Tanks Fall (`FALLING_TANKS`) | default ON; if off, tanks float | 📘 |
| Fall damage | **2 per pixel** fallen | ✅ `DS:5164` |
| Impact Damage (`DAMAGE_TANKS_ON_IMPACT`, config only) | ON = damage on landing, OFF = 2 per pixel while falling; same total | 📘 *p. 50* ✅ `2a04:0019` |
| Landing on an enemy tank | the victim takes the faller's damage **+ 50**; the faller takes half **+ 10** | ✅ `2a0e:0078`, `2a0e:00b8` |
| Fall speed | Falling Delay (`FALLING_DELAY`, default 10) = **milliseconds** paused per step of the falling animation (Borland `delay()`); 0 = no pause. With a parachute: a fixed **20 ms per pixel** and no damage | ✅ `2a1b:001e`, `2a04:004f` |
| Shields | do **not** absorb fall damage | 🟡 |

**Parachutes:** see §8.4.

### 5.3 Landscape generation

| Setting | Range | Default | Effect in v1.5 |
|---|---|---|---|
| Bumpiness (`LAND1`) | 0–100 | 20 | **none** (see below) |
| Slope (`LAND2`) | 0–100 | 20 | **none** (see below) |
| Flatten Peaks (`FLATLAND`) | on/off | off (📘; binary built-in default on 🟡) | **none** (see below) |
| Random Land (`RANDOM_LAND`) | on/off | off (📘; binary built-in default on 🟡) | each round sets Flatten Peaks = `random(2)`, Bumpiness = `random(100)`, Slope = `random(100)` ✅ `32b5:0022` |
| Percent Scanned Mountains (`MTN_PERCENT`) | 0–100 | 20 | chance of using a `.MTN` picture instead (§17.1) |

Menu and ranges 📘 *p. 46*.

**Bumpiness, Slope and Flatten Peaks have no effect in v1.5** ✅. In the whole
EXE these three variables (`DS:516e`, `5170`, `5172`) are only:

- read and written by the config file code;
- edited in the Landscape menu;
- randomised by Random Land;
- saved and restored around the title-screen picture.

No terrain code reads them. A DOSBox test of 0/0, 100/0, 0/100 and 100/100,
and Flatten Peaks on, produced the same kind of landscape every time.

**Live default generator — corrected 2026-11-17** ✅. The earlier notes
mistook the midpoint-displacement routine for the live generator. That routine
exists at `2d4f:0645` / wrapper `2d90:002e`, but it is **not selected by the
stock DefaultLand object**. Its equal endpoints and 16/32/64-point subdivision
are not rules for ordinary v1.5 landscapes.

The actual path, verified in the matching shareware EXE:

- Initialisation `323a:0c88` constructs the global DefaultLand at `DS:d4be`.
  Constructor `323a:0b6b` installs vtable `DS:508c`; its first far pointer is
  **`223a:0a68`** (relocatable EXE segment), i.e. **`323a:0a68`** using our
  disassembler's Ghidra numbering. It hard-codes the parameters **20 / 20 / off**
  at object offsets `+0a / +0c / +0e`, rather than reading LAND1/LAND2/FLATLAND.
- Round setup `323a:0711–0758` selects this object when not using a scanned
  mountain and passes the current graphics bounds to wrapper `323a:083f`.
  The indirect call at `323a:0975` selects `323a:0a68`.
- Graphics setup `323a:02bd` gives a three-pixel frame:
  `xmin = 3`, `xmax = width − 4`, `ymax = height − 4`; `ymin` depends on the
  toolbar. The ordinary 640×480 and 360×480 captures have `ymin = 24`.

The generator is an **integer random walk in grade**, one step per column:

```
y = random(ymax − ymin − 60) + ymin + 40
velocity = 0
for x in the object's current direction, including both end columns:
    plot(x, y)
    if random(100) < 20:
        change = random(3) − 1
        if random(100) < 20: change *= 2
        velocity += change
        # Optional class parameter, hard-coded OFF in the shipped constructor:
        if flatten and abs(velocity) > 3: velocity = 0
    y += velocity
    if y < ymin + 40:
        y = ymin + 40
        if velocity < 0: velocity = 1
    if y > ymax − 1:
        y = ymax − 1
        if velocity > 0: velocity = 0
```

The first height is uniform from `ymin + 40` through `ymax − 21`; subsequent
heights stay within `[ymin + 40, ymax − 1]`. The far edge is the walk's result,
**not an equal endpoint**. The final column still consumes its random draws.
Wrapper `323a:0855–0873` toggles the persistent object's direction between
left-to-right and right-to-left on each generation. The title picture uses a
separate stack-local object (`46bc:0008`), so it does not toggle the global one.

**Cavern construction** ✅ `323a:08d7–095f`: split the vertical interval at
`middle = trunc((ymin + ymax) / 2)`. Run the same walk first in `[middle,ymax]`
for the floor, then in `[ymin,middle]` for the roof, sharing the RNG stream and
horizontal direction. Object flag `+2` reverses the fill side in `323a:0980`:
soil is at/below the floor and strictly above the roof's surface row.
Consequently the unmodified generated surfaces have at least **41 pixels** of
vertical separation. The roof is not a scaled or mirrored copy of the floor.

**Cavern shading** ✅ `42c2:084a–090d`: 30 **horizontal** bands about the global
field midpoint, independent of local floor/roof shapes. The row selects
`i = trunc(29 × abs(y − middle) / half_height)`. VGA palette components are
`trunc(earth_component × (i + 8) / 45)` (`DS:6270 = 45.0`). Thus the middle
is dim earth colour, not absolute black. DOSBox expands a VGA component with
`(component << 2) | (component >> 4)`.

**Earth colours** ✅ `DS:5036`: the six VGA RGB triples are
`(38,25,17), (54,36,28), (53,53,47), (20,62,20), (9,35,9), (36,54,28)`.
Ordinary generated dirt is one solid colour per round. Plain sky is VGA
`(9,9,31)` (`42c2:0336`). Imported mountains retain their own palette.

The native terrain follow-up, binary-instruction oracle and DOSBox comparison
are documented in `TERRAIN_FOLLOWUP.md`. The tribute adapts graphics bounds to
its own HUD/full viewport and uses its own RNG. Working landscape sliders
expose the original class parameters as an enhancement; Classic uses the
shipped constructor's fixed 20/20/off values. Imported PNG/Cavern clearance is
also a local extension rather than a recovered scanned-mountain placement rule.

---

## 6. Damage

### 6.1 Explosion damage ✅

Disassembled at `4d1e:0330–0362`. For every living tank:

```
d = distance from explosion centre to the tank's base point
if d < R:
    dmg = trunc((R − d) × 100.0 / R)       # factor 100.0 at DS:683c
    dmg = dmg × (weaponIndex + 1)          # DS:e344 = current weapon index
    if dmg > 100: dmg = 110
    apply dmg (through shields, §8.3)
```

- `trunc`: Borland `__ftol` in chop mode (`1000:14df`). It truncates and does
  **not** round.
- `weaponIndex` is the item's position in the weapon list (§7.1), from 0. So
  Baby Missile ×1, Missile ×2, Baby Nuke ×3, Nuke ×4, and so on. Anything
  over 100 becomes 110, which always kills (health ≤ 100).
- The same factor 100.0 appears in the nuke and dirt engines ✅ (`DS:5692`,
  `DS:52aa`).
- `d` is measured to the tank's stored coordinate, its **base** row 🟡.

### 6.2 Other damage sources

| Source | Damage | Source |
|---|---|---|
| Direct hit, no shield | all remaining health | ✅ `4d1e:007c` |
| Direct hit on a shield | **10** to the shield, no explosion, 1000 Hz beep | ✅ `4d1e:0047` |
| Fall | 2 per pixel | ✅ |
| Lightning (Hostile Environment on) | 10, direct contact only | 🟡 `480f:000c` |
| Death "spark rays" animation | 20 within a radius of 45–104 | 🟡 |
| Laser | 10-point pulses | 🟡 |

### 6.3 Death

When a tank's health reaches 0 it plays one of **11 death animations** (§15),
some of which cause further damage. Kills are credited to the shooter (§10).

---

## 7. Weapons

### 7.1 Weapon table

Cost, bundle, radius (the manual's "estimate") and arms level: 📘 *p. 23*. The
table base value is from `DS:1200`, 52-byte entries ✅. It is the number each
handler uses (radius, warhead count, tunnel budget…).

| # | Weapon | Cost / bundle | Radius (📘) | Arms | Base value ✅ | Behaviour |
|---|---|---|---|---|---|---|
| 0 | Baby Missile | $400 / 10 | 10 | 0 | 10 | plain blast; **unlimited** |
| 1 | Missile | $1,875 / 5 | 20 | 0 | 20 | plain blast |
| 2 | Baby Nuke | $10,000 / 3 | 40 | 0 | 40 | plain blast (nuke texture) |
| 3 | Nuke | $12,000 / 1 | 75 | 1 | 75 | plain blast (nuke texture) |
| 4 | Leap Frog | $10,000 / 2 | 20,25,30 | 3 | 3 | 3 successive blasts, radii **30 → 25 → 20**; each hop relaunched at speed ÷ 1.5 🟡 |
| 5 | Funky Bomb | $7,000 / 2 | 80 | 4 | 80 | see §7.2 |
| 6 | MIRV | $10,000 / 3 | 20 | 2 | 0 | splits at the top of its arc: parent + **4** warheads (Missile blasts); a hit **before** the apex is a dud 📘🟡 |
| 7 | Death's Head | $20,000 / 1 | 35 | 4 | 1 | like MIRV: parent + **8** large warheads 🟡 |
| 8 | Napalm | $10,000 / 10 | – | 2 | 15 | burning liquid, up to 15 emitters (§7.2) |
| 9 | Hot Napalm | $20,000 / 2 | – | 4 | 20 | up to 20 emitters |
| 10 | Tracer | $10 / 20 | 0 | 0 | 0 | no damage |
| 11 | Smoke Tracer | $500 / 10 | 0 | 1 | 1 | trail stays until cleared |
| 12 | Baby Roller | $5,000 / 10 | 10 | 2 | 10 | rolls downhill to a valley or tank; rolls off shields 📘 |
| 13 | Roller | $6,000 / 5 | 20 | 2 | 20 | ″ |
| 14 | Heavy Roller | $6,750 / 2 | 45 | 3 | 45 | ″ |
| 15 | Riot Charge | $2,000 / 10 | 36 | 2 | 0 | clears a wedge of dirt from your turret, half-angle **45°**, size 36 🟡 |
| 16 | Riot Blast | $5,000 / 5 | 60 | 3 | 1 | wedge half-angle **60°**, size 60 🟡 |
| 17 | Riot Bomb | $5,000 / 5 | 30 | 3 | 30 | projectile; clears a circle; no tank damage |
| 18 | Heavy Riot Bomb | $4,750 / 2 | 45 | 3 | 45 | ″ |
| 19 | Baby Digger | $3,000 / 10 | – | 0 | −10 | tunnels; fizzles on a tank |
| 20 | Digger | $2,500 / 5 | – | 0 | −20 | ″ |
| 21 | Heavy Digger | $6,750 / 2 | – | 1 | −35 | ″ |
| 22 | Baby Sandhog | $10,000 / 10 | – | 0 | 10 | tunnels with explosive heads |
| 23 | Sandhog | $16,750 / 5 | – | 0 | 20 | ″ |
| 24 | Heavy Sandhog | $25,000 / 2 | – | 1 | 35 | ″ |
| 25 | Dirt Clod | $5,000 / 10 | 20 | 0 | 20 | sphere of dirt |
| 26 | Dirt Ball | $5,000 / 5 | 35 | 0 | 35 | ″ |
| 27 | Ton of Dirt | $6,750 / 2 | 70 | 1 | 70 | ″ |
| 28 | Liquid Dirt | $5,000 / 10 | – | 2 | −20 | flowing dirt that fills holes |
| 29 | Dirt Charge | $5,000 / 5 | – | 1 | 0 | upward dirt spray (§7.2) |
| 30 | Earth Disrupter | $5,000 / 10 | – | 0 | 0 | forces floating dirt to fall |
| 31 | Plasma Blast | $9,000 / 5 | 10–75 | 3 | 0 | energy weapon (§7.3) |
| 32 | Laser | $5,000 / 5 | – | 2 | 0 | energy weapon (§7.3) |

Notes:
- **Explosion damage multiplier** = index + 1 (§6.1). Weapons late in the list
  that use the standard blast routine therefore kill almost anywhere inside
  their radius.
- **Scale** option (`EXPLOSION_SCALE`): Normal / Medium / Large multiply blast
  sizes by **1 / 2 / 3** (in 320-pixel-wide mode **0.5 / 0.75 / 1**). Only some
  weapon families are affected 🟡.
- Weapons that don't work with guidance (guidance is not used up): MIRV,
  Death's Head, Riot Charge, Riot Blast, Plasma Blast 📘 *p. 29*.

### 7.2 Weapon behaviour details 🟡

- **Funky Bomb:**
  1. A radius-20 coloured burst.
  2. **5–10 curved trails** to random points within **±80 px** horizontally
     (`x + random(160) − 80`, clipped to the field).
  3. Landing bursts of radius 15–24.
  4. A final scaled radius-40 blast.

  Trails stop at terrain, tanks or the field edge. A shield stops the bomb
  itself with the usual 10-point chip and no chain reaction.
- **MIRV / Death's Head:** the parent keeps flying as the central warhead;
  children spread from it at the apex. One Contact Trigger covers all warheads.
- **Rollers:** on hitting ground, move along the surface toward lower ground
  (falling one pixel at a time), and explode at an obstruction, valley or tank.
  They respect the wall type.
- **Diggers:** branching tunnels with a budget of twice the absolute table
  value (20/40/70); no explosive heads.
- **Sandhogs:**
  - Up to **20** live tunnel heads, with 10/20/35 total heads created.
  - A new branch every 8 eligible ticks; heads keep their direction for up to
    20 steps.
  - Heads follow neighbouring dirt, not tanks.
  - A head with no dirt left fires a charge of radius **10 × scale**, with
    rounded radial damage × (index + 1).
  - Direct tank contact chips 10.
- **Dirt spheres:** grow in place with unscaled radii 20/35/70; they bury tanks
  without lifting them.
- **Dirt Charge:** an ordinary shot, then a random upward spray of dirt reaching
  ⅓ of the field height. Extra Dirt (`EXTRA_DIRT`) makes much more.
- **Napalm / Hot Napalm / Liquid Dirt** (shared fluid routine):
  - The liquid flows down and sideways, with 100 active queue slots.
  - Emitter limit: 15 (Napalm), 20 (Hot), 20 (Liquid Dirt).
  - Napalm flames grow per emitter, then burn for 50 cycles. Heat damage is
    applied per emitter, so deep pools burn hotter (📘).
  - Liquid Dirt deposits terrain.
  - With tunneling on, napalm that burrows into dirt can fizzle; a Contact
    Trigger prevents it 📘 *p. 24*.
- **Earth Disrupter:** 100 flash/speaker cycles, then a forced collapse, even at
  100 % Suspend Dirt. It does nothing on settled ground.
- **Tracer / Smoke Tracer:** no explosion. The smoke trail stays independently
  of the Trace Paths option.

### 7.3 Energy weapons

Both use **batteries** 📘 *p. 27*. After firing you choose how many to spend
(keys 0–9; prompt "Batteries to discharge:" ✅ string).

- **Plasma Blast** 🟡: centred on your own tank, which is excluded; the turret
  angle doesn't matter. Charge 0–10 batteries; the radius ranges from the
  Missile radius to the Nuke radius. It doesn't damage dirt and leaves no crater.
- **Laser** 🟡: a straight beam along the barrel.
  - Energy comes from your power; it costs 40 per pixel, plus 40 more through
    dirt.
  - It makes small dirt cuts and does 10-point damage pulses.
  - It cuts through shields, except Super Mag, which turns laser energy into
    shield charge (capped at 200).

---

## 8. Accessories

### 8.1 Accessory table 📘 *p. 28*

| Item | Cost / bundle | Arms |
|---|---|---|
| Heat Guidance | $10,000 / 6 | 2 |
| Ballistic Guidance | $10,000 / 2 | 2 |
| Horz Guidance | $15,000 / 5 | 1 |
| Vert Guidance | $20,000 / 5 | 1 |
| Lazy Boy | $20,000 / 2 | 3 |
| Parachute | $10,000 / 8 | 2 |
| Battery | $5,000 / 10 | 2 |
| Mag Deflector | $10,000 / 2 | 2 |
| Shield | $20,000 / 3 | 3 |
| Force Shield | $25,000 / 3 | 3 |
| Heavy Shield | $30,000 / 2 | 4 |
| Super Mag | $40,000 / 2 | 4 |
| Auto Defense | $1,500 / 1 (price varies, §9.4) | 3 |
| Fuel Tank | $10,000 / 10 | 3 |
| Contact Trigger | $1,000 / 25 | 3 |

### 8.2 Guidance

You select guidance before firing (`g`, Tank Control Panel or status bar). It is
**used up only when it applies** and resets to None after each shot 📘 *p. 29*.

- **Target choice:** Ballistic, Horizontal, Vertical and Lazy Boy ask
  "Choose Target" after firing.
  - Keys `1`–`9`, `0` pick a living tank counted from the left (0 = 10th);
    the target point is the tank's base.
  - Mouse: left-click = any point; right-click = the nearest tank within 100 px.
  - There's no guidance in Simultaneous mode 🟡.

| Guidance | Behaviour |
|---|---|
| **Heat** | Locks onto the first living non-owner tank (**teammates included**) within **40 px** ✅ `DS:5186`, then steers to it. No target choice. |
| **Ballistic** | Uses your angle and solves the **power** to hit the target. It corrects for wind (as a tilt of gravity) but **not** for air viscosity 📘. If the shot is impossible, the power is clamped to health × 10. |
| **Horizontal** | When level with the target's row, flies flat toward it, then detonates on reaching its column. |
| **Vertical** | When above (or below) the target's column, heads straight for it, then detonates. |
| **Lazy Boy** | Flies directly to the chosen point, at up to 500 steps per second. It climbs around dirt and other tanks and digs through soil, and explodes on any tank contact or at the point (even in mid-air). It ignores shields' interception and magnetic push. |

Steering acceleration = target offset × `k·dt / √distance`, with
**k = 10,000** (Heat, Vertical) and **15,000** (Horizontal) ✅ `DS:3224`,
`DS:3228`. It is skipped below distance² 0.001 ✅ `DS:321c`. Details 🟡
(`2e50:0001–0c30`, `2a4a:17a7–19d8`).

Non-ballistic guidance forces "contact" (no tunneling) at launch, without using
a Contact Trigger 🟡.

### 8.3 Shields ✅ `DS:617c` (five 16-byte records)

| Shield | Strength | Outline radius | Colour (VGA 0–63) | Special |
|---|---|---|---|---|
| Mag Deflector | **55** | 16 | 63, 63, 23 | magnetic push |
| Shield | **100** | 15 | 63, 63, 63 | – |
| Force Shield | **100** | 15 | 63, 23, 63 | reflects shots |
| Heavy Shield | **150** | 16 | 63, 63, 63 | – |
| Super Mag | **200** | 16 | 63, 53, 33 | push + reflect + laser-proof |

Rules:

- **Direct hit** on a shield: **10 points** to the shield, no explosion ✅.
- **Explosion damage** goes to the shield first. If it's more than the
  remaining strength, the shield collapses and the rest goes to the tank 🟡.
- **No random "shield failures"**: collapse only happens when strength runs
  out 🟡.
- **Your own shield never stops your own shot** 📘 *p. 31*, but it does absorb
  your own secondary damage (e.g. napalm heat).
- **Magnetic push** (Mag Deflector, Super Mag) 🟡:
  - Applies to enemy shells with nonzero horizontal speed, within ±15 px
    horizontally and up to (screen height − 1)/4 above the tank.
  - Each step adds `50 / FireDelay` to upward speed (`2a4a:28b4`; 50.0 ✅
    `DS:1cf2`).
  - There's no speed threshold: fast shots simply cross the zone too quickly.
- **Force reflection** 🟡: mirror bounce off the shield outline, keeping
  **70 %** of the speed ✅ `DS:1d60` (factor 2.0 ✅ `DS:1d5c`). It costs the
  shield `round(incoming speed / 100)`.
- **Colour fades** with strength:
  `channel = base × HP / maxHP` (integer division) 🟡.
- **Changing shields:** energising a shield replaces the current one, and the
  old one is lost 📘 *p. 15*.

### 8.4 Parachutes, batteries, fuel

- **Parachute** 📘 *p. 31*, 🟡 rule:
  - State is **deployed** (default) or passive; the safety threshold is
    1–100, default **5**.
  - Before a fall the game predicts the damage (2 × pixels).
  - A parachute is used if deployed and `threshold < predicted` (threshold 0 =
    always). It cancels all fall damage, unless the tank lands on an enemy.
  - A parachute descent is drawn at a fixed 20 ms per pixel ✅.
- **Battery:** +10 health, capped at 100 📘 *p. 14*. Also the charge for
  energy weapons. In Simultaneous mode batteries fire automatically when
  health drops below 91 📘 *p. 51*.
- **Fuel Tank** = 10 units 📘. Moving one pixel ✅ (`3667:0443`):
  - Always costs **1** fuel.
  - **Uphill:** if the tank's leading edge meets dirt, it climbs 1–3 px and
    pays **1 extra fuel per pixel climbed**. A climb of **4 px or more is
    blocked**.
  - **Flat or downhill:** stepping down up to 3 px costs nothing extra. If the
    ground drops **4 px or more**, the tank moves and then **slips and falls**
    (normal fall damage; control returns to aiming) 📘 *p. 16*.

### 8.5 Contact Trigger, Auto Defense

- **Contact Trigger:** the shot explodes on first contact (tunneling off for
  that shot); one trigger covers every warhead of the shot 📘 *p. 32*. A
  trigger only changes **where** a Digger or Sandhog starts; it doesn't replace
  its effect 🟡.
- **Auto Defense:** bought once and kept for the whole game. At each round start
  you get the defensive controls (shields, parachutes, batteries) before combat
  📘 *p. 32*. Price: §9.4.

---

## 9. Economy

### 9.1 Cash

- **Cash at Start** (`INITIAL_CASH`): $0–$1,000,000, default $0. If above 0,
  everyone shops before round 1 📘.
- Points **are** money: every award in §10 is added to cash (a negative award
  can't take cash below 0) 🟡.

### 9.2 Interest ✅ (`20dc`)

```
interest = cash × trunc(InterestRate × 10000) ÷ 10000     (exact integer maths)
cash += interest
interestEarned += interest     (shown in the shop's "Update" box)
```

- Interest Rate (`INTEREST_RATE`): 0–0.30, default **0.05** ✅ `DS:5190`.
- Paid on money in the bank at the start of a round 📘 *p. 42*.

### 9.3 Shop 📘 *pp. 18–21*

- Items you **can't afford are hidden**, and reappear when you can.
- Space buys one bundle.
- Max **99** per item. If a full bundle would go over 99, you get only the rest
  at about a **20 % markup** 📘.
- **Arms Level** (`ARMS`, 0–4, default 4): only items with arms ≤ level are
  offered.
- **Useless Items** (`USELESS_ITEMS`, manual default off): hides items with no
  effect, e.g. Smoke Tracer when Trace Paths is on.
- **Sell-back offer:** `round(price × qty × 0.80 / bundle)`; **0.65** with Free
  Market 🟡. The offer only changes between rounds.

### 9.4 Prices that change

- **Free Market** (`FREE_MARKET`, default off). Per item, each round 🟡
  (`34e0:035e`), with the step = **Interest Rate** ✅ (`3523:0035`):
  ```
  demand = 0.7·demand + 0.3·(buysThisRound / players)
  ratio  = 0.7·ratio  + 0.3·(price / basePrice)² / 10
  price  = trunc(price × (1 + InterestRate × (demand − ratio)))
  ```
  Both start at 0.1. The market is saved between games in `SCORCH.MKT` 📘.
- **Auto Defense** (a "repeated-use" item) is priced as an annuity over the
  rounds left 🟡:
  `base × ((1 + r) − (1 + r)^(−roundsLeft)) / r`, with `r` = Interest Rate.

---

## 10. Scoring

### 10.1 Awards ✅ (`4098:0263`, `4098:0308`, `4098:00f3`)

| Event | Basic | Standard / Greedy |
|---|---|---|
| Kill an enemy | **+4000** | **+500** |
| Kill a teammate | −2000 | −2000 |
| Kill yourself | −1500 | −1500 |
| Damage an enemy tank | 0 | **+30 × damage** |
| Damage an enemy shield | 0 | **+2 × damage** |
| Damage yourself or a teammate | 0 | **−15 × damage** |
| Survive the round (pool) | **players × 1000** | **5000** |

- The scoring mode flag is `DS:5148` (0 = Basic). Default: Standard 📘.
- The survival pool is shared by the survivors. In team modes the winning team
  splits it, and dead members get half a share 🟡.
- A kill also adds 5 to the killer's per-victim "grudge" counter
  (`tank+0x52`) ✅, which Cyborg uses when picking targets 🟡.
- **Retreat** (`r`): you escape by helicopter while your tank blows up; you get
  no points and the enemy gets none for you 📘 *p. 12*.

### 10.2 Mass Kill ✅

System Menu → Mass Kill ("Mass kill everyone?") ends the round at once. In the
manual's words (*p. 33*), it kills everyone and gives them "an equal portion of
whatever points remained to be won in the round". The code defines those points
precisely.

Confirming sets a flag (`DS:e1e2`, `48ed:0031`). At the next turn check
(`4250:00ab`) the game runs `33a1:0e84`:

1. Removes all projectiles in flight.
2. Pays the split below (`4098:0001`).
3. Drops every survivor's shield and destroys every tank.
4. Sets the alive count `DS:e340` to 0, so the normal survival pool
   (`4098:00f3`) pays **nothing**.

With `N` = tanks still alive:

```
Standard / Greedy:
    pool = 5000 + 500·N + Σ over survivors (30 × health + 2 × shield strength)
Basic:
    pool = 1000 × players + 4000·N
each survivor gets  trunc(pool / N)
```

So the pool is everything still "on the table": the survival pool, a kill bonus
for every survivor, and (outside Basic) the per-hit points that the survivors'
remaining health and shields were worth. Dead tanks get nothing.

### 10.3 Ranking

- **Basic / Standard:** by accumulated score money 🟡.
- **Greedy:** by **net worth** = cash + every held item valued at **80 %** of its
  current unit price 🟡.

---

## 11. Computer players

Types 📘 *p. 7*, logic 🟡 (class id = menu index + 1; human = 0).

| Type | How it plays |
|---|---|
| **Moron** | power = `(random(health) + 1) × 10`; random angle 0–180, re-rolled until it points toward a side with enemies; random weapon. |
| **Shooter** | targets the nearest enemy; the starting angle is the line of sight; power from a closed-form ballistic solver; then searches the angle ±1°. Deadly only with a clear line of fire 📘. |
| **Poolshark** | Shooter, plus wall-bounce fine-tuning when walls are Rubber or Spring. |
| **Tosser** | 1st shot: a ranging lob into open space (angle `random(70) + 10`, mirrored toward the crowd). Then once per turn, using where the last shot landed: overshot → power −10; short and high → power +10 and angle +2 toward vertical (not in the 85–95° band); short and low → power +10. |
| **Chooser** | uses Shooter if it has a clear shot; else Poolshark if walls are Rubber/Spring; else Spoiler. Never Moron or Tosser. |
| **Spoiler** | random target; starts at 65° / 115°, then the angle search. Near-perfect with wind; no viscosity correction 📘. |
| **Cyborg** | Spoiler's aim with a start angle of `85 − w` (or `95 + w`), where `w = |3·wind/10| + viscosity/2`, max 70. Picks weakened, winning or grudge-holding targets 📘. |
| **Unknown** | rolls one of the 7 types once, kept secret 📘. |

- **Power solver** (`2e50:05e9`): closed-form inversion of the trajectory, with
  wind folded in as a tilt of gravity, and **no drag**. So viscosity defeats
  the "smart" AIs 📘.
- **Buying** (if Computers Buy): Moron buys weighted-random. All others follow a
  fixed list (best shield, parachute, missiles up to 5, …).
- Computers choose their own names, never matching a human's 📘.

---

## 12. Environment

| Setting | Values (default) | Notes |
|---|---|---|
| Sky (`SKY`) | Plain, Stormy, Stars, Shaded, Sunset, Cavern, Black, **Random** | Random never picks Black 📘. Cavern has a ceiling (new in 1.5; §5.3). |
| Hostile Environment | **ON** / OFF | if off, hostile skies still act but do no damage 📘 |

- **Stormy sky lightning** 🟡:
  - Each weather tick has a 1-in-4 chance of a strike; otherwise it flickers
    with thunder (2–5 flashes).
  - A bolt starts at a random sky point and walks down. Each segment has a
    20 % branch chance, up to depth 13.
  - It stops at terrain or a tank, doing **10** damage on direct contact
    (only with Hostile Environment on). The screen flashes white.
- **No meteors in v1.5** ✅: the EXE has no meteor text or setting. The author's
  FAQ says meteor showers were added in v1.1, so they existed in older versions.

---

## 13. Play modes, order, teams

- **Mode** (`PLAY_MODE`) 📘 *pp. 47, 51–52*:
  - **Sequential** (default): one shot per turn.
  - **Synchronous**: everyone aims, and all shots fly together.
  - **Simultaneous**: real time on one keyboard.
    - Each human picks 6 keys: turn clockwise, turn counter-clockwise, power
      up, power down, fire, change weapon.
    - You can't fire again until your shot explodes, but you can keep aiming.
    - `i`, `t`, `r` and the mouse are disabled, so shields work only with Auto
      Defense, parachutes are always deployed, and batteries are automatic.
    - Tanks flash bright for power up, dark for power down, red for a weapon
      change.
- **Play Order** (`PLAY_ORDER`): **Random** (default; a random order every
  turn), Losers-first, Winners-first, Round-robin (random, but the first shot
  of each round moves on one player) 📘.
- **Teams** (`TEAM_MODE`): **None**, Standard, Corporate (one shared bank
  account per team), Vicious (standard until one team is left, then everyone
  for themselves) 📘 *p. 52*.
- **Fast Computers**: speeds the game up while only computers are fighting 📘.

---

## 14. Controls

Main screen 📘 *pp. 11–13*:

| Key | Action |
|---|---|
| ↑ / ↓ | power **±1** per step (hold to repeat) ✅ |
| PgUp / PgDn | power **±15** per step ✅ |
| ← / → | turn the turret counter-clockwise / clockwise |
| **Shift** + arrows | same step, **half the repeat rate** ✅ |
| **Alt** + arrows | **one step per key press** (no repeat) ✅ |
| Tab / Shift-Tab | next / previous weapon |
| Space / Enter | fire |
| `t` | Tank Control Panel |
| `i` | Inventory Panel |
| `k` | "No kibitzing, please." |
| `r` | retreat (asks first) |
| `u` | status (walls, rounds left) |
| `1`–`9`, `0` | info about tank N from the left |
| F1 / Alt-S | System Menu: Clear Screen, Mass Kill, Reassign Players/Teams, Sound, Save, Restore, New Game, Quit |

- **Key repeat** ✅ (key loop `38b6:0049`; key table `38d6:0038`).
  - The game installs its own keyboard handler (`31f9:0000`). It records which
    keys are held and ignores the keyboard's built-in auto-repeat.
  - It then polls the held key, rate-limited by its own timer counter
    (`DS:ee78`). The timer is reprogrammed to **≈ 3649 Hz** (65536/200 = 327 →
    1,193,182 / 327; `4b59:0063`, tick handler `4b59:0000`).
  - Repeat rate: at most one step per **15 ticks ≈ 243 steps/s**, or 30 ticks
    (≈ 122/s) with Shift. With Alt, it waits for the key to be released.
  - So holding ↑ takes power from 0 to 1000 in about 4 s, PgUp in about a
    quarter of a second, and ← / → turn about 243°/s (upper limits; slow
    drawing can lower them).
  - Left/Right turn the angle by ±1° per step (`2a16:12e0`); Up/Down/PgUp/PgDn
    call `changePower(±1 / ±15)` (`3ac0:001e`).
  - **Simultaneous mode** uses its own 6-key map: ±1° angle and ±2 power per
    step 🟡 (`4270:00ce–0106`).
- **Status bar keys:** `b` battery, `p` parachute toggle, `s` select shield,
  `e` engage shield, `g` guidance, `f` fuel/move, `-` trigger.
- **Tank Control Panel extras:** `v` parachute threshold, `t` triggers.
- **Mouse:** left/right click on Power or Angle to change it; click the weapon
  name to cycle; **both buttons together** fire; click the player name for the
  Tank Control Panel (right-click opens the inventory).
- **Menus:** each item has a hotkey letter; the hotkey goes to the next value
  and **Alt + hotkey** to the previous one 📘 *p. 37*.

---

## 15. Presentation

- **HUD** 📘 *p. 9*: one top bar, `Power: 345  Angle: 35  <name>  <icon> 3 Dirt Clod`.
  The optional Status Bar adds `Max: 1000  n[batt] n[para] n[shield] % n[guid] n[trig] n[fuel]`.
- **Palette regions** 🟡: sky < 0x50; dirt 0x50 (plain) or 0x58–0x68 (shaded);
  objects ≥ 0x69; explosion band 200–239 (ring index `0xDD − r·20/R`); laser
  0xE6; tracer 0xE0. Palette cycling is limited by VGA retrace (~70 Hz).
- **Nuke explosions** (DOSBox ✅, Nuke and Baby Nuke):
  - A filled disc with a mottled **red-and-black "plasma" texture** of
    irregular cells, unlike the concentric fire rings of ordinary blasts.
  - It grows to full radius in about 1 s, shimmers for about 2 s, then
    vanishes at once and leaves the crater (timings at DOSBox 20,000 cycles).
- **Death animations** 🟡 (picked at random, `271b:0005`): thud; 1, 2 or 3
  escalating blasts; Funky Bomb; reflected spark rays (20 damage within
  45–104 px); Dirt Charge; 6 flame plumes; sinking dissolve (not in Cavern);
  grey bubbles; hull pixels flying apart.
- **Sound** (PC speaker only) 📘 *p. 39*:
  - Sound on/off.
  - Flight Sounds Off / **POS** (pitch follows height) / VEL (pitch follows
    speed).
  - Firing chirp 🟡: 1000, 2500, 4000, 5500, 7000, 8500, 10000 Hz.
  - Riot weapons: a rising/falling siren. Shield hit: a 1000 Hz beep ✅.
- **Talking tanks** 📘 *p. 48*: Off (default) / Computers / All, with Talk
  Probability 0–100 (default 100). Speech comes from `TALK1.CFG` (attack) and
  `TALK2.CFG` (death); see §17.2.
- **Bomb icon:** Invisible / Small / **Big**. **Trace Paths:** shots leave
  trails in the owner's colour 📘.

---

## 16. Settings reference (`scorch.cfg`)

The EXE writes exactly these 48 keys ✅ (format strings in the data segment).
The file is plain text, one `KEY=value` per line. Defaults are 📘 unless noted.

| Key | Menu | Range / values | Default |
|---|---|---|---|
| `MAXPLAYERS` | Players | 2–10 | 2 |
| `MAXROUNDS` | Rounds | 1–1000 | 10 |
| `SOUND` | Sound | on/off | on |
| `FLY_SOUND` | Flight Sounds | off/pos/vel | off |
| `GRAPHICS_MODE` | Hardware | 320x200 … 1024x768 | 360x480 |
| `BIOS_KEYBOARD` | Hardware | on/off | off (📘; written as On under DOSBox ✅) |
| `LOWMEM` | Small Memory | on/off | off |
| `POINTER` | Pointer | none/mouse/joystick | mouse |
| `FIRE_DELAY` | Firing Delay | 0–500 | 100 ✅ |
| `FALLING_DELAY` | Falling Delay | 0–1000 ms | 10 |
| `INTEREST_RATE` | Interest Rate | 0–0.30 | 0.05 ✅ |
| `INITIAL_CASH` | Cash at Start | 0–1,000,000 | 0 |
| `COMPUTERS_BUY` | Computers Buy | on/off | on |
| `FREE_MARKET` | Free Market | on/off | off |
| `SCORING` | Scoring Mode | basic/standard/greedy | standard |
| `AIR_VISCOSITY` | Air Viscosity | 0–20 | 0 |
| `GRAVITY` | Gravity | 0.05–10 | 0.2 ✅ |
| `SUSPEND_DIRT` | Suspend Dirt | 0–100 | 0 (§5.1) |
| `FALLING_TANKS` | Tanks Fall | on/off | on |
| `EDGES_EXTEND` | Borders Extend | 0–10000 | 75 |
| `ELASTIC` | Effect of Walls | §4.3 | none |
| `SKY` | Sky | §12 | random |
| `MAX_WIND` | Max. Wind | 0–500 | 200 ✅ |
| `CHANGING_WIND` | Changing Wind | on/off | off |
| `HOSTILE_ENVIRONMENT` | Hostile Environment | on/off | on |
| `LAND1` | Bumpiness | 0–100 | 20 |
| `LAND2` | Slope | 0–100 | 20 |
| `FLATLAND` | Flatten Peaks | on/off | off (binary built-in: on 🟡) |
| `RANDOM_LAND` | Random Land | on/off | off (binary built-in: on 🟡) |
| `MTN_PERCENT` | Percent Scanned Mountains | 0–100 | 20 |
| `FAST_COMPUTERS` | Fast Computers | on/off | off |
| `TALKING_TANKS` | Talking Tanks | off/computers/all | off |
| `TALK_PROBABILITY` | Talk Probability | 0–100 | 100 |
| `TALK_DELAY` | (config only) | – | 18 ✅ |
| `ATTACK_COMMENTS` | Attack file | filename | talk1.cfg |
| `DIE_COMMENTS` | Die file | filename | talk2.cfg |
| `PLAY_MODE` | Mode | sequential/synchronous/simultaneous | sequential |
| `STATUS_BAR` | Status Bar | on/off | off |
| `PLAY_ORDER` | Play Order | random/losers-first/winners-first/round-robin | random |
| `TEAM_MODE` | Teams | none/standard/corporate/vicious | none |
| `ARMS` | Arms Level | 0–4 | 4 |
| `BOMB_ICON` | Bomb icon | invisible/small/big | big |
| `TUNNELLING` | Tunneling | on/off | on (binary built-in: off 🟡) |
| `EXPLOSION_SCALE` | Scale | normal/medium/large | normal |
| `TRACE` | Trace Paths | on/off | off |
| `EXTRA_DIRT` | Extra Dirt | on/off | off |
| `USELESS_ITEMS` | Useless Items | on/off | off (binary built-in: on 🟡) |
| `DAMAGE_TANKS_ON_IMPACT` | (archaic) Impact Damage | on/off | on |

The file also contains `MOUSE_RATE=0.50`, which is read but not in the list
of write keys above ✅. A file saved by the original in DOSBox starts with a
`;` comment header and uses values like `On`/`Off`, `Standard`, `Random`,
`360x480`, `0.200000`.

"Binary built-in" is what the EXE uses when the key is missing from
`scorch.cfg`. The game writes a full config on first run, so in practice the
manual defaults apply. Delete `scorch.cfg` (or run `FIX.BAT`) to reset 📘.

---

## 17. File formats

### 17.1 `.MTN` scanned mountains ✅

Format from 🟡 DigitalCyberSoft; ✅ all 10 shareware files decode exactly with
it (zero bytes left over).

```
Header, 24 bytes:
  0   4  magic  4D 54 BE EF  ("MT\xBE\xEF")
  4   2  version, BIG-endian = 1
  6   2  width            (u16 little-endian)
  8   2  minimum column height
 10   2  height
 12   2  ncolors (= 16)
 14   2  unused
 16   2  sky colour index (= 0)
 18   6  unused
Palette: ncolors × (R, G, B), 8-bit each (game uses value >> 2 for VGA DAC).
         Index 0 is the sky (white in all shipped files).
Body: `width` columns, left to right. Each column:
  u16 count            number of ground pixels (sky pixels are not stored)
  ceil(count/2) bytes  4-bit colour indices, low nibble first, BOTTOM-UP
                       (first pixel = bottom row, last = surface)
Surface of column x = row (height − count[x]).
```

The game picks files from a fixed whitelist of names. The shareware ships 10
(`ICE001–003`, `ROCK001–006`, `SNOW001`); the registered version had 25 more 📘.

### 17.2 Talk files (`TALK1.CFG`, `TALK2.CFG`) 📘 *p. 48*

Plain text, **one phrase per line**. A repeated line is said more often. A
**blank line means "say nothing"** that time.

### 17.3 Saved games 📘 *p. 35*

- You can save only from the **buying screen**; restore any time from the
  System Menu.
- A save holds players, money, inventory and round, and **not** the options
  (except player and round counts).
- The file starts with the magic text `HB$a8Oscorch` 🟡.

---

## 18. Verification notes

The native terrain call path, vertical bounds, endpoint rule and Cavern
construction are resolved; see §5.3 and `TERRAIN_FOLLOWUP.md`. Estimates still
marked 🟡 elsewhere retain their stated verification limits.

**Execution speed:** repeat and animation rates are upper limits. The original
also depended on the PC's drawing speed, so there is no single per-frame speed
to recover for every 1990s machine.
