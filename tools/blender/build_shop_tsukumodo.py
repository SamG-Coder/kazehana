"""Tsukumodō (つくも堂): Uncle Kenji's trading card and game shop, the player's home, as one
quality-gated building (docs/GAME_DESIGN.md, docs/TOWN_PLAN.md).

    python tools/make_shop_signs.py            (the sign atlas and the awning canvas first)
    python tools/make_noren_cloth.py           (and the noren's cotton textures)
    blender --background --factory-startup --python tools/blender/build_shop_tsukumodo.py

-> assets/buildings/shop_tsukumodo/shop_tsukumodo.gltf

A Shōwa-era two-storey shophouse in "signboard architecture" (kanban kenchiku): a flat facade of
painted ribbed metal rises to a parapet in front of a gable roof that runs back from the street.
The shop is on the ground floor, the family lives upstairs.

Local frame (Blender): origin at ground level in the middle of the footprint, +X across the
frontage (9 m), +Y towards the street (10 m deep). The street (+Y) face:
- the shopfront set back 1.2 m under the upper floor (an entrance alcove): aluminium-framed
  display windows on tiled knee walls, a pair of sliding glass doors, a roller-shutter case;
  capsule-toy machines in the alcove
- a striped canvas awning, the fascia signboard over it, a projecting vertical sign
- two upstairs windows with iron flower guards, the parapet with its coping
Through the clear glass (MAT_GLASS) the shop shows: shelves of boxed product along both side
walls, the glass singles showcase and the till, a play table with stools, posters, fluorescent
ceiling lights, the doorway to the back with its noren.
The back (-Y): the family's own door (genkan) with a porch roof, kitchen and bathroom windows,
upstairs windows with wall-mounted laundry arms, LPG cylinders, the gas water heater, air
conditioners.
"""

import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ASSET_ID = "shop_tsukumodo"
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")
SIGNS = os.path.join(ROOT, "assets", "cache", "signs", "tsukumodo_signs.png")
AWNING = os.path.join(ROOT, "assets", "cache", "signs", "tsukumodo_awning.png")
# (tools/make_shop_signs.py ATLAS: col0, row0, col1, row1 in a 4096 px square)
ATLAS = {
    "fascia": (0, 0, 4096, 512),
    "vertical": (0, 512, 512, 2560),
    "poster_new": (700, 600, 1468, 1624),
    "poster_cup": (1600, 600, 2368, 1624),
    "poster_buy": (2500, 600, 3268, 1624),
    "noren": (700, 2700, 2200, 3700),
}

W, D = 9.0, 10.0           # footprint: frontage x depth
XW = W / 2
YF, YB = D / 2, -D / 2     # street face, back face
T = 0.18                   # wall thickness
F0 = 0.15                  # shop floor (a step above the street)
F1 = 3.55                  # upper floor
TOP = 6.35                 # side walls' top (eaves)
PAR = 8.15                 # the facade's parapet top
SF = YF - 1.2              # the shopfront line (set back under the upper floor)
CEIL = 3.45                # shop ceiling
PART = -1.2                # the partition between the shop and the home behind it (its shop face)
FH = F0 + 0.18             # the home's floor: a step up from the shop and the genkan's earth floor
UP_WINS = [(-3.4, -1.3, F1 + 1.35, F1 + 2.55), (1.3, 3.4, F1 + 1.35, F1 + 2.55)]
PITCH = math.radians(18.0)
# The shop's props (shelves and stock, the showcase and till, the play table, the capsule-toy
# machines, posters) are separate assets, each approved on its own before it goes in: the
# building is built without them.
PROPS = False
EAVE_OV, BACK_OV = 0.45, 0.4

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)
rng = random.Random(9)

# ----------------------------------------------------------------------------- materials
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
mortar = P("shop_mortar", (0.56, 0.54, 0.49), DETAIL, roughness=0.85, normal_strength=0.7)
facade = P("shop_facade_metal", (0.44, 0.45, 0.40), DETAIL, roughness=0.45, metallic=0.35, normal_strength=0.2)
trim = P("shop_trim", (0.07, 0.07, 0.07), DETAIL, roughness=0.5, normal_strength=0.2)
found = P("house_foundation", (0.36, 0.35, 0.33), DETAIL, roughness=0.85, normal_strength=0.8)
alu = P("house_aluminium", (0.09, 0.075, 0.065), DETAIL, roughness=0.35, metallic=0.8, normal_strength=0.1)
silver = P("shop_silver_alu", (0.55, 0.55, 0.54), DETAIL, roughness=0.3, metallic=0.85, normal_strength=0.1)
shutter = P("shop_shutter_case", (0.42, 0.43, 0.42), DETAIL, roughness=0.45, metallic=0.6, normal_strength=0.15)
unit = P("house_unit", (0.70, 0.70, 0.67), DETAIL, roughness=0.45, metallic=0.2, normal_strength=0.15)
dark = P("house_dark", (0.02, 0.02, 0.02), DETAIL, roughness=0.6, normal_strength=0.1)
room = P("house_room", (0.28, 0.24, 0.20), DETAIL, roughness=0.9, normal_strength=0.2)
curtain = P("house_curtain", (0.62, 0.58, 0.50), DETAIL, roughness=0.95, normal_strength=0.2)
iron = P("shop_iron", (0.05, 0.05, 0.05), DETAIL, roughness=0.55, metallic=0.7, normal_strength=0.2)
# the ceilings: white rock-wool ceiling boards, matte, a faint fine grain
ceiling = P("shop_ceiling", (0.80, 0.79, 0.76), "Concrete_Floor-f4b85cb0", roughness=0.95, normal_strength=0.06, grain=0.06)
inwall = P("shop_inner_wall", (0.74, 0.73, 0.69), DETAIL, roughness=0.9, normal_strength=0.15, grain=0.15)
vinyl = P("shop_floor_vinyl", (0.50, 0.49, 0.46), "Concrete_Floor-f4b85cb0", roughness=0.55, normal_strength=0.2, grain=0.25)
shelf_m = P("shop_shelf", (0.80, 0.80, 0.78), DETAIL, roughness=0.5, metallic=0.4, normal_strength=0.1, grain=0.1)
wood = P("shop_counter_wood", (0.30, 0.19, 0.10), "Wood_Planks", roughness=0.5, normal_strength=0.5)
stool = P("shop_stool", (0.45, 0.08, 0.06), DETAIL, roughness=0.6, normal_strength=0.1)
lpg = P("shop_lpg_cylinder", (0.68, 0.69, 0.66), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.2)
tiles = kz.surface_material("shop_tiles", "Japanese_Concrete_Block_Wall", tint=(0.42, 0.40, 0.38), desaturate=0.5)
alcove_floor = kz.surface_material("shop_alcove_floor", "Concrete_Floor-f4b85cb0", tint=(0.8, 0.8, 0.8))
roof = kz.surface_material("shop_roof", "Japanese_Roof_Tiles", tint=(0.45, 0.46, 0.49), desaturate=0.85)
win_glass = kz.plain_material("kz_glow_glass", (0.025, 0.03, 0.035), roughness=0.04)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)
clear = kz.plain_material("kz_glow_shop_clear_glass", (0.9, 0.95, 0.93), roughness=0.02)
case_glass = kz.plain_material("shop_case_clear_glass", (0.9, 0.95, 0.93), roughness=0.02)
tube = kz.plain_material("kz_neon_shop_light", (0.95, 0.95, 0.92), roughness=0.3, emission=(1.0, 0.98, 0.93), emission_strength=1.0)
lamp = kz.plain_material("kz_lamp_shop_downlight", (0.9, 0.88, 0.8), roughness=0.3)
# product colours (boxed booster displays, starter decks, sleeves, board games)
STOCK = [P("shop_stock_%d" % k, c, DETAIL, roughness=0.35, normal_strength=0.05, grain=0.1) for k, c in enumerate([
    (0.50, 0.06, 0.05), (0.06, 0.13, 0.42), (0.62, 0.46, 0.05), (0.07, 0.30, 0.12), (0.32, 0.07, 0.38),
    (0.74, 0.72, 0.68), (0.06, 0.06, 0.07), (0.70, 0.28, 0.04), (0.05, 0.36, 0.42)])]
CAPS = STOCK[:5] + [STOCK[7], STOCK[8]]


def image_material(name, path, detail_prefix, roughness=0.5, normal_strength=0.3):
    """A material whose colour is a prepared image (signs, posters, canvas) with a scan's normal
    and roughness variation (painted_material, with the image instead of a flat paint)."""
    if name in bpy.data.materials:
        return bpy.data.materials[name]
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


signs = image_material("shop_signs", SIGNS, DETAIL, roughness=0.45, normal_strength=0.25)

# the home's surfaces
HOUSE_TEX = os.path.join(ROOT, "assets", "cache", "house")


