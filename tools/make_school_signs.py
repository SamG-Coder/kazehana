"""The school's signs (tools/blender/build_school.py):

    python tools/make_school_signs.py

-> assets/cache/signs/school_clock.png   (1024 x 1024): the facade clock's face, as on every
   Japanese school: white, black hour bars and numerals 3-6-9-12, the town's name small under
   the centre. (Its hands are separate in the building, set to 8:30.)
-> assets/cache/signs/school_name.png    (2048 x 1024, the plate in its top 288 rows): the name plate by the entrance: 風花町立
   風花中学校 (Kazehana Town Kazehana Junior High School) in cast bronze letters on a polished
   granite plate.
"""

import math
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "signs")
os.makedirs(OUT, exist_ok=True)
FONTS = "C:/Windows/Fonts"
BOLD = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), 120)

# ---- the clock face
N = 1024
im = Image.new("RGB", (N, N), (236, 236, 230))
d = ImageDraw.Draw(im)
c = N / 2
d.ellipse((8, 8, N - 8, N - 8), outline=(40, 40, 40), width=22)          # the rim
for k in range(60):
    a = math.radians(k * 6)
    r0 = c - 40 - (60 if k % 5 == 0 else 22)
    r1 = c - 40
    w = 22 if k % 5 == 0 else 7
    d.line((c + r0 * math.sin(a), c - r0 * math.cos(a), c + r1 * math.sin(a), c - r1 * math.cos(a)), fill=(25, 25, 25), width=w)
f = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), 110)
for n, k in ((12, 0), (3, 15), (6, 30), (9, 45)):
    a = math.radians(k * 6)
    r = c - 190
    t = str(n)
    bb = d.textbbox((0, 0), t, font=f)
    d.text((c + r * math.sin(a) - (bb[2] - bb[0]) / 2, c - r * math.cos(a) - (bb[3] - bb[1]) / 2 - bb[1]), t, font=f, fill=(25, 25, 25))
small = ImageFont.truetype(os.path.join(FONTS, "YuGothM.ttc"), 40)
t = "風花中学校"
bb = d.textbbox((0, 0), t, font=small)
d.text((c - (bb[2] - bb[0]) / 2, c + 120), t, font=small, fill=(60, 60, 60))
# a faint age: the face yellowed a little towards the edge
a = np.asarray(im).astype(np.float32) / 255
yy, xx = np.mgrid[0:N, 0:N]
rr = np.hypot(xx - c, yy - c) / c
a *= (1 - 0.06 * np.clip(rr, 0, 1) ** 2)[..., None] * np.array([1.0, 0.99, 0.95])
Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).save(os.path.join(OUT, "school_clock.png"))

# ---- the name plate: granite, bronze letters with a soft cast shadow
W, H = 2048, 288
rng = np.random.default_rng(3)
g = rng.normal(0, 1, (H // 2, W // 2)).astype(np.float32)
g = np.asarray(Image.fromarray(((g * 20 + 60).clip(0, 255)).astype(np.uint8)).resize((W, H)).filter(ImageFilter.GaussianBlur(1.2))).astype(np.float32)
stone = np.stack([g * 0.9, g * 0.9, g * 0.95], -1)
speck = rng.random((H, W)) < 0.03
stone[speck] += 60
plate = Image.fromarray(stone.clip(0, 255).astype(np.uint8))
text = "風花町立風花中学校"
mask = Image.new("L", (W, H), 0)
md = ImageDraw.Draw(mask)
size = 190
f = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), size)
bb = md.textbbox((0, 0), text, font=f)
while bb[2] - bb[0] > W - 160:
    size -= 6
    f = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), size)
    bb = md.textbbox((0, 0), text, font=f)
md.text(((W - (bb[2] - bb[0])) / 2 - bb[0], (H - (bb[3] - bb[1])) / 2 - bb[1]), text, font=f, fill=255)
shadow = mask.filter(ImageFilter.GaussianBlur(6)).point(lambda v: int(v * 0.6))
dark = Image.new("RGB", (W, H), (0, 0, 0))
plate = Image.composite(dark, plate, shadow.transform((W, H), Image.AFFINE, (1, 0, -6, 0, 1, -6)))
bronze = Image.new("RGB", (W, H), (120, 88, 48))
plate = Image.composite(bronze, plate, mask)
ImageDraw.Draw(plate).rectangle((0, 0, W - 1, H - 1), outline=(150, 150, 150), width=6)
# (on a 2048 x 1024 sheet, the plate across its top: textures are at least 1024 px each way)
sheet = Image.new("RGB", (W, 1024), (60, 60, 62))
sheet.paste(plate, (0, 0))
sheet.save(os.path.join(OUT, "school_name.png"))
print("wrote", OUT)

