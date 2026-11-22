"""Review renders of a building's inside (Tsukumodō's home: docs/home_plan.md): plan cutaways of
each floor (everything above the cut removed, seen from above) and eye-level views in its rooms,
lit by the building's own lights (lights.json) and the daylight through its windows.

    blender --background --factory-startup --python tools/blender/render_house_review.py -- <building id> <out dir>

Views (Blender frame of the building):
  plan_ground.png, plan_upper.png    cut at 2.4 m and 5.6 m
  <name>.png                         the eye-level views in VIEWS
"""

import json
import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[sys.argv.index("--") + 1:]
BID, OUT = args[0], os.path.abspath(args[1])
ONLY = set(args[2:])
os.makedirs(OUT, exist_ok=True)
GLTF = os.path.join(ROOT, "assets", "buildings", BID, BID + ".gltf")
LIGHTS = os.path.join(ROOT, "assets", "buildings", BID, "lights.json")

# eye-level views: name -> (eye, look-at), Blender frame, metres
F0, FH, F1 = 0.15, 0.33, 3.605
E = 1.45
VIEWS = {
    "shop": ((0.0, 3.4, F0 + 1.5), (0.5, -1.2, F0 + 1.2)),
    "hall_to_genkan": ((3.0, -1.1, FH + E), (2.9, -4.8, FH + 0.6)),
    "stairs_foot": ((2.6, -1.4, FH + E), (3.9, -3.6, FH + 2.2)),
    "kitchen": ((1.9, -1.6, FH + E), (-1.0, -4.4, FH + 0.9)),
    "kitchen_back": ((-1.3, -4.5, FH + E), (2.0, -1.5, FH + 1.0)),
    "washroom_bath": ((-1.9, -2.85, FH + E), (-2.4, -4.7, FH + 0.6)),
    "stockroom": ((-1.85, -1.5, FH + E), (-4.2, -2.6, FH + 0.9)),
    "upstairs_landing": ((3.0, -4.5, F1 + E), (3.0, 1.0, F1 + 1.0)),
    "corridor_across": ((3.1, 0.9, F1 + E), (-4.0, 0.9, F1 + 1.0)),
    "my_room": ((-0.4, 1.7, F1 + E), (-3.6, 4.6, F1 + 0.6)),
    "my_room_closet": ((-3.6, 2.0, F1 + E), (-0.6, 4.2, F1 + 0.8)),
    "kenji_room": ((3.3, 1.7, F1 + E), (0.8, 4.6, F1 + 0.6)),
    "living": ((2.2, 0.2, F1 + E), (-4.0, -2.2, F1 + 0.5)),
    "back_room": ((-3.9, -2.7, F1 + E), (1.0, -4.8, F1 + 0.8)),
    "toilet": ((3.3, -2.0, F1 + E), (2.5, -3.6, F1 + 1.0)),
}

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=GLTF)
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
for o in meshes:
    if o.name.startswith("kzcloth"):
        o.hide_render = True

s = bpy.context.scene
s.render.engine = "BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT" in {e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items} else "BLENDER_EEVEE"
s.render.resolution_x, s.render.resolution_y = 1600, 1000
s.view_settings.view_transform = "AgX"
s.view_settings.exposure = -0.8
world = bpy.data.worlds.new("w")
world.use_nodes = True
world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.75, 0.82, 0.95, 1)
world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.6
s.world = world
sun = bpy.data.lights.new("sun", "SUN")
sun.energy = 3.5
sun.angle = math.radians(5)
so = bpy.data.objects.new("sun", sun)
so.rotation_euler = (math.radians(40), math.radians(15), math.radians(-35))
s.collection.objects.link(so)
# the building's own lights (glTF: Blender z up; lights.json is in the Blender frame already)
for k, l in enumerate(json.load(open(LIGHTS))):
    pl = bpy.data.lights.new(f"lamp{k}", "POINT")
    pl.energy = l["intensity"] * 1500.0
    pl.color = l["colour"]
    pl.shadow_soft_size = l["radius"]
    lo = bpy.data.objects.new(f"lamp{k}", pl)
    lo.location = l["at"]
    s.collection.objects.link(lo)
cam = bpy.data.cameras.new("cam")
co = bpy.data.objects.new("cam", cam)
s.collection.objects.link(co)
s.camera = co


def shoot(name):
    s.render.filepath = os.path.join(OUT, name + ".png")
    bpy.ops.render.render(write_still=True)
    print("wrote", s.render.filepath)


# eye-level views first (the whole building)
cam.type = "PERSP"
cam.lens = 16
cam.clip_start = 0.05
for name, (eye, at) in VIEWS.items():
    if ONLY and name not in ONLY:
        continue
    co.location = eye
    co.rotation_euler = (Vector(at) - Vector(eye)).to_track_quat("-Z", "Y").to_euler()
    shoot(name)

# the plans: everything above the cut removed
if not ONLY or "plans" in ONLY:
    originals = {o.name: o.data.copy() for o in meshes}
    for plan, cut in (("plan_upper", 5.6), ("plan_ground", 2.4)):
        for o in meshes:
            o.data = originals[o.name].copy()
            bm = bmesh.new()
            bm.from_mesh(o.data)
            bm.transform(o.matrix_world)
            geom = list(bm.verts) + list(bm.edges) + list(bm.faces)
            bmesh.ops.bisect_plane(bm, geom=geom, plane_co=(0, 0, cut), plane_no=(0, 0, 1), clear_outer=True)
            bm.transform(o.matrix_world.inverted())
            bm.to_mesh(o.data)
            bm.free()
        cam.type = "ORTHO"
        cam.ortho_scale = 11.5
        co.location = (0, 0, 30)
        co.rotation_euler = (0, 0, 0)
        s.render.resolution_x, s.render.resolution_y = 1400, 1400
        shoot(plan)
