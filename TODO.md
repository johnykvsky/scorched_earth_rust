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

`game.rs` (round/turn flow, world state) · `earth.rs` (tunnels, riot and dirt effects) ·
`controls.rs` (battlefield panels/accessories) · `shields.rs` (shield state/forces/collisions) ·
`play.rs` (orders/team outcomes) · `system.rs` (paused System Menu) · `speech.rs` (phrase files) · `liquid.rs` (napalm, heat and flowing dirt) · `movement.rs` (driving and fuel) · `guidance.rs` (targeting, steering and Lazy Boy) · `energy.rs` (plasma bursts and laser beams) · `settings.rs` (session options) · `sound.rs` (generated beeps and playback) ·
`physics.rs` (shared ballistics, wind and settling rules) ·
`economy.rs` (cash, interest, awards and ranking) · `shop.rs` (buying, selling and market prices) ·
`weapons.rs` (`ITEMS` catalogue, projectiles, impacts, damage law) · `tank.rs`
(tanks and controllers) · `inventory.rs` (bounded persistent stock) · `ui.rs`
(menu input, HUD, dialogs, drawing) · `terrain.rs` (pixels, `carve`, `add_dirt`,
`settle_step`) · `ai.rs` (personalities, buying preparation, targeting and private prediction) · `gfx.rs` (drawing primitives).

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
      nasty target choice (batch 16). Shooter's original-style aiming is implemented in batch 16.
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

**Additional Start options (2026-11-17):** Player Setup has unchecked-by-default
Simplified weapons and Use default settings checkboxes. Simplified gives each
player unlimited Baby Missiles, 10 Missiles, 4 Rollers, 20 Tracers and 4 Dirt
Balls; no shops, cash awards, interest or market changes. Stock carries between
rounds. Standings and sorted turn orders use victories; completed matches skip
the cash hall of fame. Temporary defaults cover all seven option sections,
preserve the chosen roster/bindings/round count, and restore configured options
on returning to the menu. Save Changes never writes the temporary defaults.
Both switches are session-only and absent from in-game player reassignment.
Regression checks cover keyboard/mouse toggles, ten-player layout, all play
modes, ammo carry-over, team victories, Mass Kill, cash suppression, switching
back to normal play, early exit, completion, restart and checkpoint restore.

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
(PgUp/PgDn = ten steps). Walls are implemented in batch 4 below. Impact Damage
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
wind blindness; original-style aiming is implemented in batch 16. Tests cover physical units and trajectories,
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

- [x] Draw the walls as thin lines in those colours at the screen edges.
- [x] `ai::sim()` follows the walls (needed for Poolshark, batch 16).
- [x] Works for rollers and MIRV warheads too.

**Implemented:** Effect of Walls is active in the Physics menu and cycles in
the documented order, with reverse cycling and settings persistence. The
resolved wall type is shared by live projectiles, AI predictions, rendering and
`u` status. Random picks one of the six fixed types at round setup; Erratic
picks at turn preparation, before either human firing or AI planning, and
retains that choice through the shot, child warheads, hops and chain reactions.

Walls are drawn as one-pixel borders in the documented colours. The ceiling
for Concrete/Padded/Rubber/Spring is the visible playfield's top, below the
main HUD and optional status bar; side boundaries are x=0 and x=640. None and
Wrap-around leave the ceiling open. The bottom remains the ordinary terrain/
floor impact boundary. None alone uses Borders Extend.

The grid-walking projectile step resolves boundary contacts before checking
the next terrain/tank cell, retaining the time left in the step. Wrap-around
preserves height, velocity and overshoot, including negative crossings; shots
immediately check the opposite edge's dirt and tanks. Reflections reverse the
normal velocity component with factors 0.5 / 1 / 2 and retain the tangential
component. Concrete cannot be tunneled through and produces an explicit wall
impact, so a flying roller detonates instead of starting to roll on concrete.
Pre-apex MIRV/Death's Head wall impacts remain duds.

All three rollers wrap or reverse at the edge. Padded/Spring also adjust their
rolling tick speed, capped at 1000 pixels/s to keep repeated Spring bounces
bounded; Concrete detonates them and None lets them leave the field. Split
warheads and Leap Frog hops use the same active walls. Funky curves use wall
collisions too: a reflected curve continues as a ballistic child, with its
final blast still waiting for all children.

**Validation:** tests cover both side walls, ceilings/corners, reflection
factors, long-step wrap/bounce travel, opposite-edge terrain contact, all
currently playable weapons, all roller variants and repeated Spring bounces,
split warheads, Funky curves, concrete impacts/duds, Random/Erratic lifetimes,
status and HUD ceiling alignment, exact AI/live endpoints, menu cycling and
border colours. Framebuffer previews cover all six fixed wall types and the
Physics menu. Interactive play and side-by-side DOSBox checks remain manual.

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

The per-shot flag supplies the Contact Trigger override implemented in batch 14;
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
Free Market is implemented in batch 7 below.

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
Team assignment/gameplay is implemented in batch 17; its penalty relationship is shared with scoring.
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
purchases. Batch 7 adds inter-round shopping, selling, partial bundles and the full shop
controls; accessory use remains batch 8 onward, and buying personalities are implemented in batch 16.

Tests exercise ten full scoring rounds per mode, 1000-round interest growth,
penalties, fall timing, kill/pool payment guards, Greedy ranking, current prices,
opening purchase flow, menu input and settings persistence. Ten-player standings
and opening-purchase previews were visually checked.

**Done when:** money changes sensibly over a 10-round game in each mode.

## Batch 7 – Shop (L)
**Depends on:** 6

Buying screen *(pp. 18–21)*:

- [x] Between rounds, every player with cash shops in turn (computers too, if
      Computers Buy is on).
- [x] Left column: a list of items with icon, name and price per bundle. Two
      groups: **Weapons** and **Miscellaneous**, switched with Tab.
- [x] **Items you can't afford are hidden**, and reappear when you can.
- [x] Keys: Up/Down move the marker (wrapping), Home/End jump to the top or
      bottom, PgUp/PgDn change pages (wrapping), **Space buys one bundle**,
      `i` opens your inventory, `u` shows status (rounds left, interest
      earned), Esc = Done.
- [x] 99 limit: if a full bundle would go over 99, sell only the rest at about
      a 20 % markup per item.
- [x] **Selling**: in the inventory, picking an item opens a panel. `q` / 🟡 **Q9**
      Up raises the quantity, Alt-q / Down lowers it; the offer changes
      with the quantity; `a` accepts, `r` or Esc rejects. Usually sells at a
      loss. The offer only changes between rounds.
- [x] **Arms Level** 0–4 (Weapons menu, default 4): hides items above that
      level.
- [x] **Free Market** (default off): prices rise for items people buy and fall
      for ignored ones, "bouncing" a little. Saved between games (the original
      used `scorch.mkt`; ours uses `~/.local/share/scorched/market.toml`).
      🟡 **Q10**
- [x] **Useless Items** option (default off): hide items that do nothing with
      the current settings, e.g. Smoke Tracer when Trace Paths is on.

**Implemented:** the shop opens before round 1 with positive starting cash and
between rounds for each player with cash, including players killed in the last
round. Computers Buy controls bounded AI purchases in both flows; personalities
are implemented in batch 16. Interest is paid once after shopping, preserving batch 6's
opening-purchase order. There is no shop after the final round.

The paged Weapons / Miscellaneous lists support wrapping arrows and page keys,
Home/End, Tab, Space/Enter and mouse purchases. `i` opens the shop inventory,
where an item opens a quantity-based sale panel; `u` shows rounds left, cash and
interest earned. Selling uses the round's fixed prices at 80% (65% with Free
Market), rounded once per item quantity. Unlimited Baby Missiles cannot be
bought or sold. Partial purchases fill the remaining stock spaces at 120% of
the unit price, rounded to the nearest dollar, using exact cash arithmetic.

Arms Level, Free Market and Useless Items menu controls are active. Settings
filters hide redundant Smoke Tracers, parachutes when Tanks Fall is off, Earth
Disrupters when Suspend Dirt is zero and Contact Triggers when tunneling is
off. Useless Items enables those offers. All catalogue items can be stored and
sold; items awaiting later weapon/accessory batches are identified in the shop
as having their effect in a later batch. Their battlefield use remains gated.

Free Market tracks bundle purchases, applies Q10's recurrence once per completed
round using the Interest Rate, and keeps quotes fixed during each shopping
phase. Prices are bounded to $1–$4,294,967,295 per bundle. Prices, demand, ratio
and pending purchase counts are saved atomically to the platform's local data
directory (`~/.local/share/scorched/market.toml` on Linux). Invalid files report
an error and use base prices; failed writes report an error while preserving
in-memory changes. With Free Market off, base prices apply and the saved market
is left alone. Auto Defense's annuity pricing is implemented in batch 13 below.

