"""Kazehana Junior High's gym (taiikukan, docs/school_plan.md): the shell and what's built into
it, as one quality-gated building.

    python tools/make_school_textures.py      (the arena floor's courts, first)
    blender --background --factory-startup --python tools/blender/build_school_gym.py

-> assets/buildings/school_gym/school_gym.gltf, lights.json

A 1980s school gym, 40 x 32 m:
- **Lower walls:** reinforced concrete to the gallery (4.5 m), with low vent windows along the
  floor and steel sliding doors out to the grounds.
- **Upper walls:** a steel frame clad in ribbed metal, with a long band of clerestory windows down
  both sides.
- **Roof:** a shallow barrel-vaulted metal roof on arched steel trusses.
- **Annex:** along the north side (the gate's side), with the entrance hall, the equipment store
  and the toilets.
- **Inside:** the arena's maple floor with the courts' lines (basketball, two volleyball, four
  badminton); a raised stage at the east end behind its proscenium, its curtains drawn back
  (still cloth, tier D: docs/cloth_plan.md); the gallery round three sides with its rail; two
  basketball backboards hung from the trusses; a wood wainscot; high-bay lights; the clock over
  the stage.

Local frame (Blender): origin at ground level in the middle of the footprint, +X east (towards the
stage), +Y south; the entrance faces -Y.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fixture_kit as fk  # noqa: E402
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ASSET_ID = "school_gym"
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")
TEX = os.path.join(ROOT, "assets", "cache", "school")
SIGNS = os.path.join(ROOT, "assets", "cache", "signs")

XE, YN, YS = 20.0, -16.0, 16.0       # the footprint: x -20..20, y -16..16
YA = -12.0                           # the arena's north wall (the annex in front of it)
T = 0.25
FL = 0.5                             # the arena floor
GAL = 4.5                            # the gallery's floor, the concrete's top
EAVE = 9.5                           # the eaves (the trusses' feet)
RISE = 3.0                           # the barrel roof's rise over the arena's width
STAGE_X = 13.0                       # the proscenium's line
STAGE_H = FL + 1.0
ANNEX_TOP = 3.9

kz.reset_scene()
bpy.context.scene.collection.children.link(bpy.data.collections.new("GYM"))

# ----------------------------------------------------------------------------- materials
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
concrete = kz.surface_material("school_concrete", "Precast_Concrete_Wall", tint=(0.92, 0.88, 0.78), desaturate=0.55)
cladding = None   # (the Fab scan, set once texture_material exists: below)
roof_m = P("gym_roof", (0.28, 0.36, 0.42), DETAIL, roughness=0.45, metallic=0.6, normal_strength=0.2, grain=0.15)
inwall = P("gym_inner_wall", (0.82, 0.81, 0.76), DETAIL, roughness=0.85, normal_strength=0.12, grain=0.12)
panel = P("gym_wainscot", (0.66, 0.52, 0.35), "Wood_Planks", roughness=0.5, normal_strength=0.35)
stage_wood = P("gym_stage_wood", (0.50, 0.36, 0.22), "Wood_Planks", roughness=0.45, normal_strength=0.4)
steel = P("gym_steel", (0.55, 0.58, 0.57), DETAIL, roughness=0.45, metallic=0.7, normal_strength=0.05, grain=0.05)
rail = P("gym_rail", (0.30, 0.42, 0.33), DETAIL, roughness=0.5, metallic=0.4, normal_strength=0.05)
alu = P("gym_aluminium", (0.62, 0.62, 0.60), DETAIL, roughness=0.3, metallic=0.85, normal_strength=0.05, grain=0.05)
# (painted steel: a matte eggshell paint, not bare metal; the frame a shade darker)
door_m = P("gym_steel_door", (0.52, 0.58, 0.55), DETAIL, roughness=0.6, metallic=0.0, normal_strength=0.15, grain=0.06)
door_frame = P("gym_door_frame", (0.36, 0.42, 0.40), DETAIL, roughness=0.55, metallic=0.0, normal_strength=0.12, grain=0.05)
pull_m = P("gym_door_pull", (0.08, 0.08, 0.08), DETAIL, roughness=0.5, metallic=0.3, normal_strength=0.05)
plinth = P("gym_plinth", (0.42, 0.41, 0.39), DETAIL, roughness=0.85, normal_strength=0.8)
annex_floor = P("gym_annex_floor", (0.52, 0.56, 0.50), "Concrete_Floor-f4b85cb0", roughness=0.45, normal_strength=0.15, grain=0.2)
velvet = P("gym_stage_curtain", (0.36, 0.05, 0.07), DETAIL, roughness=0.95, normal_strength=0.3, grain=0.25)
velvet.use_backface_culling = False
board_m = P("gym_backboard", (0.88, 0.90, 0.92), DETAIL, roughness=0.1, normal_strength=0.02, grain=0.02)
orange = P("gym_hoop", (0.85, 0.32, 0.06), DETAIL, roughness=0.4, metallic=0.4, normal_strength=0.05)
dark = P("gym_dark", (0.05, 0.05, 0.05), DETAIL, roughness=0.6, normal_strength=0.1)
glass = kz.plain_material("kz_glow_gym_clear_glass", (0.88, 0.93, 0.92), roughness=0.02)
wired = kz.plain_material("gym_wired_clear_glass", (0.80, 0.86, 0.85), roughness=0.05)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)
lampm = kz.plain_material("kz_neon_gym_light", (0.95, 0.95, 0.92), roughness=0.3, emission=(1.0, 0.97, 0.9), emission_strength=1.0)


def texture_material(name, stem, normal_strength=1.0):
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
    nm.inputs["Strength"].default_value = normal_strength
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(os.path.join(TEX, stem + "_orm.png"), "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    return m


# Fab (Quixel Megascans) scans, prepared by tools/prep_fab_textures.py: the floor's boards, the
# walls' plywood, the cladding's corrugated metal (2 m a texture)
court = texture_material("gym_arena_floor", "fab_gym_floor", normal_strength=0.8)
plywood = texture_material("gym_plywood", "fab_plywood", normal_strength=0.35)
groove_m = P("gym_plywood_joint", (0.30, 0.22, 0.14), "Wood_Planks", roughness=0.8, normal_strength=0.2)
cladding = texture_material("gym_cladding", "fab_corrugated", normal_strength=1.0)
line_mats = {c: P(f"gym_court_line_{c}", rgb, DETAIL, roughness=0.15, normal_strength=0.02, grain=0.02) for c, rgb in (("white", (0.92, 0.92, 0.9)), ("yellow", (0.85, 0.65, 0.06)), ("green", (0.06, 0.42, 0.18)))}
ceiling_wood = texture_material("gym_ceiling_boards", "gym_ceiling", normal_strength=0.6)
truss_m = P("gym_truss", (0.86, 0.85, 0.80), DETAIL, roughness=0.45, metallic=0.4, normal_strength=0.05, grain=0.05)
blackout = P("gym_blackout_curtain", (0.06, 0.08, 0.18), DETAIL, roughness=0.95, normal_strength=0.3, grain=0.2)
blackout.use_backface_culling = False
silver = P("gym_stage_curtain_grey", (0.55, 0.55, 0.56), DETAIL, roughness=0.8, metallic=0.2, normal_strength=0.3, grain=0.2)
silver.use_backface_culling = False
stair_steel = P("gym_stair_steel", (0.24, 0.30, 0.28), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.1)
_, _n, _orm = kz.fab_textures(DETAIL)
clock_mat = bpy.data.materials.new("gym_clock_face")
clock_mat.use_nodes = True
_t = clock_mat.node_tree.nodes.new("ShaderNodeTexImage")
_t.image = kz._image(os.path.join(SIGNS, "school_clock.png"), "sRGB")
clock_mat.node_tree.links.new(_t.outputs["Color"], clock_mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"])
_tn = clock_mat.node_tree.nodes.new("ShaderNodeTexImage")
_tn.image = kz._image(_n, "Non-Color")
_nm = clock_mat.node_tree.nodes.new("ShaderNodeNormalMap")
_nm.inputs["Strength"].default_value = 0.02
clock_mat.node_tree.links.new(_tn.outputs["Color"], _nm.inputs["Color"])
clock_mat.node_tree.links.new(_nm.outputs["Normal"], clock_mat.node_tree.nodes["Principled BSDF"].inputs["Normal"])

walls = kz.MeshBuilder("walls", uv_scale=0.5)
ply = kz.MeshBuilder("plywood", uv_scale=0.5)               # (the texture: 2 m of panels)
cladb = kz.MeshBuilder("cladding", uv_scale=0.5)            # (its UVs turned at the finish: ribs upright)
lines = kz.MeshBuilder("court_lines", uv_scale=1.0)
ceil = kz.MeshBuilder("ceiling", uv_scale=0.5)             # (2 m of boards)
inside = kz.MeshBuilder("interior", uv_scale=1.0)
fit = kz.MeshBuilder("fittings", uv_scale=1.0)
roofb = kz.MeshBuilder("roof", uv_scale=0.5)
DOORS, LIGHTS, UV_FACES = [], [], []

# ----------------------------------------------------------------------------- helpers
NORTH = lambda u, d: (u, YN - d)
SOUTH = lambda u, d: (u, YS + d)
WEST = lambda u, d: (-XE - d, u)
EAST = lambda u, d: (XE + d, u)
YF = lambda y: (lambda u, d: (u, y + d))
XF = lambda x: (lambda u, d: (x + d, u))


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
    n = max(1, round((u1 - u0) / pane))
    fb(fit, F, u0, u1, d - 0.004, d + 0.004, z0, z1, mat)
    for k in range(n + 1):
        u = u0 + (u1 - u0) * k / n
        fb(fit, F, u - 0.03, u + 0.03, d - 0.04, d + 0.04, z0, z1, alu)
    for z in (z0, z1 - 0.05):
        fb(fit, F, u0, u1, d - 0.04, d + 0.04, z, z + 0.05, alu)


def door_leaf(name, pivot):
    b = kz.MeshBuilder(name, uv_scale=1.0)
    DOORS.append((b, Vector(pivot)))
    return b


def steel_doors(F, u0, u1, z0, z1, d, tag, wall=None):
    """A pair of painted steel sliding doors hung on one face of the wall (their middle at depth
    d), in a pressed-steel frame round the opening (`wall`: the wall's two faces, which the
    frame's jambs and head line). The leaves lap the frame on their outer edges and top, and
    each other at the meeting stiles (no light round them); each has the same wired-glass vision
    panel at the same height, an aluminium kick plate and a recessed pull on both faces."""
    m = (u0 + u1) / 2
    out = 1.0 if d >= 0 else -1.0
    if wall is not None:
        w0, w1 = wall
        for (a, b) in ((u0, u0 + 0.05), (u1 - 0.05, u1)):            # the jambs, lining the opening
            fb(fit, F, a, b, w0 - 0.025, w1 + 0.025, z0, z1, door_frame)
        fb(fit, F, u0, u1, w0 - 0.025, w1 + 0.025, z1 - 0.05, z1, door_frame)  # the head
    for (a, b, sgn, k) in ((u0 - 0.04, m + 0.012, "n", 0), (m - 0.012, u1 + 0.04, "p", 1)):
        dd = d + out * 0.012 * k                     # (the second leaf a little proud: they lap)
        leaf = door_leaf(f"kzdoor_slide_{sgn}{int((b - a) * 1000) - 20}_{tag}_{sgn}", F((a + b) / 2, dd) + (z0,))
        fb(leaf, F, a, b, dd - 0.02, dd + 0.02, z0 + 0.008, z1 + 0.04, door_m)
        c = (a + b) / 2
        fb(leaf, F, c - 0.2, c + 0.2, dd - 0.022, dd + 0.022, z0 + 1.35, z0 + 1.85, wired)  # vision panel
        fb(leaf, F, c - 0.22, c + 0.22, dd - 0.023, dd + 0.023, z0 + 1.33, z0 + 1.35, door_frame)
        fb(leaf, F, c - 0.22, c + 0.22, dd - 0.023, dd + 0.023, z0 + 1.85, z0 + 1.87, door_frame)
        fb(leaf, F, a + 0.02, b - 0.02, dd - 0.022, dd + 0.022, z0 + 0.02, z0 + 0.27, alu)   # kick plate
        pu = b - 0.1 if sgn == "n" else a + 0.1                                               # the pulls
        for s in (-1, 1):
            fb(leaf, F, pu - 0.018, pu + 0.018, dd + s * 0.02, dd + s * 0.023, z0 + 0.85, z0 + 1.2, pull_m)
    fb(fit, F, u0 - 0.3, u1 + 0.3, d + out * 0.035, d + out * 0.12, z1 + 0.05, z1 + 0.2, door_frame)  # the rail's hood


def arch_z(y):
    """The roof's underside over the arena at y (the barrel: a circular arc, its chord across
    the arena from y = YA to YS at EAVE, rising RISE at the middle)."""
    half = (YS - YA) / 2
    yc = (YA + YS) / 2
    R = (half ** 2 + RISE ** 2) / (2 * RISE)
    return EAVE + RISE - R + math.sqrt(max(0.0, R * R - (y - yc) ** 2))


def lamp(x, y, z, intensity=0.09, radius=0.5, rng=26.0):
    LIGHTS.append({"at": [round(x, 3), round(y, 3), round(z - 0.3, 3)], "colour": [1.0, 0.96, 0.9], "intensity": intensity, "radius": radius, "range": rng, "cone_deg": None})


# ----------------------------------------------------------------------------- the lower walls
# the arena's walls in concrete to the gallery: low vent windows along the floor, steel sliding
# doors out to the grounds (two on the south, one at the west end), the doors from the annex
VENTS = lambda u0, u1: [(u, u + 1.2, FL + 0.1, FL + 0.55) for u in [u0 + 2.0 + 4.0 * k for k in range(int((u1 - u0 - 3) / 4.0))]]
south_doors = [(-12.0, -9.6), (2.0, 4.4)]
GDOOR = (10.2, 11.4)                # the gallery's door out to the steel stair (south side)
holes = VENTS(-XE, STAGE_X - 1) + [(a, b, FL, FL + 2.4) for a, b in south_doors] + [(GDOOR[0], GDOOR[1], GAL, GAL + 2.1)]
holes = [h for h in holes if not any(h[0] < b and h[1] > a for a, b in south_doors) or h[3] > FL + 1]
wall(walls, SOUTH, -XE, XE, 0.0, GAL + 0.4, holes, concrete, -T, 0.0)
S_HOLES = list(holes)
for (a, b, z0, z1) in holes:
    if z1 - z0 < 1:
        sashes(SOUTH, a, b, z0, z1, mat=frosted, pane=0.6)
for k, (a, b) in enumerate(south_doors):
    steel_doors(SOUTH, a, b, FL, FL + 2.4, 0.06, f"south{k}", wall=(-T, 0.0))
west_holes = VENTS(YA, YS) + [(1.0, 3.4, FL, FL + 2.4)]
west_holes = [h for h in west_holes if not (h[0] < 3.4 and h[1] > 1.0) or h[3] > FL + 1]
wall(walls, WEST, YA, YS, 0.0, GAL + 0.4, west_holes, concrete, -T, 0.0)
W_HOLES = list(west_holes)
for (a, b, z0, z1) in west_holes:
    if z1 - z0 < 1:
        sashes(WEST, a, b, z0, z1, mat=frosted, pane=0.6)
steel_doors(WEST, 1.0, 3.4, FL, FL + 2.4, 0.06, "west", wall=(-T, 0.0))
wall(walls, EAST, YA, YS, 0.0, GAL + 0.4, [], concrete, -T, 0.0)
# the arena's north wall over the annex: doors through from the entrance hall
NDOORS = [(-1.8, 1.8)]
wall(walls, YF(YA), -XE, XE, 0.0, GAL + 0.4, [(a, b, FL, FL + 2.4) for a, b in NDOORS], concrete, -T / 2, T / 2)
for (a, b) in NDOORS:
    steel_doors(YF(YA), a, b, FL, FL + 2.4, -(T / 2 + 0.07), "arena", wall=(-T / 2, T / 2))   # (hung on the hall side: the arena side has its columns)
# a plinth band and the drain channel along the foot
for F, a, b in ((SOUTH, -XE, XE), (WEST, YN, YS), (EAST, YN, YS), (NORTH, -XE, XE)):
    fb(walls, F, a, b, 0.0, 0.03, -0.3, FL + 0.05, plinth)

# ----------------------------------------------------------------------------- the annex
# the entrance hall (glass doors under a canopy), the equipment store to the west, the toilets
# to the east; a flat roof
wall(walls, NORTH, -XE, XE, 0.0, ANNEX_TOP, [(-2.0, 2.0, 0.15, 2.6), (-14.0, -10.0, 1.6, 2.4), (9.0, 11.0, 1.8, 2.4), (13.0, 15.0, 1.8, 2.4)], concrete, -T, 0.0)
for (a, b, z0, z1) in [(-14.0, -10.0, 1.6, 2.4), (9.0, 11.0, 1.8, 2.4), (13.0, 15.0, 1.8, 2.4)]:
    sashes(NORTH, a, b, z0, z1, mat=frosted, pane=1.0)
for F in (WEST, EAST):
    wall(walls, F, YN, YA, 0.0, ANNEX_TOP, [], concrete, -T, 0.0)
walls.box((-XE, YN, ANNEX_TOP), (XE, YA, ANNEX_TOP + 0.25), concrete)
fb(walls, NORTH, -XE - 0.05, XE + 0.05, -0.05, 0.25, ANNEX_TOP + 0.1, ANNEX_TOP + 0.45, concrete)   # its fascia
for (a, b, name) in ((-2.0, 0.0, "kzdoor_slide_n980_gym_entrance_l"), (0.0, 2.0, "kzdoor_slide_p980_gym_entrance_r")):
    leaf = door_leaf(name, NORTH((a + b) / 2, -0.12) + (0.15,))
    fb(leaf, NORTH, a + 0.01, b - 0.01, -0.135, -0.105, 0.0, 2.45, alu)
    fb(leaf, NORTH, a + 0.07, b - 0.07, -0.122, -0.118, 0.1, 2.38, glass)
fb(fit, NORTH, -3.4, 3.4, 0.0, 2.6, 2.95, 3.15, concrete)                          # the canopy
for u in (-3.1, 3.1):
    p = NORTH(u, 2.3)
    fit.cylinder((p[0], p[1], 0.0), 0.15, 2.95, concrete, segments=20)
fit.box((-3.4, YN - 2.6, 0.0), (3.4, YN, 0.15), plinth)
inside.box((-XE + T, YN + T, 0.0), (XE - T, YA - T / 2, 0.15), annex_floor)
for x in (-6.0, 6.0):
    wall(inside, XF(x), YN + T, YA - T / 2, 0.15, ANNEX_TOP, [(YA - 1.2, YA - 0.4, 0.15, 2.2)], inwall, -0.06, 0.06)
inside.box((-XE + T, YN + T, ANNEX_TOP - 0.02), (XE - T, YA - T / 2, ANNEX_TOP), inwall)
for x in (-13.0, 0.0, 13.0):
    lamp(x, (YN + YA) / 2, ANNEX_TOP, intensity=0.015, radius=0.3, rng=8.0)
    inside.cylinder((x, (YN + YA) / 2, ANNEX_TOP - 0.06), 0.25, 0.06, lampm, segments=20)

# ----------------------------------------------------------------------------- the upper walls
# a steel frame clad in ribbed metal from the gallery to the eaves; the clerestory windows down
# both long sides; the end walls up under the barrel's curve
UP0 = GAL + 0.4
UP_HOLES = {}
CLER = (5.6, 8.6)
for F, y in ((YF(YA), YA), (SOUTH, YS)):
    d0, d1 = (-T / 2, T / 2) if y == YA else (-T, 0.0)
    holes = [(u, u + 3.6, CLER[0], CLER[1]) for u in [-19.0 + 4.0 * k for k in range(10)] if not (y == YS and u < GDOOR[1] < u + 3.6)]
    if y == YS:
        holes.append((GDOOR[0], GDOOR[1], UP0 - 0.4, GAL + 2.1))
    UP_HOLES[y] = list(holes)
    wall(cladb, F, -XE, XE, UP0, EAVE, holes, cladding, d0, d1)
    for (a, b, z0, z1) in holes:
        if z1 - z0 < 3.2 and z0 > UP0:
            sashes(F, a, b, z0, z1, d=(d0 + d1) / 2, mat=glass, pane=1.2)
# the end walls (east and west): straight to the eaves, then strips up under the arch
for F, x in ((WEST, -XE), (EAST, XE)):
    wall(cladb, F, YA, YS, UP0, EAVE, [(-6.0 + 3.0 * k, -4.0 + 3.0 * k, CLER[0], CLER[1]) for k in range(5)] if x < 0 else [], cladding, -T, 0.0)
    if x < 0:
        for k in range(5):
            sashes(F, -6.0 + 3.0 * k, -4.0 + 3.0 * k, CLER[0], CLER[1], pane=1.0)
    n = 28
    for k in range(n):
        y0 = YA + (YS - YA) * k / n
        y1 = YA + (YS - YA) * (k + 1) / n
        fb(cladb, F, y0, y1, -T, 0.0, EAVE, arch_z((y0 + y1) / 2) + 0.2, cladding)
# the concrete's cap where the cladding starts (a drip line)
for F, a, b in ((SOUTH, -XE, XE), (WEST, YA, YS), (EAST, YA, YS)):
    fb(walls, F, a, b, -T, 0.06, GAL + 0.4, GAL + 0.5, concrete)

# ----------------------------------------------------------------------------- the roof
# the barrel vault: panels across the arena (along y), overhanging the eaves and the ends; its
# standing seams; gutters along the eaves; downpipes
OV = 0.6
NS = 28
for k in range(NS):
    y0 = YA - OV + (YS - YA + 2 * OV) * k / NS
    y1 = YA - OV + (YS - YA + 2 * OV) * (k + 1) / NS
    z0, z1 = arch_z(max(YA, min(YS, y0))) + 0.15, arch_z(max(YA, min(YS, y1))) + 0.15
    if y0 < YA:
        z0 = arch_z(YA) + 0.15 - (YA - y0) * 0.2
    if y1 > YS:
        z1 = arch_z(YS) + 0.15 - (y1 - YS) * 0.2
    c = Vector((0, (y0 + y1) / 2, (z0 + z1) / 2))
    ang = math.atan2(z1 - z0, y1 - y0)
    L = math.hypot(y1 - y0, z1 - z0) + 0.01
    roofb.oriented_box(c, (2 * XE + 2 * OV, L, 0.12), Matrix.Rotation(ang, 3, "X"), roof_m)
    for j in range(int((2 * XE + 2 * OV) / 0.5) + 1):        # standing seams every 50 cm
        x = -XE - OV + 0.25 + j * 0.5
        roofb.oriented_box(c + Matrix.Rotation(ang, 3, "X") @ Vector((x, 0, 0.08)), (0.025, L, 0.05), Matrix.Rotation(ang, 3, "X"), roof_m)
for y in (YA - OV, YS + OV):
    zg = (arch_z(YA) if y < 0 else arch_z(YS)) + 0.15 - OV * 0.2 - 0.12
    fk.tube(fit, [(-XE - OV, y, zg), (XE + OV, y, zg)], 0.09, alu, seg=12)                         # the gutter
    for x in (-XE + 0.4, XE - 0.4):
        fk.tube(fit, [(x, y, zg), (x, y, 0.2)], 0.05, alu, seg=10)                                  # downpipes

# ----------------------------------------------------------------------------- inside
# the arena's floor (the courts' texture), the stage, the wainscot, the gallery and its rail,
# the trusses, the backboards, the lights, the clock
inside.box((-XE + T, YA + T / 2, 0.0), (XE - T, YS - T, FL - 0.002), plinth)
AX0, AX1, AY0, AY1 = -XE + T, STAGE_X, YA, YS - T
UV_FACES.append(("floor", (AX0, AX1, AY0, AY1)))
# the walls' inner faces: birch plywood panels from the floor to the eaves round the doors,
# vents and windows; a hardwood rail at 2 m (where balls hit); wood-clad columns between the
# window bays
for (F, a, bb, d0, d1, hl) in ((SOUTH, -XE + T, STAGE_X, -T - 0.02, -T, S_HOLES), (WEST, YA + T / 2, YS - T, -T - 0.02, -T, W_HOLES)):
    wall(ply, F, a, bb, FL, GAL, hl, plywood, d0, d1)
    wall(inside, F, a, bb, FL + 1.98, FL + 2.06, hl, stage_wood, d0 - 0.025, d0)
wall(ply, YF(YA), -XE + T, STAGE_X, FL, GAL, [(a, b, FL, FL + 2.4) for a, b in NDOORS], plywood, T / 2, T / 2 + 0.02)
wall(inside, YF(YA), -XE + T, STAGE_X, FL + 1.98, FL + 2.06, [(a, b, FL, FL + 2.4) for a, b in NDOORS], stage_wood, T / 2 + 0.02, T / 2 + 0.045)
wall(ply, SOUTH, -XE + T, STAGE_X, GAL, EAVE, UP_HOLES[YS], plywood, -T - 0.02, -T)
wall(ply, YF(YA), -XE + T, STAGE_X, GAL, EAVE, UP_HOLES[YA], plywood, T / 2, T / 2 + 0.02)
wall(ply, WEST, YA + T / 2, YS - T, GAL, EAVE, [(-6.0 + 3.0 * k, -4.0 + 3.0 * k, CLER[0], CLER[1]) for k in range(5)], plywood, -T - 0.02, -T)
for k in range(28):                                    # the west gable's inside, up under the arch
    y0 = YA + (YS - YA) * k / 28
    y1 = YA + (YS - YA) * (k + 1) / 28
    fb(ply, WEST, y0, y1, -T - 0.02, -T, EAVE, arch_z((y0 + y1) / 2), plywood)
# the sheets' joints: shallow grooves every 0.91 m across and 1.82 m up, round the openings
def joints(F, u0, u1, z0, z1, hl, d_face, out):
    u = u0 + 0.91
    while u < u1 - 0.05:
        for (a, bb, za, zb) in free_rects(u - 0.004, u + 0.004, z0, z1, hl):
            fb(ply, F, a, bb, d_face, d_face + out * 0.004, za, zb, groove_m)
        u += 0.91
    z = z0 + 1.82
    while z < z1 - 0.05:
        for (a, bb, za, zb) in free_rects(u0, u1, z - 0.004, z + 0.004, hl):
            fb(ply, F, a, bb, d_face, d_face + out * 0.004, za, zb, groove_m)
        z += 1.82


joints(SOUTH, -XE + T, STAGE_X, FL, EAVE, S_HOLES + UP_HOLES[YS], -T - 0.02, -1)
joints(YF(YA), -XE + T, STAGE_X, FL, EAVE, [(a, b, FL, FL + 2.4) for a, b in NDOORS] + UP_HOLES[YA], T / 2 + 0.02, 1)
joints(WEST, YA + T / 2, YS - T, FL, EAVE, W_HOLES + [(-6.0 + 3.0 * k, -4.0 + 3.0 * k, CLER[0], CLER[1]) for k in range(5)], -T - 0.02, -1)
def wall_columns(doors):
    """The columns on a long wall: one every 4 m, but never in a doorway: a column that would
    stand in one becomes a pair either side of the door, framing it (tools: town audit)."""
    xs = []
    for x in [-19.2 + 4.0 * k for k in range(9)]:
        hit = [(a, b) for a, b in doors if a - 0.16 < x < b + 0.16]
        xs += [x] if not hit else [hit[0][0] - 0.17, hit[0][1] + 0.17]
    return sorted(set(round(x, 3) for x in xs))


for (y0, y1, doors) in ((YS - T - 0.14, YS - T, south_doors), (YA + T / 2, YA + T / 2 + 0.14, NDOORS)):
    for x in wall_columns(doors):
        inside.box((x - 0.16, y0, FL), (x + 0.16, y1, EAVE), stage_wood)
# the stage: its floor, its front, steps up at both sides, the proscenium and its curtains
SY0, SY1 = -8.0, 8.0
inside.box((STAGE_X, YA + T / 2, 0.0), (XE - T, YS - T, STAGE_H), stage_wood)
for k in range(8):                                        # the under-stage stores' doors
    y = SY0 + 0.1 + (k + 0.5) * (SY1 - SY0 - 0.2) / 8
    w = (SY1 - SY0 - 0.2) / 8 / 2 - 0.02
    if abs(abs(y) - 3.9) < 0.7 + w:                       # (none behind the steps up)
        continue
    inside.box((STAGE_X - 0.012, y - w, FL + 0.06), (STAGE_X + 0.001, y + w, STAGE_H - 0.06), panel)
    fk.tube(fit, [(STAGE_X - 0.012, y - 0.15, STAGE_H - 0.2), (STAGE_X - 0.05, y - 0.15, STAGE_H - 0.2), (STAGE_X - 0.05, y + 0.15, STAGE_H - 0.2), (STAGE_X - 0.012, y + 0.15, STAGE_H - 0.2)], 0.008, alu, seg=6, caps=False)
# (inside the proscenium's opening, |y| < 7, clear of the side drapes: steps that lead onto the
# stage, not into its wall; 0.25 m risers, within a person's step)
for sy in (-1, 1):
    y = sy * 3.9
    for k in range(4):
        inside.box((STAGE_X - 1.2 + k * 0.3, y - 0.7, 0.0), (STAGE_X, y + 0.7, FL + (k + 1) * 0.25), stage_wood)
PRO = (-7.0, 7.0, STAGE_H, STAGE_H + 5.0)
wall(inside, XF(STAGE_X), YA + T / 2, YS - T, STAGE_H, EAVE, [PRO], inwall, -0.15, 0.15)
wall(ply, XF(STAGE_X), YA + T / 2, YS - T, STAGE_H, EAVE, [PRO], plywood, -0.17, -0.15)       # (its face to the hall: panelled)
joints(XF(STAGE_X), YA + T / 2, YS - T, STAGE_H, EAVE, [PRO], -0.17, -1)
for k in range(28):                                    # up under the arch
    y0 = YA + (YS - YA) * k / 28
    y1 = YA + (YS - YA) * (k + 1) / 28
    fb(inside, XF(STAGE_X), y0, y1, -0.15, 0.15, EAVE, arch_z((y0 + y1) / 2), inwall)
    fb(ply, XF(STAGE_X), y0, y1, -0.17, -0.15, EAVE, arch_z((y0 + y1) / 2), plywood)
fb(inside, XF(STAGE_X), PRO[0] - 0.15, PRO[1] + 0.15, -0.2, -0.15, PRO[3], PRO[3] + 0.4, stage_wood)   # the head's trim
for sy in (-1, 1):
    fb(inside, XF(STAGE_X), (PRO[0] if sy < 0 else PRO[1]) - 0.15, (PRO[0] if sy < 0 else PRO[1]) + 0.15, -0.2, 0.15, STAGE_H, PRO[3], stage_wood)


def drape(y0, y1, z0, z1, x, folds, gather=1.0):
    """A heavy curtain hanging in folds (still: tier D), from z1 down to z0 between y0 and y1."""
    bm = fit.bm
    n = max(8, int(abs(y1 - y0) / 0.08))
    rows = 6
    verts = []
    for j in range(rows + 1):
        z = z1 - (z1 - z0) * j / rows
        row = []
        for i in range(n + 1):
            t = i / n
            y = y0 + (y1 - y0) * t
            depth = 0.12 * (0.6 + 0.4 * j / rows) * gather
            row.append(bm.verts.new((x + depth * math.sin(2 * math.pi * t * folds), y, z)))
        verts.append(row)
    faces = []
    for j in range(rows):
        for i in range(n):
            q = (verts[j][i], verts[j][i + 1], verts[j + 1][i + 1], verts[j + 1][i])
            faces.append(bm.faces.new(q))   # (one sheet: the material is double-sided)
    fk._mat(fit, faces, velvet)


def drape_along(p0, p1, z0, z1, out, folds, mat, depth=0.12):
    """A curtain hanging in folds from z1 to z0 along the line p0..p1 (plan points), its folds
    standing out towards `out` (a horizontal unit vector)."""
    p0, p1, out = Vector((p0[0], p0[1], 0.0)), Vector((p1[0], p1[1], 0.0)), Vector((out[0], out[1], 0.0))
    bm = fit.bm
    n = max(8, int((p1 - p0).length / 0.08))
    rows = 6
    grid = []
    for j in range(rows + 1):
        z = z1 - (z1 - z0) * j / rows
        row = []
        for i in range(n + 1):
            t = i / n
            q = p0.lerp(p1, t) + out * (depth * (0.5 + 0.5 * math.sin(2 * math.pi * t * folds)) * (0.7 + 0.3 * j / rows))
            row.append(bm.verts.new((q.x, q.y, z)))
        grid.append(row)
    faces = [bm.faces.new((grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i])) for j in range(rows) for i in range(n)]
    fk._mat(fit, faces, mat)


drape(PRO[0], PRO[0] + 2.2, STAGE_H, PRO[3], STAGE_X + 0.35, 9)
drape(PRO[1] - 2.2, PRO[1], STAGE_H, PRO[3], STAGE_X + 0.35, 9)
drape(PRO[0], PRO[1], PRO[3] - 0.9, PRO[3], STAGE_X + 0.25, 40, gather=0.6)                # the valance
# the grey curtain across the back of the stage, the flags in front of it (the Hinomaru, the
# school's flag), on their poles
drape_along((XE - 1.3, -7.5), (XE - 1.3, 7.5), STAGE_H, PRO[3] + 0.6, (-1, 0, 0), 30, silver, depth=0.18)
UV_FACES.append(("flag", ("flag_japan.png", -2.3)))
UV_FACES.append(("flag", ("flag_school.png", 2.3)))
# behind the stage: its back wall's panelling
fb(inside, EAST, YA + T / 2, YS - T, -T - 0.02, -T, STAGE_H, STAGE_H + 4.5, panel)
for y in (-4.0, 4.0):
    lamp(16.0, y, PRO[3] + 1.0, intensity=0.03, radius=0.3, rng=12.0)
# the gallery round the south, north and west walls at GAL, its rail; a steel ladder up at the
# north-west corner
GW = 1.0
for (x0, x1, y0, y1) in ((-XE + T, STAGE_X, YS - T - GW, YS - T), (-XE + T, STAGE_X, YA + T / 2, YA + T / 2 + GW), (-XE + T, -XE + T + GW, YA + T / 2 + GW, YS - T - GW)):
    inside.box((x0, y0, GAL - 0.2), (x1, y1, GAL), steel)
    inside.box((x0, y0, GAL), (x1, y1, GAL + 0.01), annex_floor)
# its rail along the inner edges: posts, top and mid rails, mesh bars
edges = [((-XE + T + GW, YS - T - GW), (STAGE_X, YS - T - GW)), ((-XE + T + GW, YA + T / 2 + GW), (STAGE_X, YA + T / 2 + GW)), ((-XE + T + GW, YA + T / 2 + GW), (-XE + T + GW, YS - T - GW))]
for (a, b) in edges:
    a, b = Vector(a + (0,)), Vector(b + (0,))
    L = (b - a).length
    for z in (GAL + 0.5, GAL + 1.1):
        fk.tube(fit, [a + Vector((0, 0, z)), b + Vector((0, 0, z))], 0.025, rail, seg=10)
    n = int(L / 1.5)
    for k in range(n + 1):
        p = a + (b - a) * (k / n)
        fit.box((p.x - 0.03, p.y - 0.03, GAL), (p.x + 0.03, p.y + 0.03, GAL + 1.1), rail)
    nb = int(L / 0.12)
    for k in range(nb):
        p = a + (b - a) * ((k + 0.5) / nb)
        fit.box((p.x - 0.006, p.y - 0.006, GAL + 0.05), (p.x + 0.006, p.y + 0.006, GAL + 1.1), rail)
for k in range(15):                                            # the ladder
    z = FL + 0.3 + k * 0.28
    fk.tube(fit, [(-XE + T + 0.1, YA + T / 2 + 1.3, z), (-XE + T + 0.1, YA + T / 2 + 1.75, z)], 0.015, steel, seg=8)
for y in (YA + T / 2 + 1.3, YA + T / 2 + 1.75):
    fk.tube(fit, [(-XE + T + 0.1, y, FL), (-XE + T + 0.1, y, GAL + 1.0)], 0.022, steel, seg=8)
# the trusses: arched steel trusses across the arena every 5 m (two chords, a zigzag web)
for x in [-17.5 + 5.0 * k for k in range(7)]:
    top, bot = [], []
    n = 20
    for i in range(n + 1):
        y = YA + (YS - YA) * i / n
        top.append(Vector((x, y, arch_z(y) - 0.05)))
        bot.append(Vector((x, y, arch_z(y) - 0.75)))
    fk.tube(fit, top, 0.09, truss_m, seg=10)
    fk.tube(fit, bot, 0.07, truss_m, seg=10)
    for i in range(n):
        fk.tube(fit, [bot[i], top[i + 1]] if i % 2 == 0 else [top[i], bot[i + 1]], 0.035, truss_m, seg=8, caps=False)
for i in range(0, 21, 2):
    y = YA + (YS - YA) * i / 20
    fk.tube(fit, [(-XE + T, y, arch_z(y) - 0.06), (STAGE_X, y, arch_z(y) - 0.06)], 0.045, truss_m, seg=8)
# the ceiling: cedar boards under the roof, following the barrel
for k in range(56):
    y0 = YA + (YS - YA) * k / 56
    y1 = YA + (YS - YA) * (k + 1) / 56
    z0, z1 = arch_z(y0) + 0.02, arch_z(y1) + 0.02
    c = Vector((0.0, (y0 + y1) / 2, (z0 + z1) / 2))
    ceil.oriented_box(c, (2 * XE - 2 * T, math.hypot(y1 - y0, z1 - z0) + 0.005, 0.03), Matrix.Rotation(math.atan2(z1 - z0, y1 - y0), 3, "X"), ceiling_wood)
# the basketball backboards, hung from the trusses over each end of the court
BY = 1.9
for (bx, face) in ((-17.4 + 1.2, 1), (10.6 - 1.2, -1)):
    hz = FL + 3.05
    fit.box((bx - 0.015, BY - 0.9, hz - 0.15), (bx + 0.015, BY + 0.9, hz + 0.9), board_m)
    for (y0, y1, z0, z1) in ((BY - 0.9, BY + 0.9, hz + 0.86, hz + 0.9), (BY - 0.9, BY + 0.9, hz - 0.15, hz - 0.11), (BY - 0.9, BY - 0.86, hz - 0.15, hz + 0.9), (BY + 0.86, BY + 0.9, hz - 0.15, hz + 0.9)):
        fit.box((bx + face * 0.016, y0, z0), (bx + face * 0.02, y1, z1), dark)
    for (y0, y1, z0, z1) in ((BY - 0.3, BY + 0.3, hz + 0.41, hz + 0.45), (BY - 0.3, BY + 0.3, hz, hz + 0.04), (BY - 0.3, BY - 0.26, hz, hz + 0.45), (BY + 0.26, BY + 0.3, hz, hz + 0.45)):
        fit.box((bx + face * 0.016, y0, z0), (bx + face * 0.02, y1, z1), orange)
    rc = Vector((bx + face * (0.15 + 0.225), BY, hz))
    fk.tube(fit, fk.ring(rc, 0.225, 0.225, 2.0, 24), 0.01, orange, seg=8, closed=True)
    fk.tube(fit, [(bx + face * 0.02, BY - 0.08, hz), (bx + face * 0.155, BY - 0.08, hz)], 0.012, orange, seg=8)
    fk.tube(fit, [(bx + face * 0.02, BY + 0.08, hz), (bx + face * 0.155, BY + 0.08, hz)], 0.012, orange, seg=8)
    # its lattice frame: two vertical trusses (either side of the board) up to the roof's
    # trusses, chords at the board and a metre behind it, zigzag webs, ties across
    top_z = arch_z(BY) - 0.8
    for sy in (-0.55, 0.55):
        xa, xb = bx - face * 0.05, bx - face * 1.0
        fk.tube(fit, [(xa, BY + sy, hz + 0.9), (xa, BY + sy, top_z)], 0.035, truss_m, seg=8)
        fk.tube(fit, [(xb, BY + sy, hz + 0.3), (xb, BY + sy, top_z)], 0.035, truss_m, seg=8)
        fk.tube(fit, [(xa, BY + sy, hz + 0.6), (xb, BY + sy, hz + 0.3)], 0.025, truss_m, seg=6)
        nz = int((top_z - hz - 0.9) / 0.6)
        for i in range(nz):
            z0 = hz + 0.9 + i * (top_z - hz - 0.9) / nz
            z1 = hz + 0.9 + (i + 1) * (top_z - hz - 0.9) / nz
            fk.tube(fit, [(xa, BY + sy, z0), (xb, BY + sy, z1)] if i % 2 == 0 else [(xb, BY + sy, z0), (xa, BY + sy, z1)], 0.018, truss_m, seg=6, caps=False)
    for z in (hz + 0.9, (hz + top_z) / 2, top_z - 0.05):
        fk.tube(fit, [(bx - face * 0.05, BY - 0.55, z), (bx - face * 0.05, BY + 0.55, z)], 0.02, truss_m, seg=6)
# blackout curtains at the long walls' upper windows, drawn back to both sides of each window
for (y, out, hl) in ((YS - T - 0.3, (0, -1, 0), UP_HOLES[YS]), (YA + T / 2 + 0.3, (0, 1, 0), UP_HOLES[YA])):
    fk.tube(fit, [(-XE + T, y, CLER[1] + 0.2), (STAGE_X, y, CLER[1] + 0.2)], 0.015, alu, seg=6)
    for (a, bb, z0, z1) in hl:
        if not (z0 >= CLER[0] - 0.01 and z1 <= CLER[1] + 0.01):
            continue
        drape_along((a - 0.15, y), (a + 0.6, y), CLER[0] - 0.3, CLER[1] + 0.2, out, 5, blackout, depth=0.1)
        drape_along((bb - 0.6, y), (bb + 0.15, y), CLER[0] - 0.3, CLER[1] + 0.2, out, 5, blackout, depth=0.1)
for (x, y) in ((-XE + T + 0.2, -6.0), (-XE + T + 0.2, 10.0)):
    fk.rbox(fit, (x - 0.02, y - 0.25, GAL + 1.6), (x + 0.3, y + 0.25, GAL + 2.2), panel, r=0.02)
    fit.box((x + 0.3, y - 0.2, GAL + 1.65), (x + 0.305, y + 0.2, GAL + 2.15), dark)
# the outside steel stair up the south wall to the gallery's door: a flight down along the wall
# eastwards from a landing by the door (westwards it ran across the south door); grating
# treads, stringers, and a rail on the open side only (none across the door)
RN = 22
RR = GAL / RN
TRD = 0.26
LX0, LX1 = GDOOR[0] - 0.6, GDOOR[1] + 0.8
LY0, LY1 = YS, YS + 1.3
fit.box((LX0, LY0, GAL - 0.12), (LX1, LY1, GAL), stair_steel)
for k in range(RN - 1):
    x0 = LX1 + k * TRD
    z = GAL - (k + 1) * RR
    fit.box((x0, LY0 + 0.1, z - 0.04), (x0 + TRD, LY1 - 0.1, z), stair_steel)
xb = LX1 + (RN - 1) * TRD
for y in (LY0 + 0.08, LY1 - 0.08):
    fk.tube(fit, [(xb, y, 0.0), (LX1, y, GAL - 0.1)], 0.05, stair_steel, seg=6)                 # stringers
# the rail on the open side: along the flight and round the landing's open edges (not the wall)
yo = LY1 - 0.08
fk.tube(fit, [(xb + 0.1, yo, 0.9), (LX1, yo, GAL + 0.9), (LX0, yo, GAL + 0.9), (LX0, LY0 + 0.1, GAL + 0.9)], 0.025, stair_steel, seg=8)
for x in (LX0, LX1, (LX0 + LX1) / 2):
    fk.tube(fit, [(x, yo, 0.0), (x, yo, GAL + 0.9)], 0.05, stair_steel, seg=8)
fk.tube(fit, [(LX0, LY0 + 0.1, 0.0), (LX0, LY0 + 0.1, GAL + 0.9)], 0.04, stair_steel, seg=8)
for k in range(0, RN, 4):
    x = LX1 + k * TRD
    fk.tube(fit, [(x, yo, GAL - k * RR), (x, yo, GAL - k * RR + 0.9)], 0.02, stair_steel, seg=6)
steel_doors(SOUTH, GDOOR[0], GDOOR[1], GAL, GAL + 2.1, 0.06, "gallery", wall=(-T, 0.0))
# ball guards over the low vent windows outside: steel bars
for (a, bb, z0, z1) in S_HOLES:
    if z1 - z0 < 1:
        for k in range(7):
            u = a + (bb - a) * (k + 0.5) / 7
            fb(fit, SOUTH, u - 0.008, u + 0.008, 0.05, 0.07, z0, z1, steel)
        fb(fit, SOUTH, a, bb, 0.03, 0.08, z0 - 0.02, z0, steel)
        fb(fit, SOUTH, a, bb, 0.03, 0.08, z1, z1 + 0.02, steel)
# the high-bay lights: three rows down the arena
for x in (-15.0, -7.5, 0.0, 7.5):
    for y in (YA + 6.0, (YA + YS) / 2, YS - 6.0):
        z = arch_z(y) - 1.6
        fk.tube(fit, [(x, y, arch_z(y) - 0.7), (x, y, z + 0.3)], 0.012, steel, seg=6)
        fk.loft(fit, [fk.ring((x, y, z + 0.3), 0.12, 0.12, 2.0, 20), fk.ring((x, y, z), 0.3, 0.3, 2.0, 20)], alu, outward=True)
        fit.cylinder((x, y, z - 0.005), 0.28, 0.01, lampm, segments=20)
        lamp(x, y, z, intensity=0.08)
# the courts' lines, painted on the boards: basketball (white), two volleyball (yellow), four
# badminton (green)
def paint(pts, col, w=0.05):
    for (pa, pb) in zip(pts, pts[1:]):
        pa, pb = Vector((pa[0], pa[1], FL + 0.0012)), Vector((pb[0], pb[1], FL + 0.0012))
        L = (pb - pa).length
        if L < 1e-4:
            continue
        lines.oriented_box((pa + pb) / 2, (L + w * 0.5, w, 0.0008), Matrix.Rotation(math.atan2((pb - pa).y, (pb - pa).x), 3, "Z"), line_mats[col])


def paint_rect(x0, y0, x1, y1, col, w=0.05):
    paint([(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)], col, w)


def paint_arc(cx, cy, r, col, a0=0.0, a1=360.0, w=0.05, n=48):
    paint([(cx + r * math.cos(math.radians(a0 + (a1 - a0) * k / n)), cy + r * math.sin(math.radians(a0 + (a1 - a0) * k / n))) for k in range(n + 1)], col, w)


cbx, cby = -3.4, 1.9
paint_rect(cbx - 14, cby - 7.5, cbx + 14, cby + 7.5, "white")
paint([(cbx, cby - 7.5), (cbx, cby + 7.5)], "white")
paint_arc(cbx, cby, 1.8, "white")
for sgn in (-1, 1):
    ex = cbx + sgn * 14
    hx = ex - sgn * 1.575
    paint_rect(min(ex, ex - sgn * 5.8), cby - 2.45, max(ex, ex - sgn * 5.8), cby + 2.45, "white")
    paint_arc(ex - sgn * 5.8, cby, 1.8, "white")
    aa = math.degrees(math.asin(6.6 / 6.75))
    if sgn > 0:
        paint_arc(hx, cby, 6.75, "white", 180 - aa, 180 + aa)
    else:
        paint_arc(hx, cby, 6.75, "white", -aa, aa)
    for yy in (cby - 6.6, cby + 6.6):
        paint([(ex, yy), (hx - sgn * 6.75 * math.cos(math.radians(aa)), yy)], "white")
for vx in (-11.0, 4.2):
    paint_rect(vx - 4.5, cby - 9, vx + 4.5, cby + 9, "yellow")
    paint([(vx - 4.5, cby), (vx + 4.5, cby)], "yellow")
    for yy in (cby - 3, cby + 3):
        paint([(vx - 4.5, yy), (vx + 4.5, yy)], "yellow")
for cx in (-15.0, -7.5, 0.5, 8.0):
    paint_rect(cx - 3.05, cby - 6.7, cx + 3.05, cby + 6.7, "green", w=0.04)
    paint([(cx - 3.05, cby), (cx + 3.05, cby)], "green", w=0.04)
    for yy in (cby - 1.98, cby + 1.98):
        paint([(cx - 3.05, yy), (cx + 3.05, yy)], "green", w=0.04)
# the clock over the stage, facing the arena
UV_FACES.append(("clock", (STAGE_X - 0.16, 0.0, PRO[3] + 1.0, 0.4)))

# ----------------------------------------------------------------------------- uv faces, finish
def uv_object(name, faces):
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
    kz.coll("GYM").objects.link(ob)


faces = []
for kind, data in UV_FACES:
    if kind == "floor":
        x0, x1, y0, y1 = data
        # (the texture: 32.75 x 27.75 m from x = -19.75, y = -12; v up = +y... the image's top is
        # the arena's south side)
        # (the scan's boards run up the image: turned to run along the hall)
        faces.append(([(x0, y0, FL), (x1, y0, FL), (x1, y1, FL), (x0, y1, FL)], [(y0 / 2, x0 / 2), (y0 / 2, x1 / 2), (y1 / 2, x1 / 2), (y1 / 2, x0 / 2)], court))
    if kind == "flag":
        fname, fy = data
        fm = bpy.data.materials.get(fname) or bpy.data.materials.new(fname)
        if not fm.node_tree or len(fm.node_tree.nodes) < 3:
            fm.use_nodes = True
            fm.use_backface_culling = False
            tt = fm.node_tree.nodes.new("ShaderNodeTexImage")
            tt.image = kz._image(os.path.join(SIGNS, fname), "sRGB")
            fm.node_tree.links.new(tt.outputs["Color"], fm.node_tree.nodes["Principled BSDF"].inputs["Base Color"])
            tn2 = fm.node_tree.nodes.new("ShaderNodeTexImage")
            tn2.image = kz._image(_n, "Non-Color")
            nm2 = fm.node_tree.nodes.new("ShaderNodeNormalMap")
            nm2.inputs["Strength"].default_value = 0.1
            fm.node_tree.links.new(tn2.outputs["Color"], nm2.inputs["Color"])
            fm.node_tree.links.new(nm2.outputs["Normal"], fm.node_tree.nodes["Principled BSDF"].inputs["Normal"])
            fm.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.9
        fx = XE - 1.5
        z0, z1 = STAGE_H + 2.4, STAGE_H + 3.6
        w = 0.9
        # (facing -X, into the hall: the viewer's right is +Y)
        faces.append(([(fx, fy - w, z0), (fx, fy + w, z0), (fx, fy + w, z1), (fx, fy - w, z1)], [(0, 0), (1, 0), (1, 1), (0, 1)], fm))
        fk.tube(fit, [(fx + 0.02, fy - w - 0.1, z1 + 0.03), (fx + 0.02, fy + w + 0.1, z1 + 0.03)], 0.015, alu, seg=6)
    if kind == "clock":
        x, y, z, r = data
        n = 40
        for k in range(n):
            a0, a1 = 2 * math.pi * k / n, 2 * math.pi * (k + 1) / n
            p0 = (x, y + math.cos(a0) * r, z + math.sin(a0) * r)
            p1 = (x, y + math.cos(a1) * r, z + math.sin(a1) * r)
            # facing -X (into the arena): the viewer's right is +Y
            faces.append(([(x, y, z), p0, p1], [(0.5, 0.5), (0.5 + 0.5 * math.cos(a0), 0.5 + 0.5 * math.sin(a0)), (0.5 + 0.5 * math.cos(a1), 0.5 + 0.5 * math.sin(a1))], clock_mat))
uv_object("faces", faces)
fit.box((STAGE_X - 0.15, -0.46, PRO[3] + 0.55), (STAGE_X - 0.13, 0.46, PRO[3] + 1.45), alu)

for b in (walls, inside, fit, roofb, ply, ceil, lines):
    b.finish(collection="GYM")
_cl = cladb.finish(collection="GYM")
for _d in _cl.data.uv_layers.active.data:
    _d.uv = (_d.uv[1], -_d.uv[0])
for b, pivot in DOORS:
    ob = b.finish(collection="GYM")
    ob.data.transform(Matrix.Translation(-pivot))
    ob.location = pivot

import json  # noqa: E402

os.makedirs(os.path.dirname(OUT), exist_ok=True)
with open(os.path.join(os.path.dirname(OUT), "lights.json"), "w") as f:
    json.dump(LIGHTS, f, indent=1)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT, len(LIGHTS), "lights", len(DOORS), "doors")
