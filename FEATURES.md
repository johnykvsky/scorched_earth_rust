# Scorched Earth (1991, Wendell Hicken) – feature list

Checked against the original v1.5 manual (`SCORCH.DOC`). Details per feature are in `TODO.md`.

Status legend: [x] in prototype · [~] partial/simplified · [ ] not yet

## Core gameplay
- [x] Turn-based artillery: set angle + power, fire, hit the other tanks
- [x] 2–10 players on one screen, hot-seat
- [x] Human or computer-controlled players
- [x] Multiple rounds; last tank or team standing wins the round
- [x] Money-based scoring for damage, kills and survival; cash standings
- [x] Scoring modes: Basic / Standard / Greedy
- [x] Teams: None / Standard / Corporate (shared bank) / Vicious
- [x] Play order: Random, Losers-first, Winners-first, Round-robin
- [x] Fast Computers speeds battles when only computers remain
- [x] Play modes: Sequential, Synchronous queued volleys, Simultaneous shared-keyboard combat
- [x] Max power limited by tank health (power = health × 10)
- [x] Tank movement using fuel (slope costs, slips, parachutes and retained aiming turn)
- [x] Seven selectable tank shapes; fixed emplacement fuel restrictions and computer-only triple turret

## Physics & environment
- [x] Gravity, ballistic trajectories
- [x] Wind (random per round)
- [x] Changing wind (changes between turns), max wind setting
- [x] Adjustable gravity and air viscosity
- [x] Suspend Dirt, Tanks Fall, Borders Extend options
- [x] Tunneling: shots burrow into dirt before exploding (on by default; thin layers can be crossed)
- [x] Destructible pixel terrain
- [x] Falling dirt after explosions
- [x] Tanks fall when ground is removed; 2 damage per pixel, on landing or during descent
- [x] Parachutes: deployed/passive, safety threshold, predicted falls and 20 ms descent
- [x] Wall types: None, Concrete, Rubber, Spring, Wrap-around, Padded, Random, Erratic
      (shared live/AI collisions; rollers and child warheads included)
- [x] Sky types: Plain, Stormy, Stars, Shaded, Sunset, Cavern, Black, Random (Random excludes Black)
- [x] Original integer random-walk terrain, verified bounds and six solid dirt colours
- [x] Cavern's independent half-field floor/roof walks and global horizontal shading; binary oracle and DOSBox comparison
- [x] Stormy lightning, shield absorption, neutral scoring and harmless Hostile Environment off
- [x] Ground flattened under each tank at placement
- [x] Bumpiness, Slope, Flatten Peaks, Classic Terrain, per-round Random Land
- [x] User-selected mountains folder: MTN / PNG loading, percentage selection and fallback

## Weapons (original list)
- [x] Baby Missile (unlimited), [x] Missile, [x] Baby Nuke, [x] Nuke
- [x] Leap Frog (three sequential shrinking blasts)
- [x] Funky Bomb (opening burst, 5–10 curved trails and final blast)
- [x] MIRV (splits into warheads at apex)
- [x] Death's Head (nine apex warheads)
- [x] Napalm, Hot Napalm (downhill pooling, depth-based heat and fifty burn cycles)
- [x] Tracer, [x] Smoke Tracer (persistent path)
- [x] Baby Roller, [x] Roller, [x] Heavy Roller (roll off enemy shields)
- [x] Riot Charge, Riot Blast, Riot Bomb, Heavy Riot Bomb (harmless dirt removal; falls can hurt)
- [x] Baby Digger, Digger, Heavy Digger, Baby Sandhog, Sandhog, Heavy Sandhog (bounded branching tunnelers)
- [x] Dirt Clod, [x] Dirt Ball, [x] Ton of Dirt, [x] Liquid Dirt, [x] Dirt Charge (Extra Dirt supported)
- [x] Earth Disrupter (forces floating dirt to fall, overriding Suspend Dirt)
- [x] Plasma Blast, Laser (battery discharge, radiation, terrain cuts and shield interactions)