def texture_material(name, stem, normal_strength=1.0):
    """A material from prepared base / normal / ORM maps (tools/make_house_textures.py)."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(os.path.join(HOUSE_TEX, stem + "_base.png"), "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(os.path.join(HOUSE_TEX, stem + "_normal.png"), "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = normal_strength
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(os.path.join(HOUSE_TEX, stem + "_orm.png"), "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    return m


tatami = texture_material("home_tatami", "tatami", normal_strength=0.6)
tatami_edge = P("home_tatami_side", (0.16, 0.14, 0.09), DETAIL, roughness=0.8, normal_strength=0.2, grain=0.2)
heri = P("home_tatami_heri", (0.05, 0.05, 0.045), DETAIL, roughness=0.8, normal_strength=0.15, grain=0.1)
plaster = P("home_plaster", (0.74, 0.68, 0.56), DETAIL, roughness=0.95, normal_strength=0.25, grain=0.3)     # juraku-style earth plaster
ceil_wood = P("home_ceiling_wood", (0.55, 0.42, 0.28), "Wood_Planks", roughness=0.6, normal_strength=0.4)
home_floor = kz.surface_material("home_floor_wood", "Wooden_Planks", tint=(0.75, 0.62, 0.48), desaturate=0.2)
home_wood = P("home_wood", (0.36, 0.24, 0.14), "Wood_Planks", roughness=0.45, normal_strength=0.4)
cushion_floor = P("home_cushion_floor", (0.70, 0.66, 0.58), "Concrete_Floor-f4b85cb0", roughness=0.5, normal_strength=0.15, grain=0.2)
bath_tile = P("home_bath_tile", (0.66, 0.74, 0.76), "Concrete_Floor-f4b85cb0", roughness=0.25, normal_strength=0.3, grain=0.1)
fusuma_paper = P("home_fusuma_paper", (0.80, 0.75, 0.62), DETAIL, roughness=0.9, normal_strength=0.1, grain=0.25)
lacquer = P("home_lacquer", (0.03, 0.025, 0.02), DETAIL, roughness=0.3, normal_strength=0.05, grain=0.05)
canvas = image_material("shop_awning_canvas", AWNING, "Wood_Planks", roughness=0.85, normal_strength=0.15)


def _set_mat(b, verts, mat):
    idx = b._mat_index(mat)
    for f in {f for v in verts if v.is_valid for f in v.link_faces}:
        f.material_index = idx


def rcyl(b, p0, p1, r, mat, seg=12):
    """A cylinder from p0 to p1 (any direction)."""
    p0, p1 = Vector(p0), Vector(p1)
    d = p1 - p0
    res = bmesh.ops.create_cone(b.bm, cap_ends=True, segments=seg, radius1=r, radius2=r, depth=d.length)
    rot = Vector((0, 0, 1)).rotation_difference(d.normalized()).to_matrix().to_4x4()
    bmesh.ops.transform(b.bm, matrix=Matrix.Translation((p0 + p1) / 2) @ rot, verts=res["verts"])
    _set_mat(b, res["verts"], mat)


def sphere(b, c, r, mat, sub=2, squash=1.0):
    res = bmesh.ops.create_icosphere(b.bm, subdivisions=sub, radius=r)
    bmesh.ops.transform(b.bm, matrix=Matrix.Translation(Vector(c)) @ Matrix.Diagonal((1, 1, squash, 1)), verts=res["verts"])
    _set_mat(b, res["verts"], mat)


def obox(b, c, size, t, n, mat, bevel=0.0):
    """A box centred at c, size (along t, along n, up), t and n horizontal unit vectors."""
    res = bmesh.ops.create_cube(b.bm, size=1.0)
    verts = res["verts"]
    rot = Matrix((Vector(t), Vector(n), Vector((0, 0, 1)))).transposed().to_4x4()
    bmesh.ops.transform(b.bm, matrix=Matrix.Translation(Vector(c)) @ rot @ Matrix.Diagonal((*size, 1.0)).to_4x4(), verts=verts)
    if bevel > 0:
        edges = list({e for v in verts for e in v.link_edges})
        out = bmesh.ops.bevel(b.bm, geom=edges, offset=bevel, segments=3, profile=0.5, affect="EDGES")
        verts = list(verts) + list(out["verts"])
    _set_mat(b, verts, mat)


def outdoor_unit(b, base, t, n, w=0.8, h=0.56, d=0.29):
    """An air-conditioner outdoor unit with its back to a wall (as build_house_modern_gable.py)."""
    t, n, base = Vector(t), Vector(n), Vector(base)
    up = Vector((0, 0, 1))
    obox(b, base + n * 0.22 + up * 0.04, (w + 0.2, 0.42, 0.08), t, n, found, 0.01)
    for k in (-1, 1):
        obox(b, base + n * 0.2 + t * (k * w * 0.36) + up * 0.1, (0.05, d, 0.04), t, n, dark, 0.005)
    c = base + n * (0.06 + d / 2) + up * (0.12 + h / 2)
    obox(b, c, (w, d, h), t, n, unit, 0.018)
    obox(b, c + up * (h / 2 + 0.006), (w + 0.02, d + 0.02, 0.012), t, n, unit, 0.004)
    front = c + n * (d / 2)
    fc = front + t * (-w * 0.17)
    rcyl(b, fc - n * 0.012, fc + n * 0.002, 0.2, dark, seg=28)
    rcyl(b, fc - n * 0.03, fc - n * 0.012, 0.07, unit, seg=16)
    for k in range(-7, 8):
        z = k * 0.026
        chord = 2 * (max(0.0, 0.2 ** 2 - z ** 2)) ** 0.5
        if chord > 0.02:
            obox(b, fc + up * z + n * 0.006, (chord, 0.006, 0.005), t, n, unit)
    for k in (-2, -1, 0, 1, 2):
        obox(b, fc + t * (k * 0.075) + n * 0.009, (0.006, 0.006, 0.4), t, n, unit)
    for k in range(10):
        obox(b, c - t * (w / 2) + up * (-h / 2 + 0.08 + k * 0.042), (0.004, d * 0.8, 0.018), t, n, dark)
    obox(b, front + t * (w * 0.33) + up * 0.12, (0.18, 0.01, 0.12), t, n, unit, 0.004)


def uv_object(name, faces):
    """An object of quads with their own UVs (signs, posters, the noren): faces = [(4 corners,
    4 uvs, material)]."""
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new()
    mats = []
    for corners, uvs, mat in faces:
        if mat not in mats:
            mats.append(mat)
        f = bm.faces.new([bm.verts.new(c) for c in corners])
        f.material_index = mats.index(mat)
        for l, uv in zip(f.loops, uvs):
            l[uvl].uv = uv
    bm.normal_update()
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for m in mats:
        me.materials.append(m)
    ob = bpy.data.objects.new(name, me)
    kz.coll("HOUSE").objects.link(ob)
    return ob


def atlas_uv(key, flip=False):
    """The UVs of an atlas piece for a quad's corners in the order bottom-left, bottom-right,
    top-right, top-left (as seen from its front)."""
    c0, r0, c1, r1 = ATLAS[key]
    u0, u1, v0, v1 = c0 / 4096, c1 / 4096, 1 - r1 / 4096, 1 - r0 / 4096
    if flip:
        u0, u1 = u1, u0
    return [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]


DOOR_LEAVES = []


def door_leaf(name, pivot, yaw=0.0):
    """A door leaf of its own: a node named for play::door (kzdoor_hinge_<n|p><deg> /
    kzdoor_slide_<n|p><mm>[_auto]), its origin at `pivot` (the hinge; any point of a sliding
    leaf), turned `yaw` degrees about +Z (a sliding leaf slides along its own x: yaw 90 for one
    in a wall running along y). Build into the returned builder in world coordinates."""
    b = kz.MeshBuilder(name, uv_scale=1.0)
    DOOR_LEAVES.append((b, Vector(pivot), yaw))
    return b


walls = kz.MeshBuilder("walls", uv_scale=0.5)
fittings = kz.MeshBuilder("fittings", uv_scale=1.0)
clad = kz.MeshBuilder("cladding", uv_scale=0.25)
roofing = kz.MeshBuilder("roof", uv_scale=0.5)
inside = kz.MeshBuilder("interior", uv_scale=1.0)
awn = kz.MeshBuilder("awning", uv_scale=0.5)
sign_faces = []


# ----------------------------------------------------------------------------- facade frames
# A frame maps (u along the face, d out from it) to plan (x, y); boxes are given as u, d and z ranges
FRONT = lambda u, d: (u, YF + d)
BACK = lambda u, d: (u, YB - d)
EAST = lambda u, d: (XW + d, u)
WEST = lambda u, d: (-XW - d, u)
SHOP = lambda u, d: (u, SF + d)


def fb(b, F, u0, u1, d0, d1, z0, z1, mat):
    pts = [F(u, d) for u in (u0, u1) for d in (d0, d1)]
    b.box((min(p[0] for p in pts), min(p[1] for p in pts), z0), (max(p[0] for p in pts), max(p[1] for p in pts), z1), mat)


def free_rects(u0, u1, z0, z1, holes):
    """The face u0..u1 x z0..z1 minus the holes (u0, u1, z0, z1), as rectangles."""
    xs = sorted({u0, u1} | {h[0] for h in holes} | {h[1] for h in holes})
    out = []
    for a, b in zip(xs[:-1], xs[1:]):
        if b - a < 1e-4:
            continue
        mid = (a + b) / 2
        blocked = sorted((h[2], h[3]) for h in holes if h[0] < mid < h[1])
        z = z0
        for bz0, bz1 in blocked:
            if bz0 > z + 1e-4:
                out.append((a, b, z, min(bz0, z1)))
            z = max(z, bz1)
        if z < z1 - 1e-4:
            out.append((a, b, z, z1))
    return out


def wall(b, F, u0, u1, z0, z1, holes, mat, d0=-T, d1=0.0):
    for a, bb, za, zb in free_rects(u0, u1, z0, z1, holes):
        fb(b, F, a, bb, d0, d1, za, zb, mat)


def window(F, u0, u1, z0, z1, glass, sill=True, curtains=True):
    """An aluminium sash window in an opening of a wall T thick."""
    FR = 0.045
    if curtains:
        fb(walls, F, u0, u0 + (u1 - u0) * 0.3, -0.35, -0.33, z0, z1, curtain)
        fb(walls, F, u1 - (u1 - u0) * 0.25, u1, -0.35, -0.33, z0, z1, curtain)
        fb(walls, F, u0, u1, -0.9, -0.85, z0, z1, room)
    fb(fittings, F, u0, u1, -0.07, -0.065, z0, z1, glass)
    for (a, bb, za, zb) in ((u0, u1, z0, z0 + FR), (u0, u1, z1 - FR, z1), (u0, u0 + FR, z0, z1), (u1 - FR, u1, z0, z1)):
        fb(fittings, F, a, bb, -0.09, -0.01, za, zb, alu)
    if u1 - u0 > 0.9:
        m = (u0 + u1) / 2
        fb(fittings, F, m - 0.03, m + 0.03, -0.085, -0.02, z0, z1, alu)
    if sill:
        fb(fittings, F, u0 - 0.04, u1 + 0.04, -0.02, 0.05, z0 - 0.04, z0, alu)
    fb(walls, F, u0 - 0.002, u0, -T, 0.0, z0, z1, mortar)
    fb(walls, F, u1, u1 + 0.002, -T, 0.0, z0, z1, mortar)


# ----------------------------------------------------------------------------- the shell
# side walls (full depth), the back wall, a foundation band
SIDE_HOLES = [(-1.0, -0.2, F1 + 1.0, F1 + 1.7)]
wall(walls, EAST, YB, YF, 0.0, TOP, SIDE_HOLES, mortar)
window(EAST, -1.0, -0.2, F1 + 1.0, F1 + 1.7, frosted, curtains=False)
WEST_HOLES = [(2.4, 3.6, F1 + 0.9, F1 + 2.0)]     # your room's side window
wall(walls, WEST, YB, YF, 0.0, TOP, WEST_HOLES, mortar)
window(WEST, 2.4, 3.6, F1 + 0.9, F1 + 2.0, clear, curtains=False)

BACK_DOOR = (2.3, 3.3, F0, F0 + 2.05)
BALCONY_DOOR = (-1.0, 1.2, F1, F1 + 2.0)
# (the kitchen's window over the sink, the bath's high window, upstairs: the back room's window,
# the balcony's glass doors, the toilet's and the stair landing's little windows)
BACK_HOLES = [BACK_DOOR, (-0.9, 0.4, F0 + 1.05, F0 + 1.95), (-3.8, -3.1, F0 + 1.3, F0 + 1.95),
              (-3.3, -1.5, F1 + 0.9, F1 + 2.0), BALCONY_DOOR, (1.8, 2.2, F1 + 1.3, F1 + 1.9), (3.0, 4.0, F1 + 1.0, F1 + 1.9)]
wall(walls, BACK, -XW, XW, 0.0, TOP, BACK_HOLES, mortar)
window(BACK, -0.9, 0.4, F0 + 1.05, F0 + 1.95, frosted, curtains=False)
window(BACK, -3.8, -3.1, F0 + 1.3, F0 + 1.95, frosted, curtains=False)
window(BACK, -3.3, -1.5, F1 + 0.9, F1 + 2.0, clear, curtains=False)
window(BACK, 1.8, 2.2, F1 + 1.3, F1 + 1.9, frosted, curtains=False)
window(BACK, 3.0, 4.0, F1 + 1.0, F1 + 1.9, frosted, curtains=False)
# the back door: a dark-brown aluminium door with a frosted strip, its frame, the porch
u0, u1, z0, z1 = BACK_DOOR
# the leaf: hinged at its west edge, opening outwards (clockwise seen from above)
back_leaf = door_leaf("kzdoor_hinge_n95_back", BACK(u0 + 0.005, -0.045) + (z0,))
fb(back_leaf, BACK, u0 + 0.005, u1 - 0.005, -0.06, -0.03, z0 + 0.005, z1 - 0.005, P("shop_back_door", (0.16, 0.10, 0.06), "Wood_Planks", roughness=0.5, normal_strength=0.6))
for (a, bb, za, zb) in ((u0 - 0.06, u0, z0, z1 + 0.06), (u1, u1 + 0.06, z0, z1 + 0.06), (u0 - 0.06, u1 + 0.06, z1, z1 + 0.06)):
    fb(fittings, BACK, a, bb, -0.07, 0.02, za, zb, alu)
fb(back_leaf, BACK, u0 + 0.14, u0 + 0.26, -0.03, -0.025, z0 + 0.4, z1 - 0.3, frosted)
fb(back_leaf, BACK, u1 - 0.15, u1 - 0.12, -0.03, 0.02, z0 + 0.9, z0 + 1.3, silver)           # handle
fb(fittings, BACK, u0 - 0.6, u1 + 0.6, 0.0, 0.9, F0 + 2.3, F0 + 2.38, unit)          # porch roof
fb(fittings, BACK, u0 - 0.6, u1 + 0.6, 0.88, 0.92, F0 + 2.22, F0 + 2.38, silver)
for a in (u0 - 0.6, u1 + 0.56):
    fb(fittings, BACK, a, a + 0.04, 0.0, 0.88, F0 + 2.26, F0 + 2.3, silver)       # brackets
fb(fittings, BACK, u0 - 0.5, u1 + 0.5, 0.0, 0.8, 0.0, F0, found)                  # step
fb(fittings, BACK, u1 + 0.12, u1 + 0.3, 0.0, 0.05, F0 + 1.95, F0 + 2.08, lamp)    # door lamp
fb(fittings, BACK, u1 + 0.15, u1 + 0.3, 0.0, 0.03, F0 + 1.3, F0 + 1.45, silver)   # intercom

# foundation band round the sides and back (the street side is the shopfront)
for F, a, bb in ((EAST, YB, YF), (WEST, YB, YF), (BACK, -XW, XW)):
    fb(walls, F, a, bb, -0.02, 0.012, -0.45, 0.32, found)
fb(clad, FRONT, -XW, XW, -0.05, 0.02, -0.45, F0, tiles)   # the alcove's front edge, down into the ground

# floors and ceilings inside: the shop (Y PART .. the shopfront), the home behind it and upstairs
X0, X1 = -XW + T, XW - T          # inside the side walls
Y0, Y1 = YB + T, YF - T           # inside the back wall, the street wall (upstairs)
YP = PART - 0.1                   # the partition's home face
# the stairs: against the east wall, from the hall by the shop's doorway up towards the back
STEPS = 14
RISE = (F1 - FH) / STEPS
TREAD = 0.215
SX0 = 3.52
SY = YP - 0.15                    # the first riser
STOP = SY - (STEPS - 1) * TREAD   # the last riser (up onto the landing)
GK = -3.8                         # the genkan's step up (its earth floor behind, to the back door)
inside.box((X0, YP, 0.0), (X1, SF, F0), vinyl)                                       # shop floor
inside.box((X0, PART, CEIL - 0.02), (X1, SF, CEIL), ceiling)                         # shop ceiling
for (a, b_, c, d) in free_rects(X0, X1, Y0, Y1, [(SX0 - 0.03, X1, STOP, SY)]):       # the upper floor (the stairwell open)
    walls.box((a, c, CEIL + 0.005), (b_, d, F1), ceiling)
for (a, b_, c, d) in free_rects(X0, X1, Y0, YP, [(SX0 - 0.03, X1, STOP, SY)]):       # the home's ceiling
    inside.box((a, c, CEIL - 0.02), (b_, d, CEIL), ceiling)
walls.box((X0, Y0, TOP - 0.05), (X1, Y1, TOP), ceil_wood)                            # upstairs ceiling
# the shop's inner wall linings, and the partition to the home with its doorway
for x0, x1 in ((X0, X0 + 0.01), (X1 - 0.01, X1)):
    inside.box((x0, PART, F0), (x1, SF, CEIL), inwall)
DOORWAY = (2.6, 3.5, F0, F0 + 2.1)
wall(inside, lambda u, d: (u, PART + d), X0, X1, F0, CEIL, [DOORWAY], inwall, d0=-0.1, d1=0.0)
# the noren in the doorway: two cotton panels hanging from a rod, with a slit between them. Its
# fabric is cloth (its own node, simulated by the game, walked through: tools/blender/noren_mesh.py)
nz1 = DOORWAY[3]
NOREN_Y = PART + 0.07
rcyl(inside, (DOORWAY[0] - 0.05, NOREN_Y, nz1 - 0.03), (DOORWAY[1] + 0.05, NOREN_Y, nz1 - 0.03), 0.012, wood)

# ----------------------------------------------------------------------------- the home (docs/home_plan.md)
# Ground floor, behind the shop (Y0 .. YP), west to east:
#   stockroom (front) over the washroom and the bath (back) | kitchen and dining, the window over
#   the sink | the hall from the shop's doorway back to the genkan and the back door, the stairs
#   against the east wall
# Upstairs (F1), street to back:
#   your room (west) and Kenji's (east), each with its closet (oshiire) | the corridor across |
#   the living room, the stairwell and the corridor beside it down the east | the back room (the
#   balcony's glass doors), the toilet, the stair landing
# Walls are 0.1 thick, centred on their lines; doors are their own nodes (fusuma and glass doors
# slide, the bath's and the toilet's swing).
HT = 0.05                                    # half a partition wall
XF = lambda x: (lambda u, d: (x + d, u))     # a wall along Y at x
YF_ = lambda y: (lambda u, d: (u, y + d))    # a wall along X at y


def partition(F, u0, u1, z0, z1, holes, mat=None):
    wall(inside, F, u0, u1, z0, z1, holes, mat or inwall, d0=-HT, d1=HT)


def line(F, u0, u1, z0, z1, holes, mat):
    """A lining on an outer wall's inner face (the outer walls are mortar right through)."""
    wall(inside, F, u0, u1, z0, z1, holes, mat, d0=-T - 0.012, d1=-T)


