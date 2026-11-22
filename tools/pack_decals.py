"""Pack Megascans decals (Fab texture sets) for the road shader.

    python tools/pack_decals.py   -> assets/decals/<id>_C.png, _N.png, _D.png + decals.json

Per decal, three textures (akari loads them as colour / normal / data):
  C: base colour (RGB) + opacity (A)
  N: normal map (RG)
  D: R = ambient occlusion, G = roughness, B = height (displacement)
decals.json lists them in the road shader's order (kind, physical size in metres).
"""

import glob
import json
import os

from PIL import Image

FAB = "D:/EpicVault/VaultCache/FabLibrary"
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "decals")
SIZE = 2048  # the engine keeps 2K; a 0.5 m crack at 2K is 0.25 mm per texel

KINDS = ["Asphalt_Crack", "Asphalt_Patch", "Asphalt_Pothole"]


def main():
    os.makedirs(OUT, exist_ok=True)
    out = []
    for kind in KINDS:
        for d in sorted(glob.glob(os.path.join(FAB, kind + "-*"))):
            meta = glob.glob(os.path.join(d, "**", "*.json"), recursive=True)
            meta = [m for m in meta if "metadata" not in m.replace("\\", "/").split("/")[-2:][0]]
            j = json.load(open(meta[0], encoding="utf8"))
            area = next((m["value"] for m in j.get("meta", []) if m.get("key") == "scanArea"), "1x1 m")
            w, h = (float(v) for v in area.replace(" m", "").split("x"))
            files = {}
            for f in glob.glob(os.path.join(d, "**", "*.jpg"), recursive=True):
                for tag in ("BaseColor", "Opacity", "Normal", "Roughness", "AO", "Displacement"):
                    if f.endswith(f"_{tag}.jpg"):
                        files[tag] = f
            if not all(t in files for t in ("BaseColor", "Opacity", "Normal", "Roughness")):
                print("skip", d)
                continue
            ident = os.path.basename(d).lower()
            # keep the decal's proportions: fit it in a square, the rest transparent padding
            fw, fh = (SIZE, max(4, round(SIZE * h / w))) if w >= h else (max(4, round(SIZE * w / h)), SIZE)
            pad = {"RGB": (128, 128, 255), "L": 0}

            def load(t, mode, fill=None):
                im = Image.open(files[t]).convert(mode).resize((fw, fh), Image.LANCZOS)
                if (fw, fh) == (SIZE, SIZE):
                    return im
                sq = Image.new(mode, (SIZE, SIZE), fill if fill is not None else pad[mode])
                sq.paste(im, ((SIZE - fw) // 2, (SIZE - fh) // 2))
                return sq
            c = load("BaseColor", "RGB", (0, 0, 0))
            a = load("Opacity", "L")
            c.putalpha(a)
            c.save(os.path.join(OUT, f"{ident}_C.png"))
            load("Normal", "RGB").save(os.path.join(OUT, f"{ident}_N.png"))
            ao = load("AO", "L", 255) if "AO" in files else Image.new("L", (SIZE, SIZE), 255)
            disp = load("Displacement", "L", 128) if "Displacement" in files else Image.new("L", (SIZE, SIZE), 128)
            Image.merge("RGB", (ao, load("Roughness", "L", 200), disp)).save(os.path.join(OUT, f"{ident}_D.png"))
            # size: the square the texture covers (its longer side)
            out.append({"id": ident, "kind": kind.split("_")[1].lower(), "size": max(w, h), "scan": [w, h]})
            print(f"{ident}: {kind} {w}x{h} m")
    json.dump(out, open(os.path.join(OUT, "decals.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