**Validation:** tests cover a ten-round shop/round sequence, dead/cashless players,
Computers Buy, interest timing, partial bundles and stock caps, exact cash,
selling/rejection, affordability after selling, both category filters, page and
quantity controls, Free Market formulas and persistence/error handling.
Framebuffer previews cover buying pages, miscellaneous items, inventory,
selling and status. Interactive play and DOSBox comparison remain manual checks.

**Done when:** a 10-round game with shopping plays smoothly.

## Batch 8 – Tank Control Panel, status bar, inventory, parachutes, batteries (M)
**Depends on:** 7

How you use accessories during your turn *(pp. 10–17)*:

- [x] **`t` opens the Tank Control Panel** (a dialog): batteries, parachutes
      (state and threshold), shields (select and engage), triggers, guidance,
      fuel. Items you don't own are greyed out.
- [x] **`i` opens the Inventory Panel**: icons of everything you own. Picking
      one does the "obvious" thing: selects a weapon, energises a shield,
      toggles parachutes or triggers, selects guidance, uses a battery, or opens
      fuel/movement.
- [x] **Status Bar** option (Play Options, default off): a second HUD line,
      `Max: 1000  n[batt]  n[para]  n[shield] %  n[guid]  n[trig]  n[fuel]`,
      with keys `b` battery, `p` parachute toggle, `s` select shield, `e` engage
      shield, `g` guidance, `f` fuel, `-` trigger.
- [x] **Parachutes** *(p. 31)*: before a fall, estimate the damage. If ✅ **Q2**
      parachutes are **deployed** and the damage would be **more than the
      safety threshold** (default 5, range 1–100, `v` in the panel), use one and
      take no fall damage (unless you land on an enemy tank). Passive parachutes
      do nothing. Default: deployed.
- [x] **Battery** *(pp. 14, 31)*: +10 health (capped at 100), which raises max
      power. Batteries also power energy weapons (batch 12).
- [x] Other in-game keys *(pp. 12)*: `u` status (wall type, rounds left),
      `k` "No kibitzing, please." box, `1`–`9`/`0` info on tank N counted from
      the left, `r` **retreat** (confirm, then you escape in a helicopter while
      your tank blows up behind you: you get no points, but the enemy doesn't
      get points for killing you either).

**Implemented:** `controls.rs` owns modal Tank Control / Inventory panels,
shared accessory actions, threshold editing, information/status dialogs and
retreat confirmation. The same battery/parachute actions work from hotkeys,
panel rows, inventory and clickable status-bar fields. Owned inventory pages
wrap with arrows and PgUp/PgDn, with Home/End and mouse selection. Missing
supplies are greyed out in the control panel. Panels consume input and pause
aiming; closing one with Enter/Space cannot fire a shot. Panel key repeat is
independent of aiming's original timer rates.

The Play Options Status Bar control is active and saved with settings. The
second HUD row shows max power and supply counts/state, and wind moves below
it. Batch 13 now supplies active shield percentages and engagement.

Batteries heal 10 health up to 100 without being wasted at full health or on a
dead tank; the power ceiling rises without changing the chosen firing power.
Parachutes default to deployed, threshold 5. The `v` editor accepts 1–100,
with typing/arrows, Backspace, Enter to apply and Esc to cancel. Before a fall,
the game predicts the drop and consumes one chute only when deployed, stocked
and the predicted damage is strictly above the threshold. An open chute takes
20 ms per pixel independently of Falling Delay, prevents normal fall damage
in both damage modes, and is drawn over the falling tank. Landing on an enemy
still deals 2 × drop + 50 to the victim and drop + 10 to the faller. Damage
credit uses actual health lost. Round boundaries clear descent state while
keeping deployment/threshold preferences; new games reset them.

**Accessory implementation:** shield selection and physics are completed in
batch 13; guidance/trigger firing and consumption in batch 14; movement and
fuel consumption in batch 15. Inventory use of weapons still awaiting their
own batch reports that batch instead of consuming the item.

**Validation:** tests exercise shared panel/inventory/status actions, modal
input, battery limits, parachute deployment/threshold and timing, fall damage
and landing attribution, round preference persistence, retreat and framebuffer
rendering. Interactive play remains a manual check.

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

- [x] Add the missing ones: Leap Frog, Death's Head, Smoke Tracer, Baby Roller,
      Heavy Roller.
- [x] **Trace Paths** option (Weapons menu, default off): every shot leaves a
      trail in the owner's colour until cleared from the System Menu
      (batch 17).
- [x] **Bomb icon** option: Invisible / Small / Big (default Big).
- [x] **Scale** option: Normal / Medium / Large, making all blasts bigger. 🟡 **Q25**

**Implemented:** Leap Frog, Death's Head, Smoke Tracer, Baby Roller and Heavy
Roller now have usable behaviours and can be bought, selected from inventory
or the weapon cycle, and fired at the catalogue's existing prices/bundle sizes.
Future weapons remain guarded. Basic AI can choose held Leap Frogs and the
new heavy/roller weapons; its self-blast distance guard now includes Scale.
Buying personalities and weapon-specific aim are implemented in batch 16.

Leap Frog queues each hop only after the previous blast finishes and carves
its crater: radii 30 → 25 → 20, speed divided by 1.5 per hop, with upward
relaunch and the original weapon damage index. MIRV produces five Missile
warheads; Death's Head produces nine radius-35 warheads with its original
damage index. Both preserve the central projectile's position, velocity,
age, owner, tunneling and scale; before-apex impacts remain harmless duds.

Funky Bomb's old six ballistic bomblets are replaced by a radius-20 opening
burst, 5–10 coloured curved trails aimed within −80…+79 pixels and clipped to
the field, radius-15…24 landing bursts, then a final radius-40 × Scale blast.
Trails stop at dirt/tanks/edges and do not tunnel. The opening blast carves
before trails launch, and the final blast waits for all trails/landing bursts.
The turn waits for the entire reaction; all stages use one fired item.

All three rollers choose the downhill direction, move/fall one pixel per
150 Hz rolling tick and detonate at an uphill obstruction/valley or tank,
including their owner. Their blast radii are 10/20/45, with original damage
indices. Walls are implemented in batch 4; shield contact/rolling off shields
is implemented in batch 13 below.

Weapons menu Bomb Icon, Scale and Trace Paths controls are enabled and persist
in settings. Invisible draws no shot icon, Small is one pixel, Big is a small
bomb with a fuse. Scale is snapshotted per shot: Normal/Medium/Large = 1/2/3
for missiles, Leap Frog, MIRV/Death's Head and rollers. Dirt spheres and Funky's
opening/trail bursts retain their fixed radii; its final burst scales. This
makes the currently implemented family's scaling policy explicit; the sources
only establish some family-specific exceptions (Q25), so exact DOSBox fidelity
of the remaining family choices is still a manual research check.

Trace Paths retains owner-coloured paths in a bounded framebuffer-sized pixel
layer. Smoke Tracer persists independently of that option; ordinary Tracer and
Funky effects have temporary paths for the current shot. With Trace Paths off,
ordinary ammunition leaves no path. Both tracers cause no damage or terrain
changes. New shots clear temporary effects, while round setup clears all paths.
The shared clear action is wired to the System Menu in batch 17.

**Validation:** tests cover shop/use/fire integration for all five new weapons,
MIRV/Death's Head counts, central trajectories, damage indices and duds, ordered
Leap Frog hops, complete chain-to-next-turn flow, Funky's ranges and final-blast
guard, roller descent/obstruction/owner collisions, scale inheritance, harmless
tracers, trace persistence/clearing/owner colours, active menu options and all
three bomb icons. Framebuffer previews cover Funky trails, Death's Head,
Smoke Tracer, a scaled blast and the icon modes. Interactive play and side-by-side
DOSBox checks remain manual validation.

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

- [x] **Extra Dirt** option (Weapons menu, default off): Dirt Charges let out 🟡 **Q13**
      much more dirt.
- [x] Riot weapons and diggers hurt tanks only through **fall damage**.
      Sandhogs additionally use their documented terminal charges/contact chip.

**Implemented:** all fourteen previously unavailable items in this batch are
buyable, selectable and usable at their catalogue prices/bundles/arms levels.
`earth.rs` owns animated erosion, branching tunnel heads, dirt grains and the
Earth Disrupter pulse. These effects keep the shot resolving until complete;
normal settling then follows the shot's Suspend Dirt choice. Extra Dirt is
active in the Weapons menu and persists with settings.

