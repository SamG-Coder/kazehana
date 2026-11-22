"""The town centre's shop signboards as one asset: a node per census shop ("shopsign_h<house id>_"),
its painted face (tools/make_town_shop_signs.py) on the board its shophouse has for it, in that
shophouse's local frame (tools/blender/shophouse_spec.py), so the game places it with the same
transform as the building (crates/town/src/plots.rs).

    python tools/make_town_shop_signs.py
    blender --background --factory-startup --python tools/blender/build_shop_signs.py

-> assets/buildings/shop_signs/shop_signs.gltf
"""

import json
import math
import os
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402
import shophouse_spec as spec  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "assets", "buildings", "shop_signs", "shop_signs.gltf")
ATLAS = os.path.join(ROOT, "assets", "cache", "signs", "town_shop_signs.png")
INDEX = json.load(open(os.path.join(ROOT, "assets", "cache", "signs", "town_shop_signs.json"), encoding="utf8"))
TOWN = json.load(open(os.path.join(ROOT, "assets", "town", "town.json"), encoding="utf8"))
DETAIL = "Precast_Concrete_Wall"

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)


def image_material(name, path, detail_prefix, roughness=0.5, normal_strength=0.3):
    """Colour from a prepared image, a scan's normal and roughness variation (as the shop's signs)."""
    _, n, orm = kz.fab_textures(detail_prefix)
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(path, "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Metallic"].default_value = 0.0
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(n, "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = normal_strength
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(orm, "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    mul = nt.nodes.new("ShaderNodeMath")
    mul.operation = "MULTIPLY"
    mul.inputs[1].default_value = roughness / 0.75
    nt.links.new(sep.outputs["Green"], mul.inputs[0])
    nt.links.new(mul.outputs["Value"], bsdf.inputs["Roughness"])
    return m


paint = image_material("town_shop_signs", ATLAS, DETAIL, roughness=0.45, normal_strength=0.25)
edge = kz.painted_material("shop_trim", (0.07, 0.07, 0.07), DETAIL, roughness=0.5, normal_strength=0.2)

houses = {h["id"]: h for h in TOWN["houses"]}
n = 0
for hid, s in sorted(INDEX.items(), key=lambda kv: int(kv[0])):
    h = houses[int(hid)]
    q = h["house"]
    width = math.dist(q[0], q[1])
    style = spec.style_of(int(hid))
    x0, x1, y, z0, z1 = spec.board(spec.width_class(width), style)
    px0, py0, px1, py1 = s["px"]
    u0, u1, v0, v1 = px0 / 8192, px1 / 8192, 1 - py1 / 8192, 1 - py0 / 8192
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new()
    # the face (seen from the street, +Y), then a thin painted edge round it, its back dark
    f = bm.faces.new([bm.verts.new(v) for v in ((x1, y, z0), (x0, y, z0), (x0, y, z1), (x1, y, z1))])
    # (from the street, looking along -Y, +x is on your left: the text starts there)
    for l, uv in zip(f.loops, [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]):
        l[uvl].uv = uv
    f.material_index = 0
    for (a, b) in (((x0, z0), (x1, z0)), ((x1, z0), (x1, z1)), ((x1, z1), (x0, z1)), ((x0, z1), (x0, z0))):
        e = bm.faces.new([bm.verts.new((a[0], y, a[1])), bm.verts.new((b[0], y, b[1])), bm.verts.new((b[0], y - 0.012, b[1])), bm.verts.new((a[0], y - 0.012, a[1]))])
        e.material_index = 1
    bm.normal_update()
    me = bpy.data.meshes.new(f"shopsign_h{int(hid):03}_")
    bm.to_mesh(me)
    bm.free()
    me.materials.append(paint)
    me.materials.append(edge)
    ob = bpy.data.objects.new(f"shopsign_h{int(hid):03}_", me)
    kz.coll("HOUSE").objects.link(ob)
    n += 1

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT, n, "signs")
