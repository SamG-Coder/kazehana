"""Sign artwork for the town's sites (tools/blender/build_site.py): the konbini's fascia, the
petrol station's canopy band and price pole, the drugstore's, the co-op's, the garage's, the post
office's and the town office's name boards, the station's name boards, the shrines' and the
temple's plaques. Every brand is the town's own (no real chain's name or livery).

    python tools/make_site_signs.py

-> assets/cache/signs/site_signs.png   (8192 x 8192 atlas, the pieces' places in site_signs.json)
"""

import json
import os
import random

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "signs")
FB, FM = "C:/Windows/Fonts/YuGothB.ttc", "C:/Windows/Fonts/YuGothM.ttc"
SIZE = 8192
WHITE, INK = (246, 246, 242), (24, 24, 26)


def text_fit(d, box, text, font, fill, max_size, spacing=0.06, vertical=False):
    x0, y0, x1, y1 = box
    w, h = x1 - x0, y1 - y0
    for size in range(max_size, 10, -4):
        f = ImageFont.truetype(font, size)
        if vertical:
            hs = [d.textbbox((0, 0), ch, font=f) for ch in text]
            total = sum(b[3] - b[1] for b in hs) + size * spacing * (len(text) - 1)
            if total < h * 0.9 and size < w * 0.85:
                y = y0 + (h - total) / 2
                for ch, b in zip(text, hs):
                    cw = d.textlength(ch, font=f)
                    d.text((x0 + (w - cw) / 2, y - b[1]), ch, font=f, fill=fill)
                    y += (b[3] - b[1]) + size * spacing
                return
        else:
            ws = [d.textlength(ch, font=f) for ch in text]
            total = sum(ws) + size * spacing * (len(text) - 1)
            bb = d.textbbox((0, 0), "国", font=f)
            if total < w * 0.9 and (bb[3] - bb[1]) < h * 0.8:
                x = x0 + (w - total) / 2
                y = y0 + (h - (bb[3] - bb[1])) / 2 - bb[1]
                for ch, cw in zip(text, ws):
                    d.text((x, y), ch, font=f, fill=fill)
                    x += cw + size * spacing
                return


def weather(im, seed, amount=1.0):
    rnd = random.Random(seed)
    w, h = im.size
    noise = Image.effect_noise((max(1, w // 8), max(1, h // 8)), 40).resize((w, h), Image.BICUBIC).filter(ImageFilter.GaussianBlur(4))
    px, nz = im.load(), noise.load()
    for y in range(h):
        grime = 0.18 * amount * max(0.0, (y / h - 0.8) / 0.2)
        for x in range(w):
            n = (nz[x, y] - 128) / 128.0 * 0.05 * amount
            px[x, y] = tuple(max(0, min(255, int(c * (1 - grime + n)))) for c in px[x, y][:3])
    return im


PIECES = []   # (key, w, h, painter)


def piece(key, w, h):
    def deco(fn):
        PIECES.append((key, w, h, fn))
        return fn
    return deco


@piece("konbini_fascia", 4096, 512)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=WHITE)
    for k, col in enumerate(((236, 120, 30), (40, 150, 70), (30, 90, 170))):       # three stripes
        d.rectangle((0, h - 150 + k * 45, w, h - 110 + k * 45), fill=col)
    text_fit(d, (0, 20, w, h - 170), "ひなたマート", FB, (40, 140, 60), 300)


@piece("konbini_pole", 512, 1024)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=WHITE)
    for k, col in enumerate(((236, 120, 30), (40, 150, 70), (30, 90, 170))):
        d.rectangle((0, h - 260 + k * 70, w, h - 200 + k * 70), fill=col)
    text_fit(d, (40, 40, w - 40, h - 300), "ひなた", FB, (40, 140, 60), 200, vertical=True)


@piece("petrol_band", 4096, 384)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(200, 30, 36))
    d.rectangle((0, h - 60, w, h - 40), fill=WHITE)
    text_fit(d, (0, 10, w, h - 70), "カゼハナ石油", FB, WHITE, 260)


@piece("petrol_pole", 512, 1536)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(200, 30, 36))
    text_fit(d, (20, 30, w - 20, 520), "カゼハナ", FB, WHITE, 140, vertical=True)
    d.rectangle((30, 600, w - 30, h - 30), fill=(20, 20, 22))
    font = ImageFont.truetype(FB, 64)
    for k, (name, price) in enumerate((("レギュラー", "174"), ("ハイオク", "185"), ("軽油", "154"))):
        y = 640 + k * 290
        d.text((60, y), name, font=font, fill=WHITE)
        d.text((80, y + 90), price, font=ImageFont.truetype(FB, 140), fill=(250, 200, 60))


