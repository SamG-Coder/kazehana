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
    "closet_open": ((-2.6, 3.6, F1 + 1.3), (-0.5, 4.0, F1 + 0.9)),
    "my_room_window": ((-2.2, 1.9, F1 + E), (-2.35, 4.8, F1 + 1.5)),
    "kenji_room": ((3.3, 1.7, F1 + E), (0.8, 4.6, F1 + 0.6)),
    "living": ((2.2, 0.2, F1 + E), (-4.0, -2.2, F1 + 0.5)),
    "back_room": ((-3.9, -2.7, F1 + E), (1.0, -4.8, F1 + 0.8)),
    "toilet": ((3.3, -2.0, F1 + E), (2.5, -3.6, F1 + 1.0)),
}

# the school's main building (tools/blender/build_school.py): its own views and plan cuts
SF = [0.3, 3.9, 7.5]
SCHOOL_VIEWS = {
    "outside_south": ((-6.0, 48.0, 1.6), (0.0, 0.0, 5.5)),
    "outside_gate": ((14.0, -30.0, 1.6), (0.0, -6.0, 4.0)),
    "outside_south_near": ((-14.0, 26.0, 1.6), (4.0, 6.0, 5.0)),
    "outside_entrance": ((5.5, -16.0, 1.6), (0.0, -7.0, 2.6)),
    "outside_corner": ((52.0, -22.0, 1.6), (25.0, 0.0, 5.0)),
    "entrance_hall": ((0.0, 4.0, SF[0] + E), (0.0, -6.0, SF[0] + 1.0)),
    "corridor_2f": ((-34.0, -4.5, SF[1] + E), (30.0, -4.5, SF[1] + 1.2)),
    "classroom_1a": ((-30.6, 5.4, SF[1] + E), (-22.7, 0.4, SF[1] + 1.2)),
    "classroom_1a_back": ((-23.6, -1.8, SF[1] + E), (-31.4, 3.5, SF[1] + 1.0)),
    "stairs_west": ((-33.6, -4.6, SF[0] + E), (-38.6, 0.4, SF[0] + 2.3)),
    "staff_room": ((14.6, 5.2, SF[0] + E), (31.0, -1.0, SF[0] + 1.0)),
    "roof": ((-33.0, -3.5, 11.1 + 1.6), (30.0, 3.0, 11.3)),
    "science_room": ((-30.8, 5.4, SF[0] + E), (-22.8, 0.0, SF[0] + 0.9)),
    "home_ec": ((23.2, 5.4, SF[1] + E), (31.0, 0.0, SF[1] + 0.9)),
    "music_room": ((-3.8, 5.4, SF[2] + E), (4.4, 0.0, SF[2] + 1.3)),
    "art_room": ((21.8, 5.2, SF[2] + E), (13.6, 0.0, SF[2] + 0.8)),
    "toilet_boys": ((-32.3, -2.4, SF[0] + E), (-34.6, 5.0, SF[0] + 0.8)),
    "toilet_girls": ((32.3, -2.4, SF[1] + E), (34.6, 5.0, SF[1] + 0.8)),
    "stair_landing": ((-37.5, 0.85, SF[0] + 1.8 + E), (-37.5, 2.7, SF[0] + 1.8 + 1.2)),
    "stairs_up": ((-36.6, -4.6, SF[0] + E), (-38.6, 0.7, SF[0] + 1.9)),
    "corridor_sinks": ((-19.0, -4.2, SF[1] + E), (-26.0, -5.9, SF[1] + 0.9)),
    "corridor_plates": ((-35.0, -4.0, SF[1] + E), (10.0, -3.4, SF[1] + 2.0)),
    "close_lab": ((-29.1, -1.25, 1.65), (-29.7, -0.25, 1.15)),
    "close_corridor_taps": ((-25.0, -4.9, SF[1] + 1.35), (-26.0, -5.85, SF[1] + 0.85)),
    "close_urinals": ((-34.2, -1.5, SF[0] + 1.4), (-35.4, 0.6, SF[0] + 0.8)),
    "close_wc": ((-34.46, 3.6, SF[0] + 1.45), (-34.46, 5.6, SF[0] + 0.4)),
    "close_basin": ((-32.6, -2.6, SF[0] + 1.3), (-31.7, -1.95, SF[0] + 0.85)),
    "close_hob": ((23.9, -1.75, SF[1] + 1.4), (24.6, -0.6, SF[1] + 0.8)),
}
if BID == "school_main":
    VIEWS = SCHOOL_VIEWS
