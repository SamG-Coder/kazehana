"""Review renders of classroom furniture as it stands in a room: a desk with its chair tucked in
on a classroom-like wood floor against a pale wall, window light from the left (south), from
three sides, and each prop alone from the front.

    blender --background --factory-startup --python tools/blender/render_furniture_review.py
-> assets/qc/school_desk/review_*.png (with the worksheet and notebook_open on the desk)
"""

import math
import os

import bpy
from mathutils import Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUTDIR = os.path.join(ROOT, "assets", "qc", "school_desk")

bpy.ops.wm.read_factory_settings(use_empty=True)
s = bpy.context.scene


def load(pid, at, yaw):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(ROOT, "assets", "props", pid, pid + ".gltf"))
    for o in set(bpy.data.objects) - before:
        if o.parent is None:
            o.location = at
            o.rotation_mode = "XYZ"
            o.rotation_euler = (0, 0, math.radians(yaw))


# the desk (book box towards -Y), its chair on the student's side facing it, half tucked in
load("school_desk", (0, 0, 0), 0)
load("school_chair", (0, -0.36, 0), 0)
# your work on it: the lesson's print at the left, the subject notebook open beside it
load("worksheet", (-0.185, -0.03, 0.70), 4)
load("notebook_open", (0.135, 0.02, 0.70), -2)
# a second set behind, as in a row
load("school_desk", (0, 1.15, 0), 0)
load("school_chair", (0, 0.79, 0), 0)

# the room: a warm wood floor, a cream wall behind and one to the right
def plane(name, size, loc, rot, color, rough):
    bpy.ops.mesh.primitive_plane_add(size=size, location=loc, rotation=rot)
    o = bpy.context.object
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*color, 1)
    b.inputs["Roughness"].default_value = rough
    o.data.materials.append(m)


plane("floor", 12, (0, 0, 0), (0, 0, 0), (0.30, 0.20, 0.12), 0.5)
plane("wall", 12, (0, 2.4, 3), (math.radians(90), 0, 0), (0.80, 0.77, 0.70), 0.8)
plane("wall_r", 12, (2.6, 0, 3), (math.radians(90), 0, math.radians(90)), (0.80, 0.77, 0.70), 0.8)
world = bpy.data.worlds.new("w")
world.use_nodes = True
world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.55, 0.62, 0.75, 1)
world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.35
s.world = world
sun = bpy.data.lights.new("sun", "SUN")
sun.energy = 2.6
sun.angle = math.radians(4)
so = bpy.data.objects.new("sun", sun)
so.rotation_euler = (math.radians(58), 0, math.radians(-75))  # (low, from the left: the south windows)
s.collection.objects.link(so)
area = bpy.data.lights.new("ceiling", "AREA")
area.energy = 120
area.size = 3
ao = bpy.data.objects.new("ceiling", area)
ao.location = (0, 0, 2.9)
s.collection.objects.link(ao)

cam = bpy.data.cameras.new("cam")
co = bpy.data.objects.new("cam", cam)
s.collection.objects.link(co)
s.camera = co
s.render.engine = "BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT" in {e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items} else "BLENDER_EEVEE"
s.render.resolution_x, s.render.resolution_y = 1600, 1000
s.view_settings.view_transform = "AgX"
s.view_settings.look = "AgX - Medium High Contrast"

os.makedirs(OUTDIR, exist_ok=True)
for name, pos, target, lens in [
    ("review_threequarter", (-1.55, -2.0, 1.25), (0.0, -0.1, 0.5), 35),
    ("review_student_side", (0.25, -1.75, 0.95), (0.0, 0.0, 0.55), 40),
    ("review_side", (2.1, -0.25, 0.7), (0.0, -0.15, 0.45), 40),
    ("review_seated_eye", (0.0, -0.55, 1.10), (0.0, 0.25, 0.68), 30),
    ("review_paper", (-0.02, -0.38, 1.12), (-0.02, 0.0, 0.70), 32),
]:
    co.location = pos
    cam.lens = lens
    co.rotation_euler = (Vector(target) - Vector(pos)).to_track_quat("-Z", "Y").to_euler()
    s.render.filepath = os.path.join(OUTDIR, name + ".png")
    bpy.ops.render.render(write_still=True)
    print("wrote", s.render.filepath)