Riot Charge/Blast consume one item and remove a turret-centred wedge immediately,
without launching a projectile: radius 36/60, half-angle 45°/60°, directed by
the barrel. Riot Bomb/Heavy Riot Bomb remove radius-30/45 circles on impact.
These weapons never use direct-hit kills or explosive health damage; only
subsequent falls hurt tanks, credited to the action's owner. Diggers similarly
fizzle on direct tank hits and excavate without explosive damage.

Digger births are bounded to 20/40/70; Sandhogs to 10/20/35. At most 20 heads
are live per swarm, branching every eight ticks, with headings reconsidered
at twenty-step intervals. Heads choose neighbouring soil, never targets.
Sandhogs chip 10 on direct contact and finish with radius-10 × Scale charges,
using the documented rounded radial damage and original weapon index. Their
underground damage has a separate `Damage::Sandhog` type for the shield-bypass
contract; batch 13's regression checks now validate that these charges bypass
an engaged Super Mag and can kill its owner from below.

Dirt Clod/Ball/Ton of Dirt grow fixed radius-20/35/70 spheres. They deposit
terrain as they grow without lifting tanks or causing direct health damage.
Dirt Charge follows a normal contact trajectory, then sprays upward dirt
grains that land and deposit soil, respecting the active walls. Extra Dirt
emits four times as many grains. Earth Disrupter runs a 100-tick flash sequence,
marks all terrain for collapse and overrides Suspend Dirt for that action,
even at 100. It gives no direct damage or points; PC-speaker audio is now supplied by batch 20.

**Explicit approximations where the sources remain incomplete:** tunnel heads
run at 120 Hz, clear a radius-one corridor in two-pixel steps and expire after
120 steps; births can end early if no soil is reachable. Dirt Charge uses
80 grains, or 320 with Extra Dirt, each depositing a radius-two patch. Launch
heights range from 1/12 to 1/3 of screen height before drag/wind, with an
eight-second lifetime and a bounded speed for reflected grains. Those particle
counts, tunnel timing/width/lifetime, and exact Extra Dirt multiplier are local
policies, not recovered original constants. Riot and dirt-sphere radii remain
unscaled; Sandhog terminal charges scale. Further DOSBox comparison can refine
these choices without changing the bounded effect lifecycle.

**Validation:** tests cover wedge geometry and buried-tank escape, all new
shop/inventory/fire paths, harmless riot/digger contact, actual fall-damage
credit, digger fizzles, head/birth bounds, finite effect completion, Sandhog
contact/terminal damage, growing non-lifting spheres, Extra Dirt quantity and
deposition, extreme-gravity/wind spray under every fixed wall, Disrupter's
forced collapse at Suspend Dirt 100, menu persistence and turn advancement.
Framebuffer previews cover wedges, riot circles, diggers, Sandhogs, normal and
extra dirt spray, dirt spheres and disruption flashes. Interactive play and
DOSBox fidelity checks remain manual validation.

**Done when:** a buried tank can dig itself out, and sandhogs can kill a
shielded tank from below.

## Batch 11 – Liquids: Napalm, Hot Napalm, Liquid Dirt (L)
**Depends on:** 5, 9

| Weapon | Cost/bundle | Arms | Behaviour |
|---|---|---|---|
| Napalm | $10,000/10 | 2 | splashes, then burns; **deep pools burn hotter** 🟡 **Q14** |
| Hot Napalm | $20,000/2 | 4 | much hotter and stronger |
| Liquid Dirt | $5,000/10 | 2 | oozes out, fills holes and smooths terrain (clears a path for Rollers) |

- [x] A small liquid simulation (new `liquid.rs`): particles or cells that flow
      downhill and pool.
- [x] Napalm damage depends on pool depth. Shields don't protect you from your 🟡 **Q14**
      own napalm's heat, though they do protect you from other players'
      secondary damage *(p. 31)*.
- [x] With Tunneling on, napalm that burrows into dirt can **fizzle**
      *(p. 24)*. Contact Triggers prevent that.

**Implemented:** all three weapons are buyable, selectable and usable at their
catalogue prices/bundles/arms levels. `liquid.rs` owns a deterministic pixel
fluid shared by all three. Napalm has 15 emitters; Hot Napalm and Liquid Dirt
have 20. Across active pools there are at most 100 queued fluid cells. Cells
fall, flow diagonally and follow short level runs toward a lower ledge; fluid
occupancy makes them spread and pool rather than pass through each other.

Napalm pools settle and grow flames, then run exactly 50 burn cycles. Heat is
summed per emitter, increasing with its fluid-column depth; Hot Napalm emits
more fluid and deals twice the per-emitter heat with taller flames. Enemy
shields absorb heat before overflow reaches health. Damage uses actual health
removed, normal self/team/enemy scoring and kill attribution. Burning itself
neither carves terrain nor creates a standard blast. Death blasts still use
the normal death pipeline after all liquid effects finish.

**Source discrepancy:** this batch explicitly requests that your own napalm
heat bypass your shield, while MECHANICS §8.3 says the original absorbs own
secondary heat. Implementation follows this batch's own-heat bypass requirement;
the original behavior still needs DOSBox verification. Regression tests make
the chosen rule explicit rather than silently changing the mechanics notes.

Liquid Dirt deposits one pixel whenever a fluid cell comes to rest, filling
holes without direct health damage or lifting tanks. Deposited soil participates
in normal terrain support and post-shot settling. Projectile contacts route
straight to the liquid system, with no direct-hit kill or ordinary explosion.
The existing guidance, walls and shield interception still govern the carrier.
A Napalm/Hot Napalm carrier that stops underground with tunneling enabled
fizzles; an armed Contact Trigger or non-ballistic guidance prevents entry into
soil. A carrier that traverses a thin layer and emerges may still spill.

The turn waits for emission, flow, growth and all burn cycles before settling
terrain and advancing. Round setup clears liquids. Splash, burning and dirt
sounds use the existing bounded, non-blocking PC-speaker scheduler.

**Explicit local tuning pending DOSBox comparison:** fluid steps at 120 Hz;
each source emits every four steps, with 32/48/64 pixels per emitter for
Napalm/Hot Napalm/Liquid Dirt. Sources span 28/38 pixels around impact. Level
flow looks ahead up to eight pixels. Transport expires after ten simulated
seconds; any unsettled Liquid Dirt remainder then dissipates. Flames grow for
eight cycles, followed by 50 damage cycles at 20 Hz. Per-emitter heat is
`min(6, ceil(depth/4))`, doubled for Hot Napalm, with a bounded depth-based
reach. Fluid flow ignores wind/viscosity; edge flow wraps only with Wrap,
otherwise it is contained. Eight concurrent pools and emitter budgets bound
storage. Fluid volume, timings, heat formula, flame geometry, colours, sounds
and deterministic underground fizzle policy are local choices, not recovered
original constants. Liquids do not use explosion Scale.

**Validation:** tests cover valley flow/pooling/depth and all 50 burn cycles,
Hot Napalm's stronger heat/volume, per-emitter accumulation, own-shield bypass,
enemy absorption/overflow, harmless non-lifting Liquid Dirt fill, fixed-step
frame-size equivalence, queue/storage/field limits under all wall types,
finite fast-forward completion, real purchase/selection/fire/impact/turn flow,
actual damage/kill/self-damage/suicide awards, direct-contact harmlessness,
shield carrier interception and live tunneling fizzle prevention with triggers
and Heat Guidance. `SCORCHED_LIQUID_PREVIEW_DIR` with `cargo test liquid_previews`
exports splash, pool, burn and filled-hole framebuffer scenes. Interactive play
and DOSBox comparison remain manual validation.

**Done when:** napalm in a valley pools and burns the tank sitting there.

## Batch 12 – Energy weapons: Plasma Blast and Laser (S)
**Depends on:** 8

*(pp. 23, 27)*. Energy weapons use **batteries**: after firing you choose how
many to spend (keys 0–9); no batteries means a weak or no effect.

- [x] **Plasma Blast** ($9,000/5, radius 10–75 depending on batteries, arms 3): 🟡 **Q15**
      radiation all around your tank, hitting neighbours. The turret angle
      doesn't matter.
- [x] **Laser** ($5,000/5, arms 2): a straight beam along the barrel that 🟡 **Q15**
      cuts through mountains, **shields**, and anything else in its way.
- [x] Guidance doesn't work with Plasma Blast *(p. 29)*.

**Implemented:** both energy weapons are buyable and selectable. Space/Enter
opens "Batteries to discharge:" before spending ammunition. Type 0–10 or use
arrows; Enter/Space confirms, Esc returns to aiming without spending anything,
and Backspace restores the default (up to ten owned batteries). The charge is
clamped to available stock. Confirmation consumes one weapon and the selected
batteries, preserving guidance and Contact Triggers. AI discharge uses up to ten
owned batteries without opening a prompt; choosing energy weapons intelligently
is implemented by batch 16's AI strategy.

