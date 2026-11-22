"""Kazehana Junior High's main building (honkan): the shell, as one quality-gated building
(docs/school_plan.md).

    blender --background --factory-startup --python tools/blender/build_school.py

-> assets/buildings/school_main/school_main.gltf, lights.json

A 1970s-80s reinforced-concrete school of three storeys, 80 x 12.5 m, a flat roof with a fence.
Classrooms face south over the sports ground (long ribbons of aluminium windows under a concrete
sunshade slab each floor); the corridor runs along the north side, towards the gate. A switchback
stair hall at each end (the west one up to the roof, through a door in its penthouse), toilets
beside them; the entrance on the north side in the middle of the ground floor.

Built in: the floors, walls, ceilings, windows, doors (`kzdoor_*` nodes: classroom doors slide,
toilet and roof doors swing, the entrance's glass doors slide apart), the stairs and their rails,
the blackboards, teachers' platforms and back-wall boards of the classrooms, the ceiling lights.
Not built in (props, each approved on its own): desks, chairs, cubbies, shoe lockers, the staff
room's desks, benches, pianos, shelves.

Local frame (Blender): origin at ground level in the middle of the footprint; +X east along the
building, +Y south (the sports ground), -Y north (the corridor side, the gate).
"""

import math
import os
import sys

import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ASSET_ID = "school_main"
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")

L, D = 79.6, 12.5            # (the gate allows 80 m: the coping overhangs 5 cm each end)
XE, YS = L / 2, D / 2          # east end, south face
T = 0.25                       # outer wall thickness
HT = 0.075                     # half an inner wall
FLOORS = [0.3, 3.9, 7.5]       # finished floor of each storey
STOREY = 3.6
CEIL = 3.0                     # ceiling height
ROOF = FLOORS[-1] + STOREY     # roof slab top
PAR = ROOF + 1.0               # parapet top
X0, X1 = -XE + T, XE - T       # inside the end walls
Y0, Y1 = -YS + T, YS - T       # inside the north and south walls
YC = -2.925                    # the corridor wall's line (corridor Y0 .. YC - HT)
SLOT0 = -31.5                  # the first 9 m bay (classroom) from the west
BAY = 9.0
WC = 4.0                       # the toilets beside each stair hall
STAIR_X = (-XE + T, -35.5)     # the west stair hall (and mirrored in the east)

kz.reset_scene()
bpy.context.scene.collection.children.link(bpy.data.collections.new("SCHOOL"))

# ----------------------------------------------------------------------------- materials
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
render = P("school_render", (0.74, 0.70, 0.60), DETAIL, roughness=0.85, normal_strength=0.6, grain=0.25)
plinth = P("school_plinth", (0.42, 0.41, 0.39), DETAIL, roughness=0.85, normal_strength=0.8)
inwall = P("school_inner_wall", (0.80, 0.79, 0.74), DETAIL, roughness=0.9, normal_strength=0.12, grain=0.12)
dado = P("school_dado", (0.55, 0.62, 0.56), DETAIL, roughness=0.6, normal_strength=0.1, grain=0.1)          # the painted lower wall
ceiling = P("school_ceiling", (0.82, 0.81, 0.78), "Concrete_Floor-f4b85cb0", roughness=0.95, normal_strength=0.06, grain=0.06)
corridor_floor = P("school_corridor_vinyl", (0.52, 0.56, 0.50), "Concrete_Floor-f4b85cb0", roughness=0.45, normal_strength=0.15, grain=0.2)
class_floor = kz.surface_material("school_classroom_floor", "Wood_Planks", tint=(0.78, 0.62, 0.45), desaturate=0.15)
SCHOOL_TEX = os.path.join(ROOT, "assets", "cache", "school")