# (boards run along a room's length: floors of rooms long in Y go in their own mesh, its UVs
# turned a quarter at the finish)
floors_y = kz.MeshBuilder("floors_along_y", uv_scale=1.0)


def floor(x0, x1, y0, y1, z0, z1, mat):
    (floors_y if (y1 - y0) > 1.5 * (x1 - x0) else inside).box((x0, y0, z0), (x1, y1, z1), mat)


def ceiling_lamp(x, y, z, r=0.22):
    """A round ceiling light: a white acrylic shade on a silver rim."""
    rcyl(inside, (x, y, z - 0.012), (x, y, z), r + 0.015, silver, seg=24)
    rcyl(inside, (x, y, z - 0.07), (x, y, z - 0.012), r, tube, seg=24)


def fusuma(name, F, u0, u1, z0, z1, d, pivot_u, yaw):
    """A sliding paper door: the papered panel in a black lacquered frame, recessed pulls; on
    the face `d` out from its wall's line."""
    leaf = door_leaf(name, F(pivot_u, d) + (z0,), yaw)
    fb(leaf, F, u0 + 0.025, u1 - 0.025, d - 0.014, d + 0.014, z0 + 0.025, z1 - 0.025, fusuma_paper)
    for (a, b_, za, zb) in ((u0, u1, z0, z0 + 0.025), (u0, u1, z1 - 0.025, z1), (u0, u0 + 0.025, z0, z1), (u1 - 0.025, u1, z0, z1)):
        fb(leaf, F, a, b_, d - 0.016, d + 0.016, za, zb, lacquer)
    for k in (-1, 1):                                                      # the pulls, both faces
        pu = u1 - 0.12 if pivot_u < (u0 + u1) / 2 else u0 + 0.12
        fb(leaf, F, pu - 0.03, pu + 0.03, d + k * 0.0145 - 0.0005, d + k * 0.0145 + 0.0005, z0 + 0.82, z0 + 0.9, silver)
    return leaf


def glass_door(name, F, u0, u1, z0, z1, d, pivot_u, yaw, glass_mat):
    """A sliding wooden door with a frosted glass panel (the kitchen's; the balcony's in
    aluminium, clear)."""
    leaf = door_leaf(name, F(pivot_u, d) + (z0,), yaw)
    fm = home_wood if glass_mat is frosted else alu
    fb(leaf, F, u0 + 0.05, u1 - 0.05, d - 0.004, d + 0.004, z0 + 0.05, z1 - 0.05, glass_mat)
    for (a, b_, za, zb) in ((u0, u1, z0, z0 + 0.1), (u0, u1, z1 - 0.05, z1), (u0, u0 + 0.05, z0, z1), (u1 - 0.05, u1, z0, z1), (u0, u1, z0 + 0.95, z0 + 1.0)):
        fb(leaf, F, a, b_, d - 0.018, d + 0.018, za, zb, fm)
    return leaf


def hinged(name, F, u0, u1, z0, z1, d, mat):
    """A flush door hinged at u0 (the name gives which way it turns)."""
    leaf = door_leaf(name, F(u0 + 0.005, d) + (z0,))
    fb(leaf, F, u0 + 0.005, u1 - 0.005, d - 0.018, d + 0.018, z0 + 0.005, z1 - 0.005, mat)
    fb(leaf, F, u1 - 0.12, u1 - 0.08, d - 0.04, d + 0.04, z0 + 0.92, z0 + 0.96, silver)
    return leaf


