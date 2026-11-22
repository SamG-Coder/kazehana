"""Tsukumodō's open / closed sign (営業中 / 準備中): a printed acrylic plate hung on a cord from a
suction hook on the display window by the door. Uncle Kenji turns it round when he opens and
closes the shop (the game turns the prop: town::scene TownWorld::shop_open).

    python tools/make_shop_open_sign.py   (first)
    blender --background --factory-startup --python tools/blender/build_shop_open_sign.py -- shop_open_sign

-> assets/props/shop_open_sign/shop_open_sign.gltf

Frame (Blender): origin at the plate's bottom middle; the OPEN face is its front (-Y), the CLOSED
face its back (+Y); the hook 0.12 m above the plate's top.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fixture_kit as fk  # noqa: E402
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PID = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "shop_open_sign"
OUT = os.path.join(ROOT, "assets", "props", PID, PID + ".gltf")
FACES = os.path.join(ROOT, "assets", "cache", "signs", "shop_open_sign.png")

W, H, T = 0.26, 0.26, 0.004        # the plate (an acrylic plate 26 cm square, 4 mm)
HOOK = H + 0.12

kz.reset_scene()
COLL = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(COLL)
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
edge_m = P("open_sign_edge", (0.86, 0.85, 0.80), DETAIL, roughness=0.2, normal_strength=0.02, grain=0.02)
cord_m = P("open_sign_cord", (0.75, 0.72, 0.64), DETAIL, roughness=0.8, normal_strength=0.3, grain=0.3)
hook_m = kz.plain_material("open_sign_sucker_clear_glass", (0.90, 0.93, 0.93), roughness=0.1)
hook_metal = P("open_sign_hook", (0.72, 0.72, 0.70), DETAIL, roughness=0.3, metallic=0.9, normal_strength=0.05, grain=0.05)


def faces_material():
    m = bpy.data.materials.new("open_sign_faces")
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(FACES, "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    _, n, _ = kz.fab_textures(DETAIL)
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(n, "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = 0.01
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    bsdf.inputs["Roughness"].default_value = 0.18   # (printed acrylic: a little gloss)
    return m


faces_m = faces_material()
b = kz.MeshBuilder(PID, uv_scale=4.0)
# the plate's edge (the acrylic's thickness), then its two printed faces
b.box((-W / 2, -T / 2 + 0.0005, 0.0), (W / 2, T / 2 - 0.0005, H), edge_m)
# the cord: through the two holes, up to the hook in a V
# (the holes are printed at 27% and 73% across, 12% down: tools/make_shop_open_sign.py)
for sx in (-1, 1):
    hx = sx * W * 0.227
    hz = H * (1 - 61 / 512)
    fk.tube(b, [(hx, 0.0, hz), (hx, 0.0, H + 0.004), (hx * 0.5, 0.0, H + 0.06), (0.0, 0.0, HOOK - 0.01)], 0.0015, cord_m, seg=6)
# the suction hook: a clear cup on the glass behind (+Y: the window side) and its wire hook
fk.cyl(b, (0.0, 0.012, HOOK), 0.022, 0.006, hook_m, seg=20, axis=(0, 1, 0))
fk.tube(b, [(0.0, 0.012, HOOK), (0.0, 0.0, HOOK + 0.004), (0.0, -0.004, HOOK - 0.012)], 0.0012, hook_metal, seg=6)
ob = b.finish(collection="PROP")
bm = bmesh.new()
bm.from_mesh(ob.data)
uvl = bm.loops.layers.uv.verify()
ob.data.materials.append(faces_m)
mi = len(ob.data.materials) - 1
y0, y1 = -T / 2, T / 2
# (front -Y: OPEN, the image's left half; back +Y: CLOSED, its right half, mirrored to read right)
for (y, us, order) in ((y0, (0.0, 0.5), 1), (y1, (0.5, 1.0), -1)):
    cs = [(-W / 2, y, 0.0), (W / 2, y, 0.0), (W / 2, y, H), (-W / 2, y, H)]
    # (the faces are the sheet's top half: v 0.5 .. 1)
    uv = [(us[0], 0.5), (us[1], 0.5), (us[1], 1.0), (us[0], 1.0)]
    if order < 0:
        uv = [(us[1], 0.5), (us[0], 0.5), (us[0], 1.0), (us[1], 1.0)]
        cs, uv = cs[::-1], uv[::-1]
    f = bm.faces.new([bm.verts.new(c) for c in cs])
    f.material_index = mi
    for lp, t in zip(f.loops, uv):
        lp[uvl].uv = t
bm.normal_update()
bm.to_mesh(ob.data)
bm.free()
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
