"""Signboards for the town centre's shops, one per census shop (assets/town/census.json, the
places of kind Shop), for tools/blender/build_shop_signs.py.

    python tools/make_town_shop_signs.py

-> assets/cache/signs/town_shop_signs.png   (8192 x 8192 atlas: 3 x 24 boards, each 2720 x 340,
                                              about 320 px a metre on a 9 m frontage's board)
   assets/cache/signs/town_shop_signs.json  (house id -> the board's place in the atlas, its text)

Each board says what the census says the shop is: the family's name and the trade, the way a
country-town shop writes it (山本ベーカリー, 岡田書店, 喫茶 アオキ, 理容 シミズ...), in the colours
that trade tends to use (a barber's red, white and blue; a tofu shop's indigo; a kissaten's dark
brown), painted enamel weathered by sun and rain like Tsukumodō's board. Fonts: Yu Gothic (Windows).
"""

import json
import os
import random

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "signs")
FONT_B = "C:/Windows/Fonts/YuGothB.ttc"
FONT_M = "C:/Windows/Fonts/YuGothM.ttc"
TW, TH, COLS, ROWS = 2720, 340, 3, 24
SIZE = 8192

FAMILY = {
    "Mori": ("森", "モリ"), "Okada": ("岡田", "オカダ"), "Yamamoto": ("山本", "ヤマモト"), "Fujii": ("藤井", "フジイ"),
    "Endō": ("遠藤", "エンドウ"), "Sasaki": ("佐々木", "ササキ"), "Wada": ("和田", "ワダ"), "Ishikawa": ("石川", "イシカワ"),
    "Murakami": ("村上", "ムラカミ"), "Kobayashi": ("小林", "コバヤシ"), "Fujita": ("藤田", "フジタ"), "Yoshida": ("吉田", "ヨシダ"),
    "Watanabe": ("渡辺", "ワタナベ"), "Kaneko": ("金子", "カネコ"), "Shimizu": ("清水", "シミズ"), "Aoki": ("青木", "アオキ"),
    "Saitō": ("斎藤", "サイトウ"), "Kimura": ("木村", "キムラ"), "Kondō": ("近藤", "コンドウ"), "Shibata": ("柴田", "シバタ"),
    "Ueda": ("上田", "ウエダ"), "Nakano": ("中野", "ナカノ"), "Ikeda": ("池田", "イケダ"), "Miura": ("三浦", "ミウラ"),
    "Nishimura": ("西村", "ニシムラ"), "Hayashi": ("林", "ハヤシ"), "Morita": ("森田", "モリタ"), "Hasegawa": ("長谷川", "ハセガワ"),
    "Kudō": ("工藤", "クドウ"), "Miyazaki": ("宮崎", "ミヤザキ"), "Hara": ("原", "ハラ"), "Takeuchi": ("竹内", "タケウチ"),
    "Hashimoto": ("橋本", "ハシモト"), "Suzuki": ("鈴木", "スズキ"),
}
# trade -> (how the sign reads, from the family's kanji k / katakana kk; ground, lettering, accent)
NAVY, CREAM, RED, WHITE = (28, 40, 70), (236, 226, 200), (176, 40, 34), (240, 238, 230)
TRADE = {
    "bicycle shop": (lambda k, kk: f"{k}自転車店", (34, 74, 48), WHITE, (220, 190, 60)),
    "bookshop": (lambda k, kk: f"{k}書店", (24, 52, 96), CREAM, None),
    "bakery": (lambda k, kk: f"{kk}ベーカリー", (196, 150, 90), (60, 34, 20), (150, 40, 30)),
    "clothes shop": (lambda k, kk: f"{k}洋品店", (230, 228, 220), (30, 40, 90), (170, 30, 40)),
    "kimono shop": (lambda k, kk: f"呉服 {k}", (60, 22, 30), (230, 210, 160), None),
    "watch and glasses shop": (lambda k, kk: f"{k}時計メガネ店", (20, 60, 110), WHITE, (230, 200, 60)),
    "tea shop": (lambda k, kk: f"{k}茶舗", (46, 70, 40), (226, 214, 170), None),
    "futon shop": (lambda k, kk: f"{k}ふとん店", (230, 226, 214), (110, 40, 40), None),
    "liquor shop": (lambda k, kk: f"{k}酒店", (20, 20, 22), (234, 220, 180), RED),
    "coffee shop (kissaten)": (lambda k, kk: f"喫茶 {kk}", (58, 36, 24), (232, 206, 160), None),
    "photo studio": (lambda k, kk: f"{k}写真館", (232, 230, 224), (20, 20, 20), (200, 60, 40)),
    "old pharmacy": (lambda k, kk: f"{k}薬局", (230, 232, 228), (20, 110, 70), (20, 110, 70)),
    "tofu shop": (lambda k, kk: f"{k}豆腐店", (30, 44, 84), WHITE, None),
    "barber": (lambda k, kk: f"理容 {kk}", WHITE, (30, 40, 110), RED),
    "stationery shop": (lambda k, kk: f"{k}文具店", (240, 236, 226), (30, 70, 140), (220, 170, 40)),
    "butcher": (lambda k, kk: f"{k}精肉店", (150, 30, 32), WHITE, None),
    "ramen shop": (lambda k, kk: f"ラーメン {k}", (170, 30, 26), (250, 230, 200), (240, 200, 60)),
    "rice shop": (lambda k, kk: f"{k}米穀店", (236, 230, 210), (40, 40, 40), (150, 110, 40)),
    "hardware shop": (lambda k, kk: f"{k}金物店", (60, 64, 70), (240, 220, 120), None),
    "izakaya": (lambda k, kk: f"居酒屋 {k}", (24, 22, 20), (236, 228, 210), RED),
    "wagashi (sweets) shop": (lambda k, kk: f"{k}菓子舗", (110, 36, 60), (240, 226, 200), None),
    "florist": (lambda k, kk: f"{k}生花店", (236, 236, 228), (40, 110, 60), (200, 60, 110)),
    "tatami maker": (lambda k, kk: f"{k}畳店", (70, 84, 44), (236, 228, 196), None),
    "greengrocer": (lambda k, kk: f"{k}青果店", (40, 110, 50), WHITE, (240, 140, 40)),
    "hair salon": (lambda k, kk: f"ヘアサロン {kk}", (240, 236, 236), (140, 60, 90), None),
    "electrical shop": (lambda k, kk: f"{k}電器店", (20, 90, 160), WHITE, RED),
}