@piece("drugstore_fascia", 4096, 640)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(30, 120, 170))
    d.rectangle((0, h - 70, w, h - 40), fill=(250, 210, 50))
    text_fit(d, (0, 30, w, h - 90), "ドラッグ はなみずき", FB, WHITE, 300)


@piece("coop_board", 4096, 512)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(236, 236, 228))
    d.rectangle((20, 20, w - 20, h - 20), outline=(20, 110, 60), width=16)
    text_fit(d, (0, 40, w, h - 40), "かぜはな農協 購買店", FB, (20, 110, 60), 260)


@piece("garage_board", 4096, 512)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(240, 236, 226))
    d.rectangle((0, 0, 120, h), fill=(30, 60, 130))
    d.rectangle((w - 120, 0, w, h), fill=(30, 60, 130))
    text_fit(d, (140, 30, w - 140, h - 30), "かぜはな自動車整備", FB, (30, 60, 130), 260)


@piece("post_board", 2048, 384)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=WHITE)
    d.rectangle((0, h - 40, w, h), fill=(200, 30, 30))
    text_fit(d, (0, 10, 400, h - 50), "〒", FB, (200, 30, 30), 300)
    text_fit(d, (380, 10, w, h - 50), "風花郵便局", FB, (200, 30, 30), 240)


@piece("office_board", 2048, 384)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(70, 70, 66))
    text_fit(d, (0, 20, w, h - 20), "風花町役場 支所", FB, (236, 232, 220), 220)


@piece("station_board", 4096, 512)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=WHITE)
    text_fit(d, (0, 20, w, h - 20), "風 花 駅", FB, INK, 360)


@piece("station_name", 2048, 640)
def _(im, d):
    # the platform's station name board: hiragana large, the romaji below, the neighbours
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=WHITE)
    d.rectangle((0, h - 170, w, h - 140), fill=(30, 110, 60))
    text_fit(d, (0, 20, w, 300), "かぜはな", FB, INK, 260)
    text_fit(d, (0, 300, w, 420), "Kazehana", FB, INK, 110)
    font = ImageFont.truetype(FM, 70)
    d.text((40, h - 120), "← しもかわ", font=font, fill=INK)
    d.text((w - 480, h - 120), "かみや →", font=font, fill=INK)


@piece("hachiman_plaque", 512, 1536)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(30, 28, 26))
    d.rectangle((24, 24, w - 24, h - 24), outline=(190, 160, 80), width=14)
    text_fit(d, (40, 60, w - 40, h - 60), "八幡神社", FB, (210, 180, 90), 360, vertical=True)


@piece("inari_plaque", 512, 1280)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(30, 28, 26))
    d.rectangle((24, 24, w - 24, h - 24), outline=(190, 160, 80), width=14)
    text_fit(d, (40, 60, w - 40, h - 60), "稲荷神社", FB, (210, 180, 90), 300, vertical=True)


@piece("temple_plaque", 2048, 512)
def _(im, d):
    w, h = im.size
    d.rectangle((0, 0, w, h), fill=(40, 30, 22))
    d.rectangle((24, 24, w - 24, h - 24), outline=(190, 160, 80), width=14)
    text_fit(d, (60, 40, w - 60, h - 40), "松林寺", FB, (210, 180, 90), 320)


def main():
    atlas = Image.new("RGB", (SIZE, SIZE), (60, 60, 60))
    index = {}
    x = y = row_h = 0
    for k, (key, w, h, fn) in enumerate(PIECES):
        if x + w > SIZE:
            x, y, row_h = 0, y + row_h, 0
        im = Image.new("RGB", (w, h))
        fn(im, ImageDraw.Draw(im))
        atlas.paste(weather(im, k, 0.6), (x, y))
        index[key] = [x, y, x + w, y + h]
        x += w
        row_h = max(row_h, h)
    os.makedirs(OUT, exist_ok=True)
    atlas.save(os.path.join(OUT, "site_signs.png"))
    json.dump(index, open(os.path.join(OUT, "site_signs.json"), "w"), indent=1)
    print(len(index), "pieces")


main()