def casing(F, u0, u1, z0, z1):
    """A doorway's wooden casing (jambs and head), both faces of a partition."""
    for (a, b_, za, zb) in ((u0 - 0.04, u0, z0, z1 + 0.04), (u1, u1 + 0.04, z0, z1 + 0.04), (u0 - 0.04, u1 + 0.04, z1, z1 + 0.04)):
        fb(inside, F, a, b_, -HT - 0.01, HT + 0.01, za, zb, home_wood)


def tatami_room(x0, x1, y0, y1, z):
    """Tatami over a room's floor (up to z): mats about 0.9 x 1.8, in rows along the room's long
    side, every other row shifted by half a mat (half mats at the ends), black cloth borders
    (heri) along their long edges."""
    floor(x0, x1, y0, y1, F1, z - 0.002, tatami_edge)
    along_x = (x1 - x0) >= (y1 - y0)
    (a0, a1), (b0, b1) = ((x0, x1), (y0, y1)) if along_x else ((y0, y1), (x0, x1))
    nr = max(1, round((b1 - b0) / 0.9))
    h = (b1 - b0) / nr
    L = 2 * h
    g = 0.0025                                  # (the seams between mats show dark)
    for r in range(nr):
        c0, c1 = b0 + r * h, b0 + (r + 1) * h
        s = a0
        first = L / 2 if r % 2 else L
        while s < a1 - 1e-3:
            e = min(a1, s + first)
            first = L
            if a1 - e < 0.25 * L:   # (no sliver at the end: the last mat runs on)
                e = a1
            sv = (e - s) / L
            if along_x:
                q = [(s + g, c0 + g), (e - g, c0 + g), (e - g, c1 - g), (s + g, c1 - g)]
                uv = [(0, 0), (0, sv), (1, sv), (1, 0)]
                for cc in (c0 + g, c1 - g - 0.024):
                    inside.box((s + g, cc, z - 0.003), (e - g, cc + 0.024, z + 0.002), heri)
            else:
                q = [(c0 + g, s + g), (c1 - g, s + g), (c1 - g, e - g), (c0 + g, e - g)]
                uv = [(0, 0), (1, 0), (1, sv), (0, sv)]
                for cc in (c0 + g, c1 - g - 0.024):
                    inside.box((cc, s + g, z - 0.003), (cc + 0.024, e - g, z + 0.002), heri)
            tatami_faces.append(([(p[0], p[1], z) for p in q], uv, tatami))
            s = e


tatami_faces = []

# ---- ground floor: floors
floor(-1.6, 2.2, Y0, YP, 0.0, FH, home_floor)                              # kitchen and dining
floor(2.2, SX0, GK, YP, 0.0, FH, home_floor)                               # hall
floor(SX0, X1, SY, YP, 0.0, FH, home_floor)                                # the stairs' foot
floor(X0, -1.6, -2.7, YP, 0.0, FH, vinyl)                                  # stockroom
floor(-2.6, -1.6, Y0, -2.7, 0.0, FH, cushion_floor)                        # washroom
floor(X0, -2.6, Y0, -2.7, 0.0, FH - 0.06, bath_tile)                       # bath (a step down: it drains)
floor(2.2, X1, Y0, GK, 0.0, F0 - 0.04, alcove_floor)                       # genkan: the earth floor (tataki)
inside.box((2.2, GK - 0.03, F0 - 0.04), (SX0, GK + 0.01, FH + 0.004), home_wood)  # the step's polished edge (agarikamachi)
# ---- ground floor: walls (to the ceiling: they carry it)
DOOR_H = FH + 1.82
partition(XF(2.2), Y0, YP, F0, CEIL, [(-2.5, -1.7, F0, DOOR_H)])                       # hall | kitchen
partition(XF(-1.6), Y0, YP, F0, CEIL, [(-2.2, -1.5, F0, DOOR_H), (-3.7, -3.0, F0, DOOR_H)])  # kitchen | stockroom, washroom
partition(YF_(-2.7), X0, -1.6, F0, CEIL, [])                                            # stockroom | washroom and bath
partition(XF(-2.6), Y0, -2.7, F0, CEIL, [(-4.5, -3.8, F0, DOOR_H)], bath_tile)         # washroom | bath
for (F, u0, u1) in ((XF(2.2), -2.5, -1.7), (XF(-1.6), -2.2, -1.5), (XF(-1.6), -3.7, -3.0), (XF(-2.6), -4.5, -3.8)):
    casing(F, u0, u1, FH, DOOR_H)
# linings: plaster in the rooms, tiles in the bath
GROUND_BACK = [h for h in BACK_HOLES if h[2] < F1]
line(BACK, -2.6, X1, F0, CEIL, GROUND_BACK, inwall)
line(BACK, X0, -2.6, F0, CEIL, GROUND_BACK, bath_tile)
line(WEST, Y0, -2.7, F0, CEIL, [], bath_tile)
line(WEST, -2.7, YP, F0, CEIL, [], inwall)
line(EAST, Y0, YP, F0, CEIL, [], inwall)
inside.box((X0, -2.75, F0), (-2.6, -2.65, CEIL), bath_tile)               # (the bath's side of the stockroom wall)
# ---- the stairs: solid treads of polished wood, a handrail on the wall
for k in range(1, STEPS):
    yb, yf = SY - k * TREAD, SY - (k - 1) * TREAD
    inside.box((SX0, yb, 0.0), (X1, yf, FH + k * RISE - 0.03), inwall)
    inside.box((SX0 - 0.015, yb - 0.02, FH + k * RISE - 0.03), (X1, yf, FH + k * RISE), home_wood)
    inside.box((SX0 - 0.012, yb, FH), (SX0, yf, FH + k * RISE - 0.03), home_wood)   # the stringer
y_a, y_b = SY - 0.2, STOP + 0.1
rcyl(inside, (X1 - 0.06, y_a, FH + RISE + 0.82), (X1 - 0.06, y_b, F1 + 0.82), 0.02, home_wood)
for yy in (y_a, (y_a + y_b) / 2, y_b):
    zz = FH + 0.82 + RISE + (F1 - FH - RISE) * (y_a - yy) / (y_a - y_b)
    inside.box((X1 - 0.06, yy - 0.012, zz - 0.05), (X1, yy + 0.012, zz - 0.02), silver)
# ---- ground floor: doors
glass_door("kzdoor_slide_n760_kitchen", XF(2.2), -2.5, -1.7, F0 + 0.03, DOOR_H, HT + 0.03, -2.1, 90, frosted)
glass_door("kzdoor_slide_n680_stockroom", XF(-1.6), -2.2, -1.5, FH, DOOR_H, HT + 0.03, -1.85, 90, frosted)
glass_door("kzdoor_slide_n680_washroom", XF(-1.6), -3.7, -3.0, FH, DOOR_H, HT + 0.03, -3.35, 90, frosted)
hinged("kzdoor_hinge_n90_bath", XF(-2.6), -4.5, -3.8, FH - 0.06, DOOR_H, HT + 0.02, P("home_bath_door", (0.78, 0.80, 0.80), DETAIL, roughness=0.35, normal_strength=0.05, grain=0.05))
# ---- ground floor: lights
for (x, y) in ((0.3, -3.05), (2.85, -2.6), (2.85, -4.3), (-2.95, -2.0), (-2.1, -3.75), (-3.45, -3.75)):
    ceiling_lamp(x, y, CEIL - 0.02, 0.2 if x > -2.5 else 0.14)

# ---- upstairs: floors
FU = F1 + 0.055                                                            # the finished floor
tatami_room(X0, -0.05, 1.45, Y1, FU)                                       # your room
tatami_room(0.05, X1, 1.45, Y1, FU)                                        # Kenji's room
tatami_room(X0, 2.45, -2.45, 0.35, FU)                                     # the living room
tatami_room(X0, 1.45, Y0, -2.55, FU)                                       # the back room
floor(X0, 2.5, 0.35, 1.45, F1, FU, home_floor)                             # the corridor across
floor(2.5, X1, SY, 1.45, F1, FU, home_floor)                               # ... at the stairwell's head
floor(2.45, SX0 - 0.03, Y0, SY, F1, FU, home_floor)                        # ... down beside it
floor(SX0 - 0.03, X1, Y0, STOP, F1, FU, home_floor)                        # the landing
floor(1.45, 2.55, Y0, -2.45, F1, FU, cushion_floor)                        # the toilet
# ---- upstairs: walls (to the ceiling)
UZ0, UZ1 = F1, TOP - 0.05
UDOOR = FU + 1.8
partition(YF_(1.4), X0, X1, UZ0, UZ1, [(-1.5, -0.7, UZ0, UDOOR), (2.9, 3.7, UZ0, UDOOR)], plaster)
partition(XF(0.0), 1.4, Y1, UZ0, UZ1, [], plaster)
partition(YF_(0.4), X0, 2.5, UZ0, UZ1, [(0.6, 1.5, UZ0, UDOOR)], plaster)
partition(XF(2.5), Y0, 0.4, UZ0, UZ1, [(-3.4, -2.7, UZ0, UDOOR)], plaster)
partition(YF_(-2.5), X0, 2.5, UZ0, UZ1, [(-1.6, -0.7, UZ0, UDOOR)], plaster)
partition(XF(1.5), Y0, -2.5, UZ0, UZ1, [], plaster)
partition(XF(SX0 - 0.05), STOP, SY, UZ0, UZ1, [], plaster)                              # the stairwell's sides
partition(YF_(SY - 0.05), SX0 - 0.1, X1, UZ0, UZ1, [], plaster)
for (F, u0, u1) in ((YF_(1.4), -1.5, -0.7), (YF_(1.4), 2.9, 3.7), (YF_(0.4), 0.6, 1.5), (XF(2.5), -3.4, -2.7), (YF_(-2.5), -1.6, -0.7)):
    casing(F, u0, u1, FU, UDOOR)
# the closets (oshiire) in your room and Kenji's, back to back on the wall between: a middle
# shelf (chudan) for the folded futon, two fusuma each
for sx in (-1, 1):
    xf = sx * 0.95
    partition(XF(xf), 3.0, Y1, UZ0, UZ1, [(3.0, Y1, UZ0, UDOOR)], plaster)
    partition(YF_(3.0), min(0.0, xf), max(0.0, xf), UZ0, UZ1, [], plaster)
    inside.box((min(0.05, xf), 3.05, FU + 0.72), (max(-0.05, xf), Y1, FU + 0.76), home_wood)
    floor(min(0.05, xf), max(-0.05, xf), 3.05, Y1, F1, FU, home_wood)
    who = "mine" if sx < 0 else "kenji"
    half = (Y1 - 3.0) / 2
    d = -sx * (HT + 0.02)
    fusuma(f"kzdoor_slide_p{int(half * 1000) - 20}_closet_{who}_a", XF(xf), 3.0, 3.0 + half + 0.01, FU, UDOOR, d, 3.0 + half / 2, 90)
    fusuma(f"kzdoor_slide_n{int(half * 1000) - 20}_closet_{who}_b", XF(xf), 3.0 + half - 0.01, Y1, FU, UDOOR, d - sx * 0.035, 3.0 + 1.5 * half, 90)
