"""The farm shed (納屋, naya) beside each farmhouse that has one in the plan (55 of 61): 8 x 5 m,
one tall storey under a gable roof of corrugated metal, as quality-gated assets.

    blender --background --factory-startup --python tools/blender/build_farm_shed.py -- <scheme>
    ("a" dark stained board walls, "b" corrugated galvanised walls gone dull with age)

-> assets/buildings/farm_shed[_b]/farm_shed[_b].gltf

Where the tractor, the rice dryer and the tools live: a wide two-leaf sliding door on the long
side (towards the house), a small door and a high window at one end, the roof's corrugated sheets
over the walls with plain barge boards, on a low concrete plinth. Local frame: origin at ground
level in the middle, +X along the long side, the doors to +Y.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
SCHEME = ARGS[0] if ARGS else "a"
ASSET_ID = "farm_shed" + ("" if SCHEME == "a" else "_" + SCHEME)
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")

W, D = 8.0, 5.0
TOP = 3.4
PITCH = math.radians(22.0)
OV = 0.35

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
if SCHEME == "a":
    clad = P("shed_board", (0.08, 0.055, 0.035), "Wood_Planks", roughness=0.65, normal_strength=0.9)
else:
    clad = P("shed_corrugated", (0.42, 0.42, 0.40), DETAIL, roughness=0.55, metallic=0.55, normal_strength=0.5)
sheet = P("shed_roof_sheet", (0.30, 0.26, 0.22), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.4)
frame = P("shed_frame", (0.10, 0.07, 0.05), "Wood_Planks", roughness=0.6, normal_strength=0.6)
door_m = P("shed_door", (0.33, 0.33, 0.31), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.4)
plinth = P("house_foundation", (0.36, 0.35, 0.33), DETAIL, roughness=0.85, normal_strength=0.8)
dark = P("house_dark", (0.02, 0.02, 0.02), DETAIL, roughness=0.6, normal_strength=0.1)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)

walls = kz.MeshBuilder("walls", uv_scale=0.5)
fittings = kz.MeshBuilder("fittings", uv_scale=1.0)
roofing = kz.MeshBuilder("roof", uv_scale=0.5)

# the shell: plinth, the cladding (ribs every 0.2 m for the boards' battens / the corrugations)
walls.box((-W / 2 - 0.05, -D / 2 - 0.05, 0.0), (W / 2 + 0.05, D / 2 + 0.05, 0.25), plinth)
DOOR = (-1.8, 1.8, 0.25, 2.9)                     # the sliding doors' opening (along x, on +Y)
for (x0, x1) in ((-W / 2, DOOR[0]), (DOOR[1], W / 2)):
    walls.box((x0, D / 2 - 0.12, 0.25), (x1, D / 2, TOP), clad)
walls.box((DOOR[0], D / 2 - 0.12, DOOR[3]), (DOOR[1], D / 2, TOP), clad)
walls.box((-W / 2, -D / 2, 0.25), (W / 2, -D / 2 + 0.12, TOP), clad)
for sx in (-1, 1):
    walls.box((sx * W / 2 - (0.12 if sx > 0 else 0), -D / 2, 0.25), (sx * W / 2 + (0 if sx > 0 else 0.12), D / 2, TOP), clad)
step = 0.2 if SCHEME == "a" else 0.076
for (y, s) in ((D / 2, 1), (-D / 2, -1)):
    x = -W / 2 + step / 2
    while x < W / 2:
        if not (y > 0 and DOOR[0] < x < DOOR[1]):
            walls.box((x - 0.015, y - (0 if s > 0 else 0.02), 0.25), (x + 0.015, y + (0.02 if s > 0 else 0), TOP - 0.02), clad)
        elif y > 0:
            walls.box((x - 0.015, y, DOOR[3]), (x + 0.015, y + 0.02, TOP - 0.02), clad)
        x += step
# the end gables
rise = (D / 2) * math.tan(PITCH)
for sx in (-1, 1):
    tri = [Vector((sx * W / 2, -D / 2, TOP)), Vector((sx * W / 2, D / 2, TOP)), Vector((sx * W / 2, 0, TOP + rise))]
    walls.quad(tri if sx > 0 else [tri[1], tri[0], tri[2]], clad)
# the end door and the high window (east end)
fittings.box((W / 2, -0.4 - 0.45, 0.25), (W / 2 + 0.04, -0.4 + 0.45, 2.15), door_m)
fittings.box((W / 2, 0.9, 2.0), (W / 2 + 0.03, 1.9, 2.6), frosted)
for (y0, y1, z0, z1) in ((0.85, 1.95, 1.95, 2.0), (0.85, 1.95, 2.6, 2.65), (0.85, 0.9, 1.95, 2.65), (1.9, 1.95, 1.95, 2.65)):
    fittings.box((W / 2, y0, z0), (W / 2 + 0.05, y1, z1), frame)
# the sliding doors: two leaves on a top rail, the rail's cover, handles, the ground rail
for k, (x0, x1) in enumerate(((DOOR[0], 0.02), (-0.02, DOOR[1]))):
    y = D / 2 + 0.04 + 0.05 * k
    fittings.box((x0, y, 0.26), (x1, y + 0.04, DOOR[3] + 0.05), door_m)
    xx = x0 + 0.1
    while xx < x1 - 0.05:
        fittings.box((xx - 0.012, y + 0.04, 0.3), (xx + 0.012, y + 0.055, DOOR[3]), door_m)
        xx += 0.076
    hx = x1 - 0.2 if k == 0 else x0 + 0.2
    fittings.box((hx - 0.03, y + 0.04, 1.0), (hx + 0.03, y + 0.09, 1.3), dark)
fittings.box((DOOR[0] - 1.9, D / 2 + 0.02, DOOR[3] + 0.05), (DOOR[1] + 1.9, D / 2 + 0.16, DOOR[3] + 0.2), frame)
fittings.box((DOOR[0] - 0.1, D / 2, 0.0), (DOOR[1] + 0.1, D / 2 + 0.15, 0.26), plinth)
# the roof: corrugated sheets (ribs), barge boards
RT = 0.05
run = D / 2 + OV
slope_len = run / math.cos(PITCH)
z_eave = TOP - OV * math.tan(PITCH)
for s in (-1, 1):
    rot = Matrix.Rotation(PITCH if s < 0 else -PITCH, 3, "X")
    nrm = rot @ Vector((0, 0, 1))
    c = Vector((0, s * run / 2, z_eave + (TOP + rise - z_eave) / 2 + 0.02)) - nrm * (RT / 2)
    roofing.oriented_box(c, (W + 2 * OV, slope_len, RT), rot, sheet)
    x = -W / 2 - OV + 0.0635
    while x < W / 2 + OV:
        roofing.oriented_box(c + nrm * (RT / 2 + 0.01) + Vector((x, 0, 0)), (0.03, slope_len, 0.02), rot, sheet)
        x += 0.127
for sx in (-1, 1):
    for s in (-1, 1):
        a = Vector((sx * (W / 2 + OV), s * run, z_eave))
        b = Vector((sx * (W / 2 + OV), 0, TOP + rise + 0.03))
        mid, d = (a + b) / 2, b - a
        fittings.oriented_box(mid - Vector((0, 0, 0.06)), (0.03, d.length, 0.16), Matrix.Rotation(math.atan2(d.z, abs(d.y)) * (1 if s < 0 else -1), 3, "X"), frame)
roofing.box((-W / 2 - OV, -0.1, TOP + rise - 0.03), (W / 2 + OV, 0.1, TOP + rise + 0.05), sheet)
walls.box((-W / 2 + 0.12, -D / 2 + 0.12, TOP - 0.05), (W / 2 - 0.12, D / 2 - 0.12, TOP), dark)

for b in (walls, fittings, roofing):
    b.finish(collection="HOUSE")
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
