"""A small skinned test model (not a game asset): a column of three bones that bends back and
forth in a looping clip "sway", exported as glTF with its skin and animation. Proves the path
crates/anim + town::people + akari's deformable meshes.

    blender --background --factory-startup --python tools/blender/make_skin_test.py
-> assets/cache/skin_test/skin_test.gltf
"""
import math
import os

import bpy

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "assets", "cache", "skin_test", "skin_test.gltf")
bpy.ops.wm.read_factory_settings(use_empty=True)

# the mesh: a column 1.5 m tall, 0.15 m radius, many rings so it bends smoothly
bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.15, depth=1.5, location=(0, 0, 0.75))
col = bpy.context.object
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.subdivide(number_cuts=20)
bpy.ops.object.mode_set(mode="OBJECT")
mat = bpy.data.materials.new("skin_test_red")
mat.use_nodes = True
mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.6, 0.12, 0.08, 1)
col.data.materials.append(mat)

# the armature: three bones up the column
bpy.ops.object.armature_add(location=(0, 0, 0))
arm = bpy.context.object
bpy.ops.object.mode_set(mode="EDIT")
eb = arm.data.edit_bones
b0 = eb[0]
b0.name, b0.head, b0.tail = "base", (0, 0, 0), (0, 0, 0.5)
b1 = eb.new("mid")
b1.head, b1.tail, b1.parent = (0, 0, 0.5), (0, 0, 1.0), b0
b2 = eb.new("top")
b2.head, b2.tail, b2.parent = (0, 0, 1.0), (0, 0, 1.5), b1
bpy.ops.object.mode_set(mode="OBJECT")

# skin with automatic weights
col.select_set(True)
arm.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.object.parent_set(type="ARMATURE_AUTO")

# the clip: mid and top bend 30 degrees one way and back, 2 s loop
arm.animation_data_create()
action = bpy.data.actions.new("sway")
arm.animation_data.action = action
bpy.context.scene.frame_start, bpy.context.scene.frame_end = 1, 49
bpy.ops.object.mode_set(mode="POSE")
for f, a in ((1, 0.0), (13, 30.0), (25, 0.0), (37, -30.0), (49, 0.0)):
    for name in ("mid", "top"):
        pb = arm.pose.bones[name]
        pb.rotation_mode = "XYZ"
        pb.rotation_euler = (math.radians(a), 0, 0)
        pb.keyframe_insert("rotation_euler", frame=f)
bpy.ops.object.mode_set(mode="OBJECT")

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_yup=True, export_animations=True, export_skins=True)
print("wrote", OUT)
