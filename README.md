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

After `make build` you can start the game directly with `./target/release/scorched`,
or copy that single file anywhere; it has no other files to go with it.

The window opens at 1280×960 (the 640×480 game doubled) and can be resized;
the picture is scaled by whole numbers when it fits, keeping pixels sharp.

## How to play

In the menu use Up/Down to pick a line, Left/Right to change it (number of
players, number of rounds, and whether each player is Human or a computer:
Moron, Shooter or Spoiler), then Enter to start. Players take turns; the
current tank has a blinking arrow over it. The last tank standing wins the
round, and the final screen ranks players by rounds won, then kills. The first
player each round is random; subsequent turns follow tank positions from left
to right, wrapping at the edge.

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
| Esc | Back to menu (Esc in menu quits) |

## Layout

- `src/main.rs` – window, scaling the framebuffer to the screen
- `src/gfx.rs` – framebuffer, pixel primitives, bitmap font, bevelled panels
- `src/terrain.rs` – pixel terrain, generation, craters, falling dirt
- `src/game.rs` – game rules, weapons, projectiles, explosions, HUD, menu
- `src/ai.rs` – computer players (simulate shots, pick best, add noise)

See `FEATURES.md` for the original game's feature list and what's done so far,
`TODO.md` for the remaining work split into batches, and `MECHANICS.md` for the
original game's rules, formulas and numbers (from the manual and the binary).