Plasma radiates from the owner's tank, excludes its owner, ignores the turret
angle and terrain, and never carves dirt. Radius interpolates from Missile 20
to Nuke 75 over zero to ten batteries (the table/manual's lower value 10 is not
used; Q15 and MECHANICS §7.3 identify the Missile radius). Enemy shields absorb
first with overflow to health. It uses the standard weapon-index damage law.

Laser traces the barrel direction in one-pixel samples, spending 40 energy per
sky pixel and 80 per dirt pixel, making narrow cuts and applying 10-point pulses
to encountered shield/body pixels. Regular shields drain and can be pierced;
Super Mag stops the beam and converts its remainder to charge capped at 200,
without awarding damage money. The owner's shield is excluded. Both effects
feed actual damage into the normal economy/death pipeline, have generated sound
cues and bounded visuals, and hold the turn until the animation finishes.

**Explicit local policies pending DOSBox comparison:** laser starts with
`power × batteries` energy (zero batteries gives no beam), one-point dirt cuts,
one pulse per sampled shield/body pixel, and converts remaining energy at
40 energy per shield point. Plasma's radius interpolation, use of the normal
damage multiplier, 0.5-second rings and the laser's 0.3-second visual are local
choices; energy weapons do not scale, bounce or wrap. Generated sounds are
approximations. These tuning details are not claimed as recovered constants.

**Validation:** regression tests cover purchase/selection/fire, cancellable and
stock-limited battery input including zero/ten, guidance/trigger preservation,
AI discharge, charge/power/dirt-dependent reach, owner exclusion, plasma's
unchanged terrain and shield overflow, laser pulses/penetration, Super Mag
conversion/cap, field bounds, actual damage awards and completed turn flow.
Framebuffer previews are available with `SCORCHED_ENERGY_PREVIEW_DIR` and
`cargo test energy_previews`. Interactive play and DOSBox comparison remain
manual checks.

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

- [x] **Your own shield never protects you from your own shot**. Fire straight
      up with no wind and you die, and lose points.
- [x] "Shield failures" for lesser shields ✅ **Q16**: per the reverse
      engineering there is **no random failure**. A shield simply collapses when
      a hit is bigger than its remaining strength, and the rest of the damage
      goes to the tank.
- [x] Draw shields as a circle around the tank that flickers when hit.

**Implemented:** `shields.rs` centralises the five original shield specifications,
active strength, colours, magnetic force, interception/reflection and drawing.
Mag/Shield/Force/Heavy/Super Mag start with 55/100/100/150/200 points and outline
radii 16/15/15/16/16. `s` selects owned types plus None; `e` consumes one selected
shield to engage/replace it. Inventory selection engages that specific type.
Replacement never returns the old item; missing stock leaves the active shield
alone. Active shields are cleared at round setup, while unused stock persists.

Direct enemy hits stop at a plain/Heavy/Mag shield for a 10-point chip and no
explosion or follow-up reaction. Force/Super Mag reflect by the mirror-normal
formula at 70% speed, costing rounded incoming speed/100. Costs award points
only for actual shield strength removed; direct chips/reflection costs never
overflow into health. Blasts absorb available strength, then overflow to tank
health. Enemy shield damage earns +2 per point in Standard/Greedy, with existing
Basic and friendly-fire rules preserved. There are no random failures.

Own projectiles ignore the owner's shield and magnetic field; own weapon
blasts bypass absorption too. Falls and underground Sandhog damage bypass all
shields. All three rollers traverse an enemy shield outline downhill and drop
off its side without chipping it. Funky direct interception creates no chain;
reflected MIRV/Death's Head carriers still split when reflection forms an apex.
AI prediction uses the same contacts/forces with privately cloned shield
strength, leaving live tanks unchanged.

Mag/Super Mag affect non-owner airborne shots with horizontal velocity within
±15 pixels and up to (height−1)/4 above the tank, with no speed cutoff. The
fixed-step implementation translates the recovered reciprocal-step force to
2500 pixels/s² upward acceleration; this normalization needs exact side-by-side
DOSBox tuning, while keeping trajectories independent of animation delay.
Super Mag also has Force reflection and the laser-immunity property ready for
batch 12; actual laser energy conversion is integrated with that future weapon.
The 1000 Hz hit beep is now supplied by the PC-speaker audio batch 20.

Shield circles fade using integer VGA-channel strength scaling and flicker on
hits. Remaining percentage appears in both the main HUD (even with Status Bar
off) and the status row; the control panel shows active type/percentage.

Auto Defense is a one-token whole-game entitlement: the shop hides repeat
purchases, and using it does not consume it. Price uses the current fixed/market
base times `((1+r) − (1+r)^−rounds_left) / r`, with the zero-rate limit
`base × (rounds_left + 1)`. Quotes round to the nearest dollar, remain bounded
by the market's u32 price ceiling, and are recomputed from the raw base instead
of compounding an adjusted price. Selling the token removes the entitlement.
Before each round, human owners receive a defensive-only control/inventory
phase; no offensive actions, movement or retreat are available. Setup visits
each owner before the previously chosen first shooter begins. Computer owners
use their best held shield under a bounded starter policy; full AI accessory
strategy is implemented in batch 16.

**Validation:** tests cover the full specification table, replacement/stock,
direct chips, reflection geometry/speed/cost, shield-only scoring and overflow,
Basic scoring, own returning shots and penalties, magnetic bands and slow/fast
shots, no random failure, private AI predictions, roller traversal, a complete
nearby Leap Frog chain, nuke overflow, Sandhog/fall bypass, HUD percentages,
VGA fading/flicker, annuity limits and quotes, purchase-once ownership and
human/computer multi-round defensive setup. Framebuffer previews cover all
shield types, partial strength, hit flash and Auto Defense panels. Interactive
play and exact DOSBox comparison remain manual checks.

**Done when:** each shield behaves differently, and a Leap Frog or a nearby
nuke beats a plain Shield (the manual's strategy tip, *p. 32*).

## Batch 14 – Guidance, Lazy Boy, Contact Trigger (M)
**Depends on:** 5, 8

*(pp. 28–32)*. Choose guidance with `g` before firing. After the shot it
**resets to None**, so expensive guidance isn't wasted by accident. Weapons that
can't be guided (MIRV, Death's Head, Riot Charge, Riot Blast, Plasma Blast)
leave the guidance unused and not used up.

- [x] **Choose target** prompt, after Space, for guidance that needs it: press
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

**Implemented:** `guidance.rs` owns target selection helpers, the wind-aware
ballistic solver and per-projectile steering state. `g`/Alt-g cycles owned
guidance as before. Ballistic, Horizontal, Vertical and Lazy Boy open a
"Choose Target" phase before any ammo/accessory is spent. Digits select tank
positions from the left (0 is tenth); eliminated positions keep their numbers
but cannot be selected. Left click chooses any battlefield point; right click
chooses the nearest living tank base within 100 pixels. The existing viewport
mapping handles scaled/letterboxed windows. Esc returns to aiming with stock,
settings and the active wall choice intact. Weather pauses during the prompt.

Launch consumes one applicable guidance item and resets selection to None.
Heat needs no prompt: it locks onto the first live non-owner tank within 40
pixels, including teammates, tracks it beyond lock range and reacquires if it
dies. Ballistic uses the current barrel direction and wind/gravity to solve
power, ignores viscosity and clamps impossible shots to health-limited power.
Horizontal engages at the target row; Vertical at the target column. They then
steer along that axis and detonate at the exact selected point. Steering uses
the recovered 10000/15000 constants and small-distance guard. All ordinary
flight still uses the shared pixel/wall/shield collision code.

Lazy Boy travels directly toward the point at 500 pixels/s, excavating a narrow
path without drag, gravity, wind or shield interception/magnetic push. It reaches
the exact clicked point (including inside the chosen tank), but a different
tank causes an early impact. It feeds the normal weapon impact, scoring and
death pipeline. Guidance ends when a shot becomes a roller, shield traversal,
Leap Frog follow-up or Funky child effect. MIRV/Death's Head and immediate riot
weapons preserve guidance; non-projectile energy weapons and Earth Disrupter
also preserve it under the current implementation's projectile-only policy.
AI launch can choose a live target without a human prompt; full accessory
selection strategy is implemented in batch 16.

An armed Contact Trigger consumes one item per unguided/Ballistic projectile
shot and snapshots tunneling off. Children and hops inherit that flag without
spending more stock. Exhaustion disarms triggers; subsequent shots return to
the configured tunneling rule. Non-ballistic guidance already forces contact
and does not spend a trigger. Immediate/energy effects do not spend triggers.