# ---- the room plates (室名札), the stair halls' floor signs, the exit sign, the music board:
# one 2048 x 2048 atlas of 512 x 128 cells, four across (school_plates.json lists each cell)
import json  # noqa: E402

PLATES = ["1年A組", "1年B組", "2年A組", "2年B組", "3年A組", "3年B組", "多目的室", "理科室", "理科準備室", "給食準備室", "保健室", "相談室",
          "校長室", "放送室", "職員室", "図書室", "パソコン室", "家庭科室", "音楽室", "音楽準備室", "美術準備室", "美術室", "技術室",
          "男子トイレ", "女子トイレ", "1F", "2F", "3F", "屋上", "非常口", "昇降口",
          "体育館", "男子更衣室", "女子更衣室", "1", "2", "3", "4", "5", "6", "プール"]
A = 2048
atlas = Image.new("RGB", (A, A), (235, 235, 230))
ad = ImageDraw.Draw(atlas)
cells = {}
for i, name in enumerate(PLATES):
    cx, cy = (i % 4) * 512, (i // 4) * 128
    exit_sign = name == "非常口"
    bg, fg = ((40, 150, 80), (250, 250, 245)) if exit_sign else (((30, 60, 120), (250, 250, 245)) if name in ("男子トイレ",) else (((170, 40, 50), (250, 250, 245)) if name == "女子トイレ" else ((246, 246, 240), (20, 20, 20))))
    ad.rectangle((cx + 4, cy + 4, cx + 507, cy + 123), fill=bg, outline=(110, 110, 110), width=5)
    size = 84
    font = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), size)
    bb = ad.textbbox((0, 0), name, font=font)
    while bb[2] - bb[0] > 470:
        size -= 4
        font = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), size)
        bb = ad.textbbox((0, 0), name, font=font)
    tx = cx + (512 - (bb[2] - bb[0])) / 2 - bb[0]
    if exit_sign:
        # the running figure (a simple pictogram) at the left, the words after it
        ad.ellipse((cx + 40, cy + 20, cx + 64, cy + 44), fill=fg)
        ad.line((cx + 52, cy + 44, cx + 44, cy + 80, cx + 24, cy + 108), fill=fg, width=12)
        ad.line((cx + 44, cy + 80, cx + 72, cy + 106), fill=fg, width=12)
        ad.line((cx + 28, cy + 56, cx + 72, cy + 60), fill=fg, width=10)
        ad.rectangle((cx + 92, cy + 18, cx + 120, cy + 110), outline=fg, width=6)
        tx += 50
    ad.text((tx, cy + (128 - (bb[3] - bb[1])) / 2 - bb[1]), name, font=font, fill=fg)
    cells[name] = [cx / A, 1 - (cy + 128) / A, (cx + 512) / A, 1 - cy / A]
atlas.save(os.path.join(OUT, "school_plates.png"))
json.dump(cells, open(os.path.join(OUT, "school_plates.json"), "w", encoding="utf8"), ensure_ascii=False, indent=1)

# ---- the music room's board: a green board with white staves
mb = Image.new("RGB", (2048, 1024), (24, 62, 46))
md = ImageDraw.Draw(mb)
for s in range(3):
    top = 140 + s * 300
    for l in range(5):
        y = top + l * 34
        md.line((60, y, 1988, y), fill=(225, 230, 225), width=4)
mb = mb.filter(ImageFilter.GaussianBlur(0.8))
mb.save(os.path.join(OUT, "school_music_board.png"))
print("wrote plates", len(cells))

# ---- the flags over the gym's stage: the Hinomaru, and the school's flag (maroon, a white
# disc with 風 in it, the school's name under)
fl = Image.new("RGB", (1536, 1024), (245, 245, 240))
ImageDraw.Draw(fl).ellipse((768 - 307, 512 - 307, 768 + 307, 512 + 307), fill=(188, 0, 45))
fl.save(os.path.join(OUT, "flag_japan.png"))
sf = Image.new("RGB", (1536, 1024), (110, 20, 35))
sd = ImageDraw.Draw(sf)
sd.ellipse((768 - 290, 440 - 290, 768 + 290, 440 + 290), fill=(240, 236, 225))
fk = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), 380)
bb = sd.textbbox((0, 0), "風", font=fk)
sd.text((768 - (bb[2] - bb[0]) / 2 - bb[0], 440 - (bb[3] - bb[1]) / 2 - bb[1]), "風", font=fk, fill=(110, 20, 35))
fn = ImageFont.truetype(os.path.join(FONTS, "YuGothB.ttc"), 96)
bb = sd.textbbox((0, 0), "風花中学校", font=fn)
sd.text((768 - (bb[2] - bb[0]) / 2 - bb[0], 800), "風花中学校", font=fn, fill=(240, 220, 150))
sf.save(os.path.join(OUT, "flag_school.png"))
print("wrote the flags")