def weather(im, seed):
    """Sun fade towards the top, grime towards the bottom, speckle and a few chips."""
    rnd = random.Random(seed)
    w, h = im.size
    noise = Image.effect_noise((max(1, w // 8), max(1, h // 8)), 40).resize((w, h), Image.BICUBIC).filter(ImageFilter.GaussianBlur(4))
    px, nz = im.load(), noise.load()
    for y in range(h):
        fade = 0.12 * (1 - y / h)
        grime = 0.25 * max(0.0, (y / h - 0.75) / 0.25)
        for x in range(w):
            r, g, b = px[x, y][:3]
            n = (nz[x, y] - 128) / 128.0 * 0.07
            k = 1 - grime + n
            px[x, y] = tuple(max(0, min(255, int(c * k + (255 - c) * fade))) for c in (r, g, b))
    d = ImageDraw.Draw(im)
    for _ in range(rnd.randint(4, 10)):
        x, y = rnd.randrange(w), rnd.randrange(h)
        rr = rnd.randint(2, 6)
        d.ellipse((x - rr, y - rr, x + rr, y + rr), fill=(70, 66, 60))
    return im


def board(text, ground, ink, accent, seed):
    im = Image.new("RGB", (TW, TH), ground)
    d = ImageDraw.Draw(im)
    # a thin border in the lettering's colour, an accent stripe at each end if the trade has one
    d.rectangle((16, 16, TW - 17, TH - 17), outline=ink, width=10)
    if accent:
        for x0 in (44, TW - 104):
            d.rectangle((x0, 44, x0 + 60, TH - 45), fill=accent)
    # the name as big as fits, letter-spaced, centred
    for size in range(248, 80, -8):
        f = ImageFont.truetype(FONT_B, size)
        sp = size * 0.12
        widths = [d.textlength(ch, font=f) for ch in text]
        total = sum(widths) + sp * (len(text) - 1)
        if total < TW - (340 if accent else 180) and size < TH - 80:
            break
    x = (TW - total) / 2
    bb = d.textbbox((0, 0), "店", font=f)
    y = (TH - (bb[3] - bb[1])) / 2 - bb[1]
    for ch, wch in zip(text, widths):
        if ch != " ":
            d.text((x, y), ch, font=f, fill=ink)
        x += wch + sp
    return weather(im, seed)


def main():
    census = json.load(open(os.path.join(ROOT, "assets", "town", "census.json"), encoding="utf8"))
    shops = [p for p in census["places"] if p["kind"] == "Shop"]
    if len(shops) > COLS * ROWS:
        raise SystemExit(f"{len(shops)} shops, the atlas holds {COLS * ROWS}")
    atlas = Image.new("RGB", (SIZE, SIZE), (40, 40, 40))
    index = {}
    for k, p in enumerate(sorted(shops, key=lambda p: p["house"])):
        fam, trade = p["name"].split(" ", 1)
        if fam not in FAMILY or trade not in TRADE:
            raise SystemExit(f"no sign wording for {p['name']!r}")
        kanji, kana = FAMILY[fam]
        fn, ground, ink, accent = TRADE[trade]
        text = fn(kanji, kana)
        col, row = k % COLS, k // COLS
        x0, y0 = col * TW, row * TH
        atlas.paste(board(text, ground, ink, accent, p["house"]), (x0, y0))
        index[str(p["house"])] = {"text": text, "shop": p["name"], "px": [x0, y0, x0 + TW, y0 + TH]}
    os.makedirs(OUT, exist_ok=True)
    atlas.save(os.path.join(OUT, "town_shop_signs.png"))
    json.dump(index, open(os.path.join(OUT, "town_shop_signs.json"), "w", encoding="utf8"), ensure_ascii=False, indent=1)
    print(len(index), "boards ->", os.path.join(OUT, "town_shop_signs.png"))


main()
