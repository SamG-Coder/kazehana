"""The classroom lockers' number tags: 1 - 18 on white card in a clear plastic holder, as a
school labels its back lockers (出席番号), on one sheet for tools/blender/build_school_lockers.py.

    python tools/make_locker_tags.py
-> assets/cache/signs/locker_tags.png (6 x 3 cells, 1024 x 1024; cell n at column (n-1) % 6,
   row (n-1) // 6)
"""

import os

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "signs", "locker_tags.png")
W, H = 1024, 1024
CW, CH = W // 6, H // 3

img = Image.new("RGB", (W, H), (236, 234, 228))
d = ImageDraw.Draw(img)
font = None
for f in ("C:/Windows/Fonts/YuGothB.ttc", "C:/Windows/Fonts/arialbd.ttf", "C:/Windows/Fonts/arial.ttf"):
    if os.path.exists(f):
        font = ImageFont.truetype(f, 150)
        break
for n in range(1, 19):
    c, r = (n - 1) % 6, (n - 1) // 6
    x0, y0 = c * CW, r * CH
    # the card: off-white, a printed border, the number in a school's blocky black
    d.rectangle([x0 + 6, y0 + 6, x0 + CW - 7, y0 + CH - 7], fill=(246, 245, 240), outline=(60, 60, 60), width=4)
    t = str(n)
    bb = d.textbbox((0, 0), t, font=font)
    d.text((x0 + (CW - (bb[2] - bb[0])) / 2 - bb[0], y0 + (CH - (bb[3] - bb[1])) / 2 - bb[1]), t, fill=(24, 24, 24), font=font)
img = img.filter(ImageFilter.GaussianBlur(0.6))
os.makedirs(os.path.dirname(OUT), exist_ok=True)
img.save(OUT)
print("wrote", OUT)
