#!/usr/bin/env python3
"""Measure DOSBox/Rust terrain captures and produce an unpaired comparison sheet.

Requires Pillow. Reads capture_terrain.py's manifest.json and the
terrain_fidelity_previews test's manifest.toml. No original assets are bundled.
"""
import argparse
import json
import pathlib
import statistics
import sys
import tomllib

# The existing dis.py CLI must not shadow Python's stdlib dis during Pillow imports.
sys.path = [p for p in sys.path if pathlib.Path(p or ".").resolve() != pathlib.Path(__file__).resolve().parent]
from PIL import Image, ImageDraw, ImageFont

EARTH = [(154,101,69), (219,146,113), (215,215,190),
         (81,251,81), (36,142,36), (146,219,113)]


def measure(folder, sample, source):
    image = Image.open(folder / sample["file"]).convert("RGB")
    width,height = image.size
    if (width,height) != (640,480):
        raise ValueError("Use 640x480 captures for direct coordinate comparison")
    top = sample.get("top",24)
    bottom = sample.get("bottom",476)
    middle = (top + bottom) // 2
    palette = image.crop((3,top,width-3,bottom+1)).getcolors(maxcolors=4096)
    if palette is None: raise ValueError("Expected a VGA-sized capture palette")
    colors = {color:count for count,color in palette}
    earth = max(EARTH,key=lambda color: colors.get(color,0))
    pixels = image.load()
    if source == "DOSBox" and colors.get((255,40,40),0) < 8:
        raise ValueError(f"Incomplete original battlefield in {sample['file']}")
    cavern = sample["sky"].lower() == "cavern"
    first = middle + 40 if cavern else top + 40
    floor = [next(y for y in range(first,bottom+1) if pixels[x,y] == earth)
             for x in range(3,width-3)]
    result = dict(source=source,sky=sample["sky"].lower(),file=sample["file"],
                  earth=earth,top=top,bottom=bottom,left=floor[0],right=floor[-1],
                  minimum=min(floor),maximum=max(floor),
                  mean=statistics.fmean(floor),relief=max(floor)-min(floor),
                  mean_abs_grade=statistics.fmean(abs(a-b) for a,b in zip(floor,floor[1:])),
                  equal_ends=floor[0] == floor[-1])
    if cavern:
        ceiling = [next(y for y in range(top+40,middle+1) if pixels[x,y] != earth)
                   for x in range(3,width-3)]
        result.update(ceiling_mean=statistics.fmean(ceiling),minimum_gap=min(f-c for f,c in zip(floor,ceiling)),
                      ceiling_minimum=min(ceiling),ceiling_maximum=max(ceiling))

    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--original",type=pathlib.Path,required=True)
    parser.add_argument("--rust",type=pathlib.Path,required=True)
    parser.add_argument("--output",type=pathlib.Path,required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    original = json.loads((args.original/"manifest.json").read_text())
    rust = tomllib.loads((args.rust/"manifest.toml").read_text())
    records = [measure(folder,sample,source) for folder,samples,source in [
        (args.original,original["samples"],"DOSBox"),(args.rust,rust["samples"],"Rust")]
        for sample in samples]
    summaries = []
    for sky in ["plain","cavern"]:
        for source in ["DOSBox","Rust"]:
            group = [r for r in records if r["source"] == source and r["sky"] == sky]
            summary = dict(source=source,sky=sky,count=len(group))
            for field in ["mean","relief","mean_abs_grade","equal_ends"]:
                summary[field] = statistics.mean(r[field] for r in group)
            summary["minimum"] = min(r["minimum"] for r in group)
            summary["maximum"] = max(r["maximum"] for r in group)
            if sky == "cavern":
                summary["ceiling_mean"] = statistics.mean(r["ceiling_mean"] for r in group)
                summary["minimum_gap"] = min(r["minimum_gap"] for r in group)
            summaries.append(summary)
    (args.output/"measurements.json").write_text(json.dumps(dict(
        exe_sha256=original["exe_sha256"],summary=summaries,samples=records),indent=2)+"\n")
    text = ["# Terrain fidelity comparison", "",
            "Independent seeds; this is a distribution/visual comparison, not matching saved RNG state.", "",
            "| Source | Sky | Samples | Mean floor y | Mean relief | Mean abs. grade | Equal ends |",
            "|---|---|---:|---:|---:|---:|---:|"]
    for r in summaries:
        text.append(f"| {r['source']} | {r['sky']} | {r['count']} | {r['mean']:.2f} | "
                    f"{r['relief']:.2f} | {r['mean_abs_grade']:.3f} | {100*r['equal_ends']:.1f}% |")
    text += ["", "Native pixel units; y increases downward. Both are 640x480, measuring x=3..636 and top=24.",
             "DOS uses an inner bottom of 476; the tribute fills its viewport through 479.",
             "DOS captures include two tank pads; Rust samples are bare terrain before placement.",
             "RNGs differ. Exact algorithm parity is checked separately with instruction-oracle fixtures.", "",
             "Contact-sheet pairs use matching earth palettes, selected nearest their group's median relief.",
             "Each row contains independent shapes; columns are DOSBox and Rust."]
    (args.output/"comparison.md").write_text("\n".join(text)+"\n")

    sheet = Image.new("RGB",(960,4*390),(24,24,30))
    draw = ImageDraw.Draw(sheet)
    try: font = ImageFont.truetype("DejaVuSans.ttf",16)
    except OSError: font = ImageFont.load_default()
    row = 0
    for sky in ["plain","cavern"]:
        left = [r for r in records if r["source"] == "DOSBox" and r["sky"] == sky]
        right = [r for r in records if r["source"] == "Rust" and r["sky"] == sky]
        common = [earth for earth in EARTH if any(tuple(r["earth"]) == earth for r in left)
                  and any(tuple(r["earth"]) == earth for r in right)]
        if len(common)<2: raise ValueError("Need at least two shared palettes per sky")
        for color in common[:2]:
            for col, group, folder in [(0,left,args.original),(1,right,args.rust)]:
                palette = [r for r in group if tuple(r["earth"]) == color]
                median = statistics.median(r["relief"] for r in palette)
                selected = min(palette,key=lambda r: abs(r["relief"]-median))
                with Image.open(folder/selected["file"]) as image:
                    sheet.paste(image.convert("RGB").resize((480,360),Image.Resampling.NEAREST),
                                (col*480,row*390+30))
                draw.text((col*480+8,row*390+5),f"{selected['source']}  {sky}  {selected['file']}",fill="white",font=font)
            row += 1
    sheet.save(args.output/"contact-sheet.png")
    print("\n".join(text))


if __name__ == "__main__":
    main()
