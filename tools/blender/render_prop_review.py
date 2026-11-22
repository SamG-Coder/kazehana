"""A close review render of a prop as built (small things the gate's look-dev scene shows too far
away: food, stationery): the glTF on a pale table, framed to fit, soft daylight.

    blender --background --factory-startup --python tools/blender/render_prop_review.py -- <id> [out.png]
-> assets/qc/<id>/review.png
"""

import math
import os
import sys

import bpy
from mathutils import Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[sys.argv.index("--") + 1:]
PID = args[0]
OUT = args[1] if len(args) > 1 else os.path.join(ROOT, "assets", "qc", PID, "review.png")

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.join(ROOT, "assets", "props", PID, PID + ".gltf"))
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
pts = [o.matrix_world @ Vector(c) for o in meshes for c in o.bound_box]
lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
centre = (lo + hi) / 2
size = max((hi - lo).x, (hi - lo).z, 0.05)

# a table top and a soft sky
bpy.ops.mesh.primitive_plane_add(size=size * 8, location=(centre.x, centre.y, lo.z))
table = bpy.context.object
tm = bpy.data.materials.new("table")
tm.use_nodes = True
tm.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.75, 0.72, 0.66, 1)
tm.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.6
table.data.materials.append(tm)
world = bpy.data.worlds.new("w")
world.use_nodes = True
world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.8, 0.85, 0.95, 1)
world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.8
bpy.context.scene.world = world
sun = bpy.data.lights.new("sun", "SUN")
sun.energy = 3.0
sun.angle = math.radians(10)
so = bpy.data.objects.new("sun", sun)
so.rotation_euler = (math.radians(50), math.radians(-20), math.radians(-30))
bpy.context.scene.collection.objects.link(so)

# the camera: a little above, from the front (-Y), framing everything
cam = bpy.data.cameras.new("cam")
cam.lens = 85
co = bpy.data.objects.new("cam", cam)
bpy.context.scene.collection.objects.link(co)
dist = size * 2.6 + 0.1
co.location = centre + Vector((0, -dist, dist * 0.45))
d = centre + Vector((0, 0, (hi - lo).z * 0.1)) - co.location
co.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
bpy.context.scene.camera = co

s = bpy.context.scene
s.render.engine = "BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT" in {e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items} else "BLENDER_EEVEE"
s.render.resolution_x, s.render.resolution_y = 1600, 900
s.render.filepath = OUT
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.render.render(write_still=True)
print("wrote", OUT)
