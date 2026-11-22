"""Food and things to buy, built by family (docs/shops_food_plan.md): each family is one
quality-gated prop holding all its variants side by side, one node per catalog item (named after
the item's id in assets/data/items.json), so a family is reviewed and approved as one sheet.

    blender --background --factory-startup --python tools/blender/build_food_props.py -- <family>

families:
    food_onigiri    the konbini's onigiri, as sold: a rounded triangle of rice, the nori wrapped
                    up from the bottom, the flavour's label on the front
                    (onigiri_salmon, onigiri_tunamayo, onigiri_ume, onigiri_kombu)

-> assets/props/<family>/<family>.gltf

Frame (Blender): each item's origin under its middle, on the shelf (z = 0), its front (label)
towards -Y; variants are spaced along +X.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
FAMILY = sys.argv[sys.argv.index("--") + 1]
OUT = os.path.join(ROOT, "assets", "props", FAMILY, FAMILY + ".gltf")
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material

kz.reset_scene()
COLL = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(COLL)


def finish(ob):
    """World-projected UVs and smooth shading."""
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    kz.world_uv(bm, 20.0)
    bm.to_mesh(ob.data)
    bm.free()
    for p in ob.data.polygons:
        p.use_smooth = True
    return ob


def rounded_triangle(side, r, seg=8):
    """The outline of an onigiri: an equilateral triangle (point up) with rounded corners, in the
    XZ plane, its base on z = 0."""
    h = side * math.sqrt(3) / 2
    corners = [Vector((-side / 2, 0, 0)), Vector((side / 2, 0, 0)), Vector((0, 0, h))]
    centre = sum(corners, Vector()) / 3
    pts = []
    for i, c in enumerate(corners):
        # the corner's circle sits inside the triangle, r from both sides
        inward = (centre - c).normalized()
        cc = c + inward * (r / math.sin(math.radians(30)))
        prev, nxt = corners[i - 1], corners[(i + 1) % 3]
        # outward normals of the two sides meeting here
        n0 = (c - prev).cross(Vector((0, 1, 0))).normalized()
        n1 = (nxt - c).cross(Vector((0, 1, 0))).normalized()
        if n0.dot(centre - c) > 0:
            n0, n1 = -n0, -n1
        ang0 = math.atan2(n0.z, n0.x)
        ang1 = math.atan2(n1.z, n1.x)
        while ang1 < ang0:
            ang1 += 2 * math.pi
        if ang1 - ang0 > math.pi:
            ang0, ang1 = ang1 - 2 * math.pi, ang0
        for k in range(seg + 1):
            a = ang0 + (ang1 - ang0) * k / seg
            pts.append(cc + Vector((math.cos(a), 0, math.sin(a))) * r)
    zmin = min(p.z for p in pts)
    return [p - Vector((0, 0, zmin)) for p in pts]


def prism(name, outline, depth, mat, bevel=0.0, keep_below=None, grow=0.0):
    """The outline extruded along Y (centred), edges bevelled; `keep_below`: only faces under
    that height (a wrap); `grow`: pushed out along the normals (a layer over another)."""
    bm = bmesh.new()
    front = [bm.verts.new((p.x, -depth / 2, p.z)) for p in outline]
    back = [bm.verts.new((p.x, depth / 2, p.z)) for p in outline]
    bm.faces.new(list(reversed(front)))
    bm.faces.new(back)
    n = len(outline)
    for i in range(n):
        j = (i + 1) % n
        bm.faces.new((front[i], front[j], back[j], back[i]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    if bevel > 0:
        edges = [e for e in bm.edges if abs(e.verts[0].co.y - e.verts[1].co.y) < 1e-6]
        bmesh.ops.bevel(bm, geom=edges, offset=bevel, segments=4, profile=0.5, affect="EDGES")
    if grow:
        bm.normal_update()
        for v in bm.verts:
            v.co += v.normal * grow
    if keep_below is not None:
        dead = [f for f in bm.faces if f.calc_center_median().z > keep_below]
        bmesh.ops.delete(bm, geom=dead, context="FACES")
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    COLL.objects.link(ob)
    ob.data.materials.append(mat)
    return ob


def plate(name, c, w, h, t, mat):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co = Vector((c.x + v.co.x * w, c.y + v.co.y * t, c.z + v.co.z * h))
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    COLL.objects.link(ob)
    ob.data.materials.append(mat)
    return ob


def join(name, parts, x):
    """One node per item: its parts joined, moved to its place on the sheet."""
    bpy.ops.object.select_all(action="DESELECT")
    for o in parts:
        o.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    ob.name = name
    finish(ob)
    ob.location.x = x
    return ob


if FAMILY == "food_onigiri":
    rice = P("onigiri_rice", (0.86, 0.85, 0.80), DETAIL, roughness=0.75, normal_strength=0.9, grain=0.35)
    nori = P("onigiri_nori", (0.035, 0.05, 0.035), DETAIL, roughness=0.55, normal_strength=0.6, grain=0.3)
    white = P("onigiri_label_white", (0.85, 0.85, 0.83), DETAIL, roughness=0.4, normal_strength=0.05, grain=0.05)
    pull = P("onigiri_pull_tab", (0.75, 0.08, 0.08), DETAIL, roughness=0.4, normal_strength=0.05, grain=0.05)
    flavours = [
        ("onigiri_salmon", (0.85, 0.40, 0.25)),
        ("onigiri_tunamayo", (0.90, 0.75, 0.20)),
        ("onigiri_ume", (0.75, 0.15, 0.30)),
        ("onigiri_kombu", (0.25, 0.35, 0.18)),
    ]
    side, depth = 0.095, 0.036
    outline = rounded_triangle(side, 0.017)
    for k, (item, col) in enumerate(flavours):
        label = P(f"{item}_label", col, DETAIL, roughness=0.4, normal_strength=0.05, grain=0.05)
        body = prism(f"{item}_rice", outline, depth, rice, bevel=0.009)
        wrap = prism(f"{item}_nori", outline, depth, nori, bevel=0.009, keep_below=0.034, grow=0.0012)
        # the label on the front (the flavour's colour, a white band, the red pull tab down the middle)
        z = 0.052
        lab = plate(f"{item}_label", Vector((0, -depth / 2 - 0.0016, z)), 0.036, 0.022, 0.0006, label)
        band = plate(f"{item}_band", Vector((0, -depth / 2 - 0.0019, z - 0.002)), 0.03, 0.007, 0.0006, white)
        tab = plate(f"{item}_tab", Vector((0, -depth / 2 - 0.0022, z + 0.006)), 0.006, 0.034, 0.0006, pull)
        join(item, [body, wrap, lab, band, tab], k * 0.14)
else:
    raise SystemExit(f"unknown family {FAMILY}")

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
