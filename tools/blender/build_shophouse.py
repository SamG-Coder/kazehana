"""The town centre's shophouses (店舗併用住宅): the shop on the ground floor, the family upstairs,
as quality-gated assets, one per frontage class and style (tools/blender/shophouse_spec.py).

    blender --background --factory-startup --python tools/blender/build_shophouse.py -- <style> <width>
    (style "k" or "t", width 6 / 7 / 8 / 9)

-> assets/buildings/shophouse_<style><width>/shophouse_<style><width>.gltf

The census puts 69 shops in the centre (bicycle shops, bookshops, a bakery, tofu shops, barbers,
kissaten...): each stands in one of these, its own signboard on the front (build_shop_signs.py, a
separate asset placed with the building). Built from the reference of Shōwa-era shopping streets
in country towns (shōtengai):

- "k", kanban kenchiku (看板建築): a flat front of ribbed painted metal rising to a parapet that
  hides the gable roof behind it, aluminium upstairs windows, the shopfront a full-width run of
  aluminium-framed glass with sliding doors in the middle, a roll-shutter box over it and the
  signboard above
- "t", traditional (町家風): a timber front of dark stained boards, a tiled pent roof (hisashi)
  over the shopfront, the signboard above it, wooden lattice (kōshi) over the upstairs windows,
  the main roof's eaves to the street

Both: mortar side walls (neighbours stand close), a gable roof in kawara tiles (Megascans
Japanese Roof Tiles), the back with its windows, the family's door and an air-conditioner unit,
gutters and downpipes. Behind the shop glass: the shop room's shell (floor, walls, ceiling: dim,
empty; its fittings would be separate approved props). The building is solid (not enterable yet).
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402
import shophouse_spec as spec  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
STYLE = ARGS[0] if ARGS else "k"
W = float(ARGS[1]) if len(ARGS) > 1 else 7.0
ASSET_ID = spec.asset_id(W, STYLE)
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")

D = spec.DEPTH
XW, YF, YB = W / 2, D / 2, -D / 2
T = 0.18
F0, F1, TOP = spec.F0, spec.F1, spec.TOP
SHOP_TOP = 2.6                     # the shopfront opening's head
PITCH = math.radians(20.0)
BX0, BX1, BY, BZ0, BZ1 = spec.board(W, STYLE)

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)

# ----------------------------------------------------------------------------- materials
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
# kanban fronts: one paint per frontage (the street reads as a row of different shops)
KANBAN = {6: (0.50, 0.44, 0.30), 7: (0.16, 0.25, 0.20), 8: (0.14, 0.20, 0.29), 9: (0.33, 0.19, 0.11)}   # ochre, green, blue, rust
mortar = P("shophouse_mortar", (0.58, 0.56, 0.51), DETAIL, roughness=0.85, normal_strength=0.7)
found = P("house_foundation", (0.36, 0.35, 0.33), DETAIL, roughness=0.85, normal_strength=0.8)
front_metal = P(f"shophouse_front_{int(W)}", KANBAN[int(W)], DETAIL, roughness=0.45, metallic=0.3, normal_strength=0.2)
front_wood = P("shophouse_front_wood", (0.075, 0.048, 0.03), "Wood_Planks", roughness=0.6, normal_strength=0.7)   # stained dark
lattice = P("shophouse_lattice", (0.11, 0.07, 0.045), "Wood_Planks", roughness=0.55, normal_strength=0.5)
trim = P("shop_trim", (0.07, 0.07, 0.07), DETAIL, roughness=0.5, normal_strength=0.2)
alu = P("house_aluminium", (0.09, 0.075, 0.065), DETAIL, roughness=0.35, metallic=0.8, normal_strength=0.1)
silver = P("shop_silver_alu", (0.55, 0.55, 0.54), DETAIL, roughness=0.3, metallic=0.85, normal_strength=0.1)
shutter = P("shop_shutter_case", (0.42, 0.43, 0.42), DETAIL, roughness=0.45, metallic=0.6, normal_strength=0.15)
unit = P("house_unit", (0.70, 0.70, 0.67), DETAIL, roughness=0.45, metallic=0.2, normal_strength=0.15)
dark = P("house_dark", (0.02, 0.02, 0.02), DETAIL, roughness=0.6, normal_strength=0.1)
room = P("house_room", (0.28, 0.24, 0.20), DETAIL, roughness=0.9, normal_strength=0.2)
curtain = P("house_curtain", (0.62, 0.58, 0.50), DETAIL, roughness=0.95, normal_strength=0.2)
shop_wall = P("shophouse_shop_wall", (0.62, 0.60, 0.55), DETAIL, roughness=0.9, normal_strength=0.15, grain=0.15)
shop_ceiling = P("shop_ceiling", (0.80, 0.79, 0.76), "Concrete_Floor-f4b85cb0", roughness=0.95, normal_strength=0.06, grain=0.06)
shop_floor = P("shop_floor_vinyl", (0.50, 0.49, 0.46), "Concrete_Floor-f4b85cb0", roughness=0.55, normal_strength=0.2, grain=0.25)
tiles = kz.surface_material("shop_tiles", "Japanese_Concrete_Block_Wall", tint=(0.42, 0.40, 0.38), desaturate=0.5)
roof = kz.surface_material("shophouse_roof", "Japanese_Roof_Tiles", tint=(0.45, 0.46, 0.49), desaturate=0.85)
gutter = P("shophouse_gutter", (0.50, 0.50, 0.48), DETAIL, roughness=0.4, normal_strength=0.1)
win_glass = kz.plain_material("kz_glow_glass", (0.025, 0.03, 0.035), roughness=0.04)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)
clear = kz.plain_material("kz_glow_shop_clear_glass", (0.9, 0.95, 0.93), roughness=0.02)
door_m = P("shop_back_door", (0.16, 0.10, 0.06), "Wood_Planks", roughness=0.5, normal_strength=0.6)


def _set_mat(b, verts, mat):
    idx = b._mat_index(mat)
    for f in {f for v in verts if v.is_valid for f in v.link_faces}:
        f.material_index = idx


def rcyl(b, p0, p1, r, mat, seg=12):
    p0, p1 = Vector(p0), Vector(p1)
    d = p1 - p0
    res = bmesh.ops.create_cone(b.bm, cap_ends=True, segments=seg, radius1=r, radius2=r, depth=d.length)
    rot = Vector((0, 0, 1)).rotation_difference(d.normalized()).to_matrix().to_4x4()
    bmesh.ops.transform(b.bm, matrix=Matrix.Translation((p0 + p1) / 2) @ rot, verts=res["verts"])
    _set_mat(b, res["verts"], mat)


def obox(b, c, size, t, n, mat, bevel=0.0):
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
    for k in range(10):
        obox(b, c - t * (w / 2) + up * (-h / 2 + 0.08 + k * 0.042), (0.004, d * 0.8, 0.018), t, n, dark)


walls = kz.MeshBuilder("walls", uv_scale=0.5)
fittings = kz.MeshBuilder("fittings", uv_scale=1.0)
roofing = kz.MeshBuilder("roof", uv_scale=0.5)
inside = kz.MeshBuilder("interior", uv_scale=1.0)
facade = kz.MeshBuilder("facade", uv_scale=0.5)

# facade frames: (u along the face as seen from outside, d out from it) -> plan (x, y)
FRONT = lambda u, d: (u, YF + d)
BACK = lambda u, d: (-u, YB - d)
EAST = lambda u, d: (XW + d, -u)
WEST = lambda u, d: (-XW - d, u)


def fb(b, F, u0, u1, d0, d1, z0, z1, mat):
    pts = [F(u, d) for u in (u0, u1) for d in (d0, d1)]
    b.box((min(p[0] for p in pts), min(p[1] for p in pts), z0), (max(p[0] for p in pts), max(p[1] for p in pts), z1), mat)


def free_rects(u0, u1, z0, z1, holes):
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


def window(F, u0, u1, z0, z1, glass, curtains=True, depth=T):
    """An aluminium sash window (two leaves, a mullion) in an opening of a wall `depth` thick, the
    room dim behind it, curtains drawn back."""
    FR = 0.045
    if curtains:
        fb(walls, F, u0, u0 + (u1 - u0) * 0.28, -depth - 0.17, -depth - 0.15, z0, z1, curtain)
        fb(walls, F, u1 - (u1 - u0) * 0.24, u1, -depth - 0.17, -depth - 0.15, z0, z1, curtain)
    fb(walls, F, u0 - 0.3, u1 + 0.3, -depth - 0.8, -depth - 0.75, z0 - 0.3, z1 + 0.3, room)
    fb(fittings, F, u0, u1, -0.07, -0.065, z0, z1, glass)
    for (a, bb, za, zb) in ((u0, u1, z0, z0 + FR), (u0, u1, z1 - FR, z1), (u0, u0 + FR, z0, z1), (u1 - FR, u1, z0, z1)):
        fb(fittings, F, a, bb, -0.09, -0.01, za, zb, alu)
    if u1 - u0 > 0.9:
        m = (u0 + u1) / 2
        fb(fittings, F, m - 0.03, m + 0.03, -0.085, -0.02, z0, z1, alu)
    fb(fittings, F, u0 - 0.04, u1 + 0.04, -0.02, 0.05, z0 - 0.04, z0, alu)       # sill
    for u in (u0 - 0.002, u1):
        fb(walls, F, u, u + 0.002, -depth, 0.0, z0, z1, mortar)                     # reveals


# ----------------------------------------------------------------------------- the shell
# side walls the full depth, a little window upstairs at the back of each
SIDE_WIN = (-D / 2 + 1.2, -D / 2 + 2.0, F1 + 1.0, F1 + 1.7)
for F in (EAST, WEST):
    wall(walls, F, YB if F is WEST else -YF, YF if F is WEST else -YB, 0.0, TOP, [SIDE_WIN if F is WEST else (-SIDE_WIN[1], -SIDE_WIN[0], SIDE_WIN[2], SIDE_WIN[3])], mortar)
window(WEST, *SIDE_WIN, frosted, curtains=False)
window(EAST, -SIDE_WIN[1], -SIDE_WIN[0], SIDE_WIN[2], SIDE_WIN[3], frosted, curtains=False)
for F, a, b in ((EAST, -YF, -YB), (WEST, YB, YF), (BACK, -XW, XW)):
    fb(walls, F, a, b, -0.02, 0.012, -0.4, 0.3, found)                                  # foundation band

# the back: the family's door, the kitchen window, the bathroom's frosted one, two upstairs
BDOOR = (XW - 1.6, XW - 0.7, F0, F0 + 2.0)
BACK_WINS = [(-XW + 0.8, -XW + 2.0, F0 + 1.0, F0 + 1.9, frosted), (0.0 if W < 7 else -0.4, (0.0 if W < 7 else -0.4) + 1.0, F0 + 1.2, F0 + 1.9, frosted),
             (-XW + 0.8, -XW + 2.6, F1 + 0.9, F1 + 2.0, win_glass), (XW - 2.4, XW - 0.8, F1 + 0.9, F1 + 2.0, win_glass)]
wall(walls, BACK, -XW, XW, 0.0, TOP, [BDOOR] + [w[:4] for w in BACK_WINS], mortar)
for u0, u1, z0, z1, g in BACK_WINS:
    window(BACK, u0, u1, z0, z1, g, curtains=g is win_glass)
u0, u1, z0, z1 = BDOOR
fb(fittings, BACK, u0, u1, -0.06, -0.03, z0, z1, door_m)
fb(fittings, BACK, u0 + 0.14, u0 + 0.26, -0.03, -0.025, z0 + 0.4, z1 - 0.3, frosted)
fb(fittings, BACK, u1 - 0.15, u1 - 0.12, -0.03, 0.02, z0 + 0.9, z0 + 1.3, silver)
for (a, bb, za, zb) in ((u0 - 0.06, u0, z0, z1 + 0.06), (u1, u1 + 0.06, z0, z1 + 0.06), (u0 - 0.06, u1 + 0.06, z1, z1 + 0.06)):
    fb(fittings, BACK, a, bb, -0.07, 0.02, za, zb, alu)
fb(fittings, BACK, u0 - 0.5, u1 + 0.5, 0.0, 0.8, F0 + 2.25, F0 + 2.33, unit)            # porch roof
fb(fittings, BACK, u0 - 0.4, u1 + 0.4, 0.0, 0.7, 0.0, F0, found)                       # step
outdoor_unit(fittings, Vector((-XW + 2.6, YB - 0.012, 0)), (-1, 0, 0), (0, -1, 0))

# floors and the top (closing the box: nothing to see through)
walls.box((-XW + 0.01, YB + 0.01, TOP - 0.05), (XW - 0.01, YF - 0.01, TOP), room)
walls.box((-XW + T, YB + T, F1 - 0.15), (XW - T, YF - 0.5, F1), room)

# ----------------------------------------------------------------------------- the shopfront
# the opening between two piers, the glazing set back 0.25 m, the shop room behind it
PIER = 0.3
SU0, SU1 = -XW + PIER, XW - PIER
SET = 0.25
pier_mat = tiles if STYLE == "k" else front_wood
for (a, b) in ((-XW, SU0), (SU1, XW)):
    fb(walls, FRONT, a, b, -T, 0.02, 0.0, SHOP_TOP + 0.05, pier_mat)
fb(walls, FRONT, -XW, XW, -T, 0.0, SHOP_TOP, F1 + 0.05, mortar)                           # (the lintel behind the front)
# the shop room's shell: floor, side walls, the back partition, the ceiling (dim, empty)
RD = 4.2
inside.box((SU0, YF - RD, 0.0), (SU1, YF - SET, F0), shop_floor)
inside.box((SU0 - 0.02, YF - RD, F0), (SU0, YF - SET, SHOP_TOP + 0.4), shop_wall)
inside.box((SU1, YF - RD, F0), (SU1 + 0.02, YF - SET, SHOP_TOP + 0.4), shop_wall)
inside.box((SU0, YF - RD - 0.02, F0), (SU1, YF - RD, SHOP_TOP + 0.4), shop_wall)
inside.box((SU0, YF - RD, SHOP_TOP + 0.3), (SU1, YF - SET, SHOP_TOP + 0.32), shop_ceiling)
fb(walls, FRONT, SU0, SU1, -SET - 0.02, 0.0, 0.0, F0, found)                              # the step
# glazing: a kick plate, glass to the transom bar, the transom glass; sliding doors in the middle
frame = alu if STYLE == "k" else lattice
D0, D1 = -SET - 0.06, -SET
fb(fittings, FRONT, SU0, SU1, D0, D1, F0, F0 + 0.28, silver if STYLE == "k" else front_wood)  # kick plate
fb(fittings, FRONT, SU0, SU1, D0 + 0.02, D1 - 0.02, F0 + 0.28, SHOP_TOP, clear)
for z in (F0 + 0.28, 2.15, SHOP_TOP - 0.05):
    fb(fittings, FRONT, SU0, SU1, D0 - 0.01, D1 + 0.01, z, z + 0.05, frame)               # rails, transom bar
DW = 1.8 if W >= 7 else 1.6                                                                 # the sliding doors' width
posts = [SU0, -DW / 2, 0.0, DW / 2, SU1]
span = SU1 - SU0
for k in range(1, 4 if span > 7 else 3):                                                    # display window mullions
    for side in (-1, 1):
        u = side * (DW / 2 + k * (span / 2 - DW / 2) / (4 if span > 7 else 3))
        if abs(u) < span / 2 - 0.1:
            posts.append(u)
for u in posts:
    fb(fittings, FRONT, u - 0.03, u + 0.03, D0 - 0.01, D1 + 0.01, F0, SHOP_TOP, frame)
for u in (-0.12, 0.12):
    fb(fittings, FRONT, u - 0.012, u + 0.012, D1 + 0.01, D1 + 0.04, F0 + 0.9, F0 + 1.3, silver)  # door pulls
if STYLE == "t":
    # wooden glazing bars on the door leaves (the old-shop look)
    for side in (-1, 1):
        for k in (1, 2):
            u = side * (DW / 2) * k / 3
            fb(fittings, FRONT, u - 0.012, u + 0.012, D1, D1 + 0.015, F0 + 0.28, 2.15, lattice)
        for z in (0.9, 1.5):
            fb(fittings, FRONT, side * DW / 2 - (0.0 if side > 0 else 0.0) if side < 0 else 0.0, side * DW / 2 if side > 0 else 0.0, D1, D1 + 0.015, F0 + z, F0 + z + 0.025, lattice)

# ----------------------------------------------------------------------------- the front, upstairs
UPW = [(-XW + 0.6, -0.35, None), (0.35, XW - 0.6, None)] if W >= 7 else [(-XW + 0.8, XW - 0.8, None)]
if STYLE == "k":
    # the roll shutter's box over the opening, the signboard's frame, the ribbed metal front to the
    # parapet (the gable roof hidden behind it), the upstairs windows cut into it
    fb(fittings, FRONT, SU0 - 0.05, SU1 + 0.05, 0.0, 0.28, SHOP_TOP, SHOP_TOP + 0.26, shutter)
    fb(fittings, FRONT, SU0 - 0.05, SU1 + 0.05, 0.0, 0.3, SHOP_TOP + 0.24, SHOP_TOP + 0.27, silver)
    PAR = TOP + XW * math.tan(PITCH) + 0.35
    UZ0, UZ1 = BZ1 + 0.18, min(BZ1 + 0.18 + 1.25, PAR - 0.6)
    holes = [(u0, u1, UZ0, UZ1) for u0, u1, _ in UPW]
    for a, bb, za, zb in free_rects(-XW, XW, SHOP_TOP + 0.26, PAR, holes):
        fb(facade, FRONT, a, bb, 0.0, 0.08, za, zb, front_metal)
    # ribs every 0.25 m (the profiled sheet), stopping at the windows and the signboard
    u = -XW + 0.125
    while u < XW:
        for a, bb, za, zb in free_rects(u - 0.02, u + 0.02, SHOP_TOP + 0.26, PAR, holes + [(BX0 - 0.08, BX1 + 0.08, BZ0 - 0.08, BZ1 + 0.08)]):
            fb(facade, FRONT, a, bb, 0.08, 0.11, za, zb, front_metal)
        u += 0.25
    fb(fittings, FRONT, -XW - 0.03, XW + 0.03, -0.05, 0.16, PAR, PAR + 0.07, silver)       # coping
    fb(fittings, FRONT, -XW - 0.03, -XW + 0.03, 0.0, 0.14, SHOP_TOP + 0.26, PAR, silver)    # corner trims
    fb(fittings, FRONT, XW - 0.03, XW + 0.03, 0.0, 0.14, SHOP_TOP + 0.26, PAR, silver)
    # the parapet's back (seen from behind and above) and its sides over the side walls
    fb(walls, FRONT, -XW, XW, -0.25, 0.0, TOP, PAR, mortar)
    for u0, u1, _ in UPW:
        window(FRONT, u0, u1, UZ0, UZ1, win_glass, depth=0.11)
        fb(fittings, FRONT, u0 - 0.06, u1 + 0.06, 0.11, 0.17, UZ0 - 0.06, UZ0, silver)        # a little sill hood
else:
    # the timber front: boards, the tiled pent roof over the shopfront, the signboard above it,
    # lattice over the upstairs windows, the main roof's eaves to the street
    UZ0, UZ1 = BZ1 + 0.15, TOP - 0.35
    holes = [(u0, u1, UZ0, UZ1) for u0, u1, _ in UPW]
    for a, bb, za, zb in free_rects(-XW, XW, SHOP_TOP + 0.05, TOP, holes):
        fb(facade, FRONT, a, bb, 0.0, 0.04, za, zb, front_wood)
    # vertical board battens every 0.18 m
    u = -XW + 0.09
    while u < XW:
        for a, bb, za, zb in free_rects(u - 0.012, u + 0.012, SHOP_TOP + 0.05, TOP, holes + [(BX0 - 0.08, BX1 + 0.08, BZ0 - 0.08, BZ1 + 0.08)]):
            fb(facade, FRONT, a, bb, 0.04, 0.06, za, zb, front_wood)
        u += 0.18
    # the pent roof (hisashi): a kawara slope from the front down to the street, on brackets
    HD = 0.95
    hz1, hz0 = 3.28, 3.28 - HD * math.tan(math.radians(22))
    ang = math.radians(22)
    rot = Matrix.Rotation(-ang, 3, "X")
    slope = HD / math.cos(ang)
    roofing.oriented_box(Vector((0, YF + HD / 2, (hz0 + hz1) / 2 + 0.05)), (W + 0.1, slope, 0.1), rot, roof)
    for k in range(int((W + 0.1) / 0.3)):
        x = -(W + 0.1) / 2 + (k + 0.5) * (W + 0.1) / int((W + 0.1) / 0.3)
        tip = Vector((x, YF + HD + 0.02, hz0 + 0.03))
        rcyl(roofing, tip - Vector((0, 0.12, -0.05)), tip, 0.06, roof, seg=12)
    fb(fittings, FRONT, -XW - 0.05, XW + 0.05, HD - 0.05, HD, hz0 - 0.16, hz0 + 0.02, front_wood)  # fascia
    for u in (-XW + 0.15, 0.0, XW - 0.15):
        a, b = Vector((u, YF + 0.04, hz1 - 0.35)), Vector((u, YF + HD - 0.1, hz0 - 0.05))
        mid, d = (a + b) / 2, b - a
        fittings.oriented_box(mid, (0.08, d.length, 0.1), Matrix.Rotation(math.atan2(d.z, d.y), 3, "X"), lattice)  # bracket
    # upstairs windows behind wooden lattice
    for u0, u1, _ in UPW:
        window(FRONT, u0, u1, UZ0, UZ1, frosted, curtains=False, depth=0.07)
        u = u0 + 0.03
        while u < u1 - 0.02:
            fb(fittings, FRONT, u, u + 0.03, 0.06, 0.1, UZ0, UZ1, lattice)
            u += 0.075
        for z in (UZ0, (UZ0 + UZ1) / 2, UZ1 - 0.04):
            fb(fittings, FRONT, u0, u1, 0.06, 0.1, z, z + 0.04, lattice)
    # the shopfront's head: a wooden beam under the pent roof
    fb(fittings, FRONT, -XW, XW, 0.0, 0.08, SHOP_TOP, SHOP_TOP + 0.2, lattice)

# the signboard's backing (the face itself is the shop's own: build_shop_signs.py)
fb(fittings, FRONT, BX0 - 0.06, BX1 + 0.06, 0.0 if STYLE == "k" else 0.04, BY - YF - 0.005, BZ0 - 0.06, BZ1 + 0.06, trim if STYLE == "k" else lattice)

# ----------------------------------------------------------------------------- the roof
RT = 0.14
if STYLE == "k":
    # a gable running back from the street (ridge along Y), eaves over the side walls
    rise = XW * math.tan(PITCH)
    EOV = 0.25
    run = XW + EOV
    slope_len = run / math.cos(PITCH)
    y0, y1 = YB - 0.4, YF - 0.2
    z_eave = TOP - EOV * math.tan(PITCH)
    for s in (-1, 1):
        rot = Matrix.Rotation(-PITCH if s < 0 else PITCH, 3, "Y")
        nrm = rot @ Vector((0, 0, 1))
        c = Vector((s * run / 2, (y0 + y1) / 2, z_eave + (TOP + rise - z_eave) / 2 + 0.04)) - nrm * (RT / 2)
        roofing.oriented_box(c, (slope_len, y1 - y0, RT), rot, roof)
        xe = s * run
        rcyl(fittings, (xe + s * 0.06, y0 + 0.05, z_eave - 0.12), (xe + s * 0.06, y1 - 0.05, z_eave - 0.12), 0.055, gutter, seg=14)
        fittings.cylinder((s * (XW + 0.08), YB + 0.3, 0.0), 0.03, z_eave - 0.4, gutter, segments=10)
    roofing.box((-0.15, y0, TOP + rise - 0.02), (0.15, y1, TOP + rise + 0.14), roof)             # ridge
    # the back gable triangle
    walls.quad([Vector((XW, YB, TOP)), Vector((-XW, YB, TOP)), Vector((0, YB, TOP + rise))], mortar)
    for sx in (-1, 1):
        a, b = Vector((sx * (XW + EOV), y0, z_eave)), Vector((0, y0, TOP + rise + 0.05))
        mid, d = (a + b) / 2, b - a
        fittings.oriented_box(mid - Vector((0, 0, 0.08)), (d.length, 0.04, 0.2), Matrix.Rotation(math.atan2(d.z, d.x), 3, "Y").inverted(), trim)
else:
    # a gable along the street (ridge along X): eaves to the street and the back
    rise = (D / 2) * math.tan(PITCH)
    EOV = 0.55
    run = D / 2 + EOV
    slope_len = run / math.cos(PITCH)
    roof_w = W + 0.1
    z_eave = TOP - EOV * math.tan(PITCH)
    for s in (-1, 1):
        rot = Matrix.Rotation(PITCH if s < 0 else -PITCH, 3, "X")
        nrm = rot @ Vector((0, 0, 1))
        c = Vector((0, s * run / 2, z_eave + (TOP + rise - z_eave) / 2 + 0.04)) - nrm * (RT / 2)
        roofing.oriented_box(c, (roof_w, slope_len, RT), rot, roof)
        ye = s * run
        roofing.box((-roof_w / 2, min(ye - s * 0.06, ye + s * 0.03), z_eave - RT - 0.02), (roof_w / 2, max(ye - s * 0.06, ye + s * 0.03), z_eave + 0.04), roof)
        slope_dir = Vector((0, s * math.cos(PITCH), -math.sin(PITCH)))
        nx = int(roof_w / 0.3)
        for k in range(nx):
            x = -roof_w / 2 + (k + 0.5) * roof_w / nx
            tip = Vector((x, ye + s * 0.02, z_eave + 0.02))
            rcyl(roofing, tip - slope_dir * 0.12, tip, 0.07, roof, seg=12)
        fittings.box((-roof_w / 2, min(ye, ye - s * 0.035), z_eave - RT - 0.2), (roof_w / 2, max(ye, ye - s * 0.035), z_eave - RT + 0.01), front_wood)
        gy = ye + s * 0.08
        rcyl(fittings, (-roof_w / 2 + 0.05, gy, z_eave - 0.14), (roof_w / 2 - 0.05, gy, z_eave - 0.14), 0.055, gutter, seg=14)
        for x in (-XW + 0.2, XW - 0.2):
            if s < 0:
                fittings.cylinder((x, s * (D / 2 + 0.09), 0.0), 0.03, z_eave - 0.45, gutter, segments=10)
    rcyl(roofing, (-roof_w / 2 - 0.05, 0, TOP + rise + 0.06), (roof_w / 2 + 0.05, 0, TOP + rise + 0.06), 0.14, roof, seg=14)
    for sx in (-1, 1):
        x = sx * XW
        tri = [Vector((x, -D / 2, TOP)), Vector((x, D / 2, TOP)), Vector((x, 0, TOP + rise))]
        walls.quad(tri if sx > 0 else [tri[1], tri[0], tri[2]], mortar)

for b in (walls, fittings, roofing, inside, facade):
    b.finish(collection="HOUSE")

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
