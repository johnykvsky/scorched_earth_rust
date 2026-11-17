#!/usr/bin/env python3
"""Capture unmodified v1.5 Plain/Cavern rounds in a private DOSBox copy.

Requires dosbox, xdotool, ImageMagick import and Pillow. Controls only the
window belonging to the process it starts. The source shareware directory
must already have SCORCH.CFG (change an option and Save Changes in DOSBox).
Example: capture_terrain.py --game-dir /path/to/scorch --output /tmp/reference --samples 30
"""
import argparse
from collections import Counter
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

# The existing dis.py CLI must not shadow Python's stdlib dis during Pillow imports.
sys.path = [p for p in sys.path if pathlib.Path(p or ".").resolve() != pathlib.Path(__file__).resolve().parent]
from PIL import Image


def battlefield_ready(image):
    image = image.convert("RGB")
    neutral = sum(max(image.getpixel((x, 8))) - min(image.getpixel((x, 8))) < 3
                  for x in range(image.width)) / image.width
    if image.height != 480 or neutral < .75:
        return False
    # The HUD is drawn before Cavern generation. The first player's red hull
    # appears only once terrain generation and placement have finished.
    palette = image.crop((3,24,image.width-3,477)).getcolors(maxcolors=4096)
    return palette is not None and sum(count for count,color in palette if color == (255,40,40)) >= 8


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-dir", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--resolution", default="640x480", choices=["360x480", "640x480"])
    parser.add_argument("--boot-seconds", type=float, default=6.0)
    args = parser.parse_args()
    if not 1 <= args.samples <= 100:
        parser.error("--samples must be between 1 and 100")
    source = args.game_dir.resolve()
    if not (source / "SCORCH.CFG").is_file():
        parser.error("Source needs a SCORCH.CFG saved by the original game")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = dict(exe_sha256=hashlib.sha256((source / "SCORCH.EXE").read_bytes()).hexdigest(),
                    resolution=args.resolution, samples=[])
    manifest_path = output / "manifest.json"
    if manifest_path.exists():
        previous = json.loads(manifest_path.read_text())
        if (previous["exe_sha256"], previous["resolution"]) != (manifest["exe_sha256"], manifest["resolution"]):
            raise ValueError("Existing manifest belongs to another executable/resolution")
        manifest = previous
        validated = []
        for sample in manifest["samples"]:
            path = output / sample["file"]
            if path.is_file():
                with Image.open(path) as image:
                    if battlefield_ready(image): validated.append(sample)
        manifest["samples"] = validated
        manifest_path.write_text(json.dumps(manifest,indent=2)+"\n")
    with tempfile.TemporaryDirectory(prefix="scorched-terrain-capture-") as temporary:
        game = pathlib.Path(temporary)
        for path in source.iterdir():
            if path.is_file() and path.suffix.upper() in (".EXE", ".CFG", ".MTN"):
                shutil.copy2(path, game / path.name)
        baseline = (game / "SCORCH.CFG").read_text().splitlines()
        config = game / "dosbox.conf"
        config.write_text(f"""[sdl]
fullscreen=false
output=surface
[render]
scaler=normal2x
[cpu]
cycles=20000
[mixer]
nosound=true
[autoexec]
mount c {game}
c:
SCORCH.EXE
exit
""")
        for sky in ["Plain", "Cavern"]:
            values = dict(GRAPHICS_MODE=args.resolution, MAXPLAYERS="2", MAXROUNDS="2",
                          SKY=sky, MTN_PERCENT="0.000000", RANDOM_LAND="Off", SOUND="Off",
                          HOSTILE_ENVIRONMENT="Off", MAX_WIND="0", INITIAL_CASH="0",
                          LAND1="20", LAND2="20", FLATLAND="Off", PLAY_MODE="Sequential",
                          STATUS_BAR="Off", TALKING_TANKS="Off")
            edited = [line.split("=", 1)[0] + "=" + values.get(*line.split("=", 1))
                      if "=" in line else line for line in baseline]
            (game / "SCORCH.CFG").write_bytes(("\r\n".join(edited) + "\r\n").encode())
            for index in range(args.samples):
                destination = output / f"{sky.lower()}-{index:02}.png"
                if any(sample["file"] == destination.name for sample in manifest["samples"]):
                    continue
                process = subprocess.Popen(["dosbox", "-conf", str(config)],
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                try:
                    windows = subprocess.check_output(["xdotool", "search", "--sync", "--onlyvisible",
                                                       "--pid", str(process.pid)], timeout=15, text=True)
                    window = windows.splitlines()[0]
                    time.sleep(args.boot_seconds)
                    subprocess.run(["xdotool", "windowfocus", "--sync", window], check=True, timeout=10)
                    entered = set()
                    attempts = Counter()
                    for retry in range(60):
                        subprocess.run(["import", "-window", window, str(destination)], check=True, timeout=10)
                        with Image.open(destination) as opened:
                            image = opened.convert("RGB")
                            neutral = sum(max(image.getpixel((x, 8))) - min(image.getpixel((x, 8))) < 3
                                          for x in range(image.width)) / image.width
                            if battlefield_ready(image):
                                time.sleep(.6)
                                subprocess.run(["import", "-window", window, str(destination)], check=True, timeout=10)
                                with Image.open(destination) as confirmed:
                                    if battlefield_ready(confirmed): break
                                continue
                            if image.height == 480 and neutral >= .75:
                                time.sleep(.5)  # terrain is still painting; send no keys
                                continue
                            backdrop = image.getpixel((8,100))
                            panel = image.getpixel((image.width//2-90,170))
                            if max(backdrop)-min(backdrop)<20:
                                # A modal options panel has a broad grey centre;
                                # the main menu has the mountain picture here.
                                grey = sum(image.getpixel((x,y)) == (182,182,182)
                                           for x in range(290,350) for y in range(210,270))
                                keys = ["Escape"] if grey > 1000 else ["s"]
                            elif max(panel)-min(panel)<=3:
                                player = 1 if backdrop[0] > backdrop[1] else 2
                                attempts[player] += 1
                                keys = []
                                if player not in entered or attempts[player] % 4 == 0:
                                    keys.append("a" if player == 1 else "b")
                                    entered.add(player)
                                # Inspect after EACH Enter. Never send Return to
                                # the next screen without confirming it is setup.
                                keys.append("Return")
                            else:
                                time.sleep(1)
                                continue
                        subprocess.run(["xdotool", "windowfocus", "--sync", window], check=True, timeout=10)
                        for key in keys:
                            subprocess.run(["xdotool", "keydown", "--window", window, key], check=True)
                            time.sleep(.16)
                            subprocess.run(["xdotool", "keyup", "--window", window, key], check=True)
                            time.sleep(.8)
                    else:
                        raise RuntimeError(f"Cannot finish player setup for {destination}")
                    manifest["samples"].append(dict(sky=sky, file=destination.name, settings=values))
                    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
                    print(f"Captured {sky} {index + 1}/{args.samples}: {destination}", flush=True)
                finally:
                    process.terminate()
                    try:
                        process.wait(timeout=4)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


if __name__ == "__main__":
    main()
