"""Sign artwork for Tsukumodō (つくも堂), the player's home shop (tools/blender/build_shop_tsukumodo.py).

    python tools/make_shop_signs.py

-> assets/cache/signs/tsukumodo_signs.png   (4096 x 4096 atlas: the fascia board, the projecting
                                              sign, three posters, the noren; layout in ATLAS)
   assets/cache/signs/tsukumodo_awning.png  (1024 x 1024: the striped awning canvas, tiling)

A Shōwa-era painted signboard: navy enamel, cream lettering, a thin cream border, weathered (sun
fade towards the top, grime along the bottom edge, small chips) so it never reads as a flat
graphic. Fonts: Yu Gothic Bold (Windows).
"""

import os
import random

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "signs")
FONT_B = "C:/Windows/Fonts/YuGothB.ttc"
FONT_M = "C:/Windows/Fonts/YuGothM.ttc"
NAVY = (28, 40, 70)
CREAM = (236, 226, 200)
RED = (176, 40, 34)


def weather(im, seed):
    """Sun fade towards the top, grime towards the bottom, speckle and a few chips."""
    rnd = random.Random(seed)
    w, h = im.size
    px = im.load()
    noise = Image.effect_noise((w // 8, h // 8), 40).resize((w, h), Image.BICUBIC).filter(ImageFilter.GaussianBlur(6))
    nz = noise.load()
    for y in range(h):
        fade = 0.10 * (1 - y / h)          # sun-bleached top
        grime = 0.22 * max(0.0, (y / h - 0.8) / 0.2)  # dirt washed down to the bottom edge
        for x in range(w):
            r, g, b = px[x, y][:3]
            n = (nz[x, y] - 128) / 128.0 * 0.06
            k = 1.0 + n - grime
            r = r * k + (255 - r) * fade
            g = g * k + (255 - g) * fade
            b = b * k + (255 - b) * fade
            px[x, y] = (int(max(0, min(255, r))), int(max(0, min(255, g))), int(max(0, min(255, b))))
    d = ImageDraw.Draw(im)
    for _ in range(int(w * h / 60000)):
        x, y = rnd.randrange(w), rnd.randrange(h)
        r = rnd.uniform(1.5, 5.0)
        d.ellipse([x - r, y - r, x + r, y + r], fill=(118, 112, 104))
    return im


def fascia():
    w, h = 4096, 512
    im = Image.new("RGB", (w, h), NAVY)
    d = ImageDraw.Draw(im)
    d.rectangle([18, 18, w - 19, h - 19], outline=CREAM, width=10)
    d.rectangle([40, 40, w - 41, h - 41], outline=CREAM, width=3)
    # the name, large, left of centre; a red seal-like mark before it
    big = ImageFont.truetype(FONT_B, 330)
    name = "つくも堂"
    tw = d.textlength(name, font=big)
    x0 = 420
    d.ellipse([150, 120, 150 + 270, 120 + 270], fill=RED)
    small = ImageFont.truetype(FONT_B, 150)
    d.text((150 + 135, 120 + 128), "九", font=small, fill=CREAM, anchor="mm")
    d.text((x0, h // 2 + 10), name, font=big, fill=CREAM, anchor="lm")
    # what the shop sells, right
    mid = ImageFont.truetype(FONT_B, 120)
    sub = ImageFont.truetype(FONT_M, 82)
    xr = x0 + tw + 180
    d.line([(xr - 90, 100), (xr - 90, h - 100)], fill=CREAM, width=4)
    d.text((xr, 170), "トレーディングカード", font=mid, fill=CREAM, anchor="lm")
    d.text((xr, 330), "ゲーム・ホビー・駄菓子", font=sub, fill=CREAM, anchor="lm")
    d.text((w - 150, h - 75), "TSUKUMODŌ", font=ImageFont.truetype(FONT_M, 46), fill=CREAM, anchor="rm")
    return weather(im, 1)


def vertical():
    w, h = 512, 2048
    im = Image.new("RGB", (w, h), CREAM)
    d = ImageDraw.Draw(im)
    d.rectangle([14, 14, w - 15, h - 15], outline=NAVY, width=10)
    # top band: red, "カード"
    d.rectangle([34, 34, w - 35, 420], fill=RED)
    f1 = ImageFont.truetype(FONT_B, 118)
    for k, ch in enumerate("カード"):
        d.text((w // 2, 112 + k * 112), ch, font=f1, fill=CREAM, anchor="mm")
    f2 = ImageFont.truetype(FONT_B, 330)
    for k, ch in enumerate("つくも堂"):
        d.text((w // 2, 640 + k * 350), ch, font=f2, fill=NAVY, anchor="mm")
    return weather(im, 2)


def poster(w, h, bg, fg, lines, seed):
    """A shop poster: bold lines of text on a flat colour (no card art: that is the card UI's)."""
    im = Image.new("RGB", (w, h), bg)
    d = ImageDraw.Draw(im)
    d.rectangle([12, 12, w - 13, h - 13], outline=fg, width=8)
    y = 90
    for text, size, col in lines:
        f = ImageFont.truetype(FONT_B, size)
        d.text((w // 2, y + size // 2), text, font=f, fill=col or fg, anchor="mm")
        y += int(size * 1.25)
    return weather(im, seed)


def noren(w, h, weathered=True):
    """The doorway curtain to the back: navy cotton, the shop's name in white, split in two.
    (`weathered`: with the signboards' paint weathering; the noren's own cloth textures,
    tools/make_noren_cloth.py, take it plain and age it as fabric.)"""
    im = Image.new("RGB", (w, h), (30, 38, 64))
    d = ImageDraw.Draw(im)
    f = ImageFont.truetype(FONT_B, 300 * w // 1500)
    d.text((w // 4, h // 2), "つく", font=f, fill=CREAM, anchor="mm")
    d.text((3 * w // 4, h // 2), "も", font=f, fill=CREAM, anchor="mm")
    return weather(im, 4) if weathered else im


# The sign atlas (4096 x 4096): where each piece is, as (col0, row0, col1, row1) in pixels from the
# top left. tools/blender/build_shop_tsukumodo.py maps its UVs from the same table.
ATLAS = {
    "fascia": (0, 0, 4096, 512),
    "vertical": (0, 512, 512, 2560),
    "poster_new": (700, 600, 1468, 1624),
    "poster_cup": (1600, 600, 2368, 1624),
    "poster_buy": (2500, 600, 3268, 1624),
    "noren": (700, 2700, 2200, 3700),
}


def atlas():
    im = Image.new("RGB", (4096, 4096), NAVY)
    put = lambda key, piece: im.paste(piece.resize((ATLAS[key][2] - ATLAS[key][0], ATLAS[key][3] - ATLAS[key][1])), ATLAS[key][:2])
    put("fascia", fascia())
    put("vertical", vertical())
    put("poster_new", poster(768, 1024, (178, 30, 28), (250, 214, 60), [("新弾", 230, None), ("入荷!", 200, None), ("ミスビースト", 90, CREAM), ("第3弾 好評発売中", 60, CREAM)], 5))
    put("poster_cup", poster(768, 1024, (24, 58, 120), CREAM, [("月例", 150, None), ("ショップ大会", 110, None), ("毎月 最終日曜", 80, (250, 214, 60)), ("13:00〜 参加費300円", 54, None)], 6))
    put("poster_buy", poster(768, 1024, (244, 206, 52), (30, 30, 30), [("カード", 150, None), ("買取", 230, (178, 30, 28)), ("強化中!", 150, None), ("シングル 1枚から", 58, None)], 7))
    put("noren", noren(1500, 1000))
    return im


def awning():
    """Awning canvas, 1024 px square, tiling: navy and cream stripes 128 px wide, a woven grain."""
    im = Image.new("RGB", (1024, 1024))
    d = ImageDraw.Draw(im)
    for k in range(8):
        d.rectangle([k * 128, 0, k * 128 + 127, 1023], fill=NAVY if k % 2 == 0 else CREAM)
    px = im.load()
    rnd = random.Random(3)
    for y in range(1024):
        for x in range(1024):
            r, g, b = px[x, y]
            k = 1.0 + (0.05 if (x // 3 + y // 3) % 2 == 0 else -0.05) + rnd.uniform(-0.03, 0.03)
            px[x, y] = (int(min(255, r * k)), int(min(255, g * k)), int(min(255, b * k)))
    return im


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    atlas().save(os.path.join(OUT, "tsukumodo_signs.png"))
    awning().save(os.path.join(OUT, "tsukumodo_awning.png"))
    print("wrote", OUT)