**Local policies pending DOSBox comparison:** Horizontal/Vertical activation
uses a one-pixel/crossed-axis test and suppresses air forces after engagement
so the final leg stays aligned. Lazy Boy uses a straight digging route rather
than an unrecovered obstacle-routing routine; its one-pixel corridor, intended
tank arrival handling and field-boundary cutoff are explicit local choices.
Ballistic's analytic solver ignores walls, intervening dirt and shields; it
solves the requested angle, not a different route. Exact original trajectories
and compatibility of guidance with non-projectile effects remain research.

**Validation:** tests cover wind/drag/impossible ballistic solutions, heat lock
ordering/range/team inclusion/reacquisition, axis engagement and exact midair
arrival, Lazy Boy soil/shield bypass, early tank contact and exact chosen-tank
arrival, prompt cancellation/invalid choices, keyboard/mouse/tenth-tank mapping,
consumption/reset/exclusions, real first-contact soil collision, all MIRV child
flags, return to normal tunneling, trigger preservation under guidance and
completed live damage/death/turn flow. `SCORCHED_GUIDANCE_PREVIEW_DIR` with
`cargo test guidance_previews` exports the targeting prompt and all five paths.
Interactive play and side-by-side DOSBox checks remain manual validation.

**Done when:** each guidance type visibly changes the shot's path.

## Batch 15 – Fuel, movement, tank icons (M)
**Depends on:** 8

*(pp. 8, 16, 32)*

- [x] **Fuel Tank** ($10,000/10, arms 3): each one gives 10 fuel. `f` opens the ✅ **Q18**
      move panel; Left/Right move one pixel per fuel unit, **more on uphill
      slopes**. Hills that are too steep can't be climbed; going down too steep
      a slope makes you **slip and fall** (normal fall damage), and the turn
      goes back to aiming. Esc ends moving.
- [x] **Tank icon choice** in player setup: several tank shapes. Icons
      **without wheels or treads are fixed emplacements**: they can't buy or
      use fuel.
- [x] **Triple-turreted tank** (registered version and computers only in the
      original): fires 3 shots at a time with Missiles or Baby Missiles.

**Implemented:** `movement.rs` validates one-pixel driving steps before spending
fuel. `f`, the control panel, battlefield fuel inventory and the clickable fuel
status field all open the Move panel. Left/Right repeats through the shared
menu-key timing. Esc/D returns to aiming; Space/Enter never fires from Move.
Each Fuel Tank contributes ten units: unopened tanks remain in inventory and
only the opened tank's remainder (0–9 units) is stored in equipment. The status
bar, inventory fuel row and Move panel show units available. Unopened stock
and the remainder carry between rounds; a new game resets them.

Each step costs one unit plus its 1–3 pixel uphill rise. A four-pixel obstacle,
insufficient fuel, another live tank or a field edge blocks movement without
spending stock. Downhill drops up to three pixels are free beyond the step's
one-unit cost. Larger drops enter the normal fall/parachute animation; damage,
landing collisions, attribution and death chains use the existing rules. A
surviving driver returns to aiming with the same turn and wall choice, including
when another tank dies. Exhausted fuel closes Move. Tanks Fall off retains the
existing no-fall behavior. Weather and aiming are paused during movement.

Player Setup has seven drawn silhouettes: Wheels, Treads (the existing default),
Heavy, Bunker, Tower, Dish and Triple. Tab changes the selected player's shape;
Alt reverses; clicking the shape at the right also cycles it. Existing arrows
and number hotkeys still change controllers. Shapes are saved with settings and
used when new tanks are created. Bunker/Tower/Dish are fixed: all purchase paths
reject fuel even with Useless Items on, and all movement paths reject them.
Triple is available only to computers; changing to Human resets it to Treads,
and invalid human/triple configurations are rejected on load/save.

Triple fires three simultaneous Baby Missiles or Missiles, sharing one weapon
item, one applicable guidance item and one armed trigger. All three inherit the
shot's tunneling/guidance state. Other weapons remain single shots; normal shot
resolution waits for every projectile and its effects.

**Local presentation/geometry choices pending DOSBox comparison:** the seven
silhouettes are original drawings, not recovered icon bitmaps. All retain the
existing 15×11 collision envelope. Driving uses footprint clearance and contact
at any supporting wheel/track pixel; ordinary three-pixel support settling
resumes on exit. This avoids treating the first pixel of an uphill step as a
slip. Triple's ±5° fan and one-ammo-per-volley policy are explicit local choices;
the sources only establish that it fires three shots. AI driving decisions
are implemented in batch 16's accessory strategy.

**Validation:** tests cover flat/reverse driving and opened-tank accounting,
uphill costs and four-pixel blocks, downhill steps/slips, insufficient fuel,
field/tank collisions, crater escape without terrain edits, all four movement
entry paths, fixed fuel restrictions, retained turns/stock/walls, parachutes,
lethal falls and a surviving driver's return after a landing kill, stock/shape
persistence, controller/shape cycling and settings validation, triple projectile
counts/spread and shared ammo/trigger spending. `SCORCHED_MOVEMENT_PREVIEW_DIR`
with `cargo test movement_and_shape_previews` exports the seven silhouettes,
ten-player setup, Move panel and triple volley. Interactive play and DOSBox
comparison remain manual checks.

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

- [x] Rework `ai.rs` into these 8 types. Tosser needs memory of its last shot
      per target.
- [x] The computer **picks its own name**, never the same as a human's *(p. 7)*.
- [x] Buying (if Computers Buy is on) and using items: shields, parachutes,
      batteries, Riot Charge to dig out. 🟡 **Q20**

**Implemented:** all eight types are selectable and saved through Player Setup.
`ai.rs` now owns target/method selection, bounded private trajectory prediction,
per-target ranging state, hidden Unknown resolution, names and supply use.
The live loop animates the chosen angle/power before firing as before.

Moron uses health-quantized random power, points toward a side with enemies and
chooses a random owned weapon. Shooter chooses the nearest live enemy and
searches one-degree changes around the line-of-sight angle with a closed-form
power solution. Spoiler selects a random live enemy, starts at 65°/115° and
searches its side's angles for a wind-corrected solution. All smart power
solutions ignore drag; Shooter/Poolshark retain wind blindness. Cyborg uses
Spoiler's solver with the documented wind/viscosity-dependent starting angle
and scores weakened, winning and previously hostile enemies. Dead tanks and
teammates are excluded from deliberate targeting.

Poolshark tests real side-wall and ceiling banks with the shared wall physics
on Rubber/Spring, including Spring's speed changes. Chooser uses Shooter for a
clear sightline, Poolshark behind obstructions with Rubber/Spring, otherwise
Spoiler. Private prediction copies shield strength and terrain erasures, so
planning cannot damage live state. Tosser never uses this aiming search: its
first shot is a random ranging lob; later shots adjust once from a real landing
by ±10 power and, when short/high, two degrees toward vertical outside 85–95°.
Ranging history is per target. The first terminal impact/interception/exit of
a shot supplies its landing; later child impacts cannot overwrite it.

Each tank has AI state. A new Unknown rolls one known personality once; its
controller label remains Unknown in setup and information panels. Round setup
clears ranging history but preserves personality and grudges. Enemy damage
(including shield damage) records the attacker; enemy kills add the known +5
per-victim counter. Environmental/self/team damage does not add retaliation.
New games reset state. Computers choose unique names from our own name pool,
avoiding all human names case-insensitively; names survive round boundaries.

With Computers Buy enabled, Moron makes at most 32 affordable weighted-random
purchases. Other types prioritize an affordable best shield, parachutes,
Missiles to at least five, batteries, Riot Charge and triggers, then bounded
weapon/guidance/fuel/Auto Defense purchases. All buying goes through the normal
shop, so prices, stock limits, arms/usefulness filters, fixed tank restrictions
and market demand remain authoritative. Computers Buy off prevents purchases
but never prevents using already-owned supplies.

Before an AI plans, it heals with batteries while health is at most 90, deploys
parachutes and equips an owned shield only if it improves current strength.
Buried barrels prefer owned Riot Blast/Charge. Guidance and triggers are selected
from owned stock; target-requiring guidance receives the planned victim instead
of the first tank. Close, clear targets can prompt Laser/Plasma use; owned
napalm is considered only with surface-contact delivery. A mobile AI with fuel
and an obstructed sightline may drive up to eight pixels toward its target.
Slips use the ordinary fall pipeline and resume the same turn without another
supply/movement pass. Auto Defense uses the same preparation policy.

