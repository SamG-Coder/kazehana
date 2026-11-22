"""A MetaHuman export (tools/humans/make_character.py) into one game character glTF:
head and body on one skeleton, its textures, and the body clips from
tools/humans/export_anims.py put onto it.

    blender --background --factory-startup --python tools/humans/to_gltf.py -- <id>
-> assets/cache/humans/<id>/<id>.gltf   (a review copy: the quality gate takes it from there)

The head comes from <id>_Head.fbx at LOD0 without its face shapes (face animation is later),
the body from <id>_Body.fbx at LOD0. The body's skeleton is the master; the head's own joints
(the face's, which the body's skeleton lacks) are joined onto it under the same parents.

The clips were made on MetaHuman's standard body. They are put on ours as rotations relative
to each joint's rest (so a different rest pose, a child's or a tall man's, still takes them),
with the pelvis's movement scaled by the ratio of pelvis heights, and the root's movement taken
out (the game moves the person; the clip's speed is written to <id>.json for it).
"""
import json
import math
import os
import sys

import bpy
from mathutils import Matrix, Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
EXPORT = "D:/KazehanaHumans/export"
cid = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "test_child"
src = os.path.join(EXPORT, cid)
maps = os.path.join(src, cid, "Maps")
out_dir = os.path.join(ROOT, "assets", "cache", "humans", cid)
os.makedirs(out_dir, exist_ok=True)
info = {"id": cid}

bpy.ops.wm.read_factory_settings(use_empty=True)


def import_fbx(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=path, use_anim=True, ignore_leaf_bones=False)
    new = [o for o in bpy.data.objects if o not in before]
    # Unreal's files hang everything under an empty scaled to centimetres: taken off, each
    # object keeping where it is in the world
    bpy.context.view_layer.update()
    for o in new:
        w = o.matrix_world.copy()
        o.parent = None
        o.matrix_world = w
    return new


def drop(objs):
    for o in objs:
        bpy.data.objects.remove(o, do_unlink=True)


# --- the head: its skeleton is the master
head_objs = import_fbx(os.path.join(src, f"{cid}_Head.fbx"))
arm = next(o for o in head_objs if o.type == "ARMATURE")
arm.name = cid
head = next(o for o in head_objs if o.type == "MESH" and o.name.endswith("_LOD0"))
drop([o for o in head_objs if o.type == "MESH" and o is not head])
if head.data.shape_keys:
    head.shape_key_clear()
head.name = "head"

# faces that are see-through layers in Unreal (the eye's wet shell and edge, saliva, the tear
# line) or other LODs' material slots: removed
DROP = ("saliva", "eyeshell", "eyeedge", "cartilage", "_lod")
keep_slot = [not any(k in (m.name.lower() if m else "") for k in DROP) for m in head.data.materials]
import bmesh  # noqa: E402

bm = bmesh.new()
bm.from_mesh(head.data)
dead = [f for f in bm.faces if not keep_slot[f.material_index]]
bmesh.ops.delete(bm, geom=dead, context="FACES")
loose = [v for v in bm.verts if not v.link_faces]
bmesh.ops.delete(bm, geom=loose, context="VERTS")
bm.to_mesh(head.data)
bm.free()
for i in reversed(range(len(head.data.materials))):
    if not keep_slot[i]:
        head.data.materials.pop(index=i)
info["head_slots"] = [m.name for m in head.data.materials]

# --- the body, and its skeleton: the master, with the head's face joints joined on
body_objs = import_fbx(os.path.join(src, f"{cid}_Body.fbx"))
body = next(o for o in body_objs if o.type == "MESH" and o.name.endswith("_LOD0"))
body_arm = next(o for o in body_objs if o.type == "ARMATURE")
drop([o for o in body_objs if o not in (body, body_arm)])
body.name = "body"
head_arm = arm
parents = {b.name: (b.parent.name if b.parent else None) for b in head_arm.data.bones}
shared = {b.name for b in head_arm.data.bones if b.name in body_arm.data.bones}
info["face_joints"] = len(parents) - len(shared)
bpy.ops.object.select_all(action="DESELECT")
bpy.context.view_layer.objects.active = head_arm
head_arm.select_set(True)
bpy.ops.object.mode_set(mode="EDIT")
for eb in list(head_arm.data.edit_bones):
    if eb.name in shared:
        head_arm.data.edit_bones.remove(eb)
bpy.ops.object.mode_set(mode="OBJECT")
bpy.ops.object.select_all(action="DESELECT")
head_arm.select_set(True)
body_arm.select_set(True)
bpy.context.view_layer.objects.active = body_arm
bpy.ops.object.join()
arm = body_arm
arm.name = cid
bpy.ops.object.mode_set(mode="EDIT")
for eb in arm.data.edit_bones:
    if eb.parent is None and parents.get(eb.name) and parents[eb.name] in arm.data.edit_bones:
        eb.parent = arm.data.edit_bones[parents[eb.name]]
bpy.ops.object.mode_set(mode="OBJECT")
for mesh in (head, body):
    for m in mesh.modifiers:
        if m.type == "ARMATURE":
            m.object = arm
