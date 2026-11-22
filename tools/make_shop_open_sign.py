"""Tsukumodō's open / closed sign: the two faces of the board that hangs in the window by the door
(営業中 OPEN on one side, 準備中 CLOSED on the other), as small Japanese shops hang them: a
printed acrylic plate on a cord, white lettering on a colour.

    python tools/make_shop_open_sign.py
-> assets/cache/signs/shop_open_sign.png (1024 x 1024: in its top half, the open face on the
   left, the closed face on the right; tools/blender/build_shop_open_sign.py)
"""

import os

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "signs", "shop_open_sign.png")


def font(size, bold=True):
    for f in (("C:/Windows/Fonts/YuGothB.ttc" if bold else "C:/Windows/Fonts/YuGothM.ttc"), "C:/Windows/Fonts/meiryob.ttc", "C:/Windows/Fonts/meiryo.ttc"):
        if os.path.exists(f):
            return ImageFont.truetype(f, size)
    return ImageFont.load_default()


img = Image.new("RGB", (1024, 512), (240, 238, 232))  # (padded to 1024 square at the end: the gate wants 1024 px)
d = ImageDraw.Draw(img)
for k, (bg, jp, en) in enumerate((((28, 110, 72), "営業中", "OPEN"), ((176, 40, 38), "準備中", "CLOSED"))):
    x0 = k * 512
    # the plate: its colour, a thin white rule inset, rounded corners
    d.rounded_rectangle([x0 + 8, 8, x0 + 503, 503], radius=36, fill=(240, 238, 232))
    d.rounded_rectangle([x0 + 20, 20, x0 + 491, 491], radius=28, fill=bg)
    d.rounded_rectangle([x0 + 36, 36, x0 + 475, 475], radius=20, outline=(250, 248, 240), width=5)
    f1, f2 = font(150), font(64)
    for t, f, y in ((jp, f1, 120), (en, f2, 330)):
        bb = d.textbbox((0, 0), t, font=f)
        d.text((x0 + 256 - (bb[2] - bb[0]) / 2 - bb[0], y - bb[1]), t, fill=(252, 250, 244), font=f)
    # the cord's two holes
    for hx in (140, 372):
        d.ellipse([x0 + hx - 9, 52, x0 + hx + 9, 70], fill=(30, 30, 30))
img = img.filter(ImageFilter.GaussianBlur(0.7))
# (the faces in the top half of a 1024 square sheet; the bottom half unused)
sheet = Image.new("RGB", (1024, 1024), (20, 20, 20))
sheet.paste(img, (0, 0))
img = sheet
os.makedirs(os.path.dirname(OUT), exist_ok=True)
img.save(OUT)
print("wrote", OUT)
