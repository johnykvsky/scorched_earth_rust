# Terrain fidelity follow-up — resolved 2026-11-17

The previous implementation and research notes identified the wrong live
routine. Scorched Earth v1.5's ordinary terrain uses an **integer random walk**,
not the separate midpoint-displacement module. This follow-up corrects the
implementation, caller/bounds claims and Cavern construction in `MECHANICS.md`
§5.3. The two native-terrain questions formerly in §18 are resolved.

## Evidence and address convention

Source: the shareware [SCORCH15.zip](https://archive.org/download/msdos_SCORCH15_shareware/SCORCH15.zip).
The inspected `SCORCH.EXE` is 415,456 bytes, SHA-256
`05e4a11643ec7055b1c811531a73c5b4f5c67c463612f3e4910e557301c3179f`.
The original EXE, mountain files and phrase files remain outside this repo.

Code addresses below use `tools/re/dis.py`'s Ghidra convention: load-relative
segments plus `0x1000`. Literal far pointers in the EXE contain load-relative
segments, e.g. `223a:0a68` points to the routine labelled `323a:0a68` here.
`DS:` offsets use the data block at file offset `0x55d80`.

## Why the old investigation stalled

The midpoint wrapper at `2d90:002e` really allocates `2^n` points, seeds equal
endpoints and calls the recursive midpoint generator at `2d4f:0645`. However,
there is no relocated pointer to that wrapper and no direct caller found by
`tools/re/callers.py`. The earlier claim that an unidentified virtual method
*must* call it was an inference, not a verified fact.

Following the **actual object construction** resolves the question:

1. `323a:0c88` constructs the global DefaultLand at `DS:d4be` using `323a:0b6b`.
2. `323a:0ba0` installs **vtable `DS:508c`**. Its first pointer is
   `68 0a 3a 22`, a relocated pointer to **`323a:0a68`**.
3. The constructor writes **20, 20, 0** at object offsets `+0a,+0c,+0e`.
   LAND1, LAND2 and FLATLAND are not copied into the object.
4. Round setup `323a:0711–0758` chooses the global default object when no scanned
   mountain is selected, then calls renderer `323a:083f` with the graphics bounds.
5. Renderer `323a:0975` calls the object's first virtual method, the integer walk.

`DS:50a0`, suggested in the old follow-up, is the **abstract base** vtable.
Base constructor `323a:07f5` installs it before the DefaultLand constructor
replaces it. Following the override, rather than stopping at the base class,
identifies the concrete method. The title picture constructs a separate stack-local
DefaultLand/ScannedMountain (`46bc:0008–003d`), not the global round generator.

## Recovered rules

The full pseudocode and references are in `MECHANICS.md` §5.3.

- Initial y: `random(ymax − ymin − 60) + ymin + 40`; initial grade is zero.
- Each column changes grade with 20% probability; its −1/0/+1 acceleration
  doubles with a further 20% probability. Grade is accumulated across columns.
- The upper height limit is `ymin + 40`, with upward motion reset to **+1**
  when it hits that limit. At `ymax − 1`, positive grade resets to **0**.
- End heights are independent results of the walk. No equal-endpoint rule.
- The object alternates horizontal direction each time it renders native land.
- In the normal 640×480 capture, graphics bounds are x=3..636, y=24..476.
  The original's three-pixel frame and toolbar determine these bounds.
- Cavern splits y at its midpoint. Generate the lower-half floor **first**,
  then an independent upper-half roof using the same RNG stream and direction.
  The roof is filled upward and the floor downward. Minimum native gap: 41 px.
- The Cavern gradient is fixed by global y, not distance to local surfaces.
  Its 30 VGA shades use `(i+8)/45` of the round's earth colour.
- The six actual dirt colours at `DS:5036` are uniform fills, not depth bands.

Our non-Classic sliders now expose the original generator class's parameters:
Bumpiness changes the probability of grade changes, Slope the probability of
doubling acceleration, and Flatten Peaks resets grades outside −3..3 to zero.
This is an enhancement: v1.5 fixes those fields at 20/20/off. Classic ignores
all three settings and preserves those constructor values.

## Instruction oracle

`tools/re/terrain_oracle.py` reads the matching executable and disassembles
`323a:0980–0b6a`. A deliberately small 16-bit interpreter executes the original
integer generation and plot/clipping instructions. Only RNG and video driver
calls are replaced. Unknown instructions/calls abort. Constructor bytes, the
vtable relocation and the absence of a relocated midpoint-wrapper pointer are
checked before generation.

The injected RNG is the tribute's xorshift64, **not a reconstruction of DOS's
RNG**. This isolates the geometry/control flow for exact comparisons. The
committed `tests/fixtures/terrain-oracle.toml` contains derived numeric profiles
only: normal/reverse walks, Cavern's consecutive floor/roof RNG streams, extreme
HUD bounds, flat input and flattening behavior. Rust tests compare every column.

Reproduce without adding shareware files to the repo:

```sh
python3 tools/re/terrain_oracle.py /path/to/SCORCH.EXE > /tmp/terrain-oracle.toml
cmp tests/fixtures/terrain-oracle.toml /tmp/terrain-oracle.toml
cargo test random_walk_matches_original_instruction_oracle
```

The interpreter is specific to this routine, not a general DOS emulator. Its
comparison does not establish original RNG seeds, artwork or complete VGA/UI
pixel parity.

## DOSBox comparison

`tools/re/capture_terrain.py` runs an **unmodified EXE** in disposable copies.
It selects two humans, 640×480, Plain/Cavern, 20/20/off, no scanned mountains,
no random land, no wind and no hostile environment. Its controller inspects
each screen before Enter and controls only its own DOSBox process/window.
A manifest records the executable hash and settings; successful captures resume
without being overwritten. Slow title rendering/setup fades are handled by
screen-state checks. No personal DOSBox configuration or source game is edited.

```sh
python3 tools/re/capture_terrain.py --game-dir /path/to/scorch \
  --output /tmp/scorched-terrain-reference --samples 30
SCORCHED_TERRAIN_FIDELITY_DIR=/tmp/scorched-terrain-updated cargo test terrain_fidelity_previews
python3 tools/re/compare_terrain.py --original /tmp/scorched-terrain-reference \
  --rust /tmp/scorched-terrain-updated --output /tmp/scorched-terrain-comparison
```

The source directory needs a CFG saved by the original game. Change a setting
and choose Save Changes if it is greyed out. Capture dependencies: DOSBox,
xdotool, ImageMagick `import`, Pillow. Live GUI
execution needs the environment's normal GUI approval.

The comparison uses independent seeds. Original screenshots include two tank
pads; the Rust export is bare terrain. The tribute uses its own HUD and fills
to y=479, while the original's inner frame ends at 476. These distinctions are
reported by the measurement tool rather than hidden in a pixel-difference score.
Screenshots/contact sheets remain in `/tmp`; source assets are not committed.

## Recorded comparison (2026-11-17)

All **60 unmodified DOSBox captures** passed the completed-battlefield check:
30 Plain and 30 Cavern at 640×480. These were compared with 30 Rust renders
for each sky. The contact sheet was visually inspected after incomplete
frames were rejected and replaced. The captured red hull is required below
the toolbar before acceptance; seeing only the HUD is insufficient during
Cavern generation.

| Source | Sky | Samples | Mean floor y | Mean relief | Mean absolute grade |
|---|---|---:|---:|---:|---:|
| DOSBox | plain | 30 | 267.31 | 411.00 | 2.248 |
| Rust | plain | 30 | 267.37 | 403.73 | 2.247 |
| DOSBox | cavern | 30 | 384.36 | 185.00 | 1.638 |
| Rust | cavern | 30 | 388.20 | 187.00 | 1.755 |

Units are native pixels. These are descriptive measurements, not a
statistical equivalence test: RNG seeds differ, the original includes tank
pads, and the two viewports have different bottom borders. Original edges
were unequal in every captured Plain landscape, contradicting the old
equal-endpoint target. Cavern floor/roof geometry and global shading now
visually agree with the recovered algorithm.

Artifacts from this run: `/tmp/scorched-terrain-reference` (screenshots and
manifest), `/tmp/scorched-terrain-updated` (Rust renders), and
`/tmp/scorched-terrain-comparison` (measurements, comparison report and contact
sheet). Re-run the commands above if temporary files are no longer available.

Validation: **315 Rust tests pass**, including exact instruction-oracle
columns, known original Cavern VGA shades, reverse direction, independent
ceilings, native clamps, Classic/default equivalence, slider behavior, roof
settling and ten tank pads at both ordinary and tall multiplayer HUD bounds.
The release build, Rust formatting, diff whitespace and Python compilation
checks pass. No original executable, mountain or phrase data was added to git.

## Adaptations and limits

The recovered generator replaces the arbitrary 55–75% midpoint and the old
short roof/96-pixel gap. Native Cavern now uses the recovered two half-field
walks. The tribute supplies its own RNG, full-viewport bounds and tank pads;
it does not reproduce a DOS session from its RNG seed. Its generation direction
is kept with the game, independent of menu previews.

MTN/PNG loading remains our existing bounded decoder and scaling policy.
Cavern combined with imported art keeps ground in the lower half so the native
roof leaves placement room. That import policy is local; this follow-up does
not claim to reconstruct ScannedMountain's complete cropping/mapping logic.
Other sky artwork, storm timing and rendering-speed approximations retain
their existing documented limits. CPU-dependent per-frame drawing speed is a
note, not a missing universal constant to discover.