**Explicit local heuristics pending DOSBox comparison:** Shooter's search is
limited to ±15°, other lobbers search their facing half-plane, and each planning
probe stops after twelve simulated seconds. Bank search uses mirrored side-wall
targets plus a bounded angle/power sweep and local refinement, including ceiling
banks; it does not guarantee every possible bank. Weapon prediction follows a
Baby Missile to first impact, not every family-specific child/effect. Cyborg's
health/wins/kills/cash-rank/retaliation weights, damage-based grudge increments,
weapon preferences, eight-pixel driving rule, supply thresholds, random-buy
weights and the extended fixed shopping order are local policy. The recovered
notes establish the buying priority skeleton, not every stock cap or ordering.
Tosser remembers the first terminal child result when using multi-stage weapons.
These choices replace the prototype oracle; they are not claimed as a complete
instruction-for-instruction recreation of the original AI.

**Validation:** tests cover legal shots for all personalities, Moron's power and
side constraints, open Shooter hits versus blocked ridges and successful Spoiler
lobs, Rubber/Spring and actual ceiling banks, Chooser's method selection, Tosser's
real per-target corrections and first-impact recording, Cyborg target signals,
Unknown stability/secrecy, unique names, defensive stock use, bounded fuel/slips,
Riot escape, planned guidance targets, complete live AI turns, buying-off behavior,
priority/stock/arms limits and exact market spending, plus private prediction and
the retained viscosity weakness. `SCORCHED_AI_PREVIEW_DIR` with `cargo test
ai_previews` exports all personalities' paths and the nine-controller menu.
Interactive games and side-by-side DOSBox comparison remain manual validation.

**Done when:** each type plays recognisably like its description.

## Batch 17 – Play options, teams, System Menu (M)
**Depends on:** 6, 8

Play Options menu *(pp. 47–48)* and System Menu *(pp. 33–35)*:

- [x] **Play Order** — Random (default: random order **each turn**) /
      Losers-first / Winners-first / Round-robin (random order, but the first
      shot of each round moves on one player).
- [x] **Teams** — None / Standard / Corporate / Vicious *(p. 52)*:
  - Standard: teams work together; you lose points for hurting teammates.
  - Corporate: like Standard, but **one shared bank account** per team.
  - Vicious: like Standard while two or more teams are alive, then a
        free-for-all inside the last team.
- [x] **Fast Computers** (default off): speeds up the game while only
      computers are fighting each other.
- [x] **Talking Tanks** — Off (default) / Computers / All, and **Talk
      Probability** 0–100 (default 100). Attack and death phrases come from
      text files: **one phrase per line**, repeated lines are said more
      often, and **blank lines mean silence** *(p. 48)*. Use the same format so
      players can load their own `TALK1.CFG`/`TALK2.CFG`; keep our own phrases
      as the defaults.
- [x] **System Menu** (F1 or Alt-S, any time): Clear Screen (traces),
      **Mass Kill** (ends the round, splitting the remaining points), ✅ **Q6**
      Reassign Players (change name or controller), Reassign Teams, Sound
      on/off, Save Game, Restore Game, New Game, Quit (with confirmation).
- [x] Player setup screen *(p. 6)*: the background turns the player's colour,
      you type a name, Tab switches between human and computer, choose a tank
      icon, Enter = Done.

**Implemented:** all batch-17 Play Options are active. `play.rs` owns order,
team outcomes and round payouts. Random selects a random other living player
for each shot; Losers/Winners-first use cash (net worth in Greedy) at round
start and cycle that order. Round-robin shuffles a circle once per new game
and rotates its first shooter one position each round. Player identities and
inventories remain stable while battlefield positions and order change.

Team assignment is editable in the coloured player editor and System Menu.
The default assignment alternates teams 1/2. Standard/Corporate end when one
team remains, including when it has multiple survivors. Winning living members
have weight two and dead teammates weight one for the normal survival pool;
all winning team members receive a round win. Vicious clears runtime teams when
the last team remains, making its members enemies until an individual wins;
configured memberships return next round. AI deliberate targeting and ordinary
friendly-fire damage/kill penalties use the active relationships.

Corporate combines each member's starting cash into a shared team balance.
Damage, kills, penalties and round awards synchronize that balance; purchases
and sales synchronize before another shopper can act. Interest is paid exactly
once per shared bank each round, and its Update total is shared too. Inventory
remains individual. Corporate standings count each bank once and aggregate its
members' inventory only for Greedy worth. Reassigning teams first splits old
shared funds equally (giving the last member any remainder), then merges new
banks. This is an explicit local regrouping policy that preserves every dollar.

`system.rs` provides a true paused overlay for F1/Alt-S in all phases. World,
shot, falling, weather, speech and thinking timers stop until resume. Clear
Screen clears traces; Sound toggles immediately. Mass Kill, New Game and Quit
require confirmation. Mass Kill pays the exact remaining-health/shield/kill
pool in MECHANICS §10.2, stops every pending effect, drops all shields and marks
every tank dead, then ends the round without ordinary survival or kill awards.
It cannot pay a second time. Reassign Players/Teams use staged edits; Esc
cancels, and applying controller changes resolves human-only targeting, battery,
movement or defensive prompts rather than leaving a newly assigned AI waiting.
**Save/Restore entries are deliberately disabled until batch 22**, which owns
file persistence and the buying-screen-only save rule; no save support is
claimed in this batch. Alt-S now belongs to System Menu; Shift-S reverses
shield selection, and the control-panel Left arrow remains available.

Fast Computers uses a local six-times time scale during battles with at least
two living computer players and no living humans; menus, buying, defensive
setup and standings retain normal speed. Ballistics still use fixed simulation
steps. Talking Tanks now obeys Off/Computers/All and the exact 0–100 probability.
`speech.rs` loads optional UTF-8/ASCII attack/death files relative to the settings
folder. Repeated lines retain their weight and blank lines mean silence;
empty files are silent. Missing/invalid/oversized files report a notice and
use our own built-in phrases. No original speech assets are included.

Player Setup retains the overview's quick controller/icon selection and adds
Enter/N to edit a player on a background in that player's colour. Names accept
1–16 printable characters; typing replaces the initially selected name. Tab
switches human/computer, arrows choose controller/icon/team, Enter applies and
Esc cancels. In-game Reassign Players edits names/controllers without replacing
stock, health or identity. Human-to-computer reassignment chooses a fresh
computer name unless the user explicitly edited it. Names, icons, controllers,
team IDs and Play Options persist through Save Changes.

**Local policies pending DOSBox comparison:** Random is independent selection
without an immediate repeat, while sorted orders are fixed for a round. Exact
original random scheduling, team regrouping/rounding and dead-member win-count
policy remain comparison points. Fast speed is ×6; speech uses 0.1 seconds per
Talk Delay unit, at most 72 displayed characters per phrase and 1 MiB per file.
The overview/editor arrangement is adapted to this framebuffer. These tuning
and UI choices are explicit rather than claims of pixel-exact original behavior.

**Validation:** tests cover all order modes and rotation/live filtering,
team victory/dead-member shares/Vicious breakup and reset, Corporate awards,
penalties and interest deduplication, exact regrouping conservation, real
purchases/sales and two-round 2v2 Corporate totals through Game Over, plus a
complete 2v2 computer game using live projectiles. System tests cover pause
state, resumed shots, confirmations, Mass Kill math/one-time payout/effect
cleanup, sound/clear actions, command phase guards, staged cancellation,
controller reassignment during a guidance prompt and new-game reset. Batch 22
replaces the disabled Save/Restore guards with working checkpoint dialogs. UI/speech
tests cover coloured name editing, controller/icon/team changes, reverse shield
selection, Play Options/file editors, probability/modes and duplicate/blank
phrase weighting. `SCORCHED_PLAY_PREVIEW_DIR` with `cargo test
batch_seventeen_previews` exports menus, player/team editors, speech, Vicious
breakup and Corporate standings. Interactive keyboard/mouse play and DOSBox
comparison remain manual validation.

**Done when:** a 2v2 Corporate team game plays to the end.

## Batch 18 – Synchronous and Simultaneous modes (L) ✅
**Depends on:** 17

Play Options "Mode": Sequential (default) / Synchronous / Simultaneous
*(pp. 47, 51–52)*.

- [x] **Synchronous**: each player aims and "fires" in turn, but nothing flies;
      the tank is highlighted as ready. When everyone is ready, **all shots go
      at once**. Repeat until the round ends.
- [x] **Simultaneous**: everyone aims and fires in real time on one keyboard.
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

**Implemented:** the Play Options Mode selector activates all three modes.
`modes.rs` holds six-key maps, a staged key-capture editor, queued shot commands,
per-player AI timing/flashes/energy visuals/temporary traces, and a compact
two-column HUD supporting ten players. Player Setup K or its Keys button opens
assignment for the selected human. Duplicate bindings across active humans and
reserved Escape/F1/F11/modifier keys are rejected; maps use settings persistence
and are excluded from saved games. Player reassignment validates and applies maps.

