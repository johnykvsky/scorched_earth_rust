# TODO – remaining features, in batches

Everything still missing compared to the original Scorched Earth v1.5, split
into small batches. Each batch should be finished in one sitting: it builds,
plays, and leaves the game working. **Depends on** lists the batches that must
be done first.

Size: **S** = an hour or two, **M** = an evening, **L** = a few evenings.

### Source of truth

> The confirmed rules and numbers are collected in **`MECHANICS.md`**. This file
> tracks the work and the open questions.

The details below come from the original manual, `SCORCH.DOC` (v1.5, 1995),
and from checking the shareware v1.5 in DOSBox. Manual page numbers are given
as *(p. N)*. To look things up yourself:

- Download: <https://archive.org/details/msdos_SCORCH15_shareware> (`SCORCH15.zip`)
- Run: `dosbox -c "mount c ~/games/scorch" -c "c:" -c "SCORCH.EXE"`
- The manual is `SCORCH.DOC`, a plain text file in the zip.

The manual does **not** answer everything. Anything it leaves open is flagged
**Qn** in the batches below and listed here with what is now known.

### Other sources and how far to trust them

Two open-source projects reverse-engineered the original v1.5 `SCORCH.EXE`
(415,456 bytes, SHA-256 `05e4a116…3179f`, the same file as the archive.org
shareware). They cite the original's code and data addresses for each value.
The values below are the ones they took **from the original binary**. Both
projects also add their own extras, such as LAN play and an "Outlast bonus",
which they label as not original; those are ignored here. They also say when
they had to guess:

- **gosuwachu/scorched-earth**: <https://github.com/gosuwachu/scorched-earth>.
  Browser port. Its fidelity reports (`oracle/*_FIDELITY.md`) are checked
  against the original running in DOSBox, and it corrects several errors in
  the project below. **Most trusted; wins when the two disagree.**
- **DigitalCyberSoft/scorchedearth-python**:
  <https://github.com/DigitalCyberSoft/scorchedearth-python>. Python port built
  from static disassembly only (the original was never run). Recovery notes are
  in `port/RECOVERED_*.md`. It admits that some earlier values were wrong and
  later fixed, so use it with care.
- **The author's official FAQ**: <https://www.whicken.com/scorch/faq.html>.
  Version history only, no numbers.

**Spot check (2026-10-13).** I read the claimed values directly from our copy of
`SCORCH.EXE` (data segment at file offset `0x55d80`): 15 physics and economy
constants, 9 tunneling, guidance and damage constants, all 5 shield entries, and
the 24 weapon-table values used below. I also decoded all 10 `.MTN` files with the
claimed format. **Everything matched.** (The kill bonus first looked like a miss: `DS:5148`
turned out to be the scoring-mode flag that selects between 500 and 4000; see Q6.) All 10 mountains decoded with zero leftover
bytes.

Status in the table:

