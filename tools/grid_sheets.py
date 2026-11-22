"""Contact sheets of the aerial grid tour (kazehana --gridtour all), each cell labelled with what
the census puts there (tools/census_check.py -> shots/grid/census.json): its houses by type, the
places, and what stands empty. Cells with nothing in the census are sheeted too (fields, forest,
roads: the graphics still get reviewed).

    python tools/grid_sheets.py [rows per sheet]
-> shots/grid/sheet_rNN.jpg (one grid row a sheet, west to east), shots/grid/index.jpg (the whole
   town at a glance)
"""

import json
import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
G = os.path.join(ROOT, "shots", "grid")
census = json.load(open(os.path.join(G, "census.json"), encoding="utf8")) if os.path.exists(os.path.join(G, "census.json")) else {}
ROWS, COLS = 12, 15
font = ImageFont.truetype("C:/Windows/Fonts/meiryo.ttc", 15) if os.path.exists("C:/Windows/Fonts/meiryo.ttc") else ImageFont.truetype("C:/Windows/Fonts/arial.ttf", 15)
big = ImageFont.truetype("C:/Windows/Fonts/arial.ttf", 22)


def label(key):
    v = census.get(key)
    if not v:
        return [key + "  (nothing in the census)"]
    lines = [f"{key}  {v['people']} people"]
    if v["houses"]:
        lines.append("houses: " + ", ".join(f"{k} {n}" for k, n in v["houses"].items()))
    if v["empty_houses"]:
        lines.append("EMPTY: " + ", ".join(f"{k} {n}" for k, n in v["empty_houses"].items()))
    for p in v["empty_places"]:
        lines.append("EMPTY SITE: " + p)
    return lines


# one sheet a grid row: 15 cells, 5 x 3
W, H = 640, 360
for r in range(ROWS):
    sheet = Image.new("RGB", (W * 5, (H + 70) * 3), (18, 18, 18))
    dr = ImageDraw.Draw(sheet)
    for c in range(COLS):
        key = f"r{r:02}_c{c:02}"
        x, y = (c % 5) * W, (c // 5) * (H + 70)
        f = os.path.join(G, key + ".png")
        if os.path.exists(f):
            sheet.paste(Image.open(f).convert("RGB").resize((W, H)), (x, y + 70))
        for i, line in enumerate(label(key)[:4]):
            dr.text((x + 6, y + 2 + i * 17), line[:80], fill=(255, 110, 90) if line.startswith("EMPTY") else (235, 235, 235), font=font)
    sheet.save(os.path.join(G, f"sheet_r{r:02}.jpg"), quality=80)

# the whole town, small
w, h = 192, 108
idx = Image.new("RGB", (w * COLS, h * ROWS), (0, 0, 0))
d = ImageDraw.Draw(idx)
for r in range(ROWS):
    for c in range(COLS):
        f = os.path.join(G, f"r{r:02}_c{c:02}.png")
        if os.path.exists(f):
            idx.paste(Image.open(f).convert("RGB").resize((w, h)), (c * w, r * h))
        v = census.get(f"r{r:02}_c{c:02}")
        col = (255, 90, 70) if v and (v["empty_houses"] or v["empty_places"]) else (240, 240, 240)
        d.rectangle((c * w, r * h, c * w + w - 1, r * h + h - 1), outline=(60, 60, 60))
        d.text((c * w + 4, r * h + 2), f"{r},{c}", fill=col, font=big)
idx.save(os.path.join(G, "index.jpg"), quality=85)
print("sheets written")