## Accessories / defense
- [x] Heat Guidance, Ballistic Guidance, Horizontal Guidance, Vertical Guidance
- [x] Lazy Boy (direct digging flight to a chosen point), Contact Trigger (one per shot chain)
- [x] Parachute, Battery (restores health), Fuel Tank movement
- [x] Shields: Mag Deflector, Shield, Force Shield, Heavy Shield, Super Mag
- [x] Shield interception, reflection, magnetic force, overflow, self-hit bypass and strength HUD
- [x] Super Mag stops lasers and converts their energy into shield charge (capped at 200)
- [x] Auto Defense: buy once, annuity pricing and defensive setup before each round

## Economy
- [x] Money earned for damage/kills/survival, lost for self/team damage and kills
- [x] Opening and between-round shops: categories, paging, buying, partial bundles, inventory selling and status
- [x] Full catalogue can be bought/stored/sold; all weapon and accessory effects are implemented
- [x] Interest rate, starting cash and Computers Buy settings; exact integer cash/interest
- [x] Free market (prices move with demand; saved between games)
- [x] Arms level and Useless Items shop filters
- [x] Persistent inventory: starter ammo once per game, 99-item limit, unlimited Baby Missiles

## Computer players
- [x] Moron, Shooter, Poolshark, Tosser, Chooser, Spoiler and Cyborg aiming/targeting personalities
- [x] Unknown rolls a hidden, stable personality once per game
- [x] Tosser real-shot memory per target; Cyborg hostility/leader/weakness targeting
- [x] Computer names avoid human and other computer names
- [x] Weighted-random Moron buying and defensive-priority buying for other types
- [x] Owned shield, parachute, battery, guidance/trigger and Riot dig-out use; bounded fuel movement
- [~] Exact original AI search limits, buying weights/order and tactical thresholds await DOSBox comparison

## Presentation
- [x] Low-res 640×480 framebuffer, chunky VGA look, 8×16 DOS-style font
- [x] Single-row bevelled HUD (power, 0–90 angle, player, weapon icon and ammo); wind in the sky
- [x] Explosions with colour cycling
- [x] Trace Paths, persistent Smoke Tracer, Bomb Icon modes and explosion Scale
- [x] Talking Tanks modes/probability, own phrases and custom attack/death files (blank/repeated line weighting)
- [x] Eight random tank death animations, preserving death-blast damage and credit
- [x] Full-radius red/black nuclear plasma: one-second growth, two-second shimmer, then crater
- [x] Optional integer-scaled fullscreen (Hardware/F11)
- [x] One-time keyboard focus request on Linux/X11 startup
- [x] Generated PC-speaker-style effects: firing, explosions, deaths, shields, earth weapons and clicks
- [x] Sound on/off and Off/POS/VEL flight tones, single-voice playback
- [x] Original menu layout, all seven option dialogs, Save Changes and TOML config (pending gameplay options are greyed out)
- [x] Mouse-only match controls: aiming/held adjustment, weapon cycling, both-button fire, panels, shop, prompts and standings
- [x] Paused System Menu, Clear Screen, exact Mass Kill, player/team reassignment, Sound, confirmed New Game/Quit
- [x] Esc confirmations before leaving the battlefield or quitting from the main menu
- [x] Start checkboxes for Simplified weapons (five weapons, no shops/money, victories) and temporary default options
- [x] Confirmed reset of all options and player setup from the main-menu System menu; Save Changes persists defaults
- [x] Save from the buying screen, restore through the System Menu, filename entry and overwrite confirmation
- [x] Persistent top-ten hall of fame per scoring mode, including Corporate banks and keyboard/mouse browsing
- [x] Coloured player editor with names, human/computer switching, icons and team assignment
- [x] Tank Control Panel, battlefield Inventory Panel, optional Status Bar
- [x] Status/tank information, No Kibitzing and confirmed helicopter retreat
- [x] Owned shield selection and engagement from panels, inventory and shortcuts
- [x] Guidance targeting, consumption/reset, steering and Contact Trigger shot effects; fuel movement and fixed-shape restrictions
- [x] Six configurable keys per human in Simultaneous mode, duplicate checks and settings persistence