- ✅ **verified**: read from the original EXE or files by me. "**Disassembled**"
  means I read the actual machine code of that routine (addresses given as
  `seg:off`, the projects' numbering). Tools: `tools/re/dis.py` (disassembler
  wrapper that decodes Borland's FPU-emulator interrupts) and `tools/re/refs.py`
  (finds code using a variable), e.g.
  `python3 tools/re/dis.py SCORCH.EXE 4d1e:0330 0x40`.
- 🟡 **from reverse engineering**: cited from the binary by the projects above,
  but not checked by me (or the sources disagree). Fine to build on; confirm in
  DOSBox if it matters.
- ⚠ **still unknown**.

### Open questions

| # | Question | Status | What is known | Batch |
|---|---|---|---|---|
| Q1 | **Weapon damage** | ✅ | **Disassembled** `4d1e:0330–0362`: `dmg = trunc((R − d) × 100 / R)`, **× (weapon index + 1)**, and if the result is over 100 it becomes **110**. So the multiplier is real: Baby Missile ×1, Missile ×2, Baby Nuke ×3, Nuke ×4; weapons with high indices (e.g. Rollers ×13–15) kill almost anywhere inside their radius. The conversion is Borland `__ftol` in **chop mode, so truncation, not rounding** (both projects say "round"). Only tanks with `d < R` are hit; `d` is measured to the tank's base. A **direct hit** on an unshielded tank removes all its health, then explodes normally (`4d1e:007c`). Max health 100. | 0, 9, 10 |
| Q2 | **Fall damage** | ✅ | **2 damage per pixel** fallen (✅ `DS:5164 = 2`). A tank falls when fewer than 3 pixels support it. Parachute: predicted damage = 2 × the drop; it opens if **deployed** and the threshold (default 5) is **below** the prediction (threshold 0 = always). Landing on an enemy: the victim takes the fall damage + 50, the faller half + 10. Shields don't absorb fall damage. | 3, 8 |
| Q3 | **Suspend Dirt** | ✅ | **Disassembled** `264f:0008`: stored internally as `F = 100 − SuspendDirt`. After a shot: F = 0 → dirt never falls; F = 100 → always falls; else it falls if `random(100) < F`. So **Suspend Dirt = % chance floating dirt stays in the air**; the manual's wording is backwards. Default 0 = dirt always falls (our prototype's behaviour). | 3, 10 |
| Q4 | **Physics units** | ✅ | Launch speed = **power, used directly** (0–1000), clamped at 1000. Gravity acceleration = **2500 × Gravity** (0.2 → 500); wind acceleration = **1.25 × Wind**; drag multiplies velocity by `1 − Viscosity/10000` each step (constants ✅ verified). Wall bounce: Rubber −1.0, Padded −0.5, Spring −2.0 (🟡). **Round-start wind** (disassembled `32a3:000a`): `random(Max/2) − Max/4` (±50 at Max 200); 20 % chance ×2, and then a further 40 % chance ×2 again. **Changing Wind** (disassembled `3249:0019`): `wind += random(11) − 5` (−5…+5) each turn, clamped to ±Max Wind. | 3, 4 |
| Q5 | **Tunneling** | ✅ | ×**0.75** velocity per dirt pixel entered, and the pixel is erased; explodes when speed² < **2000** (both ✅ `DS:1d54`, `DS:1d58`). Gravity and wind are off while inside dirt. Confirmed by gosuwachu with DOSBox runtime samples. Note: the EXE's built-in default is **off**, but the manual says **on**. | 5 |
| Q6 | **Points/money** | ✅ | **Disassembled** `4098:0263`, `0308` and `00f3`. Kill: **+500**, or **+4000** in Basic (`DS:5148` is the scoring-mode flag, not the bonus, which explains my earlier "miss"); killing a teammate −2000; killing yourself −1500. Each hit on an enemy: **+30 × damage**; on an enemy's shield: +2 × damage; on yourself or a teammate: **−15 × damage**. Basic: no per-hit points. Survival pool: **5000** (Basic: **players × 1000**), shared by survivors; in team modes dead teammates get half a share (🟡). Also a killer's per-victim "grudge" counter gets +5 (used by Cyborg). **Mass Kill** (disassembled `4098:0001`): no survival pool; each of the N survivors gets `trunc(pool/N)`, with pool = 5000 + 500·N + Σ(30×health + 2×shield) (Basic: 1000×players + 4000·N). See `MECHANICS.md` §10.2. | 6, 17 |
| Q7 | **Greedy net worth** | 🟡 | Cash + every held item valued at **80 %** of its current unit price (the same as the sell-back rate). | 6 |
| Q8 | **Interest, Auto Defense** | ✅ | **Disassembled** `20dc`: **interest = cash × trunc(rate × 10000) ÷ 10000**, in exact integer (big-number) arithmetic. It is added to cash and to an "interest earned" total (`tank+0xc2`), which the shop's Update box shows. Default rate 0.05 ✅. Auto Defense price is an annuity, `base × ((1+r) − (1+r)^−rounds_left) / r` (🟡). | 6, 13 |
| Q9 | **Sell-back offer** | 🟡 | `round(price × qty × 0.80 / bundle)`; **0.65** with Free Market on. | 7 |
| Q10 | **Free Market** | 🟡 | Each round per item: `demand = 0.7·demand + 0.3·(buys/players)`; `ratio = 0.7·ratio + 0.3·(price/base)²/10`; `price = trunc(price × (1 + rate·(demand − ratio)))`. **Disassembled `3523:0035`:** the step `rate` is the **Interest Rate setting** (default 0.05), not a fixed 0.05. Both start at 0.1. | 7 |
| Q11 | **Funky Bomb, Leap Frog** | 🟡 | Funky: radius-20 burst, then **5–10 curved trails** to points within **±80 px** (✅ table value 80), landing bursts of radius 15–24, then a final radius-40 blast. Leap Frog: **3** blasts (✅ table value 3) of radius **30, 25, 20**, each hop at speed ÷ 1.5. | 9 |
| Q12 | **Diggers/Sandhogs** | 🟡 | Tunnel "births" per weapon: Sandhogs **10/20/35** (✅ table); Diggers use **2× 10/20/35** (table stores −10/−20/−35 ✅). At most 20 live heads; branches every 8 ticks; sandhogs follow dirt, not tanks. When a sandhog head runs out of dirt it fires a charge of radius 10 × scale. | 10 |
| Q13 | **Riot / Dirt Charge** | 🟡 | Riot Charge/Blast: a wedge from the tank, size **36/60**, half-angle **45°/60°**. Riot Bombs: circles of radius 30/45 (✅ table). Dirt Charge: a normal shot, then an upward random spray of dirt reaching **⅓ of the field height**. Extra Dirt: amount unknown. | 10 |
| Q14 | **Napalm** | 🟡 | Fluid with up to **15** (Napalm) / **20** (Hot) emitters (✅ table 15/20), 100 active queue slots, flames grow, then 50 burn cycles. Heat damage is per emitter. Liquid Dirt uses the same routine (✅ table −20). | 11 |
| Q15 | **Energy weapons** | 🟡 | Plasma: centred on you (you're excluded), charged with **0–10 batteries**; radius runs from the Missile radius to the Nuke radius (20–75); no crater. Laser: energy from power, costs 40 per pixel (+40 more through dirt), does 10-point damage pulses; Super Mag turns laser energy into shield charge. | 12 |
| Q16 | **Shields** | ✅ | Strength **Mag 55, Shield 100, Force 100, Heavy 150, Super Mag 200**; outline radius 16/15/15/16/16; colours (VGA 0–63) Mag (63,63,23), Shield/Heavy white, Force (63,23,63), Super Mag (63,53,33). All ✅ `DS:617c`. **Direct hit on a shield** (disassembled `4d1e:0047`): a 1000 Hz beep, **10 points** to the shield, and no explosion. Blast damage beyond the shield's strength overflows to the tank. **No random failures** (🟡). Mag push: shots within ±15 px horizontally and up to ¼ screen height above get an upward push each step (no speed threshold; fast shots just get through) (🟡). Force reflect: mirror bounce at **70 %** speed (✅ 0.7), costing speed/100 shield points. | 13 |
| Q17 | **Heat Guidance range** | ✅ | Locks onto the first live non-owner tank (**teammates included**) within **40 px** (✅ `DS:5186`), then steers. Steering strength 10,000 (Heat/Vertical) and 15,000 (Horizontal) ✅. Ballistic: solves power for your angle, corrects for wind, not drag. Lazy Boy: flies directly to the point, digging through dirt. | 14 |
| Q18 | **Movement** | ✅ | **Disassembled** `3667:0443`: 1 fuel per pixel; uphill +1 per pixel climbed (1–3); a climb of 4+ is blocked; a drop of 4+ makes the tank slip and fall. See `MECHANICS.md` §8.4. | 15 |
| Q19 | **AI aiming** | 🟡 | **Moron:** power = (random(health)+1)×10, random angle toward a side that has enemies, random weapon. **Shooter:** line-of-sight angle at its target, closed-form power. **Spoiler:** random target, starts at 65°/115°, then searches ±1°. **Cyborg:** smart target choice, starts at `85 − w` (`w = |3·wind/10| + viscosity/2`, max 70). **Tosser:** first shot is a ranging lob into open space, then each turn power ±10 (and angle +2 toward vertical) based on where the last shot landed. **Chooser:** Shooter if it has a clear shot, else Poolshark with rubber/spring walls, else Spoiler. **Unknown:** rolls one of the 7 once. The AI's power solver corrects for wind but not drag. | 16 |
| Q20 | **AI buying** | 🟡 | Moron: weighted random. Everyone else: a fixed list (best shield, parachute, missiles up to 5, …). Details in the Python port's `ai.py`. | 16 |
| Q21 | **Skies** | ✅ | Lightning as before (🟡). **Cavern** (DOSBox): a dirt ceiling polyline hangs from the top, with a gradient from dirt colour to near-black in the middle. No meteors in v1.5 ✅. See `MECHANICS.md` §5.3. | 19 |
| Q22 | **`.MTN` format** | ✅ | 24-byte header: magic `4D 54 BE EF`, version u16 **big-endian** = 1, then little-endian width, min-height, height, ncolors (16), …. Then a palette of `ncolors` × RGB (8-bit). Then `width` columns, each `u16 count` + `ceil(count/2)` bytes of 4-bit pixels, **bottom-up**, low nibble first. Sky pixels are not stored. ✅ Decoded all 10 files exactly. | 19 |
| Q23 | **Sounds** | 🟡 | Firing chirp: 1000, 2500, 4000, 5500, 7000, 8500, 10000 Hz. The riot weapons use a rising/falling siren. Others: see gosuwachu's `src/` sound code. | 20 |
| Q24 | **Death animations** | ✅ | 11 death animations (🟡, listed in `MECHANICS.md` §15). **Nuke** (DOSBox): a red-and-black mottled "plasma" disc grows ~1 s, shimmers ~2 s, then vanishes leaving the crater. | 21 |
| Q25 | **Scale** | 🟡 | Explosion size multiplier Normal/Medium/Large = **1 / 2 / 3** (in 320-wide mode: 0.5 / 0.75 / 1). Applies only to some weapon families. | 9 |
| Q26 | **Firing/Falling Delay** | ✅ | Firing Delay: time-step formula (🟡). **Falling Delay** (disassembled `2a1b:001e`): **milliseconds** (Borland `delay()`) per falling-animation step; parachute descent a fixed 20 ms per pixel. | 2 |
| Q27 | **Key repeat** | ✅ | **Disassembled** `38b6`/`38d6`: the game polls held keys against a ≈3649 Hz timer. ↑/↓ ±1, PgUp/PgDn ±15, ←/→ ±1°, at most 1 step per 15 ticks (≈243/s); Shift = 30 ticks (≈122/s); Alt = one step per press. See `MECHANICS.md` §14. | 0 |

Don't copy the original's data files into this repo (the `.MTN` mountains,
`TALK*.CFG`). They are part of the shareware, so let the player point to their
own copy instead.

### Where things live today

`game.rs` (round/turn flow, world state) · `settings.rs` (session options) ·
`physics.rs` (shared ballistics, wind and settling rules) ·
`economy.rs` (cash, interest, awards and ranking) · `shop.rs` (opening purchases) ·
`weapons.rs` (`ITEMS` catalogue, projectiles, impacts, damage law) · `tank.rs`
(tanks and controllers) · `inventory.rs` (bounded persistent stock) · `ui.rs`
(menu input, HUD, dialogs, drawing) · `terrain.rs` (pixels, `carve`, `add_dirt`,
`settle_step`) · `ai.rs` (`plan()`, `sim()`) · `gfx.rs` (drawing primitives).

---

## Batch 0 – Make the prototype match the manual (S)
**Depends on:** –

Things the prototype already has, but does differently from the original:

- [x] **Shift slows things down** in the original, it doesn't speed them up ✅ **Q27**
      *(p. 11)*: Shift = slow change, **Alt = change by exactly 1**. Holding an
      arrow repeats at ~243 steps/s (~122 with Shift). Arrows change by 1;
      PgUp/PgDn change power by 15 per step.
- [x] **Angle shown as 0–90** *(p. 9)*: "0 is horizontally level, 90 straight
      up"; you see the direction from the tank's barrel. Internally keep 0–180,
      but show `90 - |angle - 90|` in the HUD.
- [x] **HUD layout** *(p. 9)*: one bar at the top:
      `Power: 345   Angle: 35   <player name>   <weapon icon> 3 Dirt Clod`.
      **Wind is written in the sky** in the upper right, in light blue, with an
      arrow and a number, or `No Wind`. Move health off the main bar (it goes
      in the status bar, batch 8).
- [x] **MIRV**: five *Missile* warheads that split at the top of the arc. "If the
      warhead hits something before reaching apogee, it will not explode"
      *(p. 24)*. Pre-apogee impacts are now duds; all five children use Missile blasts.
- [x] **Dirt Ball radius 35** (was 30) *(p. 23)*. Funky Bomb's "80" in
      the manual is **not** a blast radius: it's how far (±80 px) its bomblets
      scatter (see Q11). Others already match: Baby Missile 10, Missile 20,
      Baby Nuke 40, Nuke 75, MIRV 20, Roller 20.
- [x] **Damage law** (Q1, disassembled): `trunc((R - d) * 100 / R) * (weapon
      index + 1)`, and if that's over 100, use 110. Only for `d < R`, measured
      to the tank's **base**, not its centre. A direct hit kills an unshielded
      tank.
- [x] **Roller** explodes with the force of a Missile (Baby Roller = Baby
      Missile) *(p. 25)*. Matching blast radii are retained. Damage uses each
      weapon's original index per Q1 (Roller ×14, Missile ×2).
- [x] **Rename our "Cyborg" to "Spoiler"**: perfect aim with wind is what the
      original calls Spoiler *(p. 7)*. The original Cyborg has the same aim plus
      nasty target choice (batch 16). Our "Shooter" (ignores wind) is only a
      stand-in until batch 16.
- [x] Max power = 10 × remaining health, 1000 at full *(p. 9)*. Already correct.
- [x] The player who goes first is random, then play goes **left to right**
      *(p. 5)*. Cycle living tanks by x position, wrapping at the right edge;
      preserve player identities between rounds.

**Validation:** regression tests cover repeat rates and Alt, angle display,
MIRV split/duds, blast damage and direct hits, blast sizes, health-limited
power, and spatial turn order. A framebuffer preview covers the HUD layout.
Side-by-side interactive comparison in DOSBox remains a manual check.

**Done when:** controls and HUD feel like the original side by side in DOSBox.

## Batch 1 – Foundations: settings and items (S)
**Depends on:** –

Groundwork so later batches only add data, not rewrites.

- [x] A `Settings` struct owned by `Game`, holding every option from the
      original's menus (see batch 2). Move the menu's current values into it.
- [x] Split `game.rs` before it grows further: `weapons.rs` (table plus
      behaviour), `ui.rs` (menus, HUD, dialogs), `tank.rs`.
- [x] Generalise `WEAPONS` into an `Item` table matching the manual's tables
      *(pp. 23, 28)*: name, kind, **cost, bundle size, blast radius, arms
      level**, category (Weapon / Miscellaneous). Copy the numbers from the
      tables in batches 9–15.
- [x] Inventory is kept between rounds. **Max 99 of any item**; Baby Missiles
      are unlimited (shown as 99) *(p. 18)*.

**Implemented:** `settings.rs` owns all planned menu options (plus config-only
fields); the current menu reads and changes `Game.settings`. Future options
are stored with their documented defaults but take effect in their own batches.
Batch 2 extends this foundation with 10-player setup, a 10-round default and
settings persistence.

`weapons.rs` holds the full 48-item catalogue and projectile behaviour;
`ui.rs` holds menu input, HUD, dialogs and object drawing; `tank.rs` holds tank
state and geometry. `inventory.rs` enforces stock limits. Catalogue rows generate
the item IDs and inventory size, preserving original weapon indices for damage.
Unimplemented items are catalogued but cannot be selected or fired yet.
Funky Bomb's opening radius is corrected to 20 from the researched rules;
its remaining fidelity work stays in batch 9.

The prototype starter stock is granted once per new game, never refilled at
round boundaries. Shop purchases and accessory use remain in later batches.
Regression tests verify round carry-over/new-game reset, stock limits,
selection/fire guards, and catalogue prices/bundles/arms against `MECHANICS.md`.

**Done when:** the game plays as before, and a new weapon is one table row plus
one `match` arm.

## Batch 2 – Main menu and options screens like the original (M)
**Depends on:** 1

The original main menu *(p. 38)*: `Start`, `Players: 2` (2–10),
`Rounds: 10` (1–1000), then the sub-menus `Sound...`, `Hardware...`,
`Economics...`, `Physics...`, `Landscape...`, `Play Options...`, `Weapons...`,
and `Save Changes`.

- [x] Same layout: a grey column of buttons on the left and the title picture
      on the right. Each button has a **hotkey** letter, highlighted (Start = S,
      Physics = Y, Play Options = T, and so on).
- [x] Sub-menus pop up as small grey dialogs. **Counter/cycle** items:
      hotkey = next, **Alt+hotkey** = previous. **Toggles** show a check box.
      Esc or a click outside closes the dialog *(p. 37)*.
- [x] Raise the player limit to **10** (needs 2 more player colours).
- [x] `Save Changes` writes the settings (ours: `~/.config/scorched/settings.toml`,
      using crates `serde`, `toml`, `dirs`). Changes not saved last only for
      this session, as in the original.
- [x] Optional, as in the original: start with a different config file,
      `scorched myconfig.toml`.
- [x] Hardware menu: only **Firing Delay** (how fast shots are drawn, ✅ **Q26**
      0–500, default 100) and **Falling Delay** (how fast tanks fall, 0–1000,
      default 10) make sense for us *(p. 41)*. Skip graphics mode, BIOS
      keyboard and small memory.

The options themselves are wired up in the batches below.

**Implemented:** the main menu uses a left button column and an original title
illustration on the right. All seven option dialogs are present; pending
features are greyed out and cannot be edited. Start opens controller setup for
up to 10 players; Done begins the game. The default is now 10 rounds, with a
1–1000 range.

Keyboard, left/right mouse clicks, Alt+hotkey, and Esc/outside-click dismissal
are supported. Mouse coordinates follow the framebuffer scale/letterboxing.
Hardware delays are active: Firing Delay changes animation speed while keeping
fixed physics steps; Falling Delay is milliseconds per falling-tank pixel,
with zero completing the fall without a drawing delay.

Startup reads the default or supplied TOML file. Missing files use defaults;
partial files inherit defaults, while invalid files produce a visible error.
Only Save Changes writes the selected file, including any custom config path.
Settings are validated before loading/saving and writes replace the file via a
temporary file. Session edits remain unsaved until explicitly saved.

Tests cover menu input and dismissal, disabled controls, all dialog layouts,
10-player placement, hardware timing, config persistence/validation, and custom
paths. Main menu and dialog framebuffer previews were visually checked.

**Done when:** every menu from the original exists. Unimplemented options are
greyed out.

## Batch 3 – Physics menu (M)
**Depends on:** 2

All from the Physics menu *(pp. 44–45)*, confirmed in DOSBox:

- [x] **Air Viscosity** 0–20, default 0: drag that slows projectiles.
- [x] **Gravity** 0.05–10, default 0.2. Uses acceleration `2500 × Gravity` (0.2 → 500), per Q4. ✅ **Q4**
- [x] **Suspend Dirt** 0–100 %, default 0: the per-shot **chance that floating
      dirt stays in the air** (Q3, disassembled; the manual's wording is
      backwards). At 0, dirt always falls.
- [x] **Tanks Fall** on/off, default on.
- [x] **Borders Extend** 0–10000, default 75: how many pixels off-screen a shot
      is tracked before it counts as a miss, so wind can bring it back. Ours
      currently drops the shot at the edge.
- [x] **Impact Damage** (config-file only, *p. 50*): fall damage when the tank ✅ **Q2**
      lands (on) or while it falls (off). Same total either way.
- [x] Wind (from the Landscape menu, *p. 46*): **Max. Wind** 0–500, default 200 ✅ **Q4**
      (rescale ours, which is ±10), and **Changing Wind**, default off: wind
      changes by −5…+5 each turn (Q4). Round-start wind:
      `random(Max/2) − Max/4`, sometimes doubled (Q4).
- [x] `ai::sim()` uses the same physics (pass a `Physics` struct). Note: the
      manual says some AIs and Ballistic Guidance **can't handle viscosity**.
      Keep that weakness.

**Implemented:** `physics.rs` provides the fixed step shared by live projectiles
and AI prediction: power is launch speed directly, speed is capped at 1000,
and integration follows move → drag → gravity → wind. The confirmed Q4 units
replace the prototype's `G=200`, `POWER_K=0.45`, and `WIND_K=4`.

The five Physics controls and Landscape's Max. Wind / Changing Wind controls
are active. Gravity cycles in 0.05 steps; other numeric controls use unit steps
(PgUp/PgDn = ten steps). Walls remain disabled until batch 4. Impact Damage
is a TOML-only `physics.impact_damage` setting.

Suspend Dirt is rolled once for each fired shot, including all child warheads
and death blasts. Dirt settling waits for projectiles and explosions to finish;
a later collapsing shot can settle dirt left suspended by earlier shots.
Tanks Fall off leaves tanks floating. Fewer than three ground pixels triggers
a fall; damage is two per fallen pixel, applied during descent or on landing,
with the same total in either mode.

Round-start wind uses the recovered conditional doubling formula. Changing
Wind applies a clamped −5…+5 step between turns, retaining the initial wind
for the first shot. Borders Extend applies to lateral exits; high arcs can
travel above the screen. The former 30-second projectile timeout is extended
to 120 seconds so large margins remain useful.

AI simulations use the same projectile step and border rules. Shooter/Spoiler
intentionally ignore drag; the prototype Shooter also retains its existing
wind blindness until batch 16. Tests cover physical units and trajectories,
off-screen return, settling probabilities and shot lifetime, fall modes,
wind rules, enabled menus, and AI predictions.

**Done when:** every option visibly changes the game, and the AI still copes
with gravity and wind changes.

## Batch 4 – Walls (M)
**Depends on:** 3

"Effect of Walls", default None *(p. 45)*. Confirmed in DOSBox: the menu cycles
None → Wrap-around → Padded → Rubber → Spring → Concrete → Random → Erratic.

| Type | Behaviour | Wall colour |
|---|---|---|
| None | no effect (Borders Extend applies) | grey |
| Concrete | blocks shots in **every** direction, top of screen included | white |
| Padded | reflects shots, absorbing some energy | green |
| Rubber | reflects shots, no energy lost | red |
| Spring | reflects with an extra "kick", like a pinball bumper | cyan |
| Wrap-around | left and right edges join | yellow |
| Random | a type picked at the start of each round | – |
| Erratic | a new type before each shot | – |

- [ ] Draw the walls as thin lines in those colours at the screen edges.
- [ ] `ai::sim()` follows the walls (needed for Poolshark, batch 16).
- [ ] Works for rollers and MIRV warheads too.

**Done when:** all types work with every weapon, and the colours match.

## Batch 5 – Tunneling (S)
**Depends on:** 1

A Weapons-menu option, **on by default** *(p. 49)*. The original's projectiles
don't explode the moment they touch dirt; they burrow in first.

- [x] Tunneling on: a projectile hitting dirt keeps going through it, slowing
      down, then explodes. With a thin enough pile it can **come out the other
      side** at lower speed. ✅ **Q5**. Rule from the binary: for **each dirt
      pixel** the shot enters, multiply its velocity by **0.75** and erase that
      pixel; once speed² < **2000**, it explodes there. No gravity or wind while
      inside dirt. Rollers, tracers and Dirt Charge never tunnel.
- [x] Tunneling off: explode on contact.
- [x] This is what makes Contact Triggers useful (batch 14) and why Napalm can
      "fizzle" inside dirt (batch 11).

**Implemented:** the Weapons menu's Tunneling checkbox is active and defaults
to on. Each projectile snapshots the setting when fired, and child warheads
inherit it. Pre-apex MIRVs retain batch 0's dud behavior on contact.

The shared projectile step walks crossed grid cells in order, including fast,
negative and diagonal movement. Each newly entered dirt pixel is erased and
multiplies velocity by 0.75; crossing an already erased pixel does not charge
again. The shot detonates when speed² is strictly below 2000. Gravity, wind
and drag affect only time spent in air and resume after leaving the dirt.
Erased pixels mark their columns for the existing post-shot settling rules.
Roller variants, tracer variants and Dirt Charge are excluded from tunneling;
their remaining weapon implementations stay in their own batches.

AI prediction uses this same step with a private terrain-erasure overlay for
each candidate shot, leaving the live battlefield unchanged. Tests cover thin
layers, buried explosions, strict cutoff, per-pixel slowing, forces, exceptions,
child settings, menu/config behavior and isolated AI predictions. A framebuffer
preview checks an explosion centered below the hillside surface.

The per-shot flag provides the override for Contact Triggers in batch 14;
Napalm's underground fizzle behavior remains part of batch 11.

**Done when:** shots into a hillside visibly explode inside it.

## Batch 6 – Money and scoring (M)
**Depends on:** 1

Economics menu *(pp. 42–43)*:

- [x] **Interest Rate** 0–0.30, default 0.05, earned on money left in the bank ✅ **Q8**
      at the start of a round.
- [x] **Cash at Start** $0–$1,000,000, default $0. If above 0, everyone shops
      before round 1.
- [x] **Computers Buy** on/off, default on.
- [x] **Scoring Mode** — **Basic / Standard / Greedy** (default Standard).
      My earlier list said "Corporate/Vicious": those are *team* modes, batch 17.
  - Basic: points only for kills and for surviving the round.
  - Standard: fewer points per kill, plus points every time you damage a ✅ **Q6**
        tank or shield.
  - Greedy: like Standard, but final rankings use **net worth** (cash plus 🟡 **Q7**
        the depreciated value of your inventory).
- [x] Points are money: you **lose** money for damaging or killing yourself
      or teammates *(pp. 31, 52)*. ✅ **Q6**
- [x] Round-over screen ranks players, with cash shown.

**Implemented:** the Economics menu enables Interest Rate, Cash at Start,
Computers Buy and Basic/Standard/Greedy scoring. Interest changes by 0.01 and
starting cash by $1,000 per menu step; TOML can specify intermediate values.
Free Market remains disabled until batch 7.

`economy.rs` uses exact arbitrary-size unsigned integer cash, including integer
interest at `trunc(rate × 10000)` basis points and an interest-earned total.
Each round pays interest once, at its start. New games reset accounts; cash
and inventory persist between rounds. Penalties clamp cash at zero.

Direct hits, blasts and falls award money for actual health removed, capped by
the victim's remaining health, so overkill and secondary explosions cannot
award the same health twice. Enemy kills pay 4000 in Basic or 500 otherwise;
self kills cost 1500, teammate kills 2000. Basic has no per-damage awards;
Standard/Greedy use +30 per enemy health point and −15 for self/team damage.
The scoring helper also supports +2 per enemy shield point for batch 13.
Team assignment/gameplay remains batch 17; its penalty relationship is ready.
Survival pools (players × 1000 or 5000) split with integer truncation and pay
once; all-dead rounds pay nothing.

Round-over and final standings rank by cash, or by cash plus 80%-valued held
inventory in Greedy, with cash always shown. Valuation uses current bundle
prices and rounds once per item type, matching the sell-back formula. The
always-held 99 Baby Missiles use the same catalogue valuation for every player.
Large balances display compactly; comparisons retain their full exact value.

Positive starting cash opens a basic purchase screen before round 1. Humans
can buy affordable complete bundles of the nine implemented weapons (excluding
unlimited Baby Missiles); computers make bounded starter purchases only when
Computers Buy is on. Interest on round 1 is paid on cash remaining after these
purchases. Inter-round shopping, selling, partial bundles, accessories and the
full shop controls remain batch 7/8; computer buying personalities remain 16.

Tests exercise ten full scoring rounds per mode, 1000-round interest growth,
penalties, fall timing, kill/pool payment guards, Greedy ranking, current prices,
opening purchase flow, menu input and settings persistence. Ten-player standings
and opening-purchase previews were visually checked.

**Done when:** money changes sensibly over a 10-round game in each mode.

## Batch 7 – Shop (L)
**Depends on:** 6

Buying screen *(pp. 18–21)*:

- [ ] Between rounds, every player with cash shops in turn (computers too, if
      Computers Buy is on).
- [ ] Left column: a list of items with icon, name and price per bundle. Two
      groups: **Weapons** and **Miscellaneous**, switched with Tab.
- [ ] **Items you can't afford are hidden**, and reappear when you can.
- [ ] Keys: Up/Down move the marker (wrapping), Home/End jump to the top or
      bottom, PgUp/PgDn change pages (wrapping), **Space buys one bundle**,
      `i` opens your inventory, `u` shows status (rounds left, interest
      earned), Esc = Done.
- [ ] 99 limit: if a full bundle would go over 99, sell only the rest at about
      a 20 % markup per item.
- [ ] **Selling**: in the inventory, picking an item opens a panel. `q` / 🟡 **Q9**
      Up raises the quantity, Alt-q / Down lowers it; the offer changes
      with the quantity; `a` accepts, `r` or Esc rejects. Usually sells at a
      loss. The offer only changes between rounds.
- [ ] **Arms Level** 0–4 (Weapons menu, default 4): hides items above that
      level.
- [ ] **Free Market** (default off): prices rise for items people buy and fall
      for ignored ones, "bouncing" a little. Saved between games (the original
      used `scorch.mkt`; ours could use `~/.local/share/scorched/market.toml`).
      🟡 **Q10**
- [ ] **Useless Items** option (default off): hide items that do nothing with
      the current settings, e.g. Smoke Tracer when Trace Paths is on.

**Done when:** a 10-round game with shopping plays smoothly.

## Batch 8 – Tank Control Panel, status bar, inventory, parachutes, batteries (M)
**Depends on:** 7

How you use accessories during your turn *(pp. 10–17)*:

- [ ] **`t` opens the Tank Control Panel** (a dialog): batteries, parachutes
      (state and threshold), shields (select and engage), triggers, guidance,
      fuel. Items you don't own are greyed out.
- [ ] **`i` opens the Inventory Panel**: icons of everything you own. Picking
      one does the "obvious" thing: selects a weapon, energises a shield,
      toggles parachutes or triggers, selects guidance, uses a battery, or opens
      fuel/movement.
- [ ] **Status Bar** option (Play Options, default off): a second HUD line,
      `Max: 1000  n[batt]  n[para]  n[shield] %  n[guid]  n[trig]  n[fuel]`,
      with keys `b` battery, `p` parachute toggle, `s` select shield, `e` engage
      shield, `g` guidance, `f` fuel, `-` trigger.
- [ ] **Parachutes** *(p. 31)*: before a fall, estimate the damage. If ✅ **Q2**
      parachutes are **deployed** and the damage would be **more than the
      safety threshold** (default 5, range 1–100, `v` in the panel), use one and
      take no fall damage (unless you land on an enemy tank). Passive parachutes
      do nothing. Default: deployed.
- [ ] **Battery** *(pp. 14, 31)*: +10 health (capped at 100), which raises max
      power. Batteries also power energy weapons (batch 12).
- [ ] Other in-game keys *(pp. 12)*: `u` status (wall type, rounds left),
      `k` "No kibitzing, please." box, `1`–`9`/`0` info on tank N counted from
      the left, `r` **retreat** (confirm, then you escape in a helicopter while
      your tank blows up behind you: you get no points, but the enemy doesn't
      get points for killing you either).

**Done when:** all accessory controls work from both the panel and the status bar.

## Batch 9 – Standard weapons (M)
**Depends on:** 1

From the weapons table *(pp. 23–25)*:

| Weapon | Cost/bundle | Radius | Arms | Notes |
|---|---|---|---|---|
| Baby Missile | $400/10 | 10 | 0 | unlimited ✅ **Q1** |
| Missile | $1,875/5 | 20 | 0 | |
| Baby Nuke | $10,000/3 | 40 | 0 | |
| Nuke | $12,000/1 | 75 | 1 | |
| Leap Frog | $10,000/2 | 20, 25, 30 | 3 | three warheads, one after another; good against shields. Per the binary the blasts **shrink: 30 → 25 → 20**, each hop relaunched at speed ÷ 1.5 🟡 **Q11** |
| Funky Bomb | $7,000/2 | 80 | 4 | multi-coloured chain reaction, stays near where it hit. "80" = bomblet scatter range (±80 px), not radius 🟡 **Q11** |
| MIRV | $10,000/3 | 20 | 2 | 5 Missile warheads (see batch 0): the parent keeps flying as the middle one, plus 4 children |
| Death's Head | $20,000/1 | 35 | 4 | like MIRV with **9** large warheads |
| Tracer | $10/20 | 0 | 0 | no damage |
| Smoke Tracer | $500/10 | 0 | 1 | leaves a coloured smoke trail that stays |
| Baby Roller | $5,000/10 | 10 | 2 | roll downhill to a valley or a tank; **roll off shields** |
| Roller | $6,000/5 | 20 | 2 | |
| Heavy Roller | $6,750/2 | 45 | 3 | |

- [ ] Add the missing ones: Leap Frog, Death's Head, Smoke Tracer, Baby Roller,
      Heavy Roller.
- [ ] **Trace Paths** option (Weapons menu, default off): every shot leaves a
      trail in the owner's colour until cleared from the System Menu
      (batch 17).
- [ ] **Bomb icon** option: Invisible / Small / Big (default Big).
- [ ] **Scale** option: Normal / Medium / Large, making all blasts bigger. 🟡 **Q25**

**Done when:** all of these are in the shop with the right prices.

## Batch 10 – Earth-destroying and earth-producing weapons (M)
**Depends on:** 3, 9

From the tables *(pp. 23, 26–27)*:

| Weapon | Cost/bundle | Radius | Arms | Behaviour |
|---|---|---|---|---|
| Riot Charge | $2,000/10 | 36 | 2 | removes a **wedge** of dirt around your turret; to dig yourself out; no projectile 🟡 **Q13** |
| Riot Blast | $5,000/5 | 60 | 3 | wider, thicker wedge |
| Riot Bomb | $5,000/5 | 30 | 3 | projectile; removes a **sphere** of dirt; no damage to tanks |
| Heavy Riot Bomb | $4,750/2 | 45 | 3 | bigger Riot Bomb |
| Baby Digger | $3,000/10 | – | 0 | tunnels on hitting ground; **fizzles if it hits a tank** 🟡 **Q12** |
| Digger | $2,500/5 | – | 0 | stronger |
| Heavy Digger | $6,750/2 | – | 1 | biggest, "often fails to reach full potential" |
| Baby Sandhog | $10,000/10 | – | 0 | tunnels, and each tunnel ends in a small charge: hits tanks from below, under shields 🟡 **Q12** |
| Sandhog | $16,750/5 | – | 0 | more warheads |
| Heavy Sandhog | $25,000/2 | – | 1 | "can potentially destroy the world" |
| Dirt Clod | $5,000/10 | 20 | 0 | sphere of dirt |
| Dirt Ball | $5,000/5 | 35 | 0 | |
| Ton of Dirt | $6,750/2 | 70 | 1 | |
| Dirt Charge | $5,000/5 | – | 1 | blows a **wedge-shaped cloud of dirt** into the air (like Riot Charge, but adds dirt) 🟡 **Q13** |
| Earth Disrupter | $5,000/10 | – | 0 | makes all floating dirt fall; only useful when Suspend Dirt > 0 |

- [ ] **Extra Dirt** option (Weapons menu, default off): Dirt Charges let out 🟡 **Q13**
      much more dirt.
- [ ] Earth-destroying weapons hurt tanks only through **fall damage**.

**Done when:** a buried tank can dig itself out, and sandhogs can kill a
shielded tank from below.

## Batch 11 – Liquids: Napalm, Hot Napalm, Liquid Dirt (L)
**Depends on:** 5, 9

| Weapon | Cost/bundle | Arms | Behaviour |
|---|---|---|---|
| Napalm | $10,000/10 | 2 | splashes, then burns; **deep pools burn hotter** 🟡 **Q14** |
| Hot Napalm | $20,000/2 | 4 | much hotter and stronger |
| Liquid Dirt | $5,000/10 | 2 | oozes out, fills holes and smooths terrain (clears a path for Rollers) |

- [ ] A small liquid simulation (new `liquid.rs`): particles or cells that flow
      downhill and pool.
- [ ] Napalm damage depends on pool depth. Shields don't protect you from your 🟡 **Q14**
      own napalm's heat, though they do protect you from other players'
      secondary damage *(p. 31)*.
- [ ] With Tunneling on, napalm that burrows into dirt can **fizzle**
      *(p. 24)*. Contact Triggers prevent that.

**Done when:** napalm in a valley pools and burns the tank sitting there.

## Batch 12 – Energy weapons: Plasma Blast and Laser (S)
**Depends on:** 8

*(pp. 23, 27)*. Energy weapons use **batteries**: after firing you choose how
many to spend (keys 0–9); no batteries means a weak or no effect.

- [ ] **Plasma Blast** ($9,000/5, radius 10–75 depending on batteries, arms 3): 🟡 **Q15**
      radiation all around your tank, hitting neighbours. The turret angle
      doesn't matter.
- [ ] **Laser** ($5,000/5, arms 2): a straight beam along the barrel that 🟡 **Q15**
      cuts through mountains, **shields**, and anything else in its way.
- [ ] Guidance doesn't work with Plasma Blast *(p. 29)*.

**Done when:** both are usable and their strength depends on the batteries spent.

## Batch 13 – Shields (M)
**Depends on:** 8

*(pp. 28, 31–32)*. Select a type with `s`, engage with `e`. Engaging replaces
any current shield, and the old one is lost. Show the percentage left in the
HUD.

| Shield | Cost/bundle | Arms | Behaviour |
|---|---|---|---|
| Mag Deflector | $10,000/2 | 2 | **upward force** on shots near and above you; fast-falling shots get through; breaks easily ✅ **Q16** |
| Shield | $20,000/3 | 3 | absorbs explosion damage; a **direct hit doesn't explode**, it only damages the shield a little ✅ **Q16** |
| Force Shield | $25,000/3 | 3 | **deflects** shots away; stronger than Shield ✅ **Q16** |
| Heavy Shield | $30,000/2 | 4 | immune to "shield failures"; takes huge damage ✅ **Q16** |
| Super Mag | $40,000/2 | 4 | strongest; **immune to lasers**; also has the Mag Deflector push and Force Shield bounce ✅ **Q16** |
| Auto Defense | $1,500/1 | 3 | lasts the whole game: at each round start you get the Tank Control Panel's defensive options before the fighting begins. Price depends on rounds left and the interest rate ✅ **Q8** |

- [ ] **Your own shield never protects you from your own shot**. Fire straight
      up with no wind and you die, and lose points.
- [ ] "Shield failures" for lesser shields ✅ **Q16**: per the reverse
      engineering there is **no random failure**. A shield simply collapses when
      a hit is bigger than its remaining strength, and the rest of the damage
      goes to the tank.
- [ ] Draw shields as a circle around the tank that flickers when hit.

**Done when:** each shield behaves differently, and a Leap Frog or a nearby
nuke beats a plain Shield (the manual's strategy tip, *p. 32*).

## Batch 14 – Guidance, Lazy Boy, Contact Trigger (M)
**Depends on:** 5, 8

*(pp. 28–32)*. Choose guidance with `g` before firing. After the shot it
**resets to None**, so expensive guidance isn't wasted by accident. Weapons that
can't be guided (MIRV, Death's Head, Riot Charge, Riot Blast, Plasma Blast)
leave the guidance unused and not used up.

- [ ] **Choose target** prompt, after Space, for guidance that needs it: press
      `1`–`9`/`0` (tanks counted from the left), or click any point with the
      mouse (right click = nearest tank).

| Item | Cost/bundle | Arms | Behaviour |
|---|---|---|---|
| Heat Guidance (Ht) | $10,000/6 | 2 | when in range of **any** enemy, flies straight to it; no target choice ✅ **Q17** |
| Ballistic Guidance (B) | $10,000/2 | 2 | uses your angle and works out the **power** needed to hit the target; corrects for wind but **not viscosity**; if impossible, it "gets confused" |
| Horz Guidance (H) | $15,000/5 | 1 | flies flat once level with the target |
| Vert Guidance (V) | $20,000/5 | 1 | drops straight down once above the target |
| Lazy Boy (L) | $20,000/2 | 3 | choose a target and it's hit; explodes **exactly where you clicked**, and early if it hits a different tank |
| Contact Trigger | $1,000/25 | 3 | the shot explodes on first contact (tunneling off for that shot); **one trigger covers all warheads** of a MIRV etc. Toggle with `-` or in the panel |

**Done when:** each guidance type visibly changes the shot's path.

## Batch 15 – Fuel, movement, tank icons (M)
**Depends on:** 8

*(pp. 8, 16, 32)*

- [ ] **Fuel Tank** ($10,000/10, arms 3): each one gives 10 fuel. `f` opens the ✅ **Q18**
      move panel; Left/Right move one pixel per fuel unit, **more on uphill
      slopes**. Hills that are too steep can't be climbed; going down too steep
      a slope makes you **slip and fall** (normal fall damage), and the turn
      goes back to aiming. Esc ends moving.
- [ ] **Tank icon choice** in player setup: several tank shapes. Icons
      **without wheels or treads are fixed emplacements**: they can't buy or
      use fuel.
- [ ] **Triple-turreted tank** (registered version and computers only in the
      original): fires 3 shots at a time with Missiles or Baby Missiles.

**Done when:** you can drive out of a crater, and fixed emplacements can't move.

## Batch 16 – Computer players as in the original (M)
**Depends on:** 3, 4, 7, 13

The manual's descriptions *(p. 7)*. Note: my earlier version of this list had
Spoiler and Shooter wrong.

| Type | Behaviour |
|---|---|
| Moron | picks an angle and power and shoots 🟡 **Q19** |
| Shooter | much deadlier, but **only with a straight line of fire** 🟡 **Q19** |
| Poolshark | acts like Shooter unless walls bounce; then banks shots off the **walls and ceiling** |
| Tosser | starts like Moron and **corrects its aim** after each shot until it hits 🟡 **Q19** |
| Chooser | has all the methods above and picks the most effective one 🟡 **Q19** |
| Spoiler | accounts for wind and gravity; near-perfect if nothing is in the way; **can't handle viscosity** |
| Cyborg | Spoiler's aim plus nasty targeting: goes for tanks that are **weakened, winning, or attacked it** before 🟡 **Q19** |
| Unknown | a random type, **kept secret** from the players |

- [ ] Rework `ai.rs` into these 8 types. Tosser needs memory of its last shot
      per target.
- [ ] The computer **picks its own name**, never the same as a human's *(p. 7)*.
- [ ] Buying (if Computers Buy is on) and using items: shields, parachutes,
      batteries, Riot Charge to dig out. 🟡 **Q20**

**Done when:** each type plays recognisably like its description.

## Batch 17 – Play options, teams, System Menu (M)
**Depends on:** 6, 8

Play Options menu *(pp. 47–48)* and System Menu *(pp. 33–35)*:

- [ ] **Play Order** — Random (default: random order **each turn**) /
      Losers-first / Winners-first / Round-robin (random order, but the first
      shot of each round moves on one player).
- [ ] **Teams** — None / Standard / Corporate / Vicious *(p. 52)*:
  - Standard: teams work together; you lose points for hurting teammates.
  - Corporate: like Standard, but **one shared bank account** per team.
  - Vicious: like Standard while two or more teams are alive, then a
        free-for-all inside the last team.
- [ ] **Fast Computers** (default off): speeds up the game while only
      computers are fighting each other.
- [ ] **Talking Tanks** — Off (default) / Computers / All, and **Talk
      Probability** 0–100 (default 100). Attack and death phrases come from
      text files: **one phrase per line**, repeated lines are said more
      often, and **blank lines mean silence** *(p. 48)*. Use the same format so
      players can load their own `TALK1.CFG`/`TALK2.CFG`; keep our own phrases
      as the defaults.
- [ ] **System Menu** (F1 or Alt-S, any time): Clear Screen (traces),
      **Mass Kill** (ends the round, splitting the remaining points), ✅ **Q6**
      Reassign Players (change name or controller), Reassign Teams, Sound
      on/off, Save Game, Restore Game, New Game, Quit (with confirmation).
- [ ] Player setup screen *(p. 6)*: the background turns the player's colour,
      you type a name, Tab switches between human and computer, choose a tank
      icon, Enter = Done.

**Done when:** a 2v2 Corporate team game plays to the end.

## Batch 18 – Synchronous and Simultaneous modes (L)
**Depends on:** 17

Play Options "Mode": Sequential (default) / Synchronous / Simultaneous
*(pp. 47, 51–52)*.

- [ ] **Synchronous**: each player aims and "fires" in turn, but nothing flies;
      the tank is highlighted as ready. When everyone is ready, **all shots go
      at once**. Repeat until the round ends.
- [ ] **Simultaneous**: everyone aims and fires in real time on one keyboard.
  - Each human chooses **6 keys** during setup (turn clockwise, turn
        counter-clockwise, power up, power down, fire, change weapon), with
        no duplicates allowed.
  - You can't fire again until your shot has exploded, but you can aim
        meanwhile. Computers act at a "medium" speed.
  - The top bar shows every tank's power and weapon in its colour. Tanks
        **flash bright** for power up, **dark** for power down, **red** for a
        weapon change.
  - `i`, `t`, `r` and the mouse don't work, so: shields only with Auto
        Defense, parachutes always deployed, and batteries fire automatically
        when health drops below 91 (tank flashes green).

**Done when:** Synchronous with 3 players and Simultaneous with 2 humans both
work.

## Batch 19 – Sky and landscape (M)
**Depends on:** 2

Landscape menu *(p. 46)*:

- [ ] **Sky**: Plain / Stormy / Stars / Shaded / Sunset / Cavern / Black /
      Random (default Random, which **never picks Black**). Stormy is a ✅ **Q21**
      **hostile** sky that throws lightning. Cavern is a new 1.5 sky (a cave
      ceiling). Black is pitch dark: "test your skills at night".
- [ ] **Hostile Environment** (default **on**): if off, hostile skies still
      act but do no damage.
- [ ] No meteors: the author's FAQ says meteor showers were added in v1.1, but
      v1.5's EXE contains no meteor text or option. Only Stormy lightning is
      hostile in v1.5. (Meteors could be an optional extra.)
- [ ] **Terrain generator like the original** (`MECHANICS.md` §5.3; open
      research in `TERRAIN_FOLLOWUP.md`):
      midpoint displacement with 4–6 subdivision levels (16–64 segments), a
      random roughness, and the points joined by straight lines. Offsets are
      `(random(17) − 8) / 8 × amp`, and `amp` shrinks by `1 / 2^H` per level.
- [ ] **Make Bumpiness, Slope and Flatten Peaks actually work** (our
      improvement: in v1.5 the generator ignores them, though the manual says
      they shape the land):
  - **Bumpiness** 0–100 (default 20) → roughness: how slowly `amp` shrinks
    (lower `H`) and how many subdivision levels (more levels = more small
    bumps).
  - **Slope** 0–100 (default 20) → overall height range of hills and valleys,
    and how far apart the two end heights may be.
  - **Flatten Peaks** (default off) → cap the height difference between
    neighbouring points, so hills can't get too steep or craggy.
  - **Defaults must look like the original:** at 20 / 20 / off the terrain
    should match v1.5's smooth 16–64-segment polylines. Compare side by side
    with the original in DOSBox.
- [ ] **"Classic terrain" option** (Landscape menu, default off): when on,
      ignore the three settings exactly as v1.5 does.
- [ ] **Random Land** (default off): each round set Flatten Peaks =
      `random(2)`, Bumpiness = `random(100)`, Slope = `random(100)`, as in
      the original.
- [ ] **Percent Scanned Mountains** 0–100 (20): how often to use a scanned ✅ **Q22**
      mountain picture instead of a random landscape. The `.MTN` format is now
      known (`MECHANICS.md` §17.1). Load `.MTN` files from a folder the player
      points to (don't ship the original's files), and PNG pictures as our own
      format.
- [ ] **Dirt colours**: pick one of several colour schemes at random each
      round (the original has 6).

**Done when:** every sky type looks distinct, lightning can damage a tank in a
Stormy sky, the default terrain looks like the original's, and moving each
landscape slider visibly changes the land (except in Classic mode).

## Batch 20 – Sound (M)
**Depends on:** 2

Sound menu *(p. 39)*. The original used only the **PC speaker**.

- [ ] Generate square-wave beeps in code (no audio files), turn them into WAV
      bytes in memory, and play them with macroquad's `audio` module.
- [ ] **Sound** on/off, default on.
- [ ] **Flight Sounds** — Off (default) / POS (pitch follows the shot's
      **height**; most people prefer this) / VEL (pitch follows its **speed**).
- [ ] Explosions, firing, deaths, menu clicks. 🟡 **Q23**

**Done when:** it sounds like a 1991 PC speaker.

## Batch 21 – Presentation polish (M)
**Depends on:** –

- [ ] Several tank **death animations** (the original has a variety). ✅ **Q24**
- [ ] **Bigger nuclear explosions** (the manual calls them "awesome"; check ✅ **Q24**
      what they look like in DOSBox).
- [ ] **Mouse control** as in the original *(pp. 5, 11)*: left/right click
      on "Power" or "Angle" to lower/raise it, click the weapon name to cycle,
      **both buttons together = fire**, click the player name for the Tank
      Control Panel (right click = inventory), and click a tank for its
      information.
- [ ] Optional extra (not in the original): integer-scaled fullscreen, and a
      choice between 640×480 and 320×200 internal resolution (the original
      supported several modes; 360×480 was its default).

**Done when:** you can play the whole game with only the mouse.

## Batch 22 – Saved games (S)
**Depends on:** 7

*(p. 35)*

- [ ] **Save Game only from the buying screen** (so the battlefield never needs
      saving), Restore any time from the System Menu. Ask before overwriting a
      file.
- [ ] As in the original, a save holds the players, money, inventory and round,
      but **not** the options. The options come from the settings file.
- [ ] Extra (not in the original): a hall of fame of best final scores.

**Done when:** you can quit after a round and continue later.

---

### Suggested order

0 → 1 → 2 → 3 → 5 → 6 → 7 → 8 → 9 → 4 → 10 → 13 → 20 → 19 → 12 → 14 →
15 → 11 → 16 → 17 → 21 → 22 → 18

Reasoning: first fix what the prototype already gets wrong, then build the
settings and options screens most batches rely on. The money → shop → tank
control panel loop is the heart of the original. Then come weapons and
defences, roughly by how cheap and fun they are. Sound and skies come early-ish
because they add a lot of the 90s feel. The hardest and least-used parts
(liquids, simultaneous play) come last.
