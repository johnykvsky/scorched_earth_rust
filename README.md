# Scorched Earth – Rust tribute

A 90s-style remake of the DOS classic: 640×480 software framebuffer, chunky
VGA-style gradients, DOS 8×16 font, scaled up with crisp nearest-neighbour pixels.
Built on [macroquad](https://macroquad.rs).

---

![Listing](https://raw.githubusercontent.com/johnykvsky/scorched_earth_rust/master/scorched-earth-rust.png)

---


## Requirements (Linux / Manjaro)

1. **Rust toolchain** (cargo + rustc). Either:
   - `sudo pacman -S rustup && rustup default stable` (system package), or
   - the official installer, no sudo needed, installs into `~/.cargo`:
     `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. **System libraries** for the window/graphics/audio (already on a standard
   Manjaro desktop): `sudo pacman -S --needed base-devel libx11 libxi mesa alsa-lib`

If `cargo` is installed in `~/.cargo/bin` but your shell can't find it, add this
to `~/.zshrc`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

(The Makefile adds that path itself, so `make` works even without it.)

## Build & run

The first build downloads dependencies from crates.io and takes a minute; later
builds are quick.

| Command | What it does |
|---|---|
| `make run` | Build optimised and start the game (**use this to play**) |
| `make build` | Only build; the binary is `target/release/scorched` |
| `make dev` | Debug build + run (compiles faster, runs a bit slower) |
| `make check` | Type-check the code without building |
| `make fmt` | Format the code (needs `rustup component add rustfmt`) |
| `make clean` | Remove the `target/` build directory |
| `make help` | List the targets |

Without make, the same thing is `cargo run --release`.
Run regression tests with `cargo test`.

After `make build` you can start the game directly with `./target/release/scorched`,
or copy that single file anywhere; it has no other files to go with it.

The window opens at 1280×960 (the 640×480 game doubled) and can be resized;
the picture is scaled by whole numbers when it fits, keeping pixels sharp.
On Linux/X11, the game requests keyboard focus once after its first frame, so
keyboard controls work when launched from a terminal. Switching to another
window afterward does not trigger another focus request.

## How to play

In the main menu use Up/Down to select a button and Enter to choose it.
Left/Right changes counters; highlighted hotkeys advance values, and
Alt+hotkey moves backward. Left/right mouse clicks also advance/reverse values.
Esc or a click outside closes an option dialog. Greyed-out options are planned
features whose gameplay is not implemented yet.

Choose Players (2–10) and Rounds (1–1000, default 10), then Start (`S`).
In player setup, choose Human or one of eight computer types for each player
with Left/Right or mouse clicks. Enter/N opens a coloured player editor for
names, controller, icon and team; Enter applies and Esc cancels. The number
keys `1`–`9`/`0` select and cycle a player's controller; Alt reverses the cycle. Choose Done or press `D`
to begin. Esc returns to the main menu.

Select **Play Options → Mode** for Sequential, Synchronous or Simultaneous.
Player Setup's **K / Keys** button edits the selected human's six Simultaneous
keys. Select an action with arrows/Enter or a click, press its new key, then
Done to apply. Esc cancels capture or the editor. Duplicate human bindings and
reserved application keys are rejected. Save Changes persists the maps.

Players take turns; the current tank has a blinking arrow over it.
The last tank standing wins the round; Standard/Corporate end when one team
remains. Basic and Standard rank players by cash; Greedy ranks by cash plus
80% of the current value of held inventory. Both round-over and final screens
show the cash standings. The first
player each round is random; subsequent turns follow tank positions from left
to right, wrapping at the edge.

The **Start → Player Setup** screen has two checkboxes, both off on launch:

- **Simplified weapons** (`S`): every player starts with unlimited Baby Missiles
  (shown as 99), 10 Missiles, 4 Rollers, 20 Tracers and 4 Dirt Balls. No other
  weapons or accessories are granted. There is no shopping before or between
  rounds, even with positive Cash at Start. Damage, kills, survival and Mass Kill
  award no money; interest and market changes are disabled. Standings count
  victories and show rounds played. Teams share victories under the ordinary
  team rules; Winners First/Losers First uses victories. Unused ammunition
  carries between rounds without refills. Simplified matches do not enter the
  money-based Hall of Fame or have a buying screen for Save Game.
- **Use default settings** (`U`): use default Sound, Hardware, Economics, Physics,
  Landscape, Play Options and Weapons values for this match. The selected
  players, names, controllers, tank icons, team assignments, key bindings and
  round count stay as chosen. Your configured options return when you leave the
  match or finish final standings. Save Changes preserves your configured
  options, including while a temporary-default match is active. A System Menu
  New Game during the match keeps its temporary options. Uncheck this box to
  start the next match with your configured options.

The checkboxes can be combined and remain selected for the current application
session until changed; they are not written to the settings file. Space/Enter
or a mouse click toggles a selected checkbox; **Done** (`D`) starts the match.

Starter ammunition is granted once per game. Unused stock carries between rounds;
other items are capped at 99, and Baby Missiles stay unlimited (displayed as 99).
The catalogue includes all original items, with twenty-eight weapons currently usable.
Opening purchases are available with positive Cash at Start, and players with
cash shop between rounds. All catalogue items can be bought, stored and sold;
the shop identifies items whose gameplay effects arrive in later batches.

## Settings

Settings load from `~/.config/scorched/settings.toml` (or the equivalent user
config directory on other platforms). Choose **Save Changes** (`C`) to keep
changes for the next launch. Unsaved changes last only for this session.
Missing files use defaults; invalid files show an error and use defaults.

To use a different file, run `./target/release/scorched myconfig.toml`, or
`cargo run --release -- myconfig.toml`. Save Changes writes to that file.
A partial TOML file can override only the options you need, for example:

```toml
players = 4
rounds = 20

[hardware]
firing_delay = 50
falling_delay = 10
```

Hardware **Firing Delay** (0–500, default 100) controls shot animation speed;
lower values are faster, and zero skips the wait. **Falling Delay** (0–1000,
default 10) sets milliseconds per falling-tank pixel; zero falls immediately.
The Physics menu (`Y`) also enables Air Viscosity, Gravity (0.05–10 in 0.05
steps), Suspend Dirt, Tanks Fall, and Borders Extend. In Landscape (`L`), Max.
Wind and Changing Wind are active. PgUp/PgDn changes counters by ten steps.
Remaining greyed-out options become active in later feature batches.

Gravity defaults to 0.2, giving acceleration 500; power is used directly as
launch speed, capped at 1000. Viscosity slows shots. Borders Extend allows shots
to leave the side edges and return with the wind. Suspend Dirt is the percentage
chance that floating soil stays suspended after a shot (0 always falls; 100
never falls). Turning Tanks Fall off leaves unsupported tanks floating.

Falls cost two health points per pixel. To apply damage while falling instead
of on landing, add this to your settings file:

```toml
[physics]
impact_damage = false
```

In Weapons (`W`), **Tunneling** (`T`) is on by default. Shots erase dirt pixels
and lose 25% of their speed for each one entered, exploding once too slow to
continue. Fast shots can pass through a thin layer. Gravity, wind and drag
pause underground. Turn Tunneling off for explosions on first dirt contact.
Rollers and tracers always retain their contact behavior. Save Changes keeps
the choice, or set `tunneling = false` in the `[weapons]` TOML section.

The total fall damage is the same in both modes. Smart computer players
account for gravity; Spoiler and Cyborg also account for wind. Their power
solvers intentionally cannot compensate for viscosity.

## Play options, teams and System Menu

**Sequential** uses the usual one-player turn. In **Synchronous**, each living
player aims and commits a shot in turn. Ready tanks have a white outline and
READY label. The final commitment launches the whole volley together; ammo,
guidance and energy batteries are spent at launch. Targeting and battery
prompts stay available while choosing a shot. All effects and deaths resolve
before the next volley. Players killed before launch lose their queued shot;
shots committed to the same volley still launch if another shot in that volley
kills their owner.

**Simultaneous** lets everyone aim and fire on the same keyboard. Each human
has six keys, with these defaults for the first two players:

| Action | Player 1 | Player 2 |
|---|---|---|
| Clockwise / counter-clockwise | Right / Left | D / A |
| Power up / down | Up / Down | W / S |
| Fire | Space | Q |
| Change weapon | Tab | E |

Angle changes by one degree and power by two units per repeat. Hold Fire to
shoot again as soon as your own shot and its effects finish; other players'
shots do not block you. You can keep aiming or change weapons during flight.
The top bar lists every player's power and weapon in their colour; `*` means
their shot is active. Tanks flash bright/dark for power changes, red for weapon
changes and green for automatic batteries.

Battlefield mouse controls, manual accessories, movement, retreat and guidance
are disabled in Simultaneous mode. Shields can be engaged during the initial
Auto Defense panel by players who own Auto Defense. Parachutes stay deployed
and open on any drop when stock is available. Batteries automatically heal a
living tank below 91 health until it reaches 91 or more, or runs out. Energy
weapons automatically use up to ten remaining batteries without a prompt.
F1 or Alt-S still pauses the game and opens the System Menu; mouse controls
work inside menus and shops. Normal controls also work during Auto Defense.

Computers use a fixed medium aiming cadence: a short planning delay, visible
turret movement and at least 1.2 seconds before firing. This timing and the
automatic maximum energy charge are local policies. In real-time play,
Changing Wind and Erratic walls update every 1.2 seconds. Synchronous volleys
share one wall choice, wind value and Suspend Dirt decision. Pending shots and
death effects finish before round awards are paid, even after only one tank or
team remains. Shop saves resume under the mode currently selected in options.

Export previews of both modes, ten-player HUD and key assignment without a window:

```sh
SCORCHED_MODES_PREVIEW_DIR=/tmp/scorched-modes cargo test concurrent_modes_previews
```

**Play Options** enables Random, Losers-first, Winners-first and Round-robin
order. Random chooses another living player each shot. Sorted orders use the
round's starting cash (net worth in Greedy); Round-robin keeps a shuffled circle
and advances its first shooter one position each round.

Choose **Teams** None, Standard, Corporate or Vicious. Team IDs default to
alternating 1/2 and can be changed in each player's coloured editor. Standard
ends when one team remains; friendly fire costs money. Winning survivors get
full-weight survival shares and dead teammates half-weight shares. Corporate
adds one shared bank per team: purchases, sales, points and penalties all affect
that balance, with interest paid once per bank. Stock remains individual and
standings show each bank once. Vicious becomes a free-for-all inside the last
team; the configured teams return next round.

**Esc** on the battlefield asks before returning to the main menu. The prompt
pauses gameplay and audio; Y or **Yes** confirms, while N, Esc or **No / Cancel**
resumes the match. Open panels and targeting prompts use Esc to close/cancel
first. Esc from the main menu asks before quitting, with the same controls.

**F1 / Alt-S** opens the System Menu and pauses the current action and timers.
Esc/F1 resumes. It provides:

- **Clear Screen:** clears traces while preserving the active shot.
- **Mass Kill:** confirmation, then an equal split of the remaining health,
  shield and kill points; ends the round with no extra survival payout.
- **Reassign Players / Teams:** edit names/controllers or team mode/membership.
  Edits apply with Done; Esc cancels. Regrouping Corporate banks splits old
  funds equally before merging new banks, preserving the total money.
- **Sound:** toggles audio.
- **Reset Options to Defaults** (`O`): available from the main menu. After
  confirmation, restores all options and player setup, including key bindings,
  to built-in defaults. Choose **Save Changes** to keep them for the next launch.
  Saved games, hall-of-fame records and market data are retained. Return to the
  main menu first when a match is in progress.
- **New Game / Quit:** require confirmation.
- **Save Game / Restore Game:** save only from the buying screen; restore from
  any phase, including the title screen. Keyboard shortcuts inside the System
  Menu are V and R.

Saved games resume at the current buyer, preserving purchases already made,
exact cash and interest totals, inventory, opened fuel, equipment preferences,
player identities, wins/kills, computer personalities and grudges, turn-order
rotation, market state and the upcoming round. Player count and total rounds
are also restored. Other options stay as loaded from the settings file, with
any changes made during this session. Restoring with different team options
splits or combines Corporate funds while preserving the total money.

The filename dialog starts with `game.toml`. Relative names use a `saves` folder
beside the settings file (normally `~/.config/scorched/saves` on Linux);
absolute paths work too. Home clears the name, Backspace edits it, Enter or
Apply submits, and Esc or Cancel leaves the game paused. Existing destinations
require Y or the Overwrite button before replacement. Files are written through
a flushed temporary file; failed reads or invalid saves leave the active game
intact. After success, dismiss the message and resume from the System Menu.

These are versioned Rust tribute TOML saves, not original DOS `.SAV` files.

**Hall of Fame:** choose **Hall of Fame** (`F`) from the main menu, or continue
past final standings. Left/Right changes scoring mode; Enter/Esc returns to the
menu. Mouse buttons provide the same actions. Each mode keeps its ten highest
final scores: cash in Basic/Standard, net worth at final prices in Greedy. All
players, including computers, qualify. Corporate teams have one shared-bank
entry, including all member inventory in Greedy. Older records win ties.
Round and player counts are shown for context; scores are not normalized for
match length, starting cash or other options.

Scores record automatically when the final round ends. Unfinished matches do
not qualify. Records persist in `hall_of_fame.toml` beside the settings file
(normally `~/.config/scorched/hall_of_fame.toml` on Linux), separately from saved
games and Save Changes. Restoring a checkpoint preserves the hall of fame;
finishing a restored match creates records for that completion. Invalid files
are preserved, with new scores held for the session; write failures also leave
session records available and show an error on the board. This local feature
has no DOS save compatibility.

To export hall-of-fame and main-menu previews:

```sh
SCORCHED_HALL_PREVIEW_DIR=/tmp/scorched-hall cargo test hall_of_fame_previews
```
Battlefield terrain, active shots and options are excluded. Restore returns to
shopping and generates a new battlefield when shopping ends.

To export saved-game dialog previews without opening a window:

```sh
SCORCHED_SAVED_GAME_PREVIEW_DIR=/tmp/scorched-saved-games cargo test saved_game_previews
```

**Fast Computers** speeds battle time by six when all living players are
computers. Human turns, shops, menus and defensive setup keep normal timing.
The fixed projectile time step stays the same.

**Talking Tanks** defaults to Off; choose Computers or All and a 0–100 Talk
Probability. Attack File and Death File accept optional UTF-8/ASCII files:
one phrase per line, repeated lines are more likely, and blank lines mean
silence. Empty files are silent. Relative paths use the settings folder;
missing/invalid files report an error and fall back to our built-in phrases.
Files are limited to 1 MiB; displayed phrases to 72 characters. No original
speech files are shipped. Save Changes keeps names, teams and these options.

Preview the menus and team scenes without a game window:

```sh
mkdir -p /tmp/scorched-play
SCORCHED_PLAY_PREVIEW_DIR=/tmp/scorched-play cargo test batch_seventeen_previews
```

## Economy

In Economics (`E`), choose Basic, Standard or Greedy scoring (`S`). Standard is
the default. Enemy kills pay $4,000 in Basic or $500 in Standard/Greedy; the
latter also pay $30 per health point actually removed. Self-damage costs $15
per point outside Basic, and killing yourself costs $1,500 in every mode.
The winner receives the survival pool: $1,000 per player in Basic, or $5,000
in Standard/Greedy. Cash never falls below zero.

Interest Rate (`I`, default 5%) applies to bank cash once at the start of each
round. Cash at Start (`C`, default $0) can be set up to $1,000,000. Menu steps
are one percentage point and $1,000 respectively; PgUp/PgDn takes ten steps.
Money and interest use exact integer arithmetic even for very long games.

With positive starting cash, players shop before round 1. Players with cash
also shop between rounds, including those killed in the previous round.
**Computers Buy** (`B`, default on) controls AI purchases in both cases.
Interest applies after shopping to the remaining cash, once per round.

Shop controls:

- Up/Down selects an item with wrapping; Home/End jumps to either end.
- PgUp/PgDn changes pages with wrapping; Tab switches Weapons/Miscellaneous.
- Space/Enter or a left click buys the displayed quantity at the displayed price.
- `I` opens inventory. Select an item to sell, then Q/Up increases the quantity,
  Alt-Q/Down decreases it, `A` accepts, and `R`/Esc rejects.
- `U` shows rounds left, interest earned and cash.
- `D`/Esc finishes buying; Esc in inventory returns to buying.

Unaffordable and full-stock items are hidden and reappear when affordable or
when stock is sold. If a bundle would exceed 99, only the remaining spaces are
sold at a 20% unit markup, rounded to the nearest dollar. Baby Missiles remain
unlimited and cannot be bought or sold. Selling normally returns 80% of the
current bundle's unit value, rounded once for the quantity sold.

Weapons options **Arms Level** (`A`, 0–4) and **Useless Items** (`U`, default off)
filter shop offers. With Useless Items off, offers that are redundant with the
settings are hidden: Smoke Tracer with Trace Paths, parachutes with Tanks Fall
off, Earth Disrupter with Suspend Dirt zero, and Contact Trigger with tunneling
off. All catalogue weapons and accessories can be stored and sold; their effects
are available through firing or the battlefield controls.

Economics **Free Market** (`F`, default off) changes prices once per completed
round using purchase demand and the Interest Rate. Quotes stay fixed while
players shop; sell-back returns 65% in this mode. Prices and demand persist
between games and launches in `~/.local/share/scorched/market.toml` (or the
platform's equivalent local data directory). Fixed-price games use base prices
and leave the saved market alone. Invalid market data and save failures report
an error. Auto Defense uses the remaining-round annuity described below.

Greedy rankings use current prices, including the same valuation of each
player's 99 Baby Missiles. Large balances are abbreviated on screen without
losing precision in the underlying balance or ranking.

## Walls

Physics (`Y`) → **Effect of Walls** (`W`) cycles None → Wrap-around → Padded →
Rubber → Spring → Concrete → Random → Erratic. Alt reverses the cycle, and
Save Changes keeps the setting.

- **None** (grey): shots can leave and return within Borders Extend; high arcs
  can travel above the screen.
- **Wrap-around** (yellow): leaving one side enters the other at the same
  height and velocity. High arcs remain possible.
- **Padded** (green): walls and ceiling reflect shots, halving the velocity
  component perpendicular to the wall.
- **Rubber** (red): reflects that component at full speed.
- **Spring** (cyan): reflects it at twice the speed, subject to the projectile
  speed limit on subsequent physics steps.
- **Concrete** (white): stops shots at the sides and ceiling, including shots
  with tunneling enabled.
- **Random**: one of the six fixed types for the whole round.
- **Erratic**: a type chosen before aiming/planning each shot, held throughout
  its warheads, hops and chain reactions. Consecutive choices can repeat.

The closed ceiling sits immediately below the HUD and optional status bar.
Wall borders show the active type; `U` reports it, including Random/Erratic's
resolved choice. AI prediction uses the same walls as live shots. Rollers wrap,
reflect or detonate at the edges; Padded/Spring change their rolling speed,
with a 1000 pixels/s cap. MIRV/Death's Head, Leap Frog and Funky children also
respect the walls. Concrete hits before a carrier's apex remain duds.

## Standard weapons and paths

The playable weapons are Baby Missile, Missile, Baby Nuke, Nuke, Leap Frog,
Funky Bomb, MIRV, Death's Head, Tracer, Smoke Tracer, Baby Roller, Roller,
Heavy Roller and Dirt Ball. Buy the new weapons in the shop, then choose them
with Tab/Shift-Tab or battlefield inventory.

**Leap Frog** makes three successive blasts (30 → 25 → 20), launching another
hop after each of the first two explosions at reduced speed. **MIRV** splits
at the apex into five Missile warheads, while **Death's Head** produces nine
radius-35 warheads. The central warhead preserves the carrier's trajectory;
impacts before the apex are duds. **Funky Bomb** opens with a radius-20 burst,
sends 5–10 coloured curved trails to nearby points, then finishes with a
radius-40 blast. **Rollers** move downhill and descend a pixel at a time,
exploding at an obstruction or tank. They roll over enemy shield outlines and drop off the side; all wall types
are supported.

Weapons menu (`W`) options:

- **Bomb Icon** (`B`): Invisible / Small / Big (default Big) controls the
  projectile's appearance without changing its collisions.
- **Scale** (`S`): Normal / Medium / Large multiplies supported explosion
  radii by 1 / 2 / 3. Missiles, Leap Frog, MIRV/Death's Head and rollers scale;
  Dirt Ball and Funky's opening/trail bursts stay fixed, while its final
  blast scales. Child warheads inherit the setting from the fired shot.
- **Trace Paths** (`P`, default off): every shot leaves an owner-coloured
  path that survives subsequent shots. Smoke Tracer leaves a persistent path
  even with this option off. Ordinary Tracer and Funky effects also have
  temporary trails for the current shot. Both tracers are harmless.

Paths clear when a new round starts. F1/Alt-S → Clear Screen clears paths without changing the active shot. All three weapon options are saved by Save
Changes. The precise family-specific scaling rules still need side-by-side
DOSBox validation; the implemented policy above is explicit.

## Computer players

Choose a personality in Player Setup; computers choose their own unique names
and keep them across rounds.

| Type | Behavior |
|---|---|
| Moron | Random owned weapon, angle toward an enemy side, power in health-limited steps of ten |
| Shooter | Nearest enemy, direct angle and calculated power; obstacles defeat its limited angle search |
| Poolshark | Shooter with bank shots from Rubber/Spring walls and the ceiling |
| Tosser | Ranging lob, then corrections of ten power and sometimes two degrees from its last actual landing |
| Chooser | Shooter with clear sight; otherwise Poolshark with Rubber/Spring, or Spoiler |
| Spoiler | Random enemy, wind-aware high arc and angle search; no viscosity correction |
| Cyborg | Wind-aware aiming, preferring weak, winning or previously hostile opponents |
| Unknown | Rolls one of the seven once per game and keeps its personality hidden |

Tosser remembers each target separately; new rounds clear that ranging history.
Unknown's personality and Cyborg's hostility memory survive round boundaries.
Planning uses private terrain and shield copies and cannot alter the live field.

**Computers Buy** controls purchases only. Moron buys weighted-random items;
other types prioritize shields, parachutes and supplies before more weapons.
Normal prices, arms/usefulness filters, stock caps and market demand all apply.
Computers still use owned batteries, shields and parachutes with buying off.
They can dig out with Riot Charge/Blast, select guidance and triggers, use
suitable energy/liquid weapons and make a bounded fuel move when obstructed.
Guided AI shots automatically target their chosen victim without a human prompt.

Search bounds, Cyborg's target weights, detailed shopping order and tactical
thresholds are local choices awaiting DOSBox comparison. In particular, the
smart AIs retain their viscosity weakness; Shooter/Poolshark also ignore wind.
See batch 16 in `TODO.md` for the full policy and validation limits. Export
headless path previews:

```sh
mkdir -p /tmp/scorched-ai
SCORCHED_AI_PREVIEW_DIR=/tmp/scorched-ai cargo test ai_previews
```

## Liquid weapons

**Napalm** and **Hot Napalm** spill fluid that runs downhill, spreads and pools
in valleys. Flames grow, then burn for 50 cycles. Deep pools deal more damage;
Hot Napalm produces more fluid and stronger heat. Enemy shields absorb heat
before it reaches health. Your own heat bypasses your shield under the current
Batch 11 rule (the original's own-heat shield behavior remains disputed in the
project notes). Heat earns the normal damage/kill money, including self-damage
and suicide penalties.

**Liquid Dirt** uses the same flow but deposits soil, filling holes without
direct damage or lifting tanks. Removing support is unnecessary: it adds soil
and then uses normal post-shot terrain settling. The turn waits for the entire
liquid effect. Carriers do not cause instant direct-hit kills or standard
explosions, and shields can intercept them before the spill.

With Tunneling enabled, Napalm/Hot Napalm that stop underground **fizzle**.
Arm a **Contact Trigger** (`-`) to spill on the first dirt contact instead.
Heat, Horizontal, Vertical and Lazy Boy guidance also force contact without
spending a trigger. A carrier that crosses a thin layer and emerges can spill.

Emitter counts (15/20/20), the 100-cell active queue limit and fifty burn cycles
come from the recovered notes. Fluid volume, timing, exact heat formula and
visuals are local tuning choices awaiting DOSBox comparison; see batch 11 in
`TODO.md`. Explosion Scale does not affect liquids. Export previews:

```sh
mkdir -p /tmp/scorched-liquids
SCORCHED_LIQUID_PREVIEW_DIR=/tmp/scorched-liquids cargo test liquid_previews
```

## Energy weapons

Buy **Plasma Blast** or **Laser**, select it with Tab or battlefield inventory,
then fire to open **Batteries to discharge:**. Type 0–10 or use arrows; Enter
or Space confirms, Esc cancels without spending ammunition or batteries, and
Backspace resets to the default (up to ten owned batteries).

Plasma radiates around your tank, excludes you and leaves terrain untouched.
Its radius grows from 20 to 75 with the charge, independent of barrel angle.
Enemy shields absorb the damage before health. Laser follows the barrel,
cuts narrow holes in dirt and applies 10-point damage pulses. Regular shields
can be pierced; Super Mag stops the beam and converts its remaining energy to
shield charge, capped at 200. Guidance and Contact Triggers are preserved.

The current laser energy budget is power × batteries: sky costs 40 per pixel,
dirt 80. Zero batteries produces no beam; zero-charge plasma still has radius
20. These energy/radius tuning choices await DOSBox comparison; see batch 12
in `TODO.md`. Energy weapons ignore explosion Scale and stop at field edges.

Export framebuffer previews without opening a window:

```sh
mkdir -p /tmp/scorched-energy
SCORCHED_ENERGY_PREVIEW_DIR=/tmp/scorched-energy cargo test energy_previews
```

## Earth weapons

**Riot Charge / Riot Blast** act immediately around the turret instead of
launching a projectile. Aim the barrel toward the dirt to remove: Charge makes
a radius-36 wedge with a 45° half-angle; Blast uses radius 60 and 60°. These can
clear a buried barrel. **Riot Bomb / Heavy Riot Bomb** are projectiles that
remove radius-30/45 circles. Riot weapons and diggers cause no direct health
damage; removing support can still cause a damaging fall and earns the shooter
the normal fall-damage credit.

**Baby Digger / Digger / Heavy Digger** excavate branching tunnels on soil
contact and fizzle on tank contact. **Baby Sandhog / Sandhog / Heavy Sandhog**
follow soil too, with 10-point contact hits and small scaled terminal charges.
They do not seek tanks. Each swarm has a finite birth budget and at most twenty
live heads. Their underground damage is separate from ordinary blast damage;
terminal charges bypass engaged shields, while their incoming carrier can still
be intercepted before it reaches dirt.

**Dirt Clod / Dirt Ball / Ton of Dirt** grow unscaled spheres of radius
20/35/70, burying tanks without lifting them or causing direct health damage.
**Dirt Charge** launches normally, then sprays dirt upward; the grains fall and
deposit soil. Weapons → **Extra Dirt** (`E`, default off) emits four times as
many grains. Save Changes keeps the choice.

**Earth Disrupter** needs no projectile: after its flash sequence it forces
floating terrain to settle even with Suspend Dirt at 100. Other earth effects
use the normal post-shot settling rules. The turn waits for tunnel heads,
terminal explosions, growing dirt and airborne grains to finish.

Some original details remain unknown. This implementation uses 120 Hz tunnel
heads with finite lifetimes and 80/320 dirt grains; these are explicit local
choices, not verified original constants. See batch 10 in `TODO.md` for the
full policy and validation limits.

## Tank controls and supplies

During a human turn, `T` opens the Tank Control Panel and `I` opens battlefield
inventory. Use panel hotkeys or Up/Down and Enter; inventory also supports
PgUp/PgDn, Home/End and mouse selection. Panels pause aiming. Esc closes the
current panel; dismissing it with Space/Enter never fires a shot.

**Battery** (`B`) uses one battery to restore 10 health, capped at 100, raising
maximum power without changing the chosen power. It is not consumed at full
health. **Parachutes** (`P`) toggle between deployed (default) and passive.
In the control panel, `V` edits the safety threshold (1–100, default 5): type a
number or use arrows, Backspace corrects it, Enter applies and Esc cancels.
A deployed chute is consumed before a fall only when predicted damage exceeds
the threshold. Descent takes 20 ms per pixel and causes no normal fall damage.
Landing on an enemy still hurts both tanks. Deployment and threshold preferences
carry between rounds; new games reset them.

Play Options (`T` from the main menu) enables **Status Bar** (`S`, default off).
It adds maximum power and supply counts/state below the main HUD. Supply fields
are clickable, and their hotkeys work with the bar hidden too.

`S` selects an owned shield; Shift-S reverses. `G` selects guidance; Alt-G
or Shift-G reverses. Alt-S opens the System Menu,
and None is included. `-` toggles contact triggers (`T` inside the panel).
`E` engages the selected shield, consuming one item and replacing the active
shield. Inventory selection engages that shield directly. Guidance and triggers
apply to fired projectiles as described below; `F` opens movement. Shield
percentage is visible in the main HUD and optional status row.

`U` shows walls, wind and rounds left. `K` displays "No kibitzing, please."
Number keys `1`–`9`/`0` show information about tanks counted from the left,
including eliminated tanks. `R` asks whether to retreat: `Y` confirms, `N`/Esc
cancels. A helicopter takes you out of the round; retreat awards no damage or
kill money and keeps unused stock for later rounds.

## Shields and Auto Defense

Select an owned shield with `S` and engage it with `E`; Shift-S reverses selection.
The Left arrow on the panel's shield row also reverses it.
Engagement consumes one item. Replacing a shield loses its remaining strength
and returns no inventory. The active shield lasts for the round; spares carry
forward. The five types have distinct strengths and effects:

| Shield | Strength | Effect |
|---|---:|---|
| Mag Deflector | 55 | Upward force on nearby enemy shots |
| Shield | 100 | Absorbs damage |
| Force Shield | 100 | Reflects enemy shots at 70% speed |
| Heavy Shield | 150 | Greater absorption capacity |
| Super Mag | 200 | Magnetic force and reflection; laser immunity property |

Direct hits normally chip 10 shield points without an explosion. Force
reflection costs rounded incoming speed/100. Blast damage uses the remaining
shield strength first, then overflows to tank health; failure is deterministic.
Your shield never stops your own shot or its blast. Fall damage and underground
Sandhog charges bypass it. Rollers traverse enemy shields without damaging them.
Circles fade with strength and flicker on hits; remaining percentage appears
in the HUD even when Status Bar is off. Super Mag stops incoming lasers and converts their remaining energy into shield
charge, capped at 200.

**Auto Defense** is bought once and kept across rounds. Its quote depends on
rounds left, Interest Rate and the current base/market price. The annuity is
`base × ((1+r) − (1+r)^−rounds_left) / r`; at zero interest it becomes
`base × (rounds_left + 1)`, rounded to a whole dollar. Repeat purchases are
hidden. Selling the token removes its benefit.

At each round start, human owners get defensive setup before anyone fires.
Use batteries, parachutes and shields from the control panel or its filtered
inventory, then choose Done/Esc. Offensive actions are unavailable during setup;
Esc in inventory returns to the panel. Computer owners equip their best owned
shield. The token is never consumed by setup, and the original first shooter
is preserved when setup ends.

## Guidance and Contact Triggers

Select owned guidance with `G` (Alt reverses) or battlefield inventory. Fire
with Space/Enter. Ballistic, Horizontal, Vertical and Lazy Boy ask **Choose
Target**: use `1`–`9`/`0` for tanks counted from the left, left-click a point,
or right-click within 100 pixels of a living tank. Numbers appear over live
tanks; eliminated positions retain their place. Esc cancels without spending
anything. Applicable guidance is consumed once at launch and resets to None.

- **Heat:** no target prompt; locks onto the first live non-owner tank within
  40 pixels, including teammates, then steers toward it.
- **Ballistic:** keeps your angle and solves power for the point, accounting
  for wind and gravity. It ignores viscosity and clamps impossible shots to
  maximum power; terrain, walls and shields can still spoil the shot.
- **Horizontal / Vertical:** fly normally until crossing the target row/column,
  then steer flat/vertically to the chosen point and detonate there.
- **Lazy Boy:** flies directly at 500 pixels/s, digging through dirt and
  ignoring shield interception and magnetic push. A different tank causes an
  early hit; otherwise it explodes at the exact selected point.

MIRV, Death's Head, Riot Charge/Blast and non-projectile effects (energy weapons
and Earth Disrupter) preserve the selected guidance. Follow-up hops, roller
movement and Funky trails use their normal behavior. Computers select owned
guidance and triggers automatically and use their planned victim as the target.

Toggle **Contact Triggers** with `-` or `T` inside the control panel. An armed
trigger is spent once when launching an unguided or Ballistic projectile;
tunneling is disabled for that shot and all its warheads/hops. The last trigger
automatically disarms the selection. Other guidance already forces contact
without spending a trigger. Immediate effects and energy weapons preserve them.

Trajectory details still need DOSBox comparison; the current Lazy Boy route is
a straight digging line and Horizontal/Vertical suppress air forces after
engagement. See batch 14 in `TODO.md` for local policies. Export path previews:

```sh
mkdir -p /tmp/scorched-guidance
SCORCHED_GUIDANCE_PREVIEW_DIR=/tmp/scorched-guidance cargo test guidance_previews
```

## Movement and tank shapes

Buy **Fuel Tanks** ($10,000 per ten tanks, ten fuel units each), then use `F`,
the Tank Control Panel, battlefield inventory or the fuel status field to open
Move. Left/Right drives one pixel per step. Esc or `D` returns to aiming;
Space/Enter never fires from this panel. The status bar and Move panel show
fuel units, including the remainder in an opened tank. Fuel carries between
rounds; unopened tanks can still be sold.

Moving costs one fuel per pixel, plus one per pixel climbed uphill. Climbs of
1–3 pixels work; four or more are blocked. Descents up to three pixels cost no
extra fuel; larger drops make the tank slip and use normal fall damage and
parachutes. A surviving driver keeps its turn and can still aim/fire. Empty
fuel returns to aiming. Field edges, other tanks and unaffordable climbs block
movement without spending fuel. Moving never excavates terrain.

In **Player Setup**, select a player and press Tab to cycle shapes (Alt reverses),
or click the shape at the right. Arrows still change the controller. **Wheels,
Treads and Heavy** are mobile; **Bunker, Tower and Dish** are fixed emplacements
and cannot buy or use fuel. **Triple** is computer-only and fires three
simultaneous Baby Missiles/Missiles; other weapons fire once. Changing its
controller to Human resets it to Treads. Save Changes keeps the selected shapes.

The silhouettes are original drawings. Triple currently uses a ±5° fan and one
ammo item per volley; its guidance and Contact Trigger are also spent once per
volley. Exact icon geometry and shot spread still need DOSBox comparison. See
batch 15 in `TODO.md` for the full movement policy. Export previews:

```sh
mkdir -p /tmp/scorched-movement
SCORCHED_MOVEMENT_PREVIEW_DIR=/tmp/scorched-movement cargo test movement_and_shape_previews
```

## Landscape and skies

**Landscape...** (`L` in the main menu) selects Plain, Stormy, Stars, Shaded,
Sunset, Cavern, Black or Random. Random never selects Black. Cavern provides a
destructible anchored ceiling and places tanks on the floor. The six original
solid dirt colours are chosen per round.

Native terrain follows the recovered v1.5 integer random walk: grade changes
accumulate across columns, so the two edges can have different heights.
Cavern generates separate walks in the lower and upper halves, with at least
41 pixels between native surfaces. Its horizontal shading bands follow global
height, fading toward dim earth colour in the middle.

Bumpiness controls the percentage of columns that change grade; Slope controls
how often that acceleration doubles. Flatten Peaks resets grades steeper than
three pixels per column. Their defaults are 20 / 20 / off. These are working
controls in this tribute; v1.5 hard-coded the underlying class parameters.
**Classic Terrain** ignores the sliders and uses those original fixed values.
**Random Land** chooses Bumpiness and Slope from 0–99 and randomizes Flatten
Peaks each round. These settings are saved through Save Changes. The native
caller, bounds and Cavern construction are now verified; see
[TERRAIN_FOLLOWUP.md](TERRAIN_FOLLOWUP.md) for the binary proof, instruction
oracle and DOSBox comparison workflow. The tribute retains its own RNG, HUD
bounds, tank pads and import scaling policy rather than DOS RNG/pixel parity.

Stormy lightning strikes random positions during aiming, pausing for panels
and defensive setup. Bolts and thunder remain when **Hostile Environment** is
off, but cause no damage. With it on, shields absorb first; local tuning uses
35 damage within 24 pixels and 3–8 seconds between strikes. Environmental kills
and their secondary blasts award no player damage or kill points and incur no
suicide penalty. A surviving player keeps their un-fired turn. There are no
meteors.

**Mountains Folder** opens a path editor: Enter applies, Esc cancels, and an
empty path disables external mountains. Relative paths use the settings file's
directory. **Scanned Mountains** selects the percentage of rounds that load a
random `.MTN` or `.PNG` from that folder (default 20%). Files are scaled to the
playable field. PNG alpha below 128 is empty sky; for opaque PNGs, top-connected
pixels matching the top-left colour become sky. Other pixels are solid dirt
and keep their colours. Cavern may clear extra space beneath its ceiling.
Invalid files are skipped; if none work, a notice explains the fallback to
generated terrain. Limits: 16 MiB per file, 4096 pixels per side, 4M pixels total.
No original mountain assets are included.

Export software framebuffer previews without opening a window:

```sh
mkdir -p /tmp/scorched-landscapes
SCORCHED_LANDSCAPE_PREVIEW_DIR=/tmp/scorched-landscapes cargo test landscape_previews
```

## Sound

Main menu **Sound...** (`O`) enables these options:

- **Sound** (`S`, default on): toggles all audio. Switching it off stops the
  current voice and discards pending effects.
- **Flight Sounds** (`F`): Off (default), **POS** (higher shots have higher
  pitch), or **VEL** (faster shots have higher pitch). Alt reverses the cycle.
  Flight Off silences the traveling tone while firing and other effects remain
  enabled. Choose Save Changes to keep the settings.

The game synthesizes square-wave WAV buffers in memory and plays them through
macroquad's audio backend and the normal system audio output. Firing uses the
seven recovered frequencies from 1000 to 10000 Hz; shield hits beep at 1000 Hz.
Riot weapons use a siren, with distinct effects for explosions, nukes, deaths,
weapon splits, digging, dirt, disruption and menu/shop/panel actions.

One voice plays at a time, like the PC speaker. Effects interrupt the flight
tone, which resumes afterwards; simultaneous effects use priority instead of
mixing or accumulating a backlog. Sound never waits or changes shot physics.
No sound files are needed at runtime. Frequencies documented in the original
are retained; other effect patterns, timing and flight-pitch tuning are local
approximations awaiting side-by-side listening.

To export generated WAV previews without opening a game window:

```sh
mkdir -p /tmp/scorched-sounds
SCORCHED_SOUND_PREVIEW_DIR=/tmp/scorched-sounds cargo test export_generated_sound_previews
```

The directory includes individual effects, three flight tones and
`sound-demo.wav`. Listening on the target audio device remains a manual check.

## Presentation and mouse play

Baby Nuke and Nuke now fill their blast radius with red-and-black mottled
plasma. At normal firing delay they grow for one second, shimmer for two, then
vanish and leave the crater. Explosion Scale applies to their radius as usual.
Tank deaths randomly choose among eight effects: thud, one/two/three escalating
blasts, flame plumes, grey bubbles, hull fragments and reflected sparks. These
visual variants retain the existing death-blast damage and scoring rules.

A match can be played using the mouse:

- Left/right click **Power** or **Angle** to lower/raise it; hold to repeat.
- Click the **weapon name** to cycle; right click cycles backward.
- Press **both buttons together** to fire. Release both before firing again.
  Single battlefield clicks commit on release so a fire chord cannot open a panel.
- Left click the **player name** for Tank Control; right click for Inventory.
  Click a tank for its information.
- Use the visible buttons to page inventory, buy/sell supplies, adjust battery
  discharge or parachute thresholds, drive, cancel targeting, confirm retreat,
  and continue from standings. Click a destination to choose a guided target;
  right click selects the nearest tank within 100 pixels.
- Click **System** in the lower-left corner to pause, reassign players/teams,
  restart or quit. Confirmation dialogs have Confirm and Cancel buttons.

**Hardware → Fullscreen** enables fullscreen using integer scaling and
letterboxing. F11 toggles it immediately; Save Changes persists the option.
The internal resolution is 640×480.

To export framebuffer previews without opening a window:

```sh
SCORCHED_PRESENTATION_PREVIEW_DIR=/tmp/scorched-presentation cargo test
```

## Controls

| Key | Action |
|---|---|
| Left / Right | Barrel angle ±1° |
| Up / Down | Power ±1 |
| PgUp / PgDn | Power ±15 |
| Shift (hold) | Half the aiming repeat rate |
| Alt (hold) | One aiming step per key press |
| Tab / Shift+Tab | Next / previous weapon |
| Space / Enter | Fire |
| T / I | Tank Control / battlefield inventory |
| B / P | Battery / parachute toggle |
| S / E / G / F / - | Select shield / engage / guidance / fuel / triggers |
| U / K | Round status / No Kibitzing |
| 1–9 / 0 | Information about tank 1–10 from the left |
| R | Confirm retreat |
| F1 / Alt-S | Pause/resume System Menu |
| F11 | Toggle fullscreen |
| Shift-S / Shift-G | Previous shield / guidance |
| Esc | Close panel, otherwise back to menu (Esc in menu quits) |

## Layout

- `src/main.rs` – window, scaling the framebuffer to the screen
- `src/gfx.rs` – framebuffer, pixel primitives, bitmap font, panels and persistent path layer
- `src/terrain.rs` – pixel terrain, generation, craters, falling dirt and AI prediction overlays
- `src/landscape.rs` – reproducible mountain profiles and bounded MTN/PNG loaders
- `src/environment.rs` – sky backgrounds and storm lightning
- `src/game.rs` – game state, round/turn flow, world updates
- `src/settings.rs` – session options, TOML load/save and validation
- `src/economy.rs` – exact cash, interest, scoring and rankings
- `src/shop.rs` – purchases, inventory selling, filters and persistent Free Market
- `src/physics.rs` – shared ballistics, wind and settling rules
- `src/weapons.rs` – item catalogue, projectile physics, weapon chains, impacts and damage
- `src/inventory.rs` – item stock, limits and weapon selection
- `src/tank.rs` – tanks, controllers, supply state, battery use and geometry
- `src/controls.rs` – battlefield panels, accessory actions, status bar and retreat visuals
- `src/play.rs` – turn order, team outcomes and round/Mass Kill awards
- `src/modes.rs` – shared-keyboard maps/editor, queued-shot state, per-player effects and concurrent HUD
- `src/system.rs` – paused System Menu and confirmation flow
- `src/saved_game.rs` – versioned shop checkpoints, validation and atomic persistence
- `src/hall_of_fame.rs` – exact final-score records, per-mode rankings, persistence and board
- `src/window.rs` – one-time Linux/X11 startup activation
- `src/speech.rs` – attack/death phrase loading and weighted silence
- `src/presentation.rs` – death effects and nuclear plasma rendering
- `src/liquid.rs` – shared fluid flow, napalm pooling/burning and Liquid Dirt deposition
- `src/movement.rs` – driving geometry, slope limits and fuel spending
- `src/guidance.rs` – target selection, ballistic power, steering and Lazy Boy
- `src/energy.rs` – battery-powered plasma radiation and laser tracing
- `src/earth.rs` – riot effects, branching tunnels, dirt spray and forced collapse
- `src/shields.rs` – shield specifications, strength, contacts, forces and rendering
- `src/sound.rs` – procedural WAV synthesis, bounded events and single-voice playback
- `src/ui.rs` – menu input, HUD, dialogs and object drawing
- `src/ai.rs` – eight computer types, target/method selection, ranging memory, names, supplies and private prediction

See `FEATURES.md` for the original game's feature list and what's done so far,
`TODO.md` for the remaining work split into batches, and `MECHANICS.md` for the
original game's rules, formulas and numbers (from the manual and the binary).