Synchronous stores aim, weapon, target, AI intent and energy charge until every
living player commits. No projectiles, instant earth effects, energy damage or
ammo spending happen during commitment. Ready tanks are outlined/labeled. The
volley launches together, retaining shots from owners killed by an earlier
energy discharge in that same launch; owners dead before launch are skipped.
Guidance and battery prompts, accessories and movement retain their turn flow.
Random order cannot revisit an already-ready tank. Wind, walls and suspended
dirt are shared for the volley; resolving all effects starts the next cycle.

Simultaneous polls each human map independently, applies one-degree/two-power
steps through the existing repeat timer, and accepts held fire only when that
owner's projectiles, child explosions, earth/liquid effects and energy visuals
are finished. Other owners keep firing. Deaths resolve during ongoing combat;
new firing stops when a round could end, allowing pending ordnance and death
chains to finish before scoring. Corporate/Vicious outcomes use the existing
rules. Support removal records the responsible owner for fall damage. Temporary
traces are replaced per owner, and Clear Screen clears everyone's traces.

Manual battlefield panels, movement, retreat, mouse controls and guidance are
bypassed in real-time play. Owned Auto Defense permits pre-battle shields;
computers do not replenish shields outside that setup. Parachutes are forced
deployed, retaining the saved threshold preference but ignoring it during this
mode. Automatic batteries heal only living tanks below 91 and flash green.
Power/weapon flashes and the HUD's busy marker remain independent per tank.
System Menu pauses every runtime timer and works with saves/restores/new games.

**Local policies pending original-game comparison:** held fire repeats on
completion; energy uses the maximum available charge up to ten; computer aim
uses 100 degrees/1000 power units per second after a 0.4-second planning delay,
firing no earlier than 1.2 seconds. Wind/Erratic walls have a 1.2-second real-time
cadence. The real-time HUD uses two columns and expands sky clearance with the
roster. These timings/layouts are local choices, not recovered v1.5 constants.
The shared liquid pool cap now permits ten owners, while its queue stays bounded.

**Validation:** tests exercise a complete three-human queued volley, next-cycle
readiness, energy/targeted guidance deferral, dead ready players and same-volley
casualties; two independent human inputs, aiming during flight and per-owner
refiring; automatic supplies/flashes and disabled guidance; separate earth,
liquid and energy lifecycles; fall ownership; deaths during flight and delayed
round awards; completed computer matches in both modes; pause/reassignment;
temporary trace cleanup; shop restore into both modes; ten-player key-map
persistence/conflict checks and staged key capture through Player Setup.
Framebuffer previews cover readiness, a volley, two-human combat, ten-player
HUD, Mode selection and the key editor. Live keyboard rollover, interactive
two-human play and DOSBox fidelity comparison remain manual checks. All 309
tests pass, the release build succeeds, and formatting/diff checks pass.

## Batch 19 – Sky and landscape (M) ✅
**Depends on:** 2

Landscape menu *(p. 46)*:

- [x] **Sky**: Plain / Stormy / Stars / Shaded / Sunset / Cavern / Black /
      Random (default Random, which **never picks Black**). Stormy is a ✅ **Q21**
      **hostile** sky that throws lightning. Cavern is a new 1.5 sky (a cave
      ceiling). Black is pitch dark: "test your skills at night".
- [x] **Hostile Environment** (default **on**): if off, hostile skies still
      act but do no damage.
- [x] No meteors: the author's FAQ says meteor showers were added in v1.1, but
      v1.5's EXE contains no meteor text or option. Only Stormy lightning is
      hostile in v1.5. (Meteors could be an optional extra.)
- [x] **Terrain generator like the original** (`MECHANICS.md` §5.3;
      corrected research in `TERRAIN_FOLLOWUP.md`): the live DefaultLand class
      uses an integer random walk in grade, with fixed 20% probabilities for
      changing grade and doubling acceleration. Clamp to `ymin+40..ymax-1`,
      with different reactions at the upper/lower limits. End heights are not
      forced equal. The midpoint-displacement module exists but is not selected.
- [x] **Make Bumpiness, Slope and Flatten Peaks actually work** (our
      improvement: the original constructor fixes all three class parameters):
  - **Bumpiness** 0–100 (default 20): percentage of columns changing grade.
  - **Slope** 0–100 (default 20): percentage of changes using double acceleration.
  - **Flatten Peaks** (default off): reset grade to zero when it exceeds ±3.
  - [x] **DOSBox visual follow-up:** compare 20/20/off defaults with the live
      integer walk and Cavern's independent half-field walks. The earlier
      16–64-segment/equal-endpoint target was based on the wrong routine.
- [x] **"Classic terrain" option** (Landscape menu, default off): when on,
      ignore the three settings exactly as v1.5 does.
- [x] **Random Land** (default off): each round set Flatten Peaks =
      `random(2)`, Bumpiness = `random(100)`, Slope = `random(100)`, as in
      the original.
- [x] **Percent Scanned Mountains** 0–100 (20): how often to use a scanned ✅ **Q22**
      mountain picture instead of a random landscape. The `.MTN` format is now
      known (`MECHANICS.md` §17.1). Load `.MTN` files from a folder the player
      points to (don't ship the original's files), and PNG pictures as our own
      format.
- [x] **Dirt colours**: pick one of several colour schemes at random each
      round (the original has 6).

**Implementation and native-terrain fidelity follow-up complete (2026-11-17).**

**Done when:** every sky type looks distinct, lightning can damage a tank in a
Stormy sky, the default terrain looks like the original's, and moving each
landscape slider visibly changes the land (except in Classic mode).

Implementation: `landscape.rs` now implements the live `323a:0a68` walk,
selected by vtable `DS:508c` in the DefaultLand constructor. `MECHANICS.md` §5.3
and `TERRAIN_FOLLOWUP.md` document the full caller/bounds proof and correct the
previous midpoint-displacement assumption. Native Cavern uses two independent
walks, floor first then roof, in the lower/upper vertical halves. No arbitrary
55–75% midpoint, equal endpoints, shallow roof or 96-pixel gap remains. The
persistent native generator reverses direction between uses. The class's
original 20/20/off defaults match Classic; non-Classic exposes its parameters
through working sliders. The tribute keeps its own RNG and HUD/full-viewport
bounds, so this is algorithm fidelity rather than reproducing a DOS RNG seed.

All seven skies render distinctly; Random chooses the six non-Black skies.
Cavern has a top-anchored destructible roof, a shaded interior, and floor-based
placement. Detached roof pixels fall; tank pads preserve the roof. Six original
solid earth colours at DS:5036 are selected independently of the sky. Cavern
shading now uses the recovered 30 horizontal VGA bands around the global field
midpoint; Plain uses the original VGA (9,9,31) blue.

Stormy throws visible, randomly positioned bolts while aiming (paused during
panels and defensive setup). Hostile Environment off keeps the bolts and
thunder but prevents damage. Local tuning is a strike every 3–8 seconds after
an initial 4-second wait, 35 damage within 24 pixels, with shields absorbing
first. Storm damage, deaths and secondary blasts award no player damage/kill
money or suicide penalty. Resolution preserves the current player's un-fired
turn when they survive; a killed current player is skipped. No meteors.

The Mountains Folder editor accepts a path (Enter applies, Esc cancels, empty
disables). Relative paths use the config file's directory. Per-round percentage
selection loads user MTN or PNG files without shipping original assets. MTN
uses the documented mixed-endian header and bottom-up packed columns; PNG uses
alpha below 128 as sky, or top-connected top-left background colour for opaque
images. Nearest-neighbour scaling fits the playable field. Limits are 16 MiB,
4096 per side and 4M pixels; bad candidates are skipped, and failure shows a
notice before falling back to generated land. Cavern ensures local clearance
when combined with custom mountains. Imported colours remain destructible.

Validation covers decoder/truncation, random selection, Classic/default walk,
working sliders, roof/settling, ten-player placement, external loading, relative
paths, menu editing and neutral storm scoring. The follow-up adds an interpreter
that executes the original integer instructions with an injected RNG; fixtures
compare every floor/ceiling column against Rust, including reverse direction,
clamping, extreme HUD bounds, flattening and consecutive Cavern RNG streams.
Ten tank pads fit the recovered minimum gap without erasing the roof. Combat
unit fixtures now use fixed terrain so native high peaks cannot turn a MIRV
apex test into a random ground collision. Original-screen capture, measurement
and contact-sheet tools provide the reproducible DOSBox comparison workflow.
Framebuffer previews cover skies, sliders, imports, original Cavern shading,
and 30 native Plain/Cavern layouts. See `TERRAIN_FOLLOWUP.md` for evidence and
remaining limits on imported-mountain mapping and other sky artwork. The final
comparison uses 30 completed DOSBox landscapes per sky and 30 Rust renders per
sky, with an inspected contact sheet. All 315 tests and the release build pass.