# linings: plaster
UP_BACK = [h for h in BACK_HOLES if h[2] >= F1 - 0.01]
line(BACK, X0, X1, UZ0, UZ1, UP_BACK, plaster)
line(WEST, Y0, Y1, UZ0, UZ1, WEST_HOLES, plaster)
line(EAST, Y0, Y1, UZ0, UZ1, SIDE_HOLES, plaster)
line(FRONT, X0, X1, UZ0, UZ1, UP_WINS, plaster)
# ---- upstairs: doors
fusuma("kzdoor_slide_n780_my_room", YF_(1.4), -1.5, -0.7, FU, UDOOR, -(HT + 0.02), -1.1, 0)
fusuma("kzdoor_slide_n780_kenji_room", YF_(1.4), 2.9, 3.7, FU, UDOOR, -(HT + 0.02), 3.3, 0)
fusuma("kzdoor_slide_n880_living", YF_(0.4), 0.6, 1.5, FU, UDOOR, HT + 0.02, 1.05, 0)
fusuma("kzdoor_slide_p880_back_room", YF_(-2.5), -1.6, -0.7, FU, UDOOR, HT + 0.02, -1.15, 0)
hinged("kzdoor_hinge_n90_toilet", XF(2.5), -3.4, -2.7, FU, UDOOR, HT + 0.02, P("home_toilet_door", (0.62, 0.50, 0.36), "Wood_Planks", roughness=0.5, normal_strength=0.4))
# the balcony's glass doors: the west leaf slides over the fixed east one
BD0, BD1 = BALCONY_DOOR[0], BALCONY_DOOR[1]
BZ1 = BALCONY_DOOR[3]
glass_door("kzdoor_slide_p1050_balcony", BACK, BD0, 0.14, FU, BZ1, -0.06, (BD0 + 0.14) / 2, 0, clear)
fb(fittings, BACK, 0.06, BD1, -0.12, -0.08, FU, BZ1, alu)
fb(fittings, BACK, 0.11, BD1 - 0.05, -0.105, -0.095, FU + 0.05, BZ1 - 0.05, clear)
for (a, b_, za, zb) in ((BD0 - 0.04, BD0, F1, BZ1 + 0.04), (BD1, BD1 + 0.04, F1, BZ1 + 0.04), (BD0 - 0.04, BD1 + 0.04, BZ1, BZ1 + 0.04)):
    fb(fittings, BACK, a, b_, -0.14, 0.0, za, zb, alu)
fb(fittings, BACK, BD0, BD1, -0.14, 0.02, F1 - 0.02, FU, alu)             # sill
# the balcony: an aluminium deck on brackets, a railing
BX0, BX1, BD = -1.8, 2.0, 0.95
fb(fittings, BACK, BX0, BX1, 0.0, BD, F1 - 0.1, F1 - 0.04, silver)
for u in (BX0 + 0.1, (BX0 + BX1) / 2, BX1 - 0.1):
    fb(fittings, BACK, u - 0.03, u + 0.03, 0.0, BD - 0.05, F1 - 0.5, F1 - 0.1, silver)
for (u0_, u1_, d0_, d1_) in ((BX0, BX1, BD - 0.04, BD), (BX0, BX0 + 0.04, 0.0, BD), (BX1 - 0.04, BX1, 0.0, BD)):
    fb(fittings, BACK, u0_, u1_, d0_, d1_, F1 + 1.05, F1 + 1.1, silver)    # top rail
    fb(fittings, BACK, u0_, u1_, d0_, d1_, F1 - 0.04, F1 + 0.02, silver)   # bottom rail
u = BX0 + 0.02
while u < BX1:
    fb(fittings, BACK, u - 0.008, u + 0.008, BD - 0.03, BD - 0.014, F1 + 0.02, F1 + 1.05, silver)
    u += 0.11
for dd in (0.12, 0.35, 0.58, 0.81):
    fb(fittings, BACK, BX0 + 0.004, BX0 + 0.02, dd - 0.008, dd + 0.008, F1 + 0.02, F1 + 1.05, silver)
    fb(fittings, BACK, BX1 - 0.02, BX1 - 0.004, dd - 0.008, dd + 0.008, F1 + 0.02, F1 + 1.05, silver)
# ---- upstairs: lights
HOME_UP_LAMPS = ((-2.2, 3.1), (2.2, 3.1), (-1.0, -1.05), (-1.4, -3.65), (2.0, -3.65), (-1.0, 0.9), (3.0, 0.4), (3.0, -2.8))
for (x, y) in HOME_UP_LAMPS:
    ceiling_lamp(x, y, TOP - 0.05, 0.26 if x < 1.6 and abs(y) > 1.0 or abs(x) == 2.2 else 0.15)
HOME_DOWN_LAMPS = ((0.3, -3.05), (2.85, -2.6), (2.85, -4.3), (-2.95, -2.0), (-2.1, -3.75), (-3.45, -3.75))

# ----------------------------------------------------------------------------- the street face
# upper facade: painted ribbed metal from the shopfront's head to the parapet, two windows
wall(walls, FRONT, -XW, XW, CEIL, PAR, UP_WINS, facade)
for (a, bb, za, zb) in UP_WINS:
    window(FRONT, a, bb, za, zb, clear, curtains=False)
    # iron flower guard across the lower half of the window
    for z in (za + 0.05, za + 0.42):
        fb(fittings, FRONT, a - 0.05, bb + 0.05, 0.12, 0.15, z, z + 0.025, iron)
    n = int((bb - a) / 0.12)
    for k in range(n + 1):
        u = a + k * (bb - a) / n
        fb(fittings, FRONT, u - 0.01, u + 0.01, 0.125, 0.145, za + 0.05, za + 0.445, iron)
    for u in (a - 0.05, bb + 0.03):
        fb(fittings, FRONT, u, u + 0.02, 0.0, 0.15, za + 0.2, za + 0.23, iron)
# standing seams every 0.3 m (not behind the sign board or across the windows)
SIGN_Z = (3.62, 4.57)
for k in range(int(W / 0.3) + 1):
    u = -XW + 0.15 + k * 0.3
    if u > XW - 0.05:
        break
    for (a, bb, za, zb) in free_rects(u - 0.012, u + 0.012, CEIL + 0.02, PAR, UP_WINS + [(-3.85, 3.85, SIGN_Z[0] - 0.02, SIGN_Z[1] + 0.02)]):
        fb(fittings, FRONT, a, bb, 0.0, 0.025, za, zb, facade)
# the facade's returns (its thickness shows at the sides above the roof), coping, a drip band
fb(fittings, FRONT, -XW - 0.04, XW + 0.04, -0.25, 0.08, PAR, PAR + 0.07, trim)
fb(fittings, FRONT, -XW - 0.02, XW + 0.02, 0.0, 0.05, CEIL, CEIL + 0.08, trim)
fb(walls, FRONT, -XW, XW, -0.25, -T, TOP, PAR, facade)                            # parapet's back
# the fascia signboard: a frame and the painted board
fb(fittings, FRONT, -3.85, 3.85, 0.0, 0.12, SIGN_Z[0], SIGN_Z[1], trim)
y = YF + 0.121
# (seen from the street, looking -y, +x is on the viewer's left: corners from there, anticlockwise)
sign_faces.append(([(3.8, y, SIGN_Z[0] + 0.03), (-3.8, y, SIGN_Z[0] + 0.03), (-3.8, y, SIGN_Z[1] - 0.03), (3.8, y, SIGN_Z[1] - 0.03)], atlas_uv("fascia"), signs))
for u in (-2.6, 0.0, 2.6):                                                        # sign lamps
    rcyl(fittings, (u, YF + 0.02, SIGN_Z[1] + 0.12), (u, YF + 0.42, SIGN_Z[1] + 0.12), 0.012, iron)
    obox(fittings, Vector((u, YF + 0.45, SIGN_Z[1] + 0.08)), (0.22, 0.12, 0.07), Vector((1, 0, 0)), Vector((0, 1, 0)), iron, 0.01)
    fb(fittings, lambda uu, d: (uu, d), u - 0.09, u + 0.09, YF + 0.4, YF + 0.5, SIGN_Z[1] + 0.04, SIGN_Z[1] + 0.045, lamp)
# the projecting vertical sign at the frontage's east end, on two brackets
VX, VY0, VY1, VZ0, VZ1 = XW - 0.35, YF + 0.18, YF + 0.68, F1 + 0.8, F1 + 2.8
fittings.box((VX - 0.06, VY0, VZ0), (VX + 0.06, VY1, VZ1), trim)
uv = atlas_uv("vertical")
sign_faces.append(([(VX + 0.061, VY0 + 0.02, VZ0 + 0.03), (VX + 0.061, VY1 - 0.02, VZ0 + 0.03), (VX + 0.061, VY1 - 0.02, VZ1 - 0.03), (VX + 0.061, VY0 + 0.02, VZ1 - 0.03)], uv, signs))
sign_faces.append(([(VX - 0.061, VY1 - 0.02, VZ0 + 0.03), (VX - 0.061, VY0 + 0.02, VZ0 + 0.03), (VX - 0.061, VY0 + 0.02, VZ1 - 0.03), (VX - 0.061, VY1 - 0.02, VZ1 - 0.03)], uv, signs))
for z in (VZ0 + 0.25, VZ1 - 0.25):
    fittings.box((VX - 0.02, YF, z - 0.02), (VX + 0.02, VY0, z + 0.02), iron)
    rcyl(fittings, (VX, YF + 0.02, z + 0.02), (VX, VY0, z + 0.3), 0.01, iron)

# the striped awning over the alcove: a canvas slope on an aluminium frame, a valance, side cheeks
AX = 4.15
A_TOP, A_LOW, A_OUT = CEIL + 0.07, CEIL - 0.42, 0.95
L = math.hypot(A_OUT, A_TOP - A_LOW)
ang = math.atan2(A_TOP - A_LOW, A_OUT)
awn.oriented_box(Vector((0, YF + A_OUT / 2, (A_TOP + A_LOW) / 2)), (2 * AX, L, 0.012), Matrix.Rotation(-ang, 3, "X"), canvas)  # (falls away from the wall)
awn.box((-AX, YF + A_OUT - 0.006, A_LOW - 0.24), (AX, YF + A_OUT + 0.006, A_LOW + 0.005), canvas)
for sx in (-1, 1):
    x = sx * AX
    awn.quad([Vector((x, YF, A_TOP)), Vector((x, YF + A_OUT, A_LOW)), Vector((x, YF + A_OUT, A_LOW - 0.24)), Vector((x, YF, A_LOW - 0.1))][::(1 if sx > 0 else -1)], canvas)
    rcyl(fittings, (x - sx * 0.03, YF, A_TOP - 0.03), (x - sx * 0.03, YF + A_OUT, A_LOW - 0.02), 0.015, silver)
