# Scorched Earth (1991, Wendell Hicken) – feature list

Checked against the original v1.5 manual (`SCORCH.DOC`). Details per feature are in `TODO.md`.

Status legend: [x] in prototype · [~] partial/simplified · [ ] not yet

## Core gameplay
- [x] Turn-based artillery: set angle + power, fire, hit the other tanks
- [x] 2–10 players on one screen, hot-seat
- [x] Human or computer-controlled players
- [x] Multiple rounds; last tank standing wins the round
- [x] Money-based scoring for damage, kills and survival; cash standings
- [x] Scoring modes: Basic / Standard / Greedy
- [ ] Teams: None / Standard / Corporate (shared bank) / Vicious
- [ ] Play order: Random, Losers-first, Winners-first, Round-robin
- [ ] Play modes: Sequential ([x] done), Synchronous, Simultaneous
- [x] Max power limited by tank health (power = health × 10)
- [ ] Tank movement using fuel ("Fuel Tank" accessory)

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
- [ ] Parachutes (prevent fall damage)
- [ ] Wall types: None, Concrete, Rubber, Spring, Wrap-around, Padded, Random, Erratic
      (prototype: none, shots are tracked within the Borders Extend margin)
- [~] Sky types: Plain, Stormy, Stars, Shaded, Sunset, Cavern, Black, Random (prototype: 4 gradient skies)
- [~] Random terrain shapes and colors (prototype: midpoint-displacement mountains, 4 palettes)
- [ ] Hostile environment (lightning from Stormy skies)
- [x] Ground flattened under each tank at placement
- [ ] Landscape settings: Bumpiness, Slope, Flatten Peaks (unused in the original v1.5; we plan to make them work), Random Land, scanned mountains

## Weapons (original list)
- [x] Baby Missile (unlimited), [x] Missile, [x] Baby Nuke, [x] Nuke
- [ ] LeapFrog
- [x] Funky Bomb
- [x] MIRV (splits into warheads at apex)
- [ ] Death's Head
- [ ] Napalm, Hot Napalm (burning liquid that flows downhill)
- [x] Tracer, [ ] Smoke Tracer
- [ ] Baby Roller, [x] Roller, [ ] Heavy Roller
- [ ] Riot Charge, Riot Blast, Riot Bomb, Heavy Riot Bomb (dirt removers)
- [ ] Baby Digger, Digger, Heavy Digger, Baby Sandhog, Sandhog, Heavy Sandhog (tunnelers)
- [ ] Dirt Clod, [x] Dirt Ball, [ ] Ton of Dirt, [ ] Liquid Dirt, [ ] Dirt Charge
- [ ] Earth Disrupter (makes all floating dirt fall)
- [ ] Plasma Blast, Laser

## Accessories / defense
- [ ] Heat Guidance, Ballistic Guidance, Horizontal Guidance, Vertical Guidance
- [ ] Lazy Boy (automatic aiming), Contact Trigger
- [ ] Parachute, Battery (restores health), Fuel Tank
- [ ] Shields: Mag Deflector, Shield, Force Shield, Heavy Shield, Super Mag
- [ ] Auto Defense

## Economy
- [x] Money earned for damage/kills/survival, lost for self/team damage and kills
- [~] Opening purchases with Cash at Start; inter-round shop, accessories and selling are pending
- [x] Interest rate, starting cash and Computers Buy settings; exact integer cash/interest
- [ ] Free market (prices move with demand)
- [ ] Arms level (limits which weapons are available)
- [x] Persistent inventory: starter ammo once per game, 99-item limit, unlimited Baby Missiles

## Computer players
- [x] Moron, [~] Shooter, [~] Spoiler (wind-aware prototype aiming; original AI algorithms are batch 16)
- [ ] Poolshark (banks off walls), Tosser (corrects aim), Chooser (picks best method), Cyborg (smart targeting)
- [ ] Unknown (random personality)
- [~] Bounded AI opening purchases; full buying personalities and shield/parachute use are pending

## Presentation
- [x] Low-res 640×480 framebuffer, chunky VGA look, 8×16 DOS-style font
- [x] Single-row bevelled HUD (power, 0–90 angle, player, weapon icon and ammo); wind in the sky
- [x] Explosions with colour cycling
- [x] Projectile trails
- [x] "Talking tanks" speech bubbles (own phrases, not the original's)
- [ ] Tank death animations (several different kinds in original)
- [ ] PC-speaker-style sound effects
- [x] Original menu layout, all seven option dialogs, Save Changes and TOML config (pending gameplay options are greyed out)
- [~] Mouse control (menus and controller setup; battlefield controls are pending)
- [ ] Save/restore game (System Menu), Mass Kill, Retreat
- [ ] Tank Control Panel, Inventory Panel, Status Bar
- [ ] Configurable keys (6 per player in Simultaneous mode)
