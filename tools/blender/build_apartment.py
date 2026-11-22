"""The centre's small apartment blocks (アパート), as quality-gated assets: two storeys, two flats a
floor (the census puts four households in each), 16 x 8 m.

    blender --background --factory-startup --python tools/blender/build_apartment.py -- <scheme> [layout]
    (scheme "a" or "b"; layout "n", the entrance side to the north, or "s", to the garden side)

-> assets/buildings/apartment[_b][_s]/apartment[_b][_s].gltf

The kind of wooden-frame apāto every country town has, after reference photos: ceramic siding,
a low gable roof in dark tiles, the entrance side with the flats' doors under an open walkway
(the upstairs doors along it, a solid painted steel parapet), the outside steel stair at one end,
a bank of mailboxes at its foot, each door with its little frosted kitchen window and the gas
meter box; the garden side with each flat's sliding glass doors, the upstairs balconies on
brackets with their railings, an air-conditioner unit on each. Local frame as the houses: origin
at ground level in the middle, +X along the long side, the entrance side +Y ("n") or -Y ("s").
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
SCHEME = ARGS[0] if ARGS else "a"
LAYOUT = ARGS[1] if len(ARGS) > 1 else "n"
ASSET_ID = "apartment" + ("" if SCHEME == "a" else "_" + SCHEME) + ("" if LAYOUT == "n" else "_" + LAYOUT)
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")
SCHEMES = {
    # siding, trim, steel (stair, walkway, railings), roof tint, doors
    "a": ((0.60, 0.55, 0.46), (0.10, 0.08, 0.06), (0.30, 0.22, 0.16), (0.30, 0.30, 0.32), (0.32, 0.24, 0.16)),
    "b": ((0.70, 0.70, 0.68), (0.06, 0.06, 0.06), (0.16, 0.20, 0.26), (0.22, 0.24, 0.28), (0.18, 0.20, 0.24)),
}
SIDING, TRIM, STEEL, ROOF_TINT, DOOR = SCHEMES[SCHEME]

W, D = 16.0, 8.0
F0 = 0.4
FH = 2.8
F1 = F0 + FH
TOP = F0 + 2 * FH
PITCH = math.radians(16.0)
EAVE_OV, VERGE_OV = 0.5, 0.4
SIDE_T = 0.016
GAL = 1.1                                  # the walkway's depth
S = 1.0 if LAYOUT == "n" else -1.0         # the entrance side: +Y or -Y

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)

DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
siding = P("apartment_siding_" + SCHEME, SIDING, DETAIL, roughness=0.7, normal_strength=0.35)
trim = P("apartment_trim_" + SCHEME, TRIM, DETAIL, roughness=0.55, normal_strength=0.2)
steel = P("apartment_steel_" + SCHEME, STEEL, DETAIL, roughness=0.45, metallic=0.5, normal_strength=0.15)
door_m = P("apartment_door_" + SCHEME, DOOR, DETAIL, roughness=0.4, metallic=0.4, normal_strength=0.1)
found = P("house_foundation", (0.36, 0.35, 0.33), DETAIL, roughness=0.85, normal_strength=0.8)
concrete = P("apartment_slab", (0.52, 0.51, 0.48), DETAIL, roughness=0.85, normal_strength=0.6)
alu = P("house_aluminium", (0.09, 0.075, 0.065), DETAIL, roughness=0.35, metallic=0.8, normal_strength=0.1)
silver = P("shop_silver_alu", (0.55, 0.55, 0.54), DETAIL, roughness=0.3, metallic=0.85, normal_strength=0.1)
unit = P("house_unit", (0.70, 0.70, 0.67), DETAIL, roughness=0.45, metallic=0.2, normal_strength=0.15)
dark = P("house_dark", (0.02, 0.02, 0.02), DETAIL, roughness=0.6, normal_strength=0.1)
room = P("house_room", (0.28, 0.24, 0.20), DETAIL, roughness=0.9, normal_strength=0.2)
curtain = P("house_curtain", (0.62, 0.58, 0.50), DETAIL, roughness=0.95, normal_strength=0.2)
roof = kz.surface_material("apartment_roof_" + SCHEME, "Japanese_Roof_Tiles", tint=ROOF_TINT, desaturate=0.85)
glass = kz.plain_material("kz_glow_glass", (0.025, 0.03, 0.035), roughness=0.04)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)
lamp = kz.plain_material("kz_glow_porch_lamp", (0.85, 0.82, 0.75), roughness=0.3)


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


def outdoor_unit(b, base, t, n, w=0.76, h=0.54, d=0.28):
    t, n, base = Vector(t), Vector(n), Vector(base)
    up = Vector((0, 0, 1))
    c = base + n * (0.06 + d / 2) + up * (0.1 + h / 2)
    obox(b, c, (w, d, h), t, n, unit, 0.018)
    for k in (-1, 1):
        obox(b, base + n * 0.2 + t * (k * w * 0.36) + up * 0.05, (0.05, d, 0.1), t, n, dark, 0.005)
    fc = c + n * (d / 2) + t * (-w * 0.17)
    rcyl(b, fc - n * 0.012, fc + n * 0.002, 0.19, dark, seg=28)
    for k in range(-7, 8):
        z = k * 0.025
        chord = 2 * (max(0.0, 0.19 ** 2 - z ** 2)) ** 0.5
        if chord > 0.02:
            obox(b, fc + up * z + n * 0.006, (chord, 0.006, 0.005), t, n, unit)


walls = kz.MeshBuilder("walls", uv_scale=0.5)
fittings = kz.MeshBuilder("fittings", uv_scale=1.0)
roofing = kz.MeshBuilder("roof", uv_scale=0.5)

# facades as frames (u along, seen from outside, from its left end; d outward)
ENT = (lambda u, d: (u - W / 2, D / 2 + d)) if S > 0 else (lambda u, d: (W / 2 - u, -D / 2 - d))
GAR = (lambda u, d: (W / 2 - u, -D / 2 - d)) if S > 0 else (lambda u, d: (u - W / 2, D / 2 + d))
EAST = lambda u, d: (W / 2 + d, -S * (u - D / 2))
WEST = lambda u, d: (-W / 2 - d, S * (u - D / 2))


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


def sided_wall(F, length, holes):
    """A wall of ceramic siding: the core, the siding courses (455 mm, recessed joints), stopped
    round the openings; the foundation band below."""
    for a, b, za, zb in free_rects(0.0, length, F0, TOP, holes):
        fb(walls, F, a, b, -0.15, 0.0, za, zb, siding)
    z = F0 + 0.455
    while z < TOP - 0.1:
        for a, b, za, zb in free_rects(0.0, length, z - 0.004, z + 0.004, holes):
            fb(walls, F, a, b, -0.005, 0.001, za, zb, trim)
        z += 0.455
    fb(walls, F, -0.02, length + 0.02, -0.02, SIDE_T, 0.0, F0, found)
    fb(fittings, F, -0.02, length + 0.02, 0.0, 0.03, F1 - 0.04, F1 + 0.04, trim)      # the band between the floors


def window(F, u0, u1, z0, z1, g, curtains=True):
    FR = 0.045
    if curtains:
        fb(walls, F, u0, u0 + (u1 - u0) * 0.28, -0.35, -0.33, z0, z1, curtain)
        fb(walls, F, u1 - (u1 - u0) * 0.24, u1, -0.35, -0.33, z0, z1, curtain)
    fb(walls, F, u0 - 0.2, u1 + 0.2, -0.9, -0.85, z0 - 0.2, z1 + 0.2, room)
    fb(fittings, F, u0, u1, -0.07, -0.065, z0, z1, g)
    for (a, bb, za, zb) in ((u0, u1, z0, z0 + FR), (u0, u1, z1 - FR, z1), (u0, u0 + FR, z0, z1), (u1 - FR, u1, z0, z1)):
        fb(fittings, F, a, bb, -0.09, -0.01, za, zb, alu)
    if u1 - u0 > 0.9:
        m = (u0 + u1) / 2
        fb(fittings, F, m - 0.03, m + 0.03, -0.085, -0.02, z0, z1, alu)
    for u in (u0 - 0.002, u1):
        fb(walls, F, u, u + 0.002, -0.15, 0.0, z0, z1, siding)


# ----------------------------------------------------------------------------- the entrance side
# per floor two flats: a door and its kitchen window each, the gas meter box between
ent_holes = []
for fz in (F0, F1):
    for k in range(2):
        u = 1.2 + k * 8.0
        ent_holes += [(u, u + 0.85, fz + 0.02, fz + 2.0), (u + 1.4, u + 2.4, fz + 1.1, fz + 1.8)]
sided_wall(ENT, W, ent_holes)
for fz in (F0, F1):
    for k in range(2):
        u = 1.2 + k * 8.0
        fb(fittings, ENT, u, u + 0.85, -0.06, -0.02, fz + 0.02, fz + 2.0, door_m)                 # the steel door
        for (a, b, za, zb) in ((u - 0.05, u, fz, fz + 2.05), (u + 0.85, u + 0.9, fz, fz + 2.05), (u - 0.05, u + 0.9, fz + 2.0, fz + 2.05)):
            fb(fittings, ENT, a, b, -0.07, 0.01, za, zb, alu)
        fb(fittings, ENT, u + 0.66, u + 0.72, -0.02, 0.03, fz + 0.9, fz + 1.05, silver)            # handle
        fb(fittings, ENT, u + 0.05, u + 0.25, -0.02, 0.0, fz + 1.55, fz + 1.62, silver)            # nameplate
        window(ENT, u + 1.4, u + 2.4, fz + 1.1, fz + 1.8, frosted, curtains=False)
        for z in (fz + 1.12, fz + 1.3, fz + 1.48, fz + 1.66):                                     # the kitchen window's bars
            fb(fittings, ENT, u + 1.38, u + 2.42, 0.02, 0.04, z, z + 0.02, alu)
        fb(fittings, ENT, u + 3.0, u + 3.55, 0.0, 0.25, fz + 0.9, fz + 1.75, unit)                # meter box
        fb(fittings, ENT, u + 0.95, u + 1.1, 0.0, 0.08, fz + 1.9, fz + 2.05, lamp)               # door light
# the walkway upstairs: a slab on steel posts, its solid steel parapet with a top rail; the
# outside stair in front of it at the east end, climbing along the front to a landing past the
# building's end, where the walkway meets it
STEPS = 14
rise = F1 / STEPS
run = 0.24
LAND = (W - 1.1, W)                        # the landing at the walkway's end (within the building's length)
SU0 = LAND[0] - STEPS * run                # the stair's foot (along the front)
fb(fittings, ENT, 0.0, LAND[0], 0.0, GAL, F1 - 0.18, F1, concrete)                                 # walkway
fb(fittings, ENT, LAND[0], LAND[1], 0.0, GAL + 1.05, F1 - 0.18, F1, concrete)                       # landing
fb(fittings, ENT, 0.0, SU0, GAL - 0.05, GAL, F1, F1 + 1.05, steel)                                 # parapet (to the stair)
fb(fittings, ENT, 0.0, SU0, GAL - 0.08, GAL + 0.02, F1 + 1.05, F1 + 1.1, steel)
fb(fittings, ENT, SU0, LAND[0], GAL - 0.04, GAL, F1, F1 + 1.05, steel)                             # (beside the stair too)
fb(fittings, ENT, 0.0, 0.05, 0.0, GAL, F1, F1 + 1.05, steel)                                       # the walkway's far end
fb(fittings, ENT, LAND[1] - 0.05, LAND[1], 0.0, GAL + 1.05, F1, F1 + 1.05, steel)                  # the landing's sides
fb(fittings, ENT, LAND[0], LAND[1], GAL + 1.0, GAL + 1.05, F1, F1 + 1.05, steel)
for u in (0.2, 5.0, 10.0, LAND[1] - 0.12):
    fb(fittings, ENT, u, u + 0.1, GAL - 0.12, GAL - 0.02, 0.0, F1 - 0.18, steel)                    # posts
fb(fittings, ENT, LAND[1] - 0.12, LAND[1] - 0.02, GAL + 0.9, GAL + 1.0, 0.0, F1 - 0.18, steel)
fb(fittings, ENT, 0.0, LAND[0], 0.0, GAL, F1 - 0.22, F1 - 0.18, trim)                              # the soffit edge
for k in range(STEPS):
    u1 = LAND[0] - (STEPS - 1 - k) * run
    fb(fittings, ENT, u1 - run - 0.02, u1, GAL + 0.06, GAL + 1.0, (k + 1) * rise - 0.04, (k + 1) * rise, steel)


def beam(b, p0, p1, w, h, mat):
    """A beam from p0 to p1, w wide and h deep (its depth kept vertical-ish)."""
    p0, p1 = Vector(p0), Vector(p1)
    d = p1 - p0
    rot = Vector((1, 0, 0)).rotation_difference(d.normalized()).to_matrix()
    b.oriented_box((p0 + p1) / 2, (d.length, w, h), rot, mat)


for dd, rail in ((GAL + 0.04, False), (GAL + 1.02, True)):
    a, e = ENT(SU0 - 0.1, dd), ENT(LAND[0], dd)
    beam(fittings, (a[0], a[1], -0.05), (e[0], e[1], F1 - 0.12), 0.05, 0.24, steel)                 # stringer
    if rail:
        beam(fittings, (a[0], a[1], 0.9), (e[0], e[1], F1 + 0.95), 0.04, 0.04, steel)              # handrail
        for t in (0.25, 0.5, 0.75):
            q = ENT(SU0 + t * (LAND[0] - SU0), dd)
            fittings.box((q[0] - 0.02, q[1] - 0.02, t * F1 + 0.05), (q[0] + 0.02, q[1] + 0.02, t * F1 + 0.95), steel)
# mailboxes beside the stair's foot
for k in range(4):
    fb(fittings, ENT, SU0 - 1.6 + k * 0.3, SU0 - 1.32 + k * 0.3, GAL + 0.35, GAL + 0.7, 0.9, 1.25, silver)
fb(fittings, ENT, SU0 - 1.65, SU0 - 0.4, GAL + 0.37, GAL + 0.67, 0.0, 0.9, steel)
fb(walls, ENT, 0.0, LAND[1], 0.0, GAL + 1.1, 0.0, 0.06, concrete)                                       # the paved apron

# ----------------------------------------------------------------------------- the garden side
GT = Vector((GAR(1, 0)[0] - GAR(0, 0)[0], GAR(1, 0)[1] - GAR(0, 0)[1], 0.0))     # along the garden side
GN = Vector((GAR(0, 1)[0] - GAR(0, 0)[0], GAR(0, 1)[1] - GAR(0, 0)[1], 0.0))     # out from it
gar_holes = []
for fz in (F0, F1):
    for k in range(2):
        u = 1.0 + k * 8.0
        gar_holes += [(u, u + 2.6, fz + 0.05, fz + 2.0), (u + 3.6, u + 5.4, fz + 0.8, fz + 1.9)]
sided_wall(GAR, W, gar_holes)
for fz in (F0, F1):
    for k in range(2):
        u = 1.0 + k * 8.0
        window(GAR, u, u + 2.6, fz + 0.05, fz + 2.0, glass)
        window(GAR, u + 3.6, u + 5.4, fz + 0.8, fz + 1.9, glass)
        fb(fittings, GAR, u - 0.1, u + 2.7, 0.0, 0.22, fz + 2.05, fz + 2.25, unit)                 # shutter box
        if fz == F1:
            # the balcony: a slab on brackets, aluminium railing, an AC unit, the partition
            fb(fittings, GAR, u - 0.6, u + 6.0, 0.0, 0.95, F1 - 0.15, F1, concrete)
            fb(fittings, GAR, u - 0.6, u + 6.0, 0.9, 0.95, F1, F1 + 1.1, glass)
            for (a, b, za, zb) in ((u - 0.6, u + 6.0, F1 + 1.05, F1 + 1.12), (u - 0.6, u + 6.0, F1, F1 + 0.05)):
                fb(fittings, GAR, a, b, 0.88, 0.97, za, zb, alu)
            for uu in (u - 0.6, u + 2.7, u + 5.95):
                fb(fittings, GAR, uu, uu + 0.05, 0.88, 0.97, F1, F1 + 1.1, alu)
            for e in (u - 0.6, u + 6.0):
                fb(fittings, GAR, e - 0.04, e, 0.0, 0.95, F1, F1 + 1.1, alu)
            a = GAR(u + 4.2, 0.0)
            outdoor_unit(fittings, Vector((a[0], a[1], F1)), GT, GN)
            for e in (u - 0.5, u + 5.9):
                fb(fittings, GAR, e, e + 0.06, 0.0, 0.1, F1 - 0.55, F1 - 0.15, steel)              # brackets
        else:
            a = GAR(u + 4.4, 0.0)
            outdoor_unit(fittings, Vector((a[0], a[1], 0.0)), GT, GN)

# ----------------------------------------------------------------------------- the ends
for F in (EAST, WEST):
    sided_wall(F, D, [(D / 2 - 0.4, D / 2 + 0.4, F1 + 1.1, F1 + 1.8), (D / 2 - 0.4, D / 2 + 0.4, F0 + 1.1, F0 + 1.8)])
    window(F, D / 2 - 0.4, D / 2 + 0.4, F1 + 1.1, F1 + 1.8, frosted, curtains=False)
    window(F, D / 2 - 0.4, D / 2 + 0.4, F0 + 1.1, F0 + 1.8, frosted, curtains=False)
walls.box((-W / 2 + 0.01, -D / 2 + 0.01, TOP - 0.05), (W / 2 - 0.01, D / 2 - 0.01, TOP), room)

# ----------------------------------------------------------------------------- the roof
rise = (D / 2) * math.tan(PITCH)
RT = 0.12
run_r = D / 2 + EAVE_OV
slope_len = run_r / math.cos(PITCH)
roof_w = W + 2 * VERGE_OV
z_eave = TOP - EAVE_OV * math.tan(PITCH)
for s in (-1, 1):
    rot = Matrix.Rotation(PITCH if s < 0 else -PITCH, 3, "X")
    nrm = rot @ Vector((0, 0, 1))
    c = Vector((0, s * run_r / 2, z_eave + (TOP + rise - z_eave) / 2 + 0.04)) - nrm * (RT / 2)
    roofing.oriented_box(c, (roof_w, slope_len, RT), rot, roof)
    ye = s * run_r
    fittings.box((-roof_w / 2, min(ye, ye - s * 0.035), z_eave - RT - 0.18), (roof_w / 2, max(ye, ye - s * 0.035), z_eave - RT + 0.01), trim)
    gy = ye + s * 0.08
    rcyl(fittings, (-roof_w / 2 + 0.05, gy, z_eave - 0.13), (roof_w / 2 - 0.05, gy, z_eave - 0.13), 0.055, trim, seg=14)
    for x in (-W / 2 + 0.25, W / 2 - 0.25):
        fittings.cylinder((x, s * (D / 2 + 0.09), 0.0), 0.032, z_eave - 0.5, trim, segments=10)
for sx in (-1, 1):
    x = sx * W / 2
    tri = [Vector((x, -D / 2, TOP)), Vector((x, D / 2, TOP)), Vector((x, 0, TOP + rise))]
    walls.quad(tri if sx > 0 else [tri[1], tri[0], tri[2]], siding)
    for s in (-1, 1):
        a = Vector((sx * (roof_w / 2), s * run_r, z_eave + 0.02))
        b = Vector((sx * (roof_w / 2), 0, TOP + rise + 0.05))
        mid, d = (a + b) / 2, b - a
        rot = Matrix.Rotation(math.atan2(d.z, abs(d.y)) * (1 if s < 0 else -1), 3, "X")
        fittings.oriented_box(mid - Vector((0, 0, 0.07)), (0.04, d.length, 0.2), rot, trim)
roofing.box((-roof_w / 2, -0.14, TOP + rise - 0.02), (roof_w / 2, 0.14, TOP + rise + 0.12), roof)

for b in (walls, fittings, roofing):
    b.finish(collection="HOUSE")

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