rcyl(fittings, (-AX, YF + A_OUT - 0.02, A_LOW - 0.01), (AX, YF + A_OUT - 0.02, A_LOW - 0.01), 0.018, silver)
fb(fittings, FRONT, -AX, AX, 0.0, 0.06, A_TOP - 0.05, A_TOP + 0.04, silver)       # wall rail

# the alcove: floor, soffit with downlights, the cladded pilasters (the side walls' front ends)
clad.box((-XW + T, SF, 0.0), (XW - T, YF + 0.02, F0), alcove_floor)
walls.box((-XW + T, SF + 0.3, CEIL - 0.02), (XW - T, YF, CEIL), ceiling)
for x in (-2.2, 2.2):
    rcyl(fittings, (x, YF - 0.6, CEIL - 0.025), (x, YF - 0.6, CEIL - 0.015), 0.07, lamp, seg=20)
    rcyl(fittings, (x, YF - 0.6, CEIL - 0.03), (x, YF - 0.6, CEIL - 0.022), 0.085, silver, seg=20)
for sx in (-1, 1):
    x_out, x_in = sx * (XW + 0.03), sx * (XW - T - 0.02)
    clad.box((min(x_out, x_in), SF, 0.0), (max(x_out, x_in), YF + 0.03, CEIL), tiles)

# the shopfront: tiled knee walls, display windows, the sliding doors, transom, shutter case
WIN_L, WIN_R = (-XW + T, -1.35), (1.35, XW - T)
KNEE = 0.62
SH_Z = 3.0   # shutter case bottom
for (a, bb) in (WIN_L, WIN_R):
    fb(clad, SHOP, a, bb, -0.15, 0.0, 0.0, KNEE, tiles)
    fb(fittings, SHOP, a, bb, -0.06, -0.055, KNEE, SH_Z - 0.3, clear)
    for (aa, b2, za, zb) in ((a, bb, KNEE, KNEE + 0.06), (a, bb, SH_Z - 0.36, SH_Z - 0.3), (a, a + 0.06, KNEE, SH_Z - 0.3), (bb - 0.06, bb, KNEE, SH_Z - 0.3)):
        fb(fittings, SHOP, aa, b2, -0.1, 0.0, za, zb, silver)
    m = (a + bb) / 2
    fb(fittings, SHOP, m - 0.03, m + 0.03, -0.09, 0.0, KNEE, SH_Z - 0.3, silver)
    fb(fittings, SHOP, a - 0.02, bb + 0.02, 0.0, 0.06, KNEE - 0.02, KNEE + 0.01, silver)  # sill
    # posters stuck inside the glass
if PROPS:  # (each is its own asset, approved one at a time: not part of this building)
    for key, u in (("poster_new", -3.75), ("poster_cup", 3.0)):
        w_, h_ = 0.6, 0.8
        y = SF - 0.075
        z0 = KNEE + 0.55
        sign_faces.append(([(u + w_ / 2, y, z0), (u - w_ / 2, y, z0), (u - w_ / 2, y, z0 + h_), (u + w_ / 2, y, z0 + h_)], atlas_uv(key), signs))
# the doors: an automatic bi-parting pair meeting in the middle, on one track between the window
# frames (their outer stiles run against the windows' stiles)
DZ1 = SH_Z - 0.3
# (their track runs just inside the window frames: open, they slide behind the display windows)
for (a, bb, name) in ((-1.35, 0.0, "kzdoor_slide_n1300_auto_l"), (0.0, 1.35, "kzdoor_slide_p1300_auto_r")):
    leaf = door_leaf(name, SHOP((a + bb) / 2, -0.17) + (F0,))
    fb(leaf, SHOP, a + 0.04, bb - 0.04, -0.173, -0.167, F0 + 0.1, DZ1 - 0.06, clear)
    for (aa, b2, za, zb) in ((a, bb, F0 + 0.02, F0 + 0.1), (a, bb, DZ1 - 0.06, DZ1), (a, a + 0.04, F0 + 0.02, DZ1), (bb - 0.04, bb, F0 + 0.02, DZ1)):
        fb(leaf, SHOP, aa + 0.001, b2 - 0.001, -0.19, -0.15, za, zb, silver)
    edge = (bb - 0.012, bb) if a < 0 else (a, a + 0.012)
    fb(leaf, SHOP, edge[0], edge[1], -0.185, -0.155, F0 + 0.1, DZ1 - 0.06, dark)          # rubber edge
# the door operator's housing over the opening and the leaves' run, with its sensor
fb(fittings, SHOP, -2.7, 2.7, -0.22, -0.12, DZ1, SH_Z - 0.02, silver)
fb(fittings, SHOP, -1.4, 1.4, -0.12, 0.0, DZ1, SH_Z - 0.02, silver)
fb(fittings, SHOP, -0.08, 0.08, 0.0, 0.03, DZ1 + 0.06, DZ1 + 0.14, dark)
fb(fittings, SHOP, -2.7, 2.7, -0.21, -0.13, 0.0, F0 + 0.015, silver)                 # floor track
# transom glass and the head over the whole front
fb(fittings, SHOP, -XW + T, XW - T, -0.06, -0.055, SH_Z - 0.24, SH_Z, clear)
fb(fittings, SHOP, -XW + T, XW - T, -0.1, 0.0, SH_Z - 0.02, SH_Z, silver)
fb(walls, SHOP, -XW + T, XW - T, -0.2, 0.0, SH_Z, CEIL, inwall)
# the roller-shutter case and its guide rails
fb(fittings, SHOP, -XW + T, XW - T, 0.0, 0.32, SH_Z, SH_Z + 0.36, shutter)
fb(fittings, SHOP, -XW + T, XW - T, 0.02, 0.3, SH_Z - 0.015, SH_Z, dark)              # slot
for u in (-XW + T, XW - T - 0.07):
    fb(fittings, SHOP, u, u + 0.07, 0.0, 0.09, 0.0, SH_Z, shutter)

