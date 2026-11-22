"""Your futon (docs/cloth_plan.md, docs/home_plan.md): the bed laid out on the tatami at night and
folded into the closet by day, as two props:

    python tools/make_house_textures.py     (the cottons first)
    blender --background --factory-startup --python tools/blender/build_futon_props.py -- <id>

futon_laid     the shikibuton (mattress, 100 x 200 x 7 cm, in its unbleached sheet), the buckwheat
               pillow (50 x 33 cm, ticking cover) and the kakebuton (quilt, 140 x 190 cm, indigo
               check) dropped over them: the quilt is draped by Blender's cloth solver onto the
               mattress, the pillow and the floor, so its folds and the way it slumps off the
               edges are real, then given its loft (6 cm)
futon_folded   the same, folded away on the closet's shelf: the mattress in three, the quilt in
               four, the pillow on top

-> assets/props/<id>/<id>.gltf

Frame (Blender): origin on the floor under the middle; the bed's head (the pillow) towards +Y.
Both are still (tier D). Laying the futon out and folding it away (tier A) is the game's to
animate later.
"""

import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Vector, noise

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PID = sys.argv[sys.argv.index("--") + 1]
OUT = os.path.join(ROOT, "assets", "props", PID, PID + ".gltf")
TEX = os.path.join(ROOT, "assets", "cache", "house")
TILE = 0.5       # metres per texture tile (the cottons)

kz.reset_scene()
COLL = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(COLL)
rng = random.Random(3)