def tile_material(name, stem, normal_strength=1.0):
    """Tiles from tools/make_school_textures.py (base, normal, ORM: 1 m a texture)."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    for socket, file, cs in (("Base Color", "_base.png", "sRGB"),):
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = kz._image(os.path.join(SCHOOL_TEX, stem + file), cs)
        nt.links.new(t.outputs["Color"], bsdf.inputs[socket])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(os.path.join(SCHOOL_TEX, stem + "_normal.png"), "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = normal_strength
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(os.path.join(SCHOOL_TEX, stem + "_orm.png"), "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    return m


wc_tile = tile_material("school_wall_tile", "fab_white_tile", normal_strength=0.8)   # (Fab: Clean White Tiles, tools/prep_fab_textures.py)
wc_floor = tile_material("school_floor_tile", "floor_tile", normal_strength=0.8)
stair_m = P("school_stair_terrazzo", (0.62, 0.60, 0.56), "Concrete_Floor-f4b85cb0", roughness=0.5, normal_strength=0.3, grain=0.3)
nosing = P("school_stair_nosing", (0.12, 0.14, 0.12), DETAIL, roughness=0.6, normal_strength=0.1)
alu = P("school_aluminium", (0.62, 0.62, 0.60), DETAIL, roughness=0.3, metallic=0.85, normal_strength=0.05, grain=0.05)
steel = P("school_rail_steel", (0.30, 0.42, 0.33), DETAIL, roughness=0.5, metallic=0.4, normal_strength=0.1)
door_m = P("school_door", (0.70, 0.62, 0.48), "Wood_Planks", roughness=0.5, normal_strength=0.3)
board = P("school_blackboard", (0.08, 0.22, 0.16), DETAIL, roughness=0.8, normal_strength=0.1, grain=0.15)
cork = P("school_cork_board", (0.60, 0.45, 0.28), DETAIL, roughness=0.95, normal_strength=0.4, grain=0.4)
wood = P("school_wood_trim", (0.45, 0.33, 0.20), "Wood_Planks", roughness=0.5, normal_strength=0.4)
fence = P("school_fence", (0.20, 0.36, 0.24), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.1)
roof_m = P("school_roof_membrane", (0.45, 0.47, 0.45), "Concrete_Floor-f4b85cb0", roughness=0.8, normal_strength=0.5)
glass = kz.plain_material("kz_glow_school_clear_glass", (0.88, 0.93, 0.92), roughness=0.02)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)
tube = kz.plain_material("kz_neon_school_light", (0.95, 0.95, 0.92), roughness=0.3, emission=(1.0, 0.98, 0.93), emission_strength=1.0)

concrete = kz.surface_material("school_concrete", "Precast_Concrete_Wall", tint=(0.92, 0.88, 0.78), desaturate=0.55)
tile_plinth = P("school_plinth_tile", (0.30, 0.24, 0.20), DETAIL, roughness=0.4, normal_strength=0.5, grain=0.2)
grate = P("school_drain_grate", (0.08, 0.08, 0.08), DETAIL, roughness=0.6, metallic=0.7, normal_strength=0.3)
pipe = P("school_downpipe", (0.66, 0.64, 0.58), DETAIL, roughness=0.45, normal_strength=0.05, grain=0.05)
clock_hand = P("school_clock_hand", (0.04, 0.04, 0.04), DETAIL, roughness=0.4, normal_strength=0.05)
unit_m = P("school_ac_unit", (0.72, 0.72, 0.69), DETAIL, roughness=0.45, metallic=0.2, normal_strength=0.15)
dark = P("school_dark", (0.03, 0.03, 0.03), DETAIL, roughness=0.6, normal_strength=0.1)
tank_m = P("school_water_tank", (0.62, 0.70, 0.74), DETAIL, roughness=0.5, normal_strength=0.2, grain=0.2)
SIGNS = os.path.join(ROOT, "assets", "cache", "signs")


def image_material(name, path, detail_prefix, roughness=0.5, normal_strength=0.2):
    """A material whose colour is a prepared image (a sign) with a scan's normal map."""
    _, n, orm = kz.fab_textures(detail_prefix)
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(path, "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(n, "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = normal_strength
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    bsdf.inputs["Roughness"].default_value = roughness
    return m


clock_mat = image_material("school_clock_face", os.path.join(SIGNS, "school_clock.png"), DETAIL, roughness=0.3, normal_strength=0.05)
name_mat = image_material("school_name_plate", os.path.join(SIGNS, "school_name.png"), DETAIL, roughness=0.25, normal_strength=0.1)
sign_faces = []


def uv_object(name, faces):
    """Faces with their own UVs (the clock, the name plate): [(corners, uvs, material)]."""
    import bmesh
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new()
    mats = []
    for corners, uvs, mat in faces:
        if mat not in mats:
            mats.append(mat)
        f = bm.faces.new([bm.verts.new(c) for c in corners])
        f.material_index = mats.index(mat)
        for lp, uv in zip(f.loops, uvs):
            lp[uvl].uv = uv
    bm.normal_update()
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for m in mats:
        me.materials.append(m)
    ob = bpy.data.objects.new(name, me)
    kz.coll("SCHOOL").objects.link(ob)
    return ob


def outdoor_unit(b, base, t, n, w=0.8, h=0.6, d=0.3):
    """An air conditioner's outdoor unit standing on its feet, its fan grille towards n."""
    t, n, base = Vector(t), Vector(n), Vector(base)
    up = Vector((0, 0, 1))
    rot = Matrix((t, n, up)).transposed()
    c = base + up * (0.1 + h / 2)
    b.oriented_box(c, (w, d, h), rot, unit_m)
    b.oriented_box(c + up * (h / 2 + 0.006), (w + 0.02, d + 0.02, 0.012), rot, unit_m)
    for k in (-1, 1):
        b.oriented_box(base + t * (k * w * 0.36) + up * 0.05, (0.06, d + 0.1, 0.1), rot, dark)
    fc = c + n * (d / 2 + 0.003) - t * (w * 0.17)
    for kk in range(-6, 7):
        z = kk * 0.03
        chord = 2 * max(0.0, 0.2 ** 2 - z ** 2) ** 0.5
        if chord > 0.03:
            b.oriented_box(fc + up * z, (chord, 0.006, 0.006), rot, dark)
    b.oriented_box(c - t * (w / 2 + 0.002), (0.004, d * 0.8, h * 0.8), rot, dark)


DOMA = 0.15                    # the entrance's earth floor (doma): a step below the hall
doma_floor = kz.surface_material("school_doma_floor", "Japanese_Shrine_Stone_Floor", tint=(0.62, 0.60, 0.56), desaturate=0.4)
walls = kz.MeshBuilder("walls", uv_scale=0.5)
inside = kz.MeshBuilder("interior", uv_scale=1.0)
fit = kz.MeshBuilder("fittings", uv_scale=1.0)
floors = kz.MeshBuilder("floors", uv_scale=1.0)
DOORS = []
LIGHTS = []

# ----------------------------------------------------------------------------- helpers
NORTH = lambda u, d: (u, -YS - d)     # u along +X; d out of the face
SOUTH = lambda u, d: (u, YS + d)
WEST = lambda u, d: (-XE - d, u)      # u along +Y
EAST = lambda u, d: (XE + d, u)
XF = lambda x: (lambda u, d: (x + d, u))     # an inner wall along Y at x
YF = lambda y: (lambda u, d: (u, y + d))     # an inner wall along X at y


def fb(b, F, u0, u1, d0, d1, z0, z1, mat):
    pts = [F(u, d) for u in (u0, u1) for d in (d0, d1)]
    b.box((min(p[0] for p in pts), min(p[1] for p in pts), z0), (max(p[0] for p in pts), max(p[1] for p in pts), z1), mat)


def free_rects(u0, u1, z0, z1, holes):
    xs = sorted({u0, u1} | {h[0] for h in holes if u0 < h[0] < u1} | {h[1] for h in holes if u0 < h[1] < u1})
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


def wall(b, F, u0, u1, z0, z1, holes, mat, d0, d1):
    for a, bb, za, zb in free_rects(u0, u1, z0, z1, holes):
        fb(b, F, a, bb, d0, d1, za, zb, mat)


def sashes(F, u0, u1, z0, z1, d=-0.12, mat=glass, pane=1.6):
    """A ribbon of aluminium sliding sashes in an opening: frames every `pane` metres, a sill."""
    n = max(1, round((u1 - u0) / pane))
    fb(fit, F, u0, u1, d - 0.004, d + 0.004, z0, z1, mat)
    for k in range(n + 1):
        u = u0 + (u1 - u0) * k / n
        fb(fit, F, u - 0.03, u + 0.03, d - 0.04, d + 0.04, z0, z1, alu)
    for z in (z0, z1 - 0.05):
        fb(fit, F, u0, u1, d - 0.04, d + 0.04, z, z + 0.05, alu)
    fb(fit, F, u0 - 0.03, u1 + 0.03, -0.02, 0.06, z0 - 0.04, z0, alu)     # outer sill


def door_leaf(name, pivot, yaw=0.0):
    b = kz.MeshBuilder(name, uv_scale=1.0)
    DOORS.append((b, Vector(pivot), yaw))
    return b


def sliding(name, F, u0, u1, z0, z1, d, glass_top=True):
    """A classroom door: a wooden leaf with a glass panel at eye height and a recessed pull."""
    leaf = door_leaf(name, F((u0 + u1) / 2, d) + (z0,))
    if glass_top:
        # the leaf framed round a real opening, one pane in it (two sheets laid on a solid leaf
        # showed the slab behind them: the window read as a dark mirror)
        g0, g1, w0, w1 = u0 + 0.15, u1 - 0.15, z0 + 1.0, z1 - 0.2
        fb(leaf, F, u0, u1, d - 0.02, d + 0.02, z0, w0, door_m)
        fb(leaf, F, u0, u1, d - 0.02, d + 0.02, w1, z1, door_m)
        fb(leaf, F, u0, g0, d - 0.02, d + 0.02, w0, w1, door_m)
        fb(leaf, F, g1, u1, d - 0.02, d + 0.02, w0, w1, door_m)
        fb(leaf, F, g0, g1, d - 0.003, d + 0.003, w0, w1, glass)
    else:
        fb(leaf, F, u0, u1, d - 0.02, d + 0.02, z0, z1, door_m)
    for k in (-1, 1):
        pu = u0 + 0.12 if name.split("_")[2].startswith("p") else u1 - 0.12
        fb(leaf, F, pu - 0.03, pu + 0.03, d + k * 0.021 - 0.002, d + k * 0.021 + 0.002, z0 + 0.85, z0 + 1.0, alu)
    return leaf


def door_frame(F, u0, u1, z0, z1, t0, t1, face, rail=None):
    """A doorway's frame in a wall (faces t0..t1): its reveal lined in wood (sides and head, the
    head down to the leaf's top z1) and, for a sliding leaf on the `face` side, the head rail
    over the leaf's whole run (rail = (r0, r1)), which hides its top as it slides."""
    for (a, b) in ((u0 - 0.01, u0 + 0.015), (u1 - 0.015, u1 + 0.01)):
        fb(inside, F, a, b, t0 - 0.004, t1 + 0.004, z0, z1 + 0.03, wood)
    fb(inside, F, u0, u1, t0 - 0.004, t1 + 0.004, z1 + 0.003, z1 + 0.03, wood)
    if rail:
        d0 = face - 0.06 if face < 0 else face
        d1 = face if face < 0 else face + 0.06
        fb(inside, F, rail[0], rail[1], d0, d1, z1 + 0.005, z1 + 0.09, wood)


def hinged(name, F, u0, u1, z0, z1, d, mat=door_m):
    leaf = door_leaf(name, F(u0 + 0.005, d) + (z0,))
    fb(leaf, F, u0 + 0.005, u1 - 0.005, d - 0.02, d + 0.02, z0, z1, mat)
    fb(leaf, F, u1 - 0.12, u1 - 0.08, d - 0.05, d + 0.05, z0 + 0.95, z0 + 1.0, alu)
    return leaf


def light_row(x0, x1, y, z, n):
    """Fluorescent ceiling fittings in a row; the lights (lights.json) one per fitting pair."""
    for k in range(n):
        x = x0 + (x1 - x0) * (k + 0.5) / n
        inside.box((x - 0.65, y - 0.1, z - 0.06), (x + 0.65, y + 0.1, z), alu)
        inside.box((x - 0.6, y - 0.07, z - 0.068), (x + 0.6, y + 0.07, z - 0.06), tube)


# The fittings' light, doubled: since the tubes glow for the eye only (MAT_UNLIT_EMISSIVE) the
# rooms are lit by these alone, and at the first values they read dim beside the windows.
LAMP_GAIN = 2.0


def lamp(x, y, z, intensity=0.03, radius=0.4, rng=10.0):
    LIGHTS.append({"at": [round(x, 3), round(y, 3), round(z - 0.15, 3)], "colour": [1.0, 0.97, 0.93], "intensity": round(intensity * LAMP_GAIN, 4), "radius": radius, "range": rng, "cone_deg": None})


# ----------------------------------------------------------------------------- the plan
def slot(k):
    return SLOT0 + BAY * k, SLOT0 + BAY * (k + 1)


# what each 9 m bay holds, by floor: (name, kind, splits): kind "class" for a classroom (board,
# platform, two doors), "room" for a special room (one or two doors), "entrance", "staff"
PLAN = [
    [("science room", "room", 1), ("science prep|lunch prep", "half", 1), ("nurse|counselling", "half", 1), ("entrance", "entrance", 1),
     ("principal|broadcast", "half", 1), ("staff room", "staff", 2), None],
    [("1-A", "class", 1), ("1-B", "class", 1), ("2-A", "class", 1), ("2-B", "class", 1), ("library", "room", 1), ("computer room", "room", 1), ("home economics", "room", 1)],
    [("3-A", "class", 1), ("3-B", "class", 1), ("spare classroom", "class", 1), ("music room", "room", 1), ("music prep|art prep", "half", 1), ("art room", "room", 1), ("technology room", "room", 1)],
]
SOUTH_BAND = (0.85, 2.75)     # classroom windows: sill and head above the floor
NORTH_BAND = (0.9, 2.3)       # corridor windows

# ----------------------------------------------------------------------------- outer walls
# The concrete frame shows on both long faces: columns every 4.5 m standing out of the wall, the
# floor beams as bands, the walls between them set back, each bay's window deep in its opening.
# The south face has a balcony along each upper floor (a slab, a solid upstand, a steel rail),
# which shades the floor below; a sunshade slab over the top floor's windows.
OUTER_HOLES = {}
COLS = [SLOT0 + 4.5 * k for k in range(15)] + [-35.5, 35.5, -XE + 0.3, XE - 0.3]
COL_W, COL_D = 0.6, 0.16
BAL_D = 1.3


def bays(lo, hi):
    """The openings between columns from lo to hi."""
    cs = sorted(c for c in COLS if lo - 1e-3 <= c <= hi + 1e-3)
    return list(zip(cs[:-1], cs[1:]))


for fi, F0 in enumerate(FLOORS):
    zb = F0 - 0.25 if fi else 0.0
    zt = F0 + STOREY - 0.25 if fi < 2 else ROOF - 0.25
    # south: a window in every 4.5 m bay of the rooms; small windows at the toilets
    holes = []
    for a, b in bays(SLOT0, -SLOT0):
        holes.append((a + COL_W / 2 + 0.12, b - COL_W / 2 - 0.12, F0 + SOUTH_BAND[0], F0 + SOUTH_BAND[1]))
    for a in (-35.5, 31.5):
        holes.append((a + 1.2, a + WC - 1.2, F0 + 1.5, F0 + 2.3))
    wall(walls, SOUTH, -XE, XE, zb, zt + 0.25, holes, concrete, -T, 0.0)
    OUTER_HOLES[(fi, "S")] = list(holes)
    for (a, b, z0, z1) in holes:
        sashes(SOUTH, a, b, z0, z1, mat=glass if z1 - z0 > 1 else frosted)
    # north: the corridor's windows bay by bay, the entrance, the stair halls' landing windows
    holes = []
    for a, b in bays(SLOT0, -SLOT0):
        if fi == 0 and a >= -4.5 - 1e-3 and b <= 4.5 + 1e-3:
            continue
        holes.append((a + COL_W / 2 + 0.25, b - COL_W / 2 - 0.25, F0 + NORTH_BAND[0], F0 + NORTH_BAND[1]))
    if fi == 0:
        holes.append((-2.4, 2.4, DOMA, F0 + 2.3))     # the entrance doors
    for a in (-35.5, 31.5):
        holes.append((a + 1.2, a + WC - 1.2, F0 + 1.5, F0 + 2.3))
    wall(walls, NORTH, -XE, XE, zb, zt + 0.25, holes, concrete, -T, 0.0)
    OUTER_HOLES[(fi, "N")] = list(holes)
    for (a, b, z0, z1) in holes:
        if z0 > F0:
            sashes(NORTH, a, b, z0, z1, mat=glass if z1 - z0 > 1 else frosted)
    # the ends: the stair halls' windows at each landing (a tall strip up the end wall)
    for F in (WEST, EAST):
        hs = [(0.85, 2.55, F0 + 1.8 + 0.4, F0 + 1.8 + 1.6)]
        wall(walls, F, -YS, YS, zb, zt + 0.25, hs, concrete, -T, 0.0)
        OUTER_HOLES[(fi, "W" if F is WEST else "E")] = list(hs)
        for (a, b, z0, z1) in hs:
            sashes(F, a, b, z0, z1, mat=glass, pane=0.85)
    # the floor beams as bands on the long faces (the slab's edge and the beam under it)
    if fi > 0:
        for F in (NORTH, SOUTH):
            fb(walls, F, -XE, XE, 0.0, 0.08, F0 - 0.85, F0 + 0.05, concrete)
    # the south balcony at this floor (from the 2nd up): slab, upstand, rail, drain spouts
    if fi > 0:
        fb(walls, SOUTH, -35.5, 35.5, 0.0, BAL_D, F0 - 0.25, F0 - 0.05, concrete)
        fb(walls, SOUTH, -35.5, 35.5, BAL_D - 0.14, BAL_D, F0 - 0.25, F0 + 0.75, concrete)
        for u in (-35.5, 35.5 - 0.14):
            fb(walls, SOUTH, u, u + 0.14, 0.0, BAL_D, F0 - 0.25, F0 + 0.75, concrete)
        fb(walls, SOUTH, -35.5, 35.5, BAL_D - 0.16, BAL_D + 0.02, F0 + 0.75, F0 + 0.8, concrete)   # its cap
        for k in range(36):
            u = -35.3 + k * 2.0
            fb(fit, SOUTH, u - 0.02, u + 0.02, BAL_D - 0.09, BAL_D - 0.05, F0 + 0.8, F0 + 1.15, steel)
        fb(fit, SOUTH, -35.5, 35.5, BAL_D - 0.1, BAL_D - 0.04, F0 + 1.12, F0 + 1.17, steel)        # the rail
        # the balcony's floor finish, and the weather streaks under its edge
        fb(floors, SOUTH, -35.36, 35.36, 0.0, BAL_D - 0.14, F0 - 0.05, F0 - 0.03, roof_m)
# the sunshade over the top floor's windows, the parapet and its aluminium coping
fb(walls, SOUTH, -35.5, 35.5, 0.0, 1.0, ROOF - 0.25, ROOF - 0.1, concrete)
for F, a, b in ((NORTH, -XE, XE), (SOUTH, -XE, XE), (WEST, -YS, YS), (EAST, -YS, YS)):
    fb(walls, F, a, b, -T, 0.0, ROOF, PAR, concrete)
    fb(walls, F, a - 0.04, b + 0.04, -T - 0.02, 0.04, PAR, PAR + 0.05, alu)
    # the plinth: a band of dark tiles, and the drain channel along the foot of the wall
    fb(walls, F, a, b, 0.0, 0.03, -0.3, FLOORS[0] + 0.25, tile_plinth)
for F in (NORTH, SOUTH):
    off = BAL_D if F is SOUTH else 0.0
    fb(fit, F, -XE, XE, 0.05, 0.5, -0.12, 0.0, plinth)                 # the channel's concrete
    fb(fit, F, -XE, XE, 0.12, 0.43, -0.004, 0.006, grate)              # its grating
# the columns, on both long faces, up to the parapet (the grid's column at x = 0 on the north face
# stood where the entrance's glass doors meet: there it starts above the canopy)
ENT_SPAN = (-2.4, 2.4)
for c in COLS:
    for F in (NORTH, SOUTH):
        z0 = 3.3 if F is NORTH and ENT_SPAN[0] - COL_W < c < ENT_SPAN[1] + COL_W else -0.3
        fb(walls, F, c - COL_W / 2, c + COL_W / 2, 0.0, COL_D, z0, PAR, concrete)
        fb(walls, F, c - COL_W / 2 - 0.02, c + COL_W / 2 + 0.02, -0.02, COL_D + 0.02, PAR, PAR + 0.05, alu)
# downpipes: from hoppers under the parapet, down the columns every 18 m (the south's past the
# balconies' edges), into the channel
for c in (-31.5, -13.5, 4.5, 22.5):
    for F, d in ((NORTH, COL_D + 0.08), (SOUTH, BAL_D + 0.1)):
        x = c + 0.5
        p = F(x, d)
        fit.cylinder((p[0], p[1], 0.0), 0.05, ROOF - 0.15, pipe, segments=12)
        fb(fit, F, x - 0.12, x + 0.12, d - 0.1, d + 0.1, ROOF - 0.35, ROOF - 0.1, pipe)              # the hopper
        for z in [1.2] + [F0 + 1.2 for F0 in FLOORS[1:]] + [ROOF - 1.0]:
            fb(fit, F, x - 0.02, x + 0.02, (0.0 if F is NORTH else BAL_D) , d, z, z + 0.04, pipe)  # brackets
        fb(fit, F, x - 0.08, x + 0.08, d - 0.08, d + 0.25, 0.0, 0.12, pipe)                     # the shoe

# ----------------------------------------------------------------------------- the entrance
EA = (-2.4, 2.4)
# the canopy: a deep slab with a fascia carrying the school's name, on two round tiled columns;
# two steps up to the doors, a ramp with a handrail beside them
CD = 3.6
fb(fit, NORTH, EA[0] - 1.6, EA[1] + 1.6, 0.0, CD, 3.1, 3.3, concrete)
fb(fit, NORTH, EA[0] - 1.6, EA[1] + 1.6, CD - 0.12, CD, 2.85, 3.4, concrete)                    # fascia
for u in (EA[0] - 1.2, EA[1] + 1.2):
    p = NORTH(u, CD - 0.5)
    fit.cylinder((p[0], p[1], 0.0), 0.2, 3.1, tile_plinth, segments=24)
for k, (d0, z) in enumerate(((0.0, DOMA), (0.35, DOMA / 2))):
    fb(floors, NORTH, EA[0] - 1.6, EA[1] + 1.6, d0, d0 + 0.35 if k else 0.35, 0.0, z, plinth)
fb(floors, NORTH, EA[0] - 1.6, EA[1] + 1.6, 0.0, CD, 0.0, 0.02, plinth)
fb(floors, NORTH, EA[0], EA[1], -T, 0.0, 0.0, DOMA, plinth)
# the ramp to the east of the steps
RX = EA[1] + 1.8
ramp_len = 3.6
fit.oriented_box(Vector((RX + ramp_len / 2, NORTH(0, 0.55)[1], 0.15)), (ramp_len, 1.1, 0.06), Matrix.Rotation(math.atan2(0.3, ramp_len), 3, "Y"), plinth)
for d in (0.05, 1.05):
    y = NORTH(0, d)[1]
    fit.oriented_box(Vector((RX + ramp_len / 2, y, 1.05)), (ramp_len, 0.04, 0.04), Matrix.Rotation(math.atan2(0.3, ramp_len), 3, "Y"), steel)
    for k in range(4):
        x = RX + 0.2 + k * (ramp_len - 0.4) / 3
        fit.box((x - 0.02, y - 0.02, 0.0), (x + 0.02, y + 0.02, 1.05 + 0.3 * (1 - (x - RX) / ramp_len)), steel)
# the doors: glass in aluminium, sliding apart
for (a, b, name) in ((EA[0], 0.0, "kzdoor_slide_n1200_entrance_l"), (0.0, EA[1], "kzdoor_slide_p1200_entrance_r")):
    leaf = door_leaf(name, NORTH((a + b) / 2, -0.12) + (DOMA,))
    fb(leaf, NORTH, a + 0.01, b - 0.01, -0.135, -0.105, DOMA + 0.01, 2.58, alu)
    fb(leaf, NORTH, a + 0.07, b - 0.07, -0.122, -0.118, DOMA + 0.1, 2.5, glass)
    fb(leaf, NORTH, (a + b) / 2 - 0.4, (a + b) / 2 + 0.4, -0.14, -0.1, 0.95, 1.0, alu)
fb(fit, NORTH, EA[0], EA[1], -0.2, 0.0, 2.58, 2.6, alu)
fb(fit, NORTH, EA[0] - 0.06, EA[1] + 0.06, -0.2, 0.02, 2.6, 2.66, alu)
# the name plate on the fascia, a lamp under the canopy
y = -YS - CD - 0.002
plate_w, plate_h = 4.6, 0.65
sign_faces.append(([(-plate_w / 2, y, 2.9), (plate_w / 2, y, 2.9), (plate_w / 2, y, 2.9 + plate_h), (-plate_w / 2, y, 2.9 + plate_h)],
                   [(0, 1 - 288 / 1024), (1, 1 - 288 / 1024), (1, 1), (0, 1)], name_mat))
for u in (-1.2, 1.2):
    p = NORTH(u, 1.6)
    fit.cylinder((p[0], p[1], 3.06), 0.18, 0.04, tube, segments=20)
lamp(0.0, -YS - 1.6, 3.1, intensity=0.02, radius=0.3, rng=8.0)

# ----------------------------------------------------------------------------- the clock
# on a raised panel of the south parapet, over the middle of the building, facing the ground;
# its hands at 8:30 (the morning meeting)
CLK_R = 0.75
fb(walls, SOUTH, -1.2, 1.2, -T, 0.05, PAR, PAR + 1.25, concrete)
fb(walls, SOUTH, -1.24, 1.24, -T - 0.02, 0.07, PAR + 1.25, PAR + 1.3, alu)
cz = PAR + 0.55
yf = YS + 0.075
ring = []
for k in range(48):
    a = 2 * math.pi * k / 48
    ring.append(((math.cos(a) * CLK_R, yf, cz + math.sin(a) * CLK_R), (0.5 + 0.5 * math.cos(a), 0.5 + 0.5 * math.sin(a))))
for k in range(48):
    (p0, t0), (p1, t1) = ring[k], ring[(k + 1) % 48]
    # (seen from the south, +x is on the left: wound to face +Y)
    sign_faces.append(([(0, yf, cz), p1, p0], [(0.5, 0.5), (1 - t1[0], t1[1]), (1 - t0[0], t0[1])], clock_mat))
for i in range(48):
    a0, a1 = 2 * math.pi * i / 48, 2 * math.pi * (i + 1) / 48
    q0 = Vector((math.cos(a0) * (CLK_R + 0.05), 0, math.sin(a0) * (CLK_R + 0.05)))
    q1 = Vector((math.cos(a1) * (CLK_R + 0.05), 0, math.sin(a1) * (CLK_R + 0.05)))
    m = (q0 + q1) / 2
    fit.oriented_box(Vector((m.x, yf + 0.02, cz + m.z)), ((q1 - q0).length + 0.01, 0.08, 0.06), Matrix.Rotation(-math.atan2(q1.z - q0.z, q1.x - q0.x), 3, "Y"), alu)
for (ang, length, width) in ((-math.radians(255), 0.42, 0.05), (-math.radians(180), 0.62, 0.035)):
    # hour hand at 8:30 (255 degrees from 12), minute hand at 6
    dirv = Vector((math.sin(ang), 0, math.cos(ang)))
    c = Vector((0, yf + 0.012, cz)) + dirv * (length / 2 - 0.08)
    fit.oriented_box(c, (width, 0.01, length), Matrix.Rotation(ang, 3, "Y"), clock_hand)

# ----------------------------------------------------------------------------- each floor
STAIR_HOLE = {"w": (STAIR_X[0], STAIR_X[1], -2.0, 2.7), "e": (-STAIR_X[1], -STAIR_X[0], -2.0, 2.7)}
for fi, F0 in enumerate(FLOORS):
    fc = F0 + CEIL
    # the slab under this floor (the ground floor's on the earth), with the stairwells open
    holes = [(-4.5, 4.5, Y0, -1.2)] if fi == 0 else [(h[0], h[1], h[2], h[3]) for h in STAIR_HOLE.values()]
    for (a, b, c, d) in free_rects(X0, X1, Y0, Y1, holes):
        floors.box((a, c, F0 - 0.25 if fi else 0.0), (b, d, F0 - 0.02), plinth)
    # finishes: the corridor's vinyl, classrooms' wood, the toilets' tiles, the stair halls'
    for (a, b, c, d) in free_rects(-35.5, 35.5, Y0, YC, []):
        floors.box((a, c, F0 - 0.02), (b, d, F0), corridor_floor)
    for k, room in enumerate(PLAN[fi]):
        if room is None:
            continue
        a, b = slot(k)
        if room[2] == 2:
            b = slot(k + 1)[1]
        mat = class_floor if room[1] in ("class", "room", "staff") else corridor_floor
        if room[1] == "entrance":
            # the entry's earth floor (doma) a step down at the doors, then the hall
            floors.box((a, -1.2, F0 - 0.02), (b, Y1, F0), corridor_floor)
            floors.box((a, Y0, 0.0), (b, -1.2, DOMA), doma_floor)
            continue
        floors.box((a, YC, F0 - 0.02), (b, Y1, F0), mat)
    for a in (-35.5, 31.5):
        floors.box((a, YC, F0 - 0.02), (a + WC, Y1, F0), wc_floor)
    for side, (h0, h1, _, _) in STAIR_HOLE.items():
        for (a, b, c, d) in free_rects(h0, h1, Y0, Y1, [] if fi == 0 else [STAIR_HOLE[side]]):
            floors.box((a, c, F0 - 0.02), (b, d, F0), corridor_floor)
    # the ceiling, its boards under the next slab (the stairwells open)
    hole_up = [(h[0], h[1], h[2], h[3]) for h in STAIR_HOLE.values()] if fi < 2 else [STAIR_HOLE["w"]]
    for (a, b, c, d) in free_rects(X0, X1, Y0, Y1, hole_up):
        inside.box((a, c, fc), (b, d, fc + 0.02), ceiling)
    # the corridor wall: classroom doors (front and back) and transom windows above it
    cw_holes = []
    for k, room in enumerate(PLAN[fi]):
        if room is None:
            continue
        a, b = slot(k)
        if room[2] == 2:
            b = slot(k + 1)[1]
        name, kind = room[0], room[1]
        if kind == "entrance":
            cw_holes.append((a, b, F0, fc))
            continue
        tag = name.replace(" ", "_").replace("-", "").replace("|", "_")
        doors = [(b - 1.6, b - 0.7, "n")]
        if kind in ("class", "staff", "room"):
            doors.append((a + 0.7, a + 1.6, "p"))
        if kind == "half":
            m = (a + b) / 2
            doors = [(m - 1.2, m - 0.3, "n"), (m + 0.3, m + 1.2, "p")]
        for (u0, u1, sgn) in doors:
            cw_holes.append((u0, u1, F0, F0 + 2.0))
            sliding(f"kzdoor_slide_{sgn}880_{tag}_{u0:.1f}".replace(".", "_").replace("-", "m"), YF(YC), u0, u1, F0, F0 + 1.98, -HT - 0.03)
            run = (u0 - 0.9, u1 + 0.03) if sgn == "n" else (u0 - 0.03, u1 + 0.9)
            door_frame(YF(YC), u0, u1, F0, F0 + 1.98, -HT, HT, -HT, rail=run)
        # transom windows over the wall between the doors
        cw_holes.append((a + 1.8, b - 1.8, F0 + 2.25, F0 + 2.8))
    # the toilets' doorways (their hinged doors are hung further down)
    for a in (-35.5, 31.5):
        cw_holes.append((a + 1.4, a + 2.3, F0, F0 + 2.0))
    wall(inside, YF(YC), -35.5, 35.5, F0, fc, cw_holes, inwall, -HT, HT)
    for (a, b, z0, z1) in cw_holes:
        if z0 > F0 + 1:
            sashes(YF(YC), a, b, z0, z1, d=0.0, pane=1.8)
    # the dado: the lower wall of the corridor painted, both sides
    for d in (-HT - 0.006, HT):
        for (a, b, za, zb2) in free_rects(-35.5, 35.5, F0, F0 + 0.9, [h for h in cw_holes if h[2] <= F0 + 0.01]):
            fb(inside, YF(YC), a, b, d, d + 0.006, za, zb2, dado)
    # cross walls: between the bays (where rooms differ), the toilets, the stair halls
    cuts = {-35.5: "full", -31.5: "room", 31.5: "room", 35.5: "full"}
    for k in range(1, 7):
        x = SLOT0 + BAY * k
        left, right = PLAN[fi][k - 1], PLAN[fi][k]
        if left and left[2] == 2:
            continue
        cuts[x] = "room"
    for k, room in enumerate(PLAN[fi]):
        if room and room[1] == "half":
            a, b = slot(k)
            cuts[(a + b) / 2] = "room"
    for x, kind in cuts.items():
        y0 = YC if kind == "room" else YC
        partition_mat = inwall
        wall(inside, XF(x), y0 - HT, Y1, F0, fc, [], partition_mat, -HT, HT)
    # the toilets' doors: hinged, hung on the toilet's face of the wall and swinging into the
    # toilet (p90 turns the leaf towards +y). (Hung on the corridor face they swung back through
    # the wall.)
    for a in (-35.5, 31.5):
        u0 = a + 1.4
        hinged(f"kzdoor_hinge_p90_wc_{fi}_{'w' if a < 0 else 'e'}", YF(YC), u0, u0 + 0.9, F0, F0 + 1.98, HT + 0.03)
        door_frame(YF(YC), u0, u0 + 0.9, F0, F0 + 1.98, -HT, HT, HT)
    # ---- the classrooms' built-in front and back: blackboard, platform, message board
    for k, room in enumerate(PLAN[fi]):
        if room is None or room[1] != "class":
            continue
        a, b = slot(k)
        yc = (YC + Y1) / 2
        # the front (east wall): the blackboard in a wooden frame with its chalk tray, the platform
        fb(inside, XF(b), yc - 1.9, yc + 1.9, -HT - 0.03, -HT, F0 + 0.85, F0 + 2.1, wood)
        fb(inside, XF(b), yc - 1.85, yc + 1.85, -HT - 0.035, -HT - 0.03, F0 + 0.9, F0 + 2.05, board)
        fb(inside, XF(b), yc - 1.85, yc + 1.85, -HT - 0.1, -HT - 0.03, F0 + 0.85, F0 + 0.88, wood)
        inside.box((b - HT - 1.0, yc - 2.3, F0), (b - HT, yc + 2.3, F0 + 0.2), wood)
        # the back (west wall): the message board
        fb(inside, XF(a), yc - 2.5, yc + 2.5, HT, HT + 0.02, F0 + 1.0, F0 + 2.2, cork)
        fb(inside, XF(a), yc - 2.55, yc + 2.55, HT, HT + 0.025, F0 + 0.97, F0 + 1.0, wood)
    # ---- lights: two rows of fittings in every room, a row down the corridor
    for k, room in enumerate(PLAN[fi]):
        if room is None:
            continue
        a, b = slot(k)
        if room[2] == 2:
            b = slot(k + 1)[1]
        for y in (YC + 2.6, Y1 - 2.6):
            light_row(a + 0.5, b - 0.5, y, fc, 3 * room[2])
        lamp((a + b) / 2, (YC + Y1) / 2, fc, intensity=0.06 * room[2], radius=1.2, rng=12.0)
    light_row(-35.5, 35.5, (Y0 + YC) / 2, fc, 16)
    for x in (-27.0, -9.0, 9.0, 27.0):
        lamp(x, (Y0 + YC) / 2, fc, intensity=0.03, radius=0.8, rng=10.0)
    for x in ((STAIR_X[0] + STAIR_X[1]) / 2, -(STAIR_X[0] + STAIR_X[1]) / 2):
        lamp(x, 0.0, fc, intensity=0.02, radius=0.4, rng=8.0)
    for a in (-35.5, 31.5):
        lamp(a + WC / 2, (YC + Y1) / 2, fc, intensity=0.012, radius=0.3, rng=6.0)

# ----------------------------------------------------------------------------- the stairs
RISERS = 22
R = STOREY / RISERS
TR = 0.27


def stair_hall(xw, xe, top_floor):
    """A switchback stair in the hall between xw and xe: the first flight up along +Y on the west
    side, a landing across the south, the second flight back along -Y on the east side; from each
    floor to the next (and from the top floor to the roof, if `top_floor` says so)."""
    mid = (xw + xe) / 2
    runs = FLOORS if top_floor else FLOORS[:-1]
    for F0 in runs:
        for k in range(1, 11):
            y0 = -2.0 + (k - 1) * TR
            inside.box((xw, y0, F0 - 0.25), (mid - 0.05, y0 + TR, F0 + k * R), stair_m)
            inside.box((xw, y0 + TR - 0.04, F0 + k * R - 0.01), (mid - 0.05, y0 + TR, F0 + k * R + 0.005), nosing)
        land = F0 + 11 * R
        inside.box((xw, 0.7, land - 0.25), (xe, 2.7, land), stair_m)
        for k in range(1, 11):
            y1 = 0.7 - (k - 1) * TR
            inside.box((mid + 0.05, y1 - TR, land - 0.3), (xe, y1, land + k * R), stair_m)
            inside.box((mid + 0.05, y1 - TR, land + k * R - 0.01), (xe, y1 - TR + 0.04, land + k * R + 0.005), nosing)
        # the wall between the flights, the handrails
        inside.box((mid - 0.05, -2.0, F0 - 0.25), (mid + 0.05, 0.7, land + 10 * R + 0.9), inwall)
        for (x, z0, z1, ya, yb) in ((xw + 0.05, F0 + R + 0.85, land + 0.85, -2.0, 0.7), (xe - 0.05, land + R + 0.85, land + 11 * R + 0.85, 0.7, -2.0)):
            n = 12
            for i in range(n):
                t0, t1 = i / n, (i + 1) / n
                p0 = Vector((x, ya + (yb - ya) * t0, z0 + (z1 - z0) * t0))
                p1 = Vector((x, ya + (yb - ya) * t1, z0 + (z1 - z0) * t1))
                c = (p0 + p1) / 2
                dv = p1 - p0
                inside.oriented_box(c, (0.04, dv.length, 0.04), Vector((0, 1, 0)).rotation_difference(dv.normalized()).to_matrix(), steel)
        # the guard rail round the stairwell's edge on the floor above
        top = F0 + STOREY
        inside.box((xw, -2.05, top), (mid, -1.98, top + 1.1), steel)


stair_hall(STAIR_X[0], STAIR_X[1], top_floor=True)
stair_hall(-STAIR_X[1], -STAIR_X[0], top_floor=False)
# the west stair's penthouse on the roof, its door out onto the roof
PH = (STAIR_X[0], STAIR_X[1] + 0.25, -YS + T, 3.0)
PZ = ROOF + 3.0
for F, a, b in ((YF(PH[2]), PH[0], PH[1]), (YF(PH[3]), PH[0], PH[1])):
    wall(walls, F, a, b, ROOF, PZ, [], render, -0.2, 0.0)
# (the door where the stair arrives, south of the stairwell's edge: not beside its top flight,
# whose steps and handrail ran across it)
RD = (-4.2, -3.3)
wall(walls, XF(PH[1]), PH[2], PH[3], ROOF, PZ, [(RD[0], RD[1], ROOF, ROOF + 2.0)], render, -0.2, 0.0)
walls.box((PH[0], PH[2], PZ), (PH[1], PH[3], PZ + 0.2), render)
hinged("kzdoor_hinge_n100_roof", XF(PH[1]), RD[0], RD[1], ROOF, ROOF + 1.98, 0.06, P("school_roof_door", (0.35, 0.38, 0.36), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.1))
lamp((PH[0] + PH[1]) / 2, 0.0, PZ - 0.1, intensity=0.015, radius=0.3, rng=6.0)

# ----------------------------------------------------------------------------- the roof
for (a, b, c, d) in free_rects(X0, X1, Y0, Y1, [STAIR_HOLE["w"]]):
    walls.box((a, c, ROOF - 0.25), (b, d, ROOF), roof_m)
inside.box((STAIR_HOLE["e"][0], STAIR_HOLE["e"][2], ROOF - 0.25), (STAIR_HOLE["e"][1], STAIR_HOLE["e"][3], ROOF), roof_m)
# the fence inside the parapet, 1.8 m above it: posts, rails, bars
FZ0, FZ1 = PAR, PAR + 1.8
rect = [(X0 + 0.15, Y0 + 0.15), (X1 - 0.15, Y0 + 0.15), (X1 - 0.15, Y1 - 0.15), (X0 + 0.15, Y1 - 0.15)]
for i in range(4):
    p0, p1 = Vector(rect[i] + (0,)), Vector(rect[(i + 1) % 4] + (0,))
    edge = p1 - p0
    n = max(1, int(edge.length / 2.0))
    for k in range(n + 1):
        p = p0 + edge * (k / n)
        fit.box((p.x - 0.03, p.y - 0.03, ROOF), (p.x + 0.03, p.y + 0.03, FZ1), fence)
    for z in (FZ0, (FZ0 + FZ1) / 2, FZ1 - 0.03):
        lo = Vector((min(p0.x, p1.x), min(p0.y, p1.y), z))
        hi = Vector((max(p0.x, p1.x), max(p0.y, p1.y), z + 0.03))
        fit.box((lo.x - 0.015, lo.y - 0.015, lo.z), (hi.x + 0.015, hi.y + 0.015, hi.z), fence)
    nb = int(edge.length / 0.15)
    for k in range(nb):
        p = p0 + edge * ((k + 0.5) / nb)
        fit.box((p.x - 0.006, p.y - 0.006, FZ0), (p.x + 0.006, p.y + 0.006, FZ1), fence)

# ----------------------------------------------------------------------------- the fit-out
import school_fitout  # noqa: E402

school_fitout.fitout(globals())

# the roof's equipment: the elevated water tank on the stair penthouse, the classrooms' air
# conditioners in a row along the north edge, a lightning rod, the roof drains
TX, TY = (PH[0] + PH[1]) / 2, (PH[2] + PH[3]) / 2 - 1.0
for sx in (-1, 1):
    for sy in (-1, 1):
        fit.box((TX + sx * 1.1 - 0.05, TY + sy * 1.1 - 0.05, PZ + 0.2), (TX + sx * 1.1 + 0.05, TY + sy * 1.1 + 0.05, PZ + 1.0), steel)
fit.box((TX - 1.25, TY - 1.25, PZ + 1.0), (TX + 1.25, TY + 1.25, PZ + 1.08), steel)
fit.box((TX - 1.2, TY - 1.2, PZ + 1.08), (TX + 1.2, TY + 1.2, PZ + 3.08), tank_m)
for k in range(1, 4):                                    # the panels' ribs
    u = -1.2 + k * 0.6
    fit.box((TX + u - 0.02, TY - 1.23, PZ + 1.08), (TX + u + 0.02, TY + 1.23, PZ + 3.08), tank_m)
    fit.box((TX - 1.23, TY + u - 0.02, PZ + 1.08), (TX + 1.23, TY + u + 0.02, PZ + 3.08), tank_m)
for z in (PZ + 1.58, PZ + 2.08, PZ + 2.58):
    fit.box((TX - 1.23, TY - 1.23, z - 0.02), (TX + 1.23, TY + 1.23, z + 0.02), tank_m)
fit.cylinder((TX + 0.6, TY + 0.6, PZ + 3.08), 0.25, 0.12, tank_m)                       # the hatch
fit.cylinder((PH[0] + 0.4, PH[2] + 0.4, PZ + 0.2), 0.015, 3.5, steel, segments=8)      # the lightning rod
for k in range(12):
    x = -30.0 + k * 5.4
    outdoor_unit(fit, (x, Y0 + 1.2, ROOF), (1, 0, 0), (0, 1, 0))
    fit.box((x - 0.05, Y0 + 0.3, ROOF + 0.1), (x + 0.05, Y0 + 0.85, ROOF + 0.2), pipe)   # its pipes to the wall
for c in (-31.5, -13.5, 4.5, 22.5):
    for y in (Y0 + 0.3, Y1 - 0.3):
        fit.cylinder((c + 0.5, y, ROOF), 0.12, 0.05, grate, segments=12)

# ----------------------------------------------------------------------------- finish and export
for b in (walls, inside, fit, floors):
    b.finish(collection="SCHOOL")
uv_object("signs", sign_faces)
for b, pivot, yaw in DOORS:
    ob = b.finish(collection="SCHOOL")
    ob.data.transform(Matrix.Rotation(math.radians(-yaw), 4, "Z") @ Matrix.Translation(-pivot))
    ob.location = pivot
    ob.rotation_euler.z = math.radians(yaw)

import json  # noqa: E402

os.makedirs(os.path.dirname(OUT), exist_ok=True)
with open(os.path.join(os.path.dirname(OUT), "lights.json"), "w") as f:
    json.dump(LIGHTS, f, indent=1)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT, len(LIGHTS), "lights", len(DOORS), "doors")