# ----------------------------------------------------------------------------- capsule-toy machines
# a rack of eight (four wide, two high) in the alcove, backs to the west display window
def gacha(c, mat, seed):
    """One capsule-toy machine: c = the front centre at its base. A coloured cabinet with the
    coin dial and capsule chute below, a clear window above full of capsules, a header card."""
    r = random.Random(seed)
    x, y, z = c
    t, n = Vector((1, 0, 0)), Vector((0, 1, 0))
    W_, D_ = 0.44, 0.42
    base = Vector((x, y - D_ / 2, z))
    obox(fittings, base + Vector((0, 0, 0.21)), (W_, D_, 0.42), t, n, mat, 0.012)               # lower cabinet
    # upper cabinet: open-fronted (the window), back, sides, top
    obox(fittings, base + Vector((0, -D_ / 2 + 0.01, 0.66)), (W_, 0.02, 0.46), t, n, mat, 0.004)
    for sx in (-1, 1):
        obox(fittings, base + Vector((sx * (W_ / 2 - 0.01), 0, 0.66)), (0.02, D_, 0.46), t, n, mat, 0.004)
    obox(fittings, base + Vector((0, 0, 0.88)), (W_, D_, 0.02), t, n, mat, 0.004)
    front = y
    # coin mechanism: a silver plate, the dial, the coin slot, the price card
    obox(fittings, Vector((x, front + 0.004, z + 0.25)), (0.2, 0.012, 0.22), t, n, silver, 0.004)
    rcyl(fittings, (x, front + 0.01, z + 0.22), (x, front + 0.035, z + 0.22), 0.06, silver, seg=20)
    obox(fittings, Vector((x, front + 0.04, z + 0.22)), (0.1, 0.018, 0.022), t, n, silver, 0.004)  # dial handle
    obox(fittings, Vector((x + 0.06, front + 0.012, z + 0.33)), (0.008, 0.006, 0.03), t, n, dark)
    obox(fittings, Vector((x - 0.05, front + 0.012, z + 0.33)), (0.07, 0.004, 0.04), t, n, unit)
    # the chute: a dark mouth with a flap
    obox(fittings, Vector((x, front + 0.004, z + 0.08)), (0.13, 0.012, 0.09), t, n, dark, 0.006)
    obox(fittings, Vector((x, front + 0.012, z + 0.1)), (0.12, 0.006, 0.05), t, n, unit, 0.004)
    # the window and the capsules behind it (in a box open to the window)
    wz0, wz1 = z + 0.47, z + 0.83
    obox(fittings, Vector((x, front - 0.2, (wz0 + wz1) / 2)), (W_ - 0.06, 0.012, wz1 - wz0), t, n, unit)
    obox(fittings, Vector((x, front - 0.1, wz0 + 0.005)), (W_ - 0.06, 0.2, 0.01), t, n, unit)
    for k in range(14):
        cx = x + r.uniform(-0.15, 0.15)
        cy = front - r.uniform(0.04, 0.16)
        cz = wz0 + 0.045 + (k // 5) * 0.07 + r.uniform(-0.01, 0.01)
        sphere(fittings, (cx, cy, cz), 0.032, r.choice(CAPS), sub=1)
        sphere(fittings, (cx, cy, cz + 0.006), 0.0325, case_glass, sub=1, squash=0.5)  # clear top half
    obox(fittings, Vector((x, front - 0.006, (wz0 + wz1) / 2)), (W_ - 0.04, 0.004, wz1 - wz0), t, n, case_glass)
    for (dx, sx, dz, sz) in ((0, W_ - 0.02, wz0 - 0.01, 0.02), (0, W_ - 0.02, wz1 + 0.01, 0.02), (-(W_ / 2 - 0.02), 0.025, (wz0 + wz1) / 2, wz1 - wz0), ((W_ / 2 - 0.02), 0.025, (wz0 + wz1) / 2, wz1 - wz0)):
        obox(fittings, Vector((x + dx, front - 0.002, dz)), (sx, 0.014, sz), t, n, mat)
    obox(fittings, Vector((x, front + 0.003, z + 0.86)), (W_ - 0.06, 0.006, 0.05), t, n, r.choice(STOCK[5:7]))  # header


if PROPS:  # (each is its own asset, approved one at a time: not part of this building)
    GX0 = -XW + T + 0.05
    for col in range(4):
        for row in range(2):
            gacha((GX0 + 0.22 + col * 0.45, SF + 0.45, F0 + row * 0.92), [STOCK[0], STOCK[1], STOCK[5], STOCK[2]][(col + row * 2) % 4], 100 + col * 2 + row)
    obox(fittings, Vector((GX0 + 0.9, SF + 0.245, F0 + 0.92)), (1.82, 0.43, 0.02), Vector((1, 0, 0)), Vector((0, 1, 0)), iron)  # rack shelf

# ----------------------------------------------------------------------------- the shop inside
def shelf_unit(x_wall, sx, y0, y1, seed):
    """A steel shelf unit against a side wall (sx = +1 for the west wall, facing +x), 1.9 m tall,
    five shelves of boxed product."""
    r = random.Random(seed)
    xb = x_wall + sx * 0.01
    xf = x_wall + sx * 0.42
    lo, hi = min(xb, xf), max(xb, xf)
    back = (lo, lo + 0.02) if sx > 0 else (hi - 0.02, hi)
    inside.box((back[0], y0, F0), (back[1], y1, F0 + 1.9), shelf_m)
    for yy in (y0, y1 - 0.03):
        inside.box((lo, yy, F0), (hi, yy + 0.03, F0 + 1.9), shelf_m)
    for k in range(5):
        z = F0 + 0.1 + k * 0.42
        inside.box((lo, y0, z - 0.02), (hi, y1, z), shelf_m)
        # boxes along it: displays, decks, sleeves, the odd board game
        y = y0 + 0.05
        while y < y1 - 0.12:
            w_ = r.choice((0.1, 0.12, 0.18, 0.2, 0.26, 0.3))
            h_ = r.uniform(0.12, 0.3) if k < 4 else r.uniform(0.15, 0.36)
            d_ = r.uniform(0.18, 0.34)
            if y + w_ > y1 - 0.05:
                break
            if r.random() < 0.88:
                x0 = x_wall + sx * 0.04
                x1 = x0 + sx * d_
                inside.box((min(x0, x1), y, z), (max(x0, x1), y + w_ - 0.01, z + h_), r.choice(STOCK))
            y += w_ + r.uniform(0.0, 0.02)


if PROPS:  # (each is its own asset, approved one at a time: not part of this building)
    for k, (y0, y1) in enumerate(((PART + 0.1, 1.0), (1.1, 2.4), (2.5, 3.6))):
        shelf_unit(-XW + T, 1, y0, y1, 10 + k)
        shelf_unit(XW - T, -1, y0, y1, 20 + k)

    # the glass singles showcase along the back, the till on it, the pack wall behind
    CX0, CX1, CY0, CY1 = -1.6, 2.3, PART + 0.55, PART + 1.15
    inside.box((CX0, CY0, F0), (CX1, CY1, F0 + 0.62), wood)
    inside.box((CX0, CY0, F0 + 0.62), (CX1, CY1, F0 + 0.64), shelf_m)
    inside.box((CX0 + 0.03, CY0 + 0.03, F0 + 0.64), (CX1 - 0.03, CY1 - 0.03, F0 + 0.66), STOCK[6])  # black felt
    r = random.Random(31)
    for k in range(int((CX1 - CX0 - 0.2) / 0.09)):
        for row in range(3):
            if r.random() < 0.8:
                x = CX0 + 0.1 + k * 0.09
                y = CY0 + 0.1 + row * 0.15
                inside.box((x, y, F0 + 0.66), (x + 0.063, y + 0.088, F0 + 0.665), r.choice(STOCK))
                inside.box((x + 0.004, y + 0.004, F0 + 0.665), (x + 0.059, y + 0.084, F0 + 0.667), STOCK[5])
    inside.box((CX0, CY1 - 0.005, F0 + 0.64), (CX1, CY1, F0 + 0.98), case_glass)          # front glass
    inside.box((CX0, CY0, F0 + 0.975), (CX1, CY1, F0 + 0.985), case_glass)                # top glass
    for x in (CX0, CX1 - 0.02):
        inside.box((x, CY0, F0 + 0.62), (x + 0.02, CY1, F0 + 0.985), silver)
    inside.box((CX0, CY1 - 0.02, F0 + 0.965), (CX1, CY1, F0 + 0.99), silver)
    # the till
    inside.box((1.5, CY0 + 0.1, F0 + 0.985), (1.9, CY0 + 0.45, F0 + 1.08), dark)
    inside.box((1.55, CY0 + 0.15, F0 + 1.08), (1.85, CY0 + 0.18, F0 + 1.28), dark)
    inside.box((1.57, CY0 + 0.181, F0 + 1.1), (1.83, CY0 + 0.183, F0 + 1.26), STOCK[8])
    # the pack wall behind the counter: shallow shelves on the partition
    for k in range(4):
        z = F0 + 1.0 + k * 0.35
        inside.box((-1.6, PART, z - 0.02), (2.3, PART + 0.25, z), shelf_m)
        x = -1.55
        while x < 2.2:
            w_ = r.choice((0.2, 0.22, 0.26))
            if r.random() < 0.9:
                inside.box((x, PART + 0.02, z), (x + w_ - 0.015, PART + 0.22, z + r.uniform(0.12, 0.2)), r.choice(STOCK))
            x += w_
    # posters on the partition beside the doorway
    sign_faces.append(([(-2.5, PART + 0.11, F0 + 1.3), (-3.3, PART + 0.11, F0 + 1.3), (-3.3, PART + 0.11, F0 + 2.37), (-2.5, PART + 0.11, F0 + 2.37)], atlas_uv("poster_buy"), signs))

    # a play table with stools, playmats and cards laid out
    TX0, TX1, TY0, TY1 = -1.0, 1.0, 1.3, 2.2
    inside.box((TX0, TY0, F0 + 0.68), (TX1, TY1, F0 + 0.71), P("shop_table_top", (0.72, 0.70, 0.65), DETAIL, roughness=0.5, normal_strength=0.1, grain=0.1))
    for (x, y) in ((TX0 + 0.06, TY0 + 0.06), (TX1 - 0.06, TY0 + 0.06), (TX0 + 0.06, TY1 - 0.06), (TX1 - 0.06, TY1 - 0.06)):
        rcyl(inside, (x, y, F0), (x, y, F0 + 0.68), 0.02, silver)
    for (x0, mat) in ((TX0 + 0.1, STOCK[1]), (0.1, STOCK[4])):
        inside.box((x0, TY0 + 0.08, F0 + 0.71), (x0 + 0.8, TY1 - 0.08, F0 + 0.714), mat)  # playmats
        for k in range(5):
            x = x0 + 0.08 + k * 0.13
            inside.box((x, TY0 + 0.2, F0 + 0.714), (x + 0.063, TY0 + 0.288, F0 + 0.716), STOCK[5])
    for (x, y) in ((-0.5, TY0 - 0.35), (0.5, TY0 - 0.35), (-0.5, TY1 + 0.35), (0.5, TY1 + 0.35)):
        rcyl(inside, (x, y, F0), (x, y, F0 + 0.43), 0.018, silver)
        rcyl(inside, (x, y, F0 + 0.43), (x, y, F0 + 0.47), 0.16, stool, seg=20)

# fluorescent ceiling lights
SHOP_LAMPS = ((-1.8, 0.1), (1.8, 0.1), (-1.8, 2.5), (1.8, 2.5))
for (x, y) in SHOP_LAMPS:
    inside.box((x - 0.65, y - 0.12, CEIL - 0.07), (x + 0.65, y + 0.12, CEIL - 0.02), shelf_m)
    inside.box((x - 0.6, y - 0.08, CEIL - 0.075), (x + 0.6, y + 0.08, CEIL - 0.068), tube)

# ----------------------------------------------------------------------------- the roof
# a gable from the parapet back over the house; slopes down to the east and west eaves
rise = XW * math.tan(PITCH)
RT = 0.14
run = XW + EAVE_OV
slope_len = run / math.cos(PITCH)
ry0, ry1 = YB - BACK_OV, YF - 0.25
z_eave = TOP - EAVE_OV * math.tan(PITCH)
for s in (-1, 1):  # west (-1), east (+1)
    rot = Matrix.Rotation(PITCH if s > 0 else -PITCH, 3, "Y")
    nrm = rot @ Vector((0, 0, 1))
    c = Vector((s * run / 2, (ry0 + ry1) / 2, z_eave + (TOP + rise - z_eave) / 2 + 0.04)) - nrm * (RT / 2)
    roofing.oriented_box(c, (slope_len, ry1 - ry0, RT), rot, roof)
    xe = s * run
    fittings.box((min(xe, xe - s * 0.035), ry0, z_eave - RT - 0.2), (max(xe, xe - s * 0.035), ry1, z_eave - RT + 0.01), trim)
    roofing.box((min(xe - s * 0.06, xe + s * 0.03), ry0, z_eave - RT - 0.02), (max(xe - s * 0.06, xe + s * 0.03), ry1, z_eave + 0.04), roof)
    slope_dir = Vector((s * math.cos(PITCH), 0, -math.sin(PITCH)))
    n_t = int((ry1 - ry0) / 0.3)
    for k in range(n_t):
        y = ry0 + (k + 0.5) * (ry1 - ry0) / n_t
        tip = Vector((xe + s * 0.02, y, z_eave + 0.02))
        rcyl(roofing, tip - slope_dir * 0.12, tip, 0.07, roof, seg=14)
        rcyl(roofing, tip - slope_dir * 0.01, tip + slope_dir * 0.012, 0.085, roof, seg=14)
    gx = xe + s * 0.08
    rcyl(fittings, (gx, ry0 + 0.05, z_eave - 0.14), (gx, ry1 - 0.05, z_eave - 0.14), 0.06, silver, seg=16)
    for k in range(int((ry1 - ry0) / 0.9) + 1):
        y = ry0 + 0.1 + k * (ry1 - ry0 - 0.2) / max(1, int((ry1 - ry0) / 0.9))
        fittings.box((min(xe - s * 0.035, gx + s * 0.07), y - 0.012, z_eave - 0.21), (max(xe - s * 0.035, gx + s * 0.07), y + 0.012, z_eave - 0.19), alu)
    fittings.box((min(s * XW, xe), ry0, TOP - 0.04), (max(s * XW, xe), ry1, TOP - 0.02), trim)  # soffit
    # downpipes at the back and front ends of each side
    for y in (YB + 0.25, YF - 0.6):
        x = s * (XW + 0.09)
        fittings.cylinder((x, y, 0.0), 0.032, z_eave - 0.55, silver, segments=12)
        rcyl(fittings, (gx, y, z_eave - 0.18), (gx, y, z_eave - 0.3), 0.035, silver)
        rcyl(fittings, (gx, y, z_eave - 0.3), (x, y, z_eave - 0.55), 0.032, silver)
        for z in (1.0, 3.0, 5.0):
            fittings.box((min(s * XW, x + s * 0.04), y - 0.045, z), (max(s * XW, x + s * 0.04), y + 0.045, z + 0.03), alu)
# the back gable in mortar, barge boards along its verges; the ridge cap
tri = [Vector((XW, YB, TOP)), Vector((-XW, YB, TOP)), Vector((0, YB, TOP + rise))]
walls.quad(tri, mortar)
for s in (-1, 1):
    a = Vector((s * run, ry0, z_eave + 0.02))
    b = Vector((0, ry0, TOP + rise + 0.06))
    d = b - a
    rot = Matrix.Rotation(math.atan2(d.z, abs(d.x)) * s, 3, "Y")
    fittings.oriented_box((a + b) / 2 - Vector((0, 0, 0.08)), (d.length, 0.04, 0.22), rot, trim)
rcyl(roofing, (0, ry0 - 0.02, TOP + rise + 0.1), (0, ry1, TOP + rise + 0.1), 0.14, roof, seg=14)

# ----------------------------------------------------------------------------- services (back)
Yv, Xv = Vector((0, -1, 0)), Vector((1, 0, 0))
# LPG: two cylinders on a slab, chained to the wall, the regulator and the pipe to the meter
for k, x in enumerate((-0.9, -0.45)):
    yc = YB - 0.25
    rcyl(fittings, (x, yc, 0.08), (x, yc, 1.12), 0.185, lpg, seg=24)
    sphere(fittings, (x, yc, 1.12), 0.185, lpg, sub=2, squash=0.45)
    rcyl(fittings, (x, yc, 1.17), (x, yc, 1.33), 0.11, lpg, seg=20)          # protective collar
    rcyl(fittings, (x, yc, 0.0), (x, yc, 0.1), 0.17, dark, seg=20)            # foot ring
    rcyl(fittings, (x, yc, 1.2), (x, yc, 1.27), 0.03, silver, seg=12)         # valve
    rcyl(fittings, (x - 0.2, yc + 0.0, 0.95), (x + 0.2, yc + 0.0, 0.95), 0.006, iron, seg=6)  # chain
fittings.box((-1.2, YB - 0.5, 0.0), (-0.15, YB, 0.08), found)
fittings.box((-1.1, YB - 0.03, 0.85), (-0.25, YB, 1.05), iron)                    # chain bracket
rcyl(fittings, (-0.68, YB - 0.25, 1.3), (-0.68, YB - 0.05, 1.45), 0.012, silver)
obox(fittings, Vector((-0.2, YB - 0.08, 1.45)), (0.22, 0.14, 0.26), Xv, Yv, unit, 0.01)  # gas meter
obox(fittings, Vector((-0.2, YB - 0.155, 1.48)), (0.14, 0.01, 0.07), Xv, Yv, dark, 0.004)
# the gas water heater on the wall, its flue hood and pipes
obox(fittings, Vector((0.6, YB - 0.1, 1.6)), (0.36, 0.18, 0.56), Xv, Yv, unit, 0.015)
obox(fittings, Vector((0.6, YB - 0.2, 1.75)), (0.24, 0.03, 0.12), Xv, Yv, silver, 0.005)
for k in range(3):
    rcyl(fittings, (0.5 + k * 0.1, YB - 0.1, 1.32), (0.5 + k * 0.1, YB - 0.1, 0.9), 0.012, silver)
# air conditioners: one on the east side, one at the back
outdoor_unit(fittings, Vector((XW + 0.0, YB + 1.6, 0)), Vector((0, 1, 0)), Xv)
outdoor_unit(fittings, Vector((-2.8, YB, 0)), -Xv, Yv, w=0.72, h=0.6)
# wall-mounted laundry arms under the upstairs window (the poles rest on them)
for x in (-1.6, 1.8):
    fittings.box((x - 0.02, YB - 0.02, F1 + 2.15), (x + 0.02, YB, F1 + 2.3), silver)
    fittings.box((x - 0.015, YB - 0.6, F1 + 2.2), (x + 0.015, YB, F1 + 2.23), silver)
for y in (YB - 0.3, YB - 0.55):
    rcyl(fittings, (-1.8, y, F1 + 2.25), (2.0, y, F1 + 2.25), 0.016, unit, seg=12)

# ----------------------------------------------------------------------------- finish and export
for b in (walls, fittings, clad, inside, awn):
    b.finish(collection="HOUSE")
fy = floors_y.finish(collection="HOUSE")
for d in fy.data.uv_layers.active.data:
    d.uv = (d.uv[1], -d.uv[0])
for b, pivot, yaw in DOOR_LEAVES:
    ob = b.finish(collection="HOUSE")
    ob.data.transform(Matrix.Rotation(math.radians(-yaw), 4, "Z") @ Matrix.Translation(-pivot))
    ob.location = pivot
    ob.rotation_euler.z = math.radians(yaw)
rob = roofing.finish(collection="HOUSE")
# tiles run down each slope: UVs in each face's own frame (u along the eave, v up the slope)
me = rob.data
uvl = me.uv_layers.active
for poly in me.polygons:
    n = poly.normal
    h = Vector((n.x, n.y, 0))
    if h.length < 0.2 or abs(n.z) < 0.5:
        continue
    h.normalize()
    along = Vector((-h.y, h.x, 0))
    up = along.cross(n).normalized()
    for li in poly.loop_indices:
        co = me.vertices[me.loops[li].vertex_index].co
        uvl.data[li].uv = (co.dot(along) * 0.5, co.dot(up) * 0.5)
uv_object("signs", sign_faces)
uv_object("tatami", tatami_faces)
import noren_mesh  # noqa: E402
noren_mesh.build_cloth(kz.coll("HOUSE"), ((DOORWAY[0] + DOORWAY[1]) / 2, NOREN_Y, nz1 - 0.03))

# ----------------------------------------------------------------------------- the fittings' layout
# The shop's fittings are separate props (tools/blender/build_shop_props.py), placed from this
# layout (crates/town/src/plots.rs): Blender frame, metres; yaw in degrees about +Z, 0 = the prop's
# front (+Y) towards the street.
import json  # noqa: E402
lay = []
IX0, IX1 = -XW + T, XW - T
for y in (-0.7, 0.2, 1.1, 2.0, 2.9):                            # west wall shelving
    lay.append(("shop_wall_shelf", IX0 + 0.226, y, -90))
for y in (-0.7, 0.2):                                           # east wall shelving
    lay.append(("shop_wall_shelf", IX1 - 0.226, y, 90))
for y in (1.25, 1.8, 2.35):                                     # singles towers, east wall
    lay.append(("shop_tower_case", IX1 - 0.25, y, 90))
CY = PART + 0.8                                                 # the counter run along the back
lay += [("shop_showcase", -1.6, CY, 0), ("shop_showcase", -0.4, CY, 0), ("shop_counter", 0.8, CY - 0.05, 0)]
for tx in (-1.2, 1.2):                                          # two play tables, four chairs each
    lay.append(("shop_battle_table", tx, 1.65, 0))
    for dx in (-0.45, 0.45):
        lay.append(("shop_pipe_chair", tx + dx, 0.9, 0))
        lay.append(("shop_pipe_chair", tx + dx, 2.4, 180))
# the TV high in the back-west corner, on the partition, turned towards the tables
lay.append(("shop_tv", -3.75, PART + 0.05, -28, 2.35))

# ---- the stock (the card session's props, tools/cardgen --props): booster display boxes on
# the wall shelves and the counter, single cards on stands in the showcases. Each is placed in
# its fitting's own frame, then turned with it.
def put_in(fit_x, fit_y, fit_yaw, lx, ly, lz, pid):
    a = math.radians(fit_yaw)
    lay.append((pid, fit_x + lx * math.cos(a) - ly * math.sin(a), fit_y + lx * math.sin(a) + ly * math.cos(a), fit_yaw, lz))


SINGLES = ["single_stand_gold", "single_stand_holo", "single_stand_obsidian"]
for (pid, fx, fy, fyaw, *rest) in [e for e in lay if e[0] == "shop_wall_shelf"]:
    for z in (0.5, 0.85, 1.2, 1.55):
        for k in range(6):
            put_in(fx, fy, fyaw, -0.35 + k * 0.14, 0.0, z, "booster6_box")
for (pid, fx, fy, fyaw, *rest) in [e for e in lay if e[0] == "shop_tower_case"]:
    for n, z in enumerate((0.51, 0.82, 1.12, 1.42)):
        for k in range(3):
            put_in(fx, fy, fyaw, -0.12 + k * 0.12, 0.02, z + 0.004, SINGLES[(n + k) % 3])
for (pid, fx, fy, fyaw, *rest) in [e for e in lay if e[0] == "shop_showcase"]:
    for n, z in enumerate((0.31, 0.624)):
        for k in range(7):
            put_in(fx, fy, fyaw, -0.45 + k * 0.15, 0.06, z + 0.004, SINGLES[(n + k + int(fx * 7)) % 3])
for (pid, fx, fy, fyaw, *rest) in [e for e in lay if e[0] == "shop_counter"]:
    for k in range(3):
        put_in(fx, fy, fyaw, -0.45 + k * 0.1, 0.12, 0.95, "booster6_box")
with open(os.path.join(os.path.dirname(OUT), "layout.json"), "w") as f:
    json.dump([{"id": e[0], "at": [round(e[1], 3), round(e[2], 3), round(F0 + (e[4] if len(e) > 4 else 0.0), 3)], "yaw": e[3]} for e in lay], f, indent=1)
# the building's lights (crates/town/src/scene.rs): its fluorescent fittings, shining down, on
# while the shop is lit. Blender frame, metres; intensity in the renderer's units (a 100 W bulb
# is about 0.02 at 1 m), colour linear.
with open(os.path.join(os.path.dirname(OUT), "lights.json"), "w") as f:
    # (fluorescent tubes light all round: the ceiling and walls too, not just the floor)
    shop = [{"at": [x, y, CEIL - 0.14], "colour": [1.0, 0.97, 0.92], "intensity": 0.035, "radius": 0.3, "range": 9.0, "cone_deg": None}
            for (x, y) in SHOP_LAMPS]
    # (the home's round ceiling lights: warmer, dimmer)
    home = [{"at": [x, y, z - 0.12], "colour": [1.0, 0.93, 0.82], "intensity": 0.014, "radius": 0.2, "range": 6.0, "cone_deg": None}
            for (x, y, z) in [(x, y, CEIL - 0.02) for (x, y) in HOME_DOWN_LAMPS] + [(x, y, TOP - 0.05) for (x, y) in HOME_UP_LAMPS]]
    json.dump(shop + home, f, indent=1)

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