GYM_VIEWS = {
    "gym_outside_north": ((-9.0, -34.0, 1.6), (0.0, -14.0, 4.5)),
    "gym_outside_south": ((30.0, 40.0, 1.6), (0.0, 0.0, 6.0)),
    "gym_arena": ((-17.5, -7.0, 0.5 + E), (13.0, 1.0, 3.2)),
    "gym_stage": ((-6.0, 1.9, 0.5 + E), (13.0, 0.0, 3.0)),
    "gym_gallery": ((-18.6, 14.4, 4.5 + E), (8.0, -6.0, 2.5)),
    "gym_entrance": ((0.0, -15.0, 0.15 + E), (0.0, -11.0, 1.4)),
}
if BID == "school_gym":
    VIEWS = GYM_VIEWS
POOL_VIEWS = {
    "pool_outside": ((-26.0, -20.0, 1.6), (-4.0, 0.0, 1.2)),
    "pool_deck": ((-12.8, -7.4, 1.2 + E), (12.0, 4.0, 0.7)),
    "pool_lanes": ((13.6, 7.4, 1.2 + E), (-12.0, -2.0, 0.6)),
    "pool_steps": ((-8.8, -12.5, 1.6), (-8.8, -6.0, 1.6)),
}
if BID == "school_pool":
    VIEWS = POOL_VIEWS
PLAN_CUTS = {"school_main": (("plan_ground", 2.4), ("plan_2f", 6.0), ("plan_roof", 13.0)), "school_gym": (("plan_gym", 3.0),)}.get(BID, (("plan_upper", 5.6), ("plan_ground", 2.4)))
PLAN_SCALE = {"school_main": 84.0, "school_gym": 44.0}.get(BID, 11.5)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=GLTF)
for o in bpy.data.objects:   # (doors named in KZ_HIDE: open them for the review)
    if any(h and h in o.name for h in os.environ.get("KZ_HIDE", "").split(",")):
        o.hide_render = True
for o in bpy.data.objects:   # (the building's own cloth, the noren: hangs flat until the game simulates it)
    if o.name.startswith("kzcloth"):
        o.hide_render = True
# props from the building's layout, to see them in place (KZ_PROPS: their ids, comma separated)
for pid in [p for p in os.environ.get("KZ_PROPS", "").split(",") if p]:
    lay = json.load(open(os.path.join(ROOT, "assets", "buildings", BID, "layout.json")))
    for e in [e for e in lay if e["id"] == pid]:
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=os.path.join(ROOT, "assets", "props", pid, pid + ".gltf"))
        for o in set(bpy.data.objects) - before:
            if o.parent is None:
                o.location = (o.location.x + e["at"][0], o.location.y + e["at"][1], o.location.z + e["at"][2])
                o.rotation_euler.z += math.radians(e["yaw"])
meshes = [o for o in bpy.data.objects if o.type == "MESH"]

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
for k, l in enumerate(json.load(open(LIGHTS)) if os.path.exists(LIGHTS) else []):   # (an outdoor building may have none)
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
    for plan, cut in PLAN_CUTS:
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
        cam.ortho_scale = PLAN_SCALE
        co.location = (0, 0, 40)
        co.rotation_euler = (0, 0, 0)
        cam.clip_end = 200
        s.render.resolution_x, s.render.resolution_y = (2400, 520) if PLAN_SCALE > 20 else (1400, 1400)
        shoot(plan)
