"""Contact sheets of the door tour (kazehana --doortour <model>): one row per door, its four
shots (side a closed / open, side b closed / open), the door's name over the row; three doors to
a sheet.

    python tools/door_sheets.py <model>
-> shots/doors/<model>/sheet_NN.jpg
"""

import glob
import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
model = sys.argv[1]
d = os.path.join(ROOT, "shots", "doors", model)
names = dict(line.split(" ", 1) for line in open(os.path.join(d, "doors.txt"), encoding="utf8").read().splitlines() if line.strip())
W, H, PER = 480, 270, 3
font = ImageFont.truetype("C:/Windows/Fonts/arial.ttf", 16)
keys = sorted(names)
for s in range(0, len(keys), PER):
    sheet = Image.new("RGB", (W * 4, (H + 24) * PER), (20, 20, 20))
    dr = ImageDraw.Draw(sheet)
    for r, k in enumerate(keys[s:s + PER]):
        y = r * (H + 24)
        dr.text((6, y + 3), f"{k}  {names[k].split(' door ')[-1]}", fill=(240, 240, 240), font=font)
        for c, tag in enumerate(("a_closed", "a_open", "b_closed", "b_open")):
            f = os.path.join(d, f"{k}_{tag}.png")
            if os.path.exists(f):
                sheet.paste(Image.open(f).convert("RGB").resize((W, H)), (c * W, y + 24))
                dr.text((c * W + 6, y + 28), tag, fill=(255, 220, 80), font=font)
    sheet.save(os.path.join(d, f"sheet_{s // PER:02d}.jpg"), quality=82)
print(len(range(0, len(keys), PER)), "sheets")
