"""The classroom's back lockers (教室後方ロッカー), for 1-A's back wall: one compartment for each
student's bag, two rows of nine, in a powder-coated steel cabinet on a plinth, as school
furniture makers build them. Each compartment has its own door, hinged on the left, with a
finger pull and the student's number (出席番号) in a tag holder; the doors open (Akari doors:
`kzdoor_hinge_p100_locker<n>`). Your timetable is taped inside your door (the game shows it).

    python tools/make_locker_tags.py   (first)
    blender --background --factory-startup --python tools/blender/build_school_lockers.py -- school_lockers

-> assets/props/school_lockers/school_lockers.gltf

Frame (Blender): origin on the floor under the middle of the cabinet's footprint; its front (the
doors) faces -Y; compartment n (1 - 18) is in row (n-1) // 9 (0 the top), column (n-1) % 9 (0 at
the left seen from the front, -X).
"""

import json
import math
import os
import sys

import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PID = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "school_lockers"
OUT = os.path.join(ROOT, "assets", "props", PID, PID + ".gltf")
TAGS = os.path.join(ROOT, "assets", "cache", "signs", "locker_tags.png")

COLS, ROWS = 9, 2
CW, CH = 0.40, 0.42            # a compartment's opening pitch (m)
DEPTH = 0.45
PLINTH = 0.10
SHEET = 0.012                  # the carcass's panels (folded steel: thin, a lip at the front)
W = COLS * CW + 2 * SHEET
H = PLINTH + ROWS * CH + SHEET
Y0, Y1 = -DEPTH / 2, DEPTH / 2  # front, back

kz.reset_scene()
COLL = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(COLL)
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
# (the makers' standard: an ivory powder coat, the plinth a darker grey, doors the same ivory)
carcass_m = P("locker_carcass", (0.80, 0.79, 0.74), DETAIL, roughness=0.45, metallic=0.0, normal_strength=0.06, grain=0.05)
inside_m = P("locker_inside", (0.62, 0.62, 0.60), DETAIL, roughness=0.6, metallic=0.0, normal_strength=0.08, grain=0.08)
plinth_m = P("locker_plinth", (0.30, 0.31, 0.32), DETAIL, roughness=0.6, metallic=0.0, normal_strength=0.1, grain=0.08)
door_m = P("locker_door", (0.82, 0.81, 0.76), DETAIL, roughness=0.42, metallic=0.0, normal_strength=0.06, grain=0.05)
pull_m = P("locker_pull", (0.18, 0.18, 0.19), DETAIL, roughness=0.5, metallic=0.2, normal_strength=0.05, grain=0.03)
holder_m = kz.plain_material("locker_tag_holder_clear_glass", (0.92, 0.94, 0.94), roughness=0.08)


