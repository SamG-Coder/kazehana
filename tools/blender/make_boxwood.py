"""The boxwood hedge asset: the Megascans Boxwood (Fab), each of its nine shapes at its most
detailed LOD within the foliage triangle budget (docs/ASSET_QUALITY.md: 80k).

    blender --background --factory-startup --python tools/blender/make_boxwood.py

-> assets/plants/boxwood_hedge/boxwood_hedge.gltf (+ .bin, textures), meshes named VarA..VarI.
"""

import os
import re

import bpy

SRC = "D:/EpicVault/VaultCache/FabLibrary/Boxwood-ae90090d/gltf/high/boxwood_wfypfa3ha_ue_hig_extracted/standard/wfypfa3ha_tier_1_nonUE.gltf"
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "assets", "plants", "boxwood_hedge", "boxwood_hedge.gltf")
BUDGET = 80000

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=SRC)

by_var = {}
for ob in bpy.data.objects:
    if ob.type != "MESH":
        continue
    m = re.match(r"SM_\w+?_(Var[A-Z])(?:_LOD(\d))?$", ob.name.split(".")[0])
    if not m:
        continue
    lod = int(m.group(2) or 0)
    tris = sum(len(p.vertices) - 2 for p in ob.data.polygons)
    by_var.setdefault(m.group(1), []).append((lod, tris, ob))

keep = []
for var, lods in sorted(by_var.items()):
    lods.sort(key=lambda t: t[0])
    pick = next((t for t in lods if t[1] <= BUDGET and t[0] < 3), None)
    if pick is None:
        continue
    lod, tris, ob = pick
    ob.name = f"SM_boxwood_{var}"
    keep.append(ob)
    print(f"{var}: LOD{lod}, {tris} triangles")

# drop everything else (other LODs, the billboard)
for ob in list(bpy.data.objects):
    if ob.type == "MESH" and ob not in keep:
        bpy.data.objects.remove(ob, do_unlink=True)

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True, export_apply=False)
print("wrote", OUT)
