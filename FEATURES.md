# Scorched Earth (1991, Wendell Hicken) – feature list

Checked against the original v1.5 manual (`SCORCH.DOC`). Details per feature are in `TODO.md`.

Status legend: [x] in prototype · [~] partial/simplified · [ ] not yet

## Core gameplay
- [x] Turn-based artillery: set angle + power, fire, hit the other tanks
- [x] 2–10 players on one screen (prototype: 2–8), hot-seat
- [x] Human or computer-controlled players
- [x] Multiple rounds; last tank standing wins the round
- [~] Scoring (prototype: kills and round wins only; original has money-based scores)
- [ ] Scoring modes: Basic / Standard / Greedy
- [ ] Teams: None / Standard / Corporate (shared bank) / Vicious
- [ ] Play order: Random, Losers-first, Winners-first, Round-robin
- [ ] Play modes: Sequential ([x] done), Synchronous, Simultaneous
- [x] Max power limited by tank health (power = health × 10)
- [ ] Tank movement using fuel ("Fuel Tank" accessory)

## Physics & environment
- [x] Gravity, ballistic trajectories
- [x] Wind (random per round)
- [ ] Changing wind (changes between turns), max wind setting
- [ ] Adjustable gravity and air viscosity
- [ ] Suspend Dirt, Tanks Fall, Borders Extend options
- [ ] Tunneling: shots burrow into dirt before exploding (on by default)
- [x] Destructible pixel terrain
- [x] Falling dirt after explosions
- [x] Tanks fall when ground is removed, taking fall damage
- [ ] Parachutes (prevent fall damage)
- [ ] Wall types: None, Concrete, Rubber, Spring, Wrap-around, Padded, Random, Erratic
      (prototype: none, shots leave the screen)
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
- [ ] Money earned for damage/kills, lost for self-damage
- [ ] Shop between rounds (buy weapons/accessories, sell back)
- [ ] Interest rate, starting cash settings
- [ ] Free market (prices move with demand)
- [ ] Arms level (limits which weapons are available)
- [~] Inventory (prototype: fixed starting ammo refilled every round)

## Computer players
- [x] Moron, [~] Shooter, [~] Spoiler (wind-aware prototype aiming; original AI algorithms are batch 16)
- [ ] Poolshark (banks off walls), Tosser (corrects aim), Chooser (picks best method), Cyborg (smart targeting)
- [ ] Unknown (random personality)
- [ ] AI buying in shop, using shields/parachutes

## Presentation
- [x] Low-res 640×480 framebuffer, chunky VGA look, 8×16 DOS-style font
- [x] Single-row bevelled HUD (power, 0–90 angle, player, weapon icon and ammo); wind in the sky
- [x] Explosions with colour cycling
- [x] Projectile trails
- [x] "Talking tanks" speech bubbles (own phrases, not the original's)
- [ ] Tank death animations (several different kinds in original)
- [ ] PC-speaker-style sound effects
- [ ] Original menus: Sound, Hardware, Economics, Physics, Landscape, Play Options, Weapons, Save Changes
- [ ] Mouse control
- [ ] Save/restore game (System Menu), Mass Kill, Retreat
- [ ] Tank Control Panel, Inventory Panel, Status Bar
- [ ] Configurable keys (6 per player in Simultaneous mode)