def tag_material():
    m = bpy.data.materials.new("locker_tags")
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(TAGS, "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    _, n, _ = kz.fab_textures(DETAIL)
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(n, "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = 0.02
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    bsdf.inputs["Roughness"].default_value = 0.6
    return m


tags_m = tag_material()
body = kz.MeshBuilder("school_lockers", uv_scale=2.0)

# the carcass: sides, top, bottom over the plinth, back, the shelves and dividers (folded steel)
body.box((-W / 2, Y0, PLINTH), (-W / 2 + SHEET, Y1, H), carcass_m)
body.box((W / 2 - SHEET, Y0, PLINTH), (W / 2, Y1, H), carcass_m)
body.box((-W / 2, Y0, H - SHEET), (W / 2, Y1, H), carcass_m)
body.box((-W / 2, Y0, PLINTH), (W / 2, Y1, PLINTH + SHEET), carcass_m)
body.box((-W / 2, Y1 - SHEET, PLINTH), (W / 2, Y1, H), carcass_m)
body.box((-W / 2 + 0.02, Y0 + 0.05, 0.0), (W / 2 - 0.02, Y1 - 0.01, PLINTH), plinth_m)   # the plinth, set back
for r in range(1, ROWS):
    z = PLINTH + r * CH
    body.box((-W / 2, Y0, z - SHEET / 2), (W / 2, Y1, z + SHEET / 2), carcass_m)
for c in range(1, COLS):
    x = -W / 2 + SHEET + c * CW
    body.box((x - SHEET / 2, Y0, PLINTH), (x + SHEET / 2, Y1, H), carcass_m)
# the compartments' insides a shade darker (their back and floor)
for r in range(ROWS):
    for c in range(COLS):
        x0 = -W / 2 + SHEET + c * CW + SHEET / 2
        z0 = PLINTH + r * CH + SHEET / 2
        body.box((x0 + 0.002, Y1 - SHEET - 0.002, z0 + 0.002), (x0 + CW - SHEET - 0.002, Y1 - SHEET, z0 + CH - SHEET - 0.002), inside_m)
        body.box((x0 + 0.002, Y0 + 0.02, z0), (x0 + CW - SHEET - 0.002, Y1 - SHEET, z0 + 0.002), inside_m)

# the doors: one each, hinged on its left (seen from the front), a pull at the right, the tag
DOORS = []
uv_faces = []
for n in range(1, COLS * ROWS + 1):
    r, c = (n - 1) // COLS, (n - 1) % COLS
    row = ROWS - 1 - r                                     # (row 0 is the top)
    x0 = -W / 2 + SHEET + c * CW + 0.003
    x1 = x0 + CW - SHEET - 0.006 + SHEET
    z0 = PLINTH + row * CH + SHEET / 2 + 0.003
    z1 = z0 + CH - SHEET - 0.006
    pivot = Vector((x0, Y0 - 0.002, z0))
    d = kz.MeshBuilder(f"kzdoor_hinge_n100_locker{n}", uv_scale=2.0)
    d.box((x0, Y0 - 0.016, z0), (x1, Y0 - 0.002, z1), door_m)
    # its pressed edges (a shallow raised rim) and the finger pull, a recessed cup at the right
    d.box((x0 + 0.01, Y0 - 0.018, z0 + 0.01), (x1 - 0.01, Y0 - 0.016, z0 + 0.016), door_m)
    d.box((x0 + 0.01, Y0 - 0.018, z1 - 0.016), (x1 - 0.01, Y0 - 0.016, z1 - 0.01), door_m)
    d.box((x1 - 0.06, Y0 - 0.020, (z0 + z1) / 2 - 0.05), (x1 - 0.03, Y0 - 0.016, (z0 + z1) / 2 + 0.05), pull_m)
    # the number tag in its holder, top middle
    tx0, tx1 = (x0 + x1) / 2 - 0.05, (x0 + x1) / 2 + 0.05
    tz0, tz1 = z1 - 0.085, z1 - 0.03
    d.box((tx0 - 0.006, Y0 - 0.019, tz0 - 0.006), (tx1 + 0.006, Y0 - 0.016, tz1 + 0.006), holder_m)
    u0, v1 = ((n - 1) % 6) / 6, 1 - ((n - 1) // 6) / 3
    u1, v0 = u0 + 1 / 6, v1 - 1 / 3
    yy = Y0 - 0.0195
    d.bm_face = None
    DOORS.append((d, pivot, [((tx0, yy, tz0), (tx1, yy, tz0), (tx1, yy, tz1), (tx0, yy, tz1)), ((u0, v0), (u1, v0), (u1, v1), (u0, v1))]))

body.finish(collection="PROP")
import bmesh  # noqa: E402

for d, pivot, (corners, uvs) in DOORS:
    ob = d.finish(collection="PROP")
    # the tag's face (UVs into the tags sheet)
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    uvl = bm.loops.layers.uv.verify()
    if "locker_tags" not in [m.name for m in ob.data.materials]:
        ob.data.materials.append(tags_m)
    f = bm.faces.new([bm.verts.new(cn) for cn in corners])
    f.material_index = [m.name for m in ob.data.materials].index("locker_tags")
    for lp, uv in zip(f.loops, uvs):
        lp[uvl].uv = uv
    bm.normal_update()
    bm.to_mesh(ob.data)
    bm.free()
    ob.data.transform(Matrix.Translation(-pivot))
    ob.location = pivot

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
with open(os.path.join(os.path.dirname(OUT), "lockers.json"), "w") as fh:
    json.dump({"cols": COLS, "rows": ROWS, "pitch": [CW, CH], "plinth": PLINTH, "front": Y0, "width": W, "height": H}, fh, indent=1)
print("wrote", OUT)