# into metres at scale 1 (the skeleton and both meshes together)
bpy.ops.object.select_all(action="DESELECT")
for o in (arm, head, body):
    o.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
for mesh in (head, body):
    mesh.parent = arm
    mesh.matrix_parent_inverse = arm.matrix_world.inverted()
info["joints"] = len(arm.data.bones)
info["head_groups_without_joint"] = [g.name for g in head.vertex_groups if g.name not in arm.data.bones][:20]

# --- materials: Unreal's maps on a plain PBR material (SRMF: specular, roughness, metal, cavity;
# its G and B are glTF's roughness and metal)


def image(name, color=True):
    p = os.path.join(maps, name)
    if not os.path.exists(p):
        return None
    img = bpy.data.images.load(p, check_existing=True)
    img.colorspace_settings.name = "sRGB" if color else "Non-Color"
    return img


def pbr(mat_name, base, normal=None, srmf=None, alpha=False, rough=None):
    m = bpy.data.materials.new(mat_name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Metallic"].default_value = 0.0
    if rough is not None:
        bsdf.inputs["Roughness"].default_value = rough
    if base:
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = image(base)
        nt.links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
        if alpha:
            nt.links.new(t.outputs["Alpha"], bsdf.inputs["Alpha"])
            m.blend_method = "CLIP" if hasattr(m, "blend_method") else None
            m.alpha_threshold = 0.3 if hasattr(m, "alpha_threshold") else None
    if normal and image(normal, False):
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = image(normal, False)
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nt.links.new(t.outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    if srmf and image(srmf, False):
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = image(srmf, False)
        sep = nt.nodes.new("ShaderNodeSeparateColor")
        nt.links.new(t.outputs["Color"], sep.inputs["Color"])
        nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
        nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
    return m


skin_head = pbr(f"{cid}_skin_head", "Head_Basecolor.png", "Head_Normal.png", "Head_SRMF.png")
skin_body = pbr(f"{cid}_skin_body", "Body_Basecolor.png", "Body_Normal.png", "Body_SRMF.png")
teeth = pbr(f"{cid}_teeth", "Teeth_Color.png", "Teeth_Normal.png", rough=0.35)
eyes = pbr(f"{cid}_eyes", "Eyes_Color.png", "Eyes_Normal.png", rough=0.08)
lashes = pbr(f"{cid}_lashes", "Eyelashes_Color.png", alpha=True, rough=0.6)
for i, m in enumerate(head.data.materials):
    n = m.name.lower()
    head.data.materials[i] = (
        teeth if "teeth" in n else eyes if n.startswith("eye") and "lash" not in n else lashes if "lash" in n else skin_head
    )
for i in range(len(body.data.materials)):
    body.data.materials[i] = skin_body

# --- clips


def rest_rot(b):
    return b.matrix_local.to_3x3().normalized()


def baked_onto(name, path):
    """A clip baked on this character's own body (tools/humans/bake_anims.py): every joint's pose
    taken as it is, rotation, position and scale relative to its parent, correctives included
    (they slide, turn and squash the skin round the shoulders, elbows, hips and knees)."""
    objs = import_fbx(path)
    sa = next((o for o in objs if o.type == "ARMATURE"), None)
    if sa is None or sa.animation_data is None or sa.animation_data.action is None:
        drop(objs)
        return None
    act = sa.animation_data.action
    f0, f1 = (int(round(x)) for x in act.frame_range)
    shared = [b.name for b in arm.data.bones if b.name in sa.data.bones and b.name != "root"]
    units = sa.scale.x / arm.scale.x  # the clip's file is in centimetres
    # each joint's rest relative to its parent, in our skeleton and in the clip's. Unreal records
    # the clip's joint offsets from the skeleton it was made on (a grown woman's); each joint's
    # movement away from that rest (a corrective sliding, the pelvis bobbing) is put onto our
    # own rest offsets, so the bones keep this body's lengths
    rest_rel, src_rest_t = {}, {}
    for bn in shared:
        b = arm.data.bones[bn]
        rest_rel[bn] = (b.parent.matrix_local.inverted() @ b.matrix_local) if b.parent else b.matrix_local.copy()
        sb = sa.data.bones[bn]
        src_rest_t[bn] = ((sb.parent.matrix_local.inverted() @ sb.matrix_local) if sb.parent else sb.matrix_local).to_translation()
    new = bpy.data.actions.new(name)
    new.use_fake_user = True
    arm.animation_data_create()
    arm.animation_data.action = new
    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
    scene = bpy.context.scene
    for f in range(f0, f1 + 1):
        scene.frame_set(f)
        for bn in shared:
            s_pb = sa.pose.bones[bn]
            par = s_pb.parent.matrix if s_pb.parent else Matrix.Identity(4)
            rel = par.inverted() @ s_pb.matrix
            loc, rot, scl = rel.decompose()
            loc = rest_rel[bn].to_translation() + (loc - src_rest_t[bn]) * units
            want = Matrix.LocRotScale(loc, rot, scl)
            basis = rest_rel[bn].inverted() @ want
            bl, br, bs = basis.decompose()
            t_pb = arm.pose.bones[bn]
            t_pb.rotation_quaternion = br
            t_pb.location = bl
            t_pb.scale = bs
            t_pb.keyframe_insert("rotation_quaternion", frame=f)
            t_pb.keyframe_insert("location", frame=f)
            t_pb.keyframe_insert("scale", frame=f)
    fps = scene.render.fps / scene.render.fps_base
    drop(objs)
    if act.users == 0:
        bpy.data.actions.remove(act)
    arm.animation_data.action = None
    return {"frames": [f0, f1], "seconds": round((f1 - f0) / fps, 3), "baked": True}


def retarget(name, path):
    objs = import_fbx(path)
    sa = next((o for o in objs if o.type == "ARMATURE"), None)
    if sa is None or sa.animation_data is None or sa.animation_data.action is None:
        drop(objs)
        return None
    act = sa.animation_data.action
    f0, f1 = (int(round(x)) for x in act.frame_range)
    shared = [b.name for b in arm.data.bones if b.name in sa.data.bones]
    # pelvis heights at rest, for the pelvis's movement
    sp = (sa.matrix_world @ sa.data.bones["pelvis"].head_local).z
    tp = (arm.matrix_world @ arm.data.bones["pelvis"].head_local).z
    ratio = tp / sp if sp > 1e-4 else 1.0
    units = sa.scale.x / arm.scale.x  # the clip's file is in centimetres
    new = bpy.data.actions.new(name)
    new.use_fake_user = True
    arm.animation_data_create()
    arm.animation_data.action = new
    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
    scene = bpy.context.scene
    root_start = root_end = None
    for f in range(f0, f1 + 1):
        scene.frame_set(f)
        for bn in shared:
            s_pb = sa.pose.bones[bn]
            t_pb = arm.pose.bones[bn]
            s_rest = rest_rot(sa.data.bones[bn])
            t_rest = rest_rot(arm.data.bones[bn])
            # the source joint's rotation from its rest, in the armature's frame, then into ours
            delta = s_rest @ s_pb.matrix_basis.to_3x3() @ s_rest.inverted()
            q = (t_rest.inverted() @ delta @ t_rest).to_quaternion()
            t_pb.rotation_quaternion = q
            t_pb.keyframe_insert("rotation_quaternion", frame=f)
            if bn == "pelvis":
                d = s_rest @ s_pb.matrix_basis.to_translation()
                t_pb.location = t_rest.inverted() @ (d * ratio * units)
                t_pb.keyframe_insert("location", frame=f)
            if bn == "root":
                p = sa.matrix_world @ s_pb.head
                root_start = root_start or p.copy()
                root_end = p.copy()
    fps = scene.render.fps / scene.render.fps_base
    secs = (f1 - f0) / fps
    speed = ((root_end - root_start).length / secs * ratio) if root_start and secs > 0 else 0.0
    drop(objs)
    if act.users == 0:
        bpy.data.actions.remove(act)
    arm.animation_data.action = None
    return {"frames": [f0, f1], "seconds": round(secs, 3), "root_speed": round(speed, 3), "pelvis_ratio": round(ratio, 3)}


clips = {}
anim_dir = os.path.join(EXPORT, "anims")
own_dir = os.path.join(src, "anims")  # baked on this body, correctives and all (preferred)
for name in ("idle", "walk", "run"):
    own = os.path.join(own_dir, f"{name}.fbx")
    p = os.path.join(anim_dir, f"{name}.fbx")
    if os.path.exists(own):
        r = baked_onto(name, own)
    elif os.path.exists(p):
        r = retarget(name, p)
    else:
        r = None
    if r:
        clips[name] = r
info["clips"] = clips
# push each clip into the NLA so the exporter writes them all
arm.animation_data_create()
for a in bpy.data.actions:
    if a.name in clips:
        tr = arm.animation_data.nla_tracks.new()
        tr.name = a.name
        tr.strips.new(a.name, int(a.frame_range[0]), a)
for pb in arm.pose.bones:
    pb.matrix_basis = Matrix.Identity(4)

# --- out
bpy.context.view_layer.update()
top = max((head.matrix_world @ Vector(c)).z for c in head.bound_box)
feet = min((body.matrix_world @ Vector(c)).z for c in body.bound_box)
info["height_m"] = round(top - feet, 3)
bpy.ops.object.select_all(action="DESELECT")
for o in (arm, head, body):
    o.select_set(True)
bpy.context.view_layer.objects.active = arm
out = os.path.join(out_dir, f"{cid}.gltf")
bpy.ops.export_scene.gltf(
    filepath=out,
    export_format="GLTF_SEPARATE",
    use_selection=True,
    export_yup=True,
    export_skins=True,
    export_influence_nb=8,  # MetaHuman's skin: up to eight joints a vertex
    export_all_influences=False,
    export_animations=bool(clips),
    export_animation_mode="NLA_TRACKS",
    export_morph=False,
    export_image_format="AUTO",
    export_texture_dir="tex",
)
json.dump(info, open(os.path.join(out_dir, f"{cid}.json"), "w"), indent=1)
print("wrote", out, json.dumps(info))
