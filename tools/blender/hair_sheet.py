"""Head turnarounds of built people (front, 3/4, side, back, 3/4 from above) for reviewing hair.

    blender --background --factory-startup --python tools/blender/hair_sheet.py -- <models dir> <out.png> <id> <id> ...
"""
import math
import os
import sys

import bpy
from mathutils import Vector

args = sys.argv[sys.argv.index("--") + 1:]
DIR, OUT, IDS = os.path.abspath(args[0]), os.path.abspath(args[1]), args[2:]
SIZE = 260
VIEWS = [(0, 6), (35, 8), (90, 4), (180, 6), (-40, 38)]  # (turn from the front, degrees; camera height, degrees)
tmp = os.path.splitext(OUT)[0] + "_tmp"
os.makedirs(tmp, exist_ok=True)
tiles = []
for pid in IDS:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.join(DIR, f"p{pid}", f"p{pid}.gltf"))
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    pts = [o.matrix_world @ v.co for o in meshes for v in o.data.vertices]
    top = max(p.z for p in pts)
    head = [p for p in pts if p.z > top - 0.36 * (top / 1.6) ** 0.6]
    lo = Vector((min(p.x for p in head), min(p.y for p in head), min(p.z for p in head)))
    hi = Vector((max(p.x for p in head), max(p.y for p in head), max(p.z for p in head)))
    c = (lo + hi) / 2
    span = max(hi.x - lo.x, hi.z - lo.z, hi.y - lo.y) * 1.15
    sc = bpy.context.scene
    sc.render.engine = "BLENDER_EEVEE"
    sc.render.resolution_x = sc.render.resolution_y = SIZE
    sc.render.film_transparent = False
    w = bpy.data.worlds.new("w")
    sc.world = w
    w.use_nodes = True
    w.node_tree.nodes["Background"].inputs[0].default_value = (0.75, 0.76, 0.78, 1)
    w.node_tree.nodes["Background"].inputs[1].default_value = 0.6
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.5
    sun.rotation_euler = (math.radians(40), 0, math.radians(-30))
    sc.collection.objects.link(sun)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = span
    sc.collection.objects.link(cam)
    sc.camera = cam
    for k, (turn, up) in enumerate(VIEWS):
        a, e = math.radians(turn), math.radians(up)
        # (the character faces -Y in Blender; glTF import keeps that)
        d = Vector((math.sin(a) * math.cos(e), -math.cos(a) * math.cos(e), math.sin(e)))
        cam.location = c + d * 3.0
        cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
        sc.render.filepath = os.path.join(tmp, f"{pid}_{k}.png")
        bpy.ops.render.render(write_still=True)
        tiles.append(sc.render.filepath)

# stitch (one row per person) with Blender's own image API
cols = len(VIEWS)
rows = len(IDS)
W, Hh = cols * SIZE, rows * SIZE
import numpy as np
sheet = np.zeros((Hh, W, 4), dtype=np.float32)
for i, path in enumerate(tiles):
    img = bpy.data.images.load(path)
    px = np.array(img.pixels[:], dtype=np.float32).reshape(SIZE, SIZE, 4)
    r, col = i // cols, i % cols
    y0 = Hh - (r + 1) * SIZE
    sheet[y0:y0 + SIZE, col * SIZE:(col + 1) * SIZE] = px
out = bpy.data.images.new("sheet", W, Hh, alpha=True)
out.pixels[:] = sheet.ravel()
out.filepath_raw = OUT
out.file_format = "PNG"
out.save()
print("sheet", OUT)
