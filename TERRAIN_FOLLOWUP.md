# Follow-up: how the original builds its terrain

Open research left over from `MECHANICS.md` §5.3 and §18. Pick this up when we
work on TODO batch 19 (terrain), or before calling the terrain section of
`MECHANICS.md` complete.

## What we already know

- **Generator core** ✅ `2d4f:0645` (with recursion `2d4f:071d`). This is the
  only midpoint-displacement routine in `SCORCH.EXE`:
  ```
  generate(leftH, rightH, n, H, amp):
      N = 2^n − 1;  a[0] = leftH;  a[N] = rightH;  f = 1 / 2^H
      rec(0, N, amp·f, f)
  rec(lo, hi, amp, f):
      mid = (lo + hi) / 2;  stop if mid == lo or mid == hi
      a[mid] = (a[lo] + a[hi]) / 2 + (random(17) − 8) / 8 × amp
      rec(lo, mid, amp·f, f);  rec(mid, hi, amp·f, f)
  ```
  Points are doubles in an array at far pointer `DS:cf60`. Constants 8.0 at
  `DS:1f5e`, 2.0 at `DS:1f5a`. `random(n)` = `2bf9:048b`; random double in
  0…1 = `2bf9:00f7`; `pow` = `1000:1077`.
- **Its only caller** ✅ `2d90:002e` (call at `2d9b:004b`). It receives two
  vertical bounds as arguments `[bp+0xc]` and `[bp+0x10]`, then sets:
  - `mid = (bound1 + bound2) / 2`;
  - `n = 6 − random(3)` (16–64 segments);
  - `H = random double in 0…1`;
  - `amp = random(mid / 4)`;
  - both end heights = `mid`.

  It then joins the points with straight lines across the width. It also checks
  a field `+8 == 1` on an object passed in `[bp+6]` (a "land style"?).
- **Nothing calls `2d90:002e` directly.** No far or near call, no far pointer
  in the main data segment, and no immediate offset/segment pair in code
  points to it. So it's reached through a function pointer (probably a virtual
  method of a landscape object) that we haven't located.
- **Bumpiness, Slope and Flatten Peaks are never read by terrain code** ✅. Only
  the config code, the menu, Random Land and the title picture (`46bc:004a`)
  touch them. DOSBox runs at 0/0, 100/0, 0/100, 100/100 and Flatten Peaks on
  looked alike.
- **Round setup** ✅ `323a:04de` picks the dirt colour scheme (`random(6)`, table
  at `DS:5036`) and the wall type. It is a good place to look for the call that
  builds the landscape.

## Open questions

1. **End heights and vertical range.** What are the two bounds passed to
   `2d90:002e`, and are the end heights really both `mid`? (Screenshots show
   different left and right edge heights, so something else is going on:
   maybe a wider curve that's clipped, or the result is adjusted afterwards.)
   How is the **Cavern ceiling** line made: the same routine called a second
   time with other bounds?
2. **The function pointer.** Which object, method table and call site lead to
   `2d90:002e`? Finding it answers question 1, because the caller pushes the
   bounds.

## Four ways to find out

### 1. Check the reverse-engineering projects first (cheapest)

- DigitalCyberSoft `scorch/terrain.py`
  (<https://github.com/DigitalCyberSoft/scorchedearth-python>) and gosuwachu
  `src/terrain.ts` (<https://github.com/gosuwachu/scorched-earth>) cite
  `FUN_2d4f_0645`/`_071d` and note `cf60[0] == cf60[last]`.
- gosuwachu says Slope's real effect "lives in the unrecovered terrain-style
  layer (`FUN_323a_*` vtable build)", which is a strong hint about where the
  pointer is set.
- Look for: how they pick the bounds and end heights, and any cited addresses
  for the caller or vtable. **Verify every claim in the disassembly** before
  putting it in `MECHANICS.md`. Both projects have had wrong guesses before.

### 2. Search harder for the function pointer (static)

- Search the **whole file**, not just the main data segment, for far pointers
  to `2d90:002e`. The same address can be written as many segment:offset
  pairs; the linear address is `(0x2d90 − 0x1000) × 16 + 0x2e`. Borland
  vtables may sit in another segment.
- Look at **constructors that store far pointers into objects**, e.g.
  `mov word es:[bx+…], offset` followed by `mov word es:[bx+…+2], segment`, in
  the `323a` module (round setup) and around the menu preview code
  (`46bc:003d` calls `323a:049c`, `323a:0bcb`, `323a:083f`). Note
  `32b5:006d` writes `0x50a0` into an object, which may be a vtable pointer
  (`DS:50a0`).
- Check `DS:50a0` and nearby for tables of far pointers whose targets land in
  segment `2d4f`–`2d90`.
- Find the code that calls through those tables (`call far [bx+…]` or
  `call dword ptr es:[bx+…]`) during round setup.
- Tools: `tools/re/dis.py`, `tools/re/refs.py`, `tools/re/callers.py`. Use
  plain byte search (`bytes.find`), **not** `re.escape`, which silently missed
  matches earlier.

### 3. Watch it run in a debugger (definitive)

- Needs a DOSBox build with the debugger. **DOSBox-X** has one built in;
  classic DOSBox needs a special "debug" build. The installed DOSBox 0.74 has
  no debugger. DOSBox-X is **not** in Manjaro's official repos (checked
  2026-10-13); it's in the AUR (`dosbox-x` or `dosbox-x-sdl2`), or available as
  a Flatpak (`com.dosbox_x.DOSBox-X`). You'd install it yourself.
- Set a breakpoint on the generator's entry. In a running game the real segment
  is the load segment + the game's PSP offset, so read the actual CS from the
  debugger once (e.g. break on `int 21h` at startup, or search memory for the
  routine's first bytes `C8 12 00 00 56 57`).
- When it hits:
  - read the arguments on the stack (`leftH`, `rightH`, `n`, `H`, `amp`);
  - step out to see the caller and the bounds it passed;
  - walk up the stack to find the function-pointer call site (question 2).
- Do the same with **Sky = Cavern** to see whether it runs twice, and with what
  bounds.

### 4. Measure the original's output (fallback, and useful anyway)

- Generate ~30 landscapes per setup in the original with
  `tools/re/run_original.sh`, and measure from the screenshots:
  - left and right edge heights;
  - highest and lowest points;
  - number of straight segments;
  - steepest slope.
- This gives typical ranges to tune our generator against. It won't give the
  formula, but it's exactly the side-by-side check TODO batch 19 asks for
  ("default terrain looks like the original's").
- Setup:
  ```sh
  mkdir -p ~/games/scorch-test && cd ~/games/scorch-test
  unzip /path/to/SCORCH15.zip            # from archive.org, msdos_SCORCH15_shareware
  # run once, press "c" on the main menu to write SCORCH.CFG, then quit
  export GAME_DIR=~/games/scorch-test
  tools/re/run_original.sh /tmp/land01.png MAXPLAYERS=2 MTN_PERCENT=0.000000 SKY=Plain
  ```
  Set `MTN_PERCENT=0.000000` so no scanned mountains appear. With
  `INITIAL_CASH` above 0 the shop opens first, and the script doesn't handle
  that.
- The battlefield in the default 360×480 mode is captured at 720×480
  (horizontal pixels doubled).

## When done

- Put the answers into `MECHANICS.md` §5.3 with `seg:off` references, and
  remove items 1–2 from `MECHANICS.md` §18.
- Reword the remaining §18 item ("per-frame drawing speed") as a note: the
  original ran as fast as the PC allowed, so there is no single value to find.
- Update TODO batch 19's terrain tasks with the real end-height and range rules.
