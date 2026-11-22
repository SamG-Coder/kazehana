"""Curtains (docs/cloth_plan.md: tier A cloth), as props hung in Tsukumodō from its layout:

    python tools/make_house_textures.py     (the cotton first)
    blender --background --factory-startup --python tools/blender/build_curtain_props.py -- <id>

curtain_my_room   your room's street window (2.1 m wide): two panels of light sage cotton, each
                  its own `kzcloth_` node, so the game simulates them (crates/town/src/cloth.rs pins
                  each one's top band to its rail; people push through them, draughts stir them)
curtain_rail_2500 the rail they hang from: a slim aluminium channel, end caps, two wall brackets
                  (its own prop: the gate wants every part of a prop standing on its own floor)

-> assets/props/<id>/<id>.gltf

Frame (Blender): origin on the floor under the middle of the rail; the curtains' room side towards
-Y, the window and wall behind them towards +Y. Placed by tools/blender/build_shop_tsukumodo.py's
layout (z: the rail's floor).

Each panel's rest shape hangs straight, gathered into soft pleats (a curtain on its runners is
never flat), about 4 cm a cell: fine enough for folds, few enough particles for the solver.
"""

import math
import os
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PID = sys.argv[sys.argv.index("--") + 1]
OUT = os.path.join(ROOT, "assets", "props", PID, PID + ".gltf")
TEX = os.path.join(ROOT, "assets", "cache", "house")
TILE = 0.5

SPEC = {
    # rail length, the rail's height above the floor, each panel's hanging width and drop
    "curtain_my_room": dict(rail=2.5, top=2.6, panel=1.2, drop=1.5, stem="curtain"),
    "curtain_rail_2500": dict(rail=2.5, top=2.6),
}
if PID not in SPEC:
    raise SystemExit(f"unknown curtain {PID}")
S = SPEC[PID]

kz.reset_scene()
COLL = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(COLL)


def cotton(name, stem):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    m.use_backface_culling = False
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(os.path.join(TEX, stem + "_base.png"), "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(os.path.join(TEX, stem + "_normal.png"), "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = 0.7
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(os.path.join(TEX, stem + "_orm.png"), "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    return m


fabric = cotton("curtain_cotton", S["stem"]) if "stem" in S else None
alu = kz.painted_material("curtain_rail_aluminium", (0.62, 0.62, 0.60), "Precast_Concrete_Wall", roughness=0.3, metallic=0.85, normal_strength=0.05, grain=0.05)

TOP = S["top"]
RAIL_Y = 0.0          # the rail's line (the panels hang just in front of it)
CELL = 0.04
PLEAT_W, PLEAT_D = 0.12, 0.035   # gathered pleats: their wavelength and depth


def panel(name, x0, x1, phase):
    """A panel hanging from just under the rail, from x0 to x1, pleated."""
    w, drop = x1 - x0, S["drop"]
    cols = max(2, round(w / CELL))
    rows = max(2, round(drop / CELL))
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new()
    grid = []
    for j in range(rows + 1):
        z = TOP - 0.03 - drop * (1 - j / rows)
        line = []
        for i in range(cols + 1):
            x = x0 + w * i / cols
            # the pleats: deepest just under the runners, easing a little towards the hem
            ease = 0.75 + 0.25 * (j / rows)
            y = RAIL_Y - 0.05 - PLEAT_D * ease * 0.5 * (1 + math.sin(2 * math.pi * (x - x0) / PLEAT_W + phase))
            # (the fabric's own length across: its flat width, for the texture)
            line.append((bm.verts.new((x, y, z)), ((x - x0) * 1.25 / TILE, (z - (TOP - drop)) / TILE)))
        grid.append(line)
    for j in range(rows):
        for i in range(cols):
            a, b, c, d = grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]
            # wound so the front faces the room (-Y)
            f = bm.faces.new([a[0], b[0], c[0], d[0]])
            for loop, uv in zip(f.loops, [a[1], b[1], c[1], d[1]]):
                loop[uvl].uv = uv
            f.smooth = True
    bm.normal_update()
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    me.materials.append(fabric)
    ob = bpy.data.objects.new(name, me)
    COLL.objects.link(ob)
    n = sum(p.normal.y for p in me.polygons)
    assert n < 0, f"{name}: the front faces the wrong way"
    return ob


half = S["rail"] / 2
gap = 0.02
if fabric:
    panel("kzcloth_curtain_l", -half + 0.06, -gap / 2, 0.0)
    panel("kzcloth_curtain_r", gap / 2, half - 0.06, 1.3)

if not fabric:
    # the rail: a slim aluminium channel, end caps, two brackets to the wall behind
    b = kz.MeshBuilder("curtain_rail", uv_scale=4.0)
    b.box((-half, RAIL_Y - 0.015, TOP - 0.02), (half, RAIL_Y + 0.015, TOP + 0.01), alu)
    for sx in (-1, 1):
        x = sx * (half - 0.01)
        b.box((x - 0.012, RAIL_Y - 0.02, TOP - 0.03), (x + 0.012, RAIL_Y + 0.02, TOP + 0.015), alu)        # end cap
        xb = sx * (half - 0.25)
        b.box((xb - 0.02, RAIL_Y, TOP + 0.0), (xb + 0.02, RAIL_Y + 0.11, TOP + 0.03), alu)                # bracket arm
        b.box((xb - 0.03, RAIL_Y + 0.1, TOP - 0.03), (xb + 0.03, RAIL_Y + 0.115, TOP + 0.06), alu)        # wall plate
    b.finish(collection="PROP")

# The gate wants the prop's lowest point at z = 0: the whole prop is lowered by its hem's height
# (HEM, written beside it), which the layout adds back to put the rail at `top` above the floor.
HEM = min((o.matrix_world @ v.co).z for o in COLL.objects if o.type == "MESH" for v in o.data.vertices)
for o in COLL.objects:
    o.location.z -= HEM
print("hem above the floor:", round(HEM, 4))
os.makedirs(os.path.dirname(OUT), exist_ok=True)
with open(os.path.join(os.path.dirname(OUT), "hang.txt"), "w") as f:
    f.write(f"{HEM:.4f}\n")
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