def fabric(name, stem):
    """Cotton from tools/make_house_textures.py: base, weave normal, ORM roughness."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(os.path.join(TEX, stem + "_base.png"), "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(os.path.join(TEX, stem + "_normal.png"), "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = 0.8
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(os.path.join(TEX, stem + "_orm.png"), "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    return m


cover = fabric("futon_cover", "futon_cover")
sheet = fabric("futon_sheet", "futon_sheet")
ticking = fabric("futon_pillow", "pillow")


def link(name, bm, mat):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    COLL.objects.link(ob)
    ob.data.materials.append(mat)
    for p in me.polygons:
        p.use_smooth = True
    return ob


def cushion(name, size, centre, mat, round_=0.03, bulge=0.012, lumps=0.004, seed=0):
    """A stuffed thing: a box with well-rounded edges, its faces bulging out in the middle (the
    stuffing pushing the cover), a few soft lumps."""
    sx, sy, sz = size
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co = Vector((v.co.x * sx, v.co.y * sy, v.co.z * sz))
    bmesh.ops.bevel(bm, geom=list(bm.edges), offset=min(round_, sz * 0.49), segments=5, profile=0.5, affect="EDGES")
    bmesh.ops.subdivide_edges(bm, edges=[e for e in bm.edges if e.calc_length() > 0.06], cuts=6, use_grid_fill=True)
    off = Vector((seed * 7.1, seed * 3.3, seed * 1.7))
    for v in bm.verts:
        p = v.co
        # bulge: the faces' middles out (a pillow's shape), most on the top and bottom
        fx = max(0.0, 1 - (2 * p.x / sx) ** 2)
        fy = max(0.0, 1 - (2 * p.y / sy) ** 2)
        if abs(p.z) > sz * 0.3:
            p.z += math.copysign(bulge * fx * fy, p.z)
        n = noise.noise(p * 6.0 + off)
        p += v.normal * (n * lumps)
    for v in bm.verts:
        v.co += Vector(centre)
    kz.world_uv(bm, 1.0 / TILE)
    return link(name, bm, mat)


def floor_collider():
    bpy.ops.mesh.primitive_plane_add(size=6, location=(0, 0, 0))
    f = bpy.context.object
    bpy.ops.object.modifier_add(type="COLLISION")
    f.collision.thickness_outer = 0.004
    return f


def drape_quilt(name, size, at, frames=70):
    """The quilt: a sheet dropped from above and simulated (cloth solver) onto whatever collides,
    then given its loft and its stitched edge."""
    w, l = size
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=int(w / 0.025), y_subdivisions=int(l / 0.025), size=1.0, location=at)
    q = bpy.context.object
    q.name = name
    q.scale = (w, l, 1)
    bpy.ops.object.transform_apply(scale=True)
    # UVs in metres (the cotton's tile), before it moves
    bm = bmesh.new()
    bm.from_mesh(q.data)
    uv = bm.loops.layers.uv.verify()
    for f in bm.faces:
        for lp in f.loops:
            lp[uv].uv = ((lp.vert.co.x + w / 2) / TILE, (lp.vert.co.y + l / 2) / TILE)
    bm.to_mesh(q.data)
    bm.free()
    mod = q.modifiers.new("cloth", "CLOTH")
    c = mod.settings
    c.quality = 10
    c.mass = 0.6                  # (per vertex, kg: a cotton-filled quilt is heavy)
    c.tension_stiffness = 40
    c.compression_stiffness = 40
    c.shear_stiffness = 20
    c.bending_stiffness = 10.0    # a quilt bends in broad rolls, not fine creases
    c.air_damping = 2.0
    mod.collision_settings.distance_min = 0.012
    mod.collision_settings.use_self_collision = True
    mod.collision_settings.self_distance_min = 0.01
    mod.point_cache.frame_end = frames
    s = bpy.context.scene
    s.frame_start, s.frame_end = 1, frames
    for f in range(1, frames + 1):
        s.frame_set(f)
    bpy.context.view_layer.objects.active = q
    q.select_set(True)
    bpy.ops.object.modifier_apply(modifier="cloth")
    # loft: thickened downwards from the draped surface, its edges rounded
    so = q.modifiers.new("loft", "SOLIDIFY")
    so.thickness = 0.06
    so.offset = -1.0
    so.use_rim = True
    bpy.ops.object.modifier_apply(modifier="loft")
    q.data.materials.append(cover)
    for c_ in list(q.users_collection):
        c_.objects.unlink(q)
    COLL.objects.link(q)
    for p in q.data.polygons:
        p.use_smooth = True
    for o in list(COLL.objects) + [q]:
        o.select_set(False)
    return q


def collide(*obs):
    for o in obs:
        m = o.modifiers.new("collision", "COLLISION")
        o.collision.thickness_outer = 0.004
        o.collision.cloth_friction = 15.0


if PID == "futon_laid":
    MAT = (1.0, 2.0, 0.07)
    mattress = cushion("shikibuton", MAT, (0, 0, MAT[2] / 2), sheet, round_=0.03, bulge=0.008, seed=1)
    pillow = cushion("makura", (0.5, 0.33, 0.09), (0, 0.74, MAT[2] + 0.045), ticking, round_=0.04, bulge=0.02, lumps=0.006, seed=2)
    floor = floor_collider()
    collide(mattress, pillow)
    # the quilt dropped over the bed from the chest down (the pillow and a little of the
    # mattress's head left bare, as when it's just been laid out), a touch askew
    quilt = drape_quilt("kakebuton", (1.4, 1.75), (0.03, -0.18, MAT[2] + 0.25))
    bpy.data.objects.remove(floor)
    for o in (mattress, pillow):
        o.modifiers.clear()
elif PID == "futon_folded":
    # the mattress folded in three (100 x 67 cm), the quilt in four on it, the pillow on top
    z = 0.0
    for k in range(3):
        t = 0.068
        cushion(f"shikibuton_{k}", (1.0 + rng.uniform(-0.01, 0.01), 0.67, t), (rng.uniform(-0.008, 0.008), rng.uniform(-0.006, 0.006), z + t / 2), sheet, round_=0.033, bulge=0.006, seed=10 + k)
        z += t - 0.004
    for k in range(4):
        t = 0.05
        cushion(f"kakebuton_{k}", (0.95, 0.66, t), (rng.uniform(-0.01, 0.01), rng.uniform(-0.008, 0.008), z + t / 2), cover, round_=0.024, bulge=0.006, seed=20 + k)
        z += t - 0.005
    cushion("makura", (0.5, 0.33, 0.09), (0.05, -0.05, z + 0.04), ticking, round_=0.04, bulge=0.02, lumps=0.006, seed=2)
else:
    raise SystemExit(f"unknown futon {PID}")

# one node: all its parts joined (z = 0 on the floor)
obs = [o for o in COLL.objects if o.type == "MESH"]
for o in obs:   # (one UV layer, the same name on every part, so the join keeps them all)
    while len(o.data.uv_layers) > 1:
        o.data.uv_layers.remove(o.data.uv_layers[1])
    o.data.uv_layers[0].name = "UVMap"
bpy.ops.object.select_all(action="DESELECT")
for o in obs:
    o.select_set(True)
bpy.context.view_layer.objects.active = obs[0]
bpy.ops.object.join()
ob = bpy.context.view_layer.objects.active
ob.name = PID
zmin = min((ob.matrix_world @ v.co).z for v in ob.data.vertices)
ob.location.z -= zmin
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True, use_selection=False)
print("wrote", OUT)