## Batch 20 – Sound (M)
**Depends on:** 2

Sound menu *(p. 39)*. The original used only the **PC speaker**.

- [x] Generate square-wave beeps in code (no audio files), turn them into WAV
      bytes in memory, and play them with macroquad's `audio` module.
- [x] **Sound** on/off, default on.
- [x] **Flight Sounds** — Off (default) / POS (pitch follows the shot's
      **height**; most people prefer this) / VEL (pitch follows its **speed**).
- [x] Explosions, firing, deaths, menu clicks. 🟡 **Q23**

**Implemented:** macroquad's `audio` feature is enabled. `sound.rs` synthesizes
mono 44,100 Hz, 16-bit PCM square-wave WAV buffers in memory and loads a cached
bank through `load_sound_from_bytes`. No audio assets are shipped. Short
amplitude ramps soften sample boundaries while keeping square-wave timbre.

The Sound menu enables Sound (default on) and Flight Sounds Off/POS/VEL
(default Off), including hotkeys, reverse cycling and settings persistence.
POS raises pitch with projectile altitude; VEL raises it with speed. Flight
uses 32 cached pitch bins (roughly 220–2200 Hz) with integer-period seamless
loops. One representative airborne projectile drives the tone during resolving;
rollers, shield traversal, effect markers and other phases do not produce a
flight tone. Turning Sound off immediately requests silence and discards
pending effects. Turning flight mode Off leaves ordinary effects enabled.

Playback is monophonic: effects interrupt flight, higher-priority effects
replace lower-priority ones, and flight resumes after the effect ends.
Simultaneous identical events are deduplicated in a bounded bitset, and
lower-priority events are dropped rather than queued behind old sounds.
Scheduling uses frame time and never pauses physics or turn progression.

Firing uses the recovered 1000/2500/4000/5500/7000/8500/10000 Hz chirp, and
direct shield contact uses a 1000 Hz beep. Riot weapons have a rising/falling
siren. Other generated cues cover ordinary/nuclear explosions, deaths and
retreat, splits, digging, dirt deposition and Earth Disrupter's hundred-beep
pulse. Logical menu, shop and battlefield-panel actions have click feedback;
idle frames, held aiming keys and rejected actions stay quiet. Tracer impacts
remain harmless and make no explosion sound; intercepted bombs make the shield
beep without an explosion/chain cue. Each new explosion stage has a cue.

The recovered frequencies are retained, but durations, most other sweeps,
priority order, flight mapping and amplitude are explicit local sound-design
choices. Exact v1.5 audio timing still needs listening against DOSBox.
Laser/plasma cues are connected in batch 12; splash/burning cues in batch 11.

**Validation:** tests cover PCM/WAV headers and duration, amplitude bounds,
measured square-wave pitch, the firing/shield frequencies, zero-DC seamless
flight loops, POS/VEL mapping and clamps, single-voice priority/resume/retrigger,
mute and event disposal, menu persistence/clicks, action-to-cue routing,
representative-shot selection and unchanged physics with sound on/off. The
preview exporter writes individual cues, flight samples and a demo when
`SCORCHED_SOUND_PREVIEW_DIR` is set. Fifteen exported WAVs passed Python's
independent `wave` parser, and the demo played through ALSA's null device.
The audio-enabled release build succeeds; real speaker output and side-by-side
DOSBox listening remain manual checks.

**Done when:** it sounds like a 1991 PC speaker.

## Batch 21 – Presentation polish (M) ✅
**Depends on:** –

- [x] Several tank **death animations**: thud, single/double/triple escalating
      blasts, six flame plumes, grey bubbles, scattered hull pixels and reflected
      spark rays. ✅ **Q24**
- [x] **Bigger nuclear explosions**: Baby Nuke and Nuke use full-radius mottled
      red/black plasma, growing for one simulation second and shimmering for two
      before disappearing and carving the crater, following the recorded DOSBox
      observation in **Q24**. Weapon radii and Scale remain authoritative.
- [x] **Mouse control** as in the original *(pp. 5, 11)*: left/right click
      Power or Angle to lower/raise; hold to repeat; click the weapon name to
      cycle (right click reverses); **both buttons together fire**. Left/right
      click the player name opens Tank Control/Inventory; click a tank for info.
- [x] Complete mouse paths through shop categories/paging/buying/selling,
      player editor apply/cancel, equipment and inventory, parachute threshold,
      battery discharge, movement, targeting, retreat, standings and System Menu
      confirmations. A visible System button opens the paused menu.
- [x] Optional extra: integer-scaled fullscreen (Hardware option, persisted with
      Save Changes; F11 toggles immediately). Letterboxing and mouse coordinates
      share the existing viewport.

**Done when:** you can play the whole game with only the mouse.

Implemented with 286 passing tests and a release build. Coverage includes a
mouse-driven match through firing, battery discharge, damage/kill scoring and
final standings; two-button chord suppression/rearming; all mouse toolbars;
player edits; fullscreen settings round-trip; distinct death rendering and
screen-edge clipping; and the nuke growth/shimmer/crater boundary. Framebuffer
previews exported to `/tmp/scorched-presentation` and visually inspected.
Live mouse/fullscreen operation and side-by-side DOSBox fidelity remain manual
checks. Death variants are visual approximations sharing the existing death
blast damage, attribution and scoring, rather than new weapon effects.

## Batch 22 – Saved games (S) ✅
**Depends on:** 7

*(p. 35)*

- [x] **Save Game only from the buying screen** (so the battlefield never needs
      saving), Restore any time from the System Menu. Ask before overwriting a
      file.
- [x] As in the original, a save holds the players, money, inventory and round,
      but **not** the options (except player and round counts; see
      `MECHANICS.md` §17.3). Options remain those loaded from the settings file,
      including changes made in the current session.
- [x] Optional extra: a persistent hall of fame of best final scores.

**Hall of fame (2026-11-17):** keeps the top ten scores separately for Basic,
Standard (cash), and Greedy (net worth at final prices). Every participant,
including computers, qualifies; Corporate shared banks appear once, with all
members' inventory included in Greedy worth. Completed matches record after
final-round awards, once per completion; partial matches do not record. Ties
retain older entries first. Records include round and player counts for context.
Main Menu **Hall of Fame** (`F`) and the screen after final standings provide
keyboard/mouse browsing. Versioned `hall_of_fame.toml` lives beside settings,
independently of shop checkpoints and Save Changes. Exact money values persist
with atomic writes; invalid files are preserved and errors leave session scores
available. This is a local tribute feature, not a recovered DOS mechanism.

Validation covers per-mode limits, exact large scores, ties, Corporate cash and
inventory, completion timing and duplicate prevention, restart/restore, missing,
invalid and oversized files, failed writes, keyboard/mouse paths and framebuffer
previews. All 322 tests and the release build pass; the exported board and main
menu were visually inspected.

**Implemented:** `saved_game.rs` defines a strict version-1 TOML checkpoint,
separate from settings and the battlefield. It preserves the current buyer,
round/total rounds, interest-paid marker, complete player stock and exact money,
equipment preferences and opened fuel, names/controllers/icons/team IDs,
wins/kills, resolved Unknown personalities/grudges, turn-order circle and market
state. Purchases by earlier buyers are retained without running their shopping
again. Corporate account replicas are validated; restoring with different team
options conserves funds when accounts split or merge.

System Menu V opens Save Game only in Shop; R opens Restore Game in any phase.
Both have keyboard/mouse filename submission and cancellation; existing save
destinations require explicit overwrite confirmation. Relative paths use `saves`
beside the settings file; absolute filenames work. Files use the existing atomic
write helper. Missing, malformed, oversized, unsupported-version and invalid-state
files report an error without changing the live game. Restore builds a fresh
shop state, clearing old shots, terrain effects, controls and AI plans, and
reloads speech from the current options. Battlefields are generated on leaving
the shop. The format is local to this tribute, without DOS save compatibility.

**Validation:** regression tests cover round trips with arbitrary-size cash,
inventory/equipment, AI personality/grudges, market and order; current-buyer and
completed-computer purchase preservation; Corporate funds and once-per-round
interest; continuing to final standings; keyboard/mouse file dialogs and overwrite
cancel/confirm; filenames with spaces; bounded/invalid reads preserving an active
shot; and failed atomic writes preserving an existing save. Framebuffer previews
cover the shop menu, Save/Restore filename dialogs, overwrite prompt and errors.
All 293 tests pass and the release build succeeds. Live keyboard/mouse
interaction remains a manual check.

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
