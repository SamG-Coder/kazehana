"""The hamlets' farmhouses (農家), as quality-gated assets: one storey under a big irimoya
(hip-and-gable) roof of kawara tiles, 15 x 10 m, as the plan's farmhouse footprint.

    blender --background --factory-startup --python tools/blender/build_farmhouse.py -- <scheme> [layout]
    (scheme "a" silver-grey ibushi tiles and white plaster, "b" glazed brown tiles and cream
     plaster; layout "n" the entrance to the north, "s" on the garden side, "e" on the east end)

-> assets/buildings/farmhouse[_b][_s|_e]/farmhouse[...].gltf

After reference photos of rural houses in the Japanese countryside: a raised timber floor on a
stone-faced foundation, the walls white (or cream) plaster over dark stained board panels
(koshiita) between exposed dark posts; the entrance (genkan) with its two-leaf glazed sliding
doors in wooden frames and a step of stone; on the garden side the engawa, a wooden veranda the
length of the main rooms behind glass sliding doors, under deep eaves, the storm-shutter boxes
(tobukuro) at its ends; the irimoya roof: hipped below, a small gable at each end of the ridge
with its dark board face and a ventilation lattice, the ridge and hips capped in rounded tiles,
the eaves lined with round-ended tiles. Local frame as the houses: origin at ground level in the
middle of the footprint, +X along the long side, the garden side -Y.
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
ASSET_ID = "farmhouse" + ("" if SCHEME == "a" else "_" + SCHEME) + ("" if LAYOUT == "n" else "_" + LAYOUT)
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")
SCHEMES = {
    # plaster, roof tint (multiplies the kawara scan; None its own grey)
    "a": ((0.80, 0.79, 0.75), (0.52, 0.54, 0.58)),
    "b": ((0.74, 0.68, 0.56), (0.36, 0.22, 0.15)),
}
PLASTER, ROOF_TINT = SCHEMES[SCHEME]

W, D = 15.0, 10.0
F0 = 0.55                     # the raised floor
TOP = F0 + 2.75               # the wall plate
PITCH = math.radians(28.0)
EOV = 0.95                    # deep eaves (over the engawa)
ENG = 0.9                     # the engawa's depth (inside the footprint, under the eaves)

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)

DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
plaster = P("farmhouse_plaster_" + SCHEME, PLASTER, DETAIL, roughness=0.92, normal_strength=0.35, grain=0.3)
board = P("farmhouse_board", (0.07, 0.045, 0.03), "Wood_Planks", roughness=0.6, normal_strength=0.8)
post = P("farmhouse_post", (0.09, 0.06, 0.04), "Wood_Planks", roughness=0.55, normal_strength=0.6)
deck = P("farmhouse_engawa", (0.30, 0.21, 0.13), "Wood_Planks", roughness=0.45, normal_strength=0.5)
stone = kz.surface_material("farmhouse_stone", "Japanese_Concrete_Block_Wall", tint=(0.55, 0.53, 0.50), desaturate=0.6)
found = P("house_foundation", (0.36, 0.35, 0.33), DETAIL, roughness=0.85, normal_strength=0.8)
alu = P("house_aluminium", (0.09, 0.075, 0.065), DETAIL, roughness=0.35, metallic=0.8, normal_strength=0.1)
gutter = P("farmhouse_gutter", (0.12, 0.10, 0.08), DETAIL, roughness=0.45, metallic=0.4, normal_strength=0.1)
dark = P("house_dark", (0.02, 0.02, 0.02), DETAIL, roughness=0.6, normal_strength=0.1)
room = P("house_room", (0.28, 0.24, 0.20), DETAIL, roughness=0.9, normal_strength=0.2)
shoji = P("farmhouse_shoji", (0.82, 0.80, 0.72), DETAIL, roughness=0.95, normal_strength=0.1, grain=0.2)
unit = P("house_unit", (0.70, 0.70, 0.67), DETAIL, roughness=0.45, metallic=0.2, normal_strength=0.15)
roof = kz.surface_material("farmhouse_roof_" + SCHEME, "Japanese_Roof_Tiles", tint=ROOF_TINT, desaturate=0.85 if SCHEME == "a" else 0.4)
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


walls = kz.MeshBuilder("walls", uv_scale=0.5)
fittings = kz.MeshBuilder("fittings", uv_scale=1.0)
roofing = kz.MeshBuilder("roof", uv_scale=0.5)

# facades as frames: (u along the face seen from outside, from its left end; d outward)
NORTH = lambda u, d: (W / 2 - u, D / 2 + d)
SOUTH = lambda u, d: (u - W / 2, -D / 2 - d)
EAST = lambda u, d: (W / 2 + d, u - D / 2)
WEST = lambda u, d: (-W / 2 - d, D / 2 - u)
LEN = {id(NORTH): W, id(SOUTH): W, id(EAST): D, id(WEST): D}


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


KOSHI = F0 + 0.9               # the board panels' top


def facade(F, holes, inset=0.0):
    """A wall: dark board panels to KOSHI, plaster above, dark posts every 1.82 m (and at the
    openings), a head rail (nageshi) under the eaves, the stone-faced foundation."""
    L = LEN[id(F)]
    for a, b, za, zb in free_rects(0.0, L, F0, TOP, holes):
        fb(walls, F, a, b, -0.16 - inset, -inset, za, zb, plaster)
        if za < KOSHI:
            fb(walls, F, a, b, -inset, 0.025 - inset, za, min(zb, KOSHI), board)
            u = a + 0.12
            while u < b - 0.02:
                fb(walls, F, u, u + 0.03, 0.025 - inset, 0.04 - inset, za, min(zb, KOSHI), board)     # battens
                u += 0.3
    us = {0.0, L} | {h[0] for h in holes} | {h[1] for h in holes}
    u = 1.82
    while u < L - 0.3:
        if not any(h[0] - 0.2 < u < h[1] + 0.2 for h in holes):
            us.add(u)
        u += 1.82
    for u in sorted(us):
        fb(walls, F, u - 0.06, u + 0.06, -inset, 0.03 - inset, F0, TOP, post)
    fb(walls, F, -0.05, L + 0.05, -inset, 0.04 - inset, TOP - 0.22, TOP, post)                     # head rail
    fb(walls, F, -0.05, L + 0.05, -inset, 0.05 - inset, F0 - 0.05, F0 + 0.04, post)                # sill beam
    fb(walls, F, -0.05, L + 0.05, -0.2 - inset, 0.06 - inset, 0.0, F0 - 0.05, stone)               # foundation


def sash(F, u0, u1, z0, z1, g, bars=0, wood=True, inset=0.0):
    """Glazed sliding doors / a window in wooden frames (two leaves, bars across)."""
    fr = post if wood else alu
    fb(walls, F, u0 - 0.2, u1 + 0.2, -1.0 - inset, -0.95 - inset, z0 - 0.2, z1 + 0.2, room)
    fb(fittings, F, u0, u1, -0.06 - inset, -0.055 - inset, z0, z1, g)
    for (a, b, za, zb) in ((u0, u1, z0, z0 + 0.05), (u0, u1, z1 - 0.05, z1), (u0, u0 + 0.05, z0, z1), (u1 - 0.05, u1, z0, z1)):
        fb(fittings, F, a, b, -0.08 - inset, -0.01 - inset, za, zb, fr)
    m = (u0 + u1) / 2
    fb(fittings, F, m - 0.03, m + 0.03, -0.075 - inset, -0.02 - inset, z0, z1, fr)
    for k in range(1, bars + 1):
        z = z0 + k * (z1 - z0) / (bars + 1)
        fb(fittings, F, u0, u1, -0.075 - inset, -0.045 - inset, z - 0.012, z + 0.012, fr)


# ----------------------------------------------------------------------------- the layout
# which facade has the entrance (the road side), which the engawa (the garden side, -Y)
ENT = {"n": NORTH, "s": SOUTH, "e": EAST}[LAYOUT]
holes = {id(F): [] for F in (NORTH, SOUTH, EAST, WEST)}
# the engawa: along the garden side's main rooms, behind glass sliding doors (set back ENG)
ENG_U = (1.2, W - 1.2) if LAYOUT != "s" else (1.2, W - 5.0)
# the genkan: two-leaf glazed sliding doors, 1.8 m, its lamp and step
GEN_U = {"n": (W - 4.6, W - 2.8), "s": (W - 3.8, W - 2.0), "e": (3.6, 5.4)}[LAYOUT]
holes[id(ENT)].append((GEN_U[0], GEN_U[1], F0 - 0.4, F0 + 1.85))
# windows: the kitchen and bath at the back (north) corner, the side rooms at the ends
wins = {id(NORTH): [(1.0, 2.6, F0 + 0.9, F0 + 1.9, frosted, 2), (3.4, 5.2, F0 + 0.9, F0 + 1.9, glass, 1), (7.0, 8.8, F0 + 0.9, F0 + 1.9, glass, 1)],
        id(EAST): [(1.2, 3.0, F0 + 0.9, F0 + 1.9, glass, 1), (6.6, 8.4, F0 + 0.9, F0 + 1.9, glass, 1)],
        id(WEST): [(1.4, 3.2, F0 + 0.9, F0 + 1.9, glass, 1), (6.4, 8.2, F0 + 0.9, F0 + 1.9, frosted, 2)],
        id(SOUTH): []}
if LAYOUT == "s":
    wins[id(SOUTH)].append((W - 1.6, W - 0.6, F0 + 1.1, F0 + 1.8, frosted, 2))
    wins[id(NORTH)] = [(1.0, 2.6, F0 + 0.9, F0 + 1.9, frosted, 2), (4.0, 6.6, F0 + 0.9, F0 + 1.9, glass, 1), (8.4, 11.0, F0 + 0.9, F0 + 1.9, glass, 1), (12.4, 14.0, F0 + 0.9, F0 + 1.9, glass, 1)]
if LAYOUT == "e":
    wins[id(EAST)] = [(7.0, 8.8, F0 + 0.9, F0 + 1.9, glass, 1)]
for F in (NORTH, SOUTH, EAST, WEST):
    for w in wins[id(F)]:
        if F is ENT and w[1] > GEN_U[0] - 0.4 and w[0] < GEN_U[1] + 0.4:
            continue
        holes[id(F)].append(w[:4])
holes[id(SOUTH)].append((ENG_U[0], ENG_U[1], F0, TOP - 0.22))
for F in (NORTH, EAST, WEST):
    facade(F, holes[id(F)])
# the garden side: the wall stands back behind the engawa
facade(SOUTH, [h for h in holes[id(SOUTH)] if h[0] != ENG_U[0]] + [(ENG_U[0], ENG_U[1], F0, TOP)])
fb(walls, SOUTH, ENG_U[0], ENG_U[1], -0.16 - ENG, -ENG, TOP - 0.45, TOP, plaster)                # the transom wall over the doors, set back
for e in (ENG_U[0], ENG_U[1] - 0.16):
    fb(walls, SOUTH, e, e + 0.16, -ENG - 0.16, 0.0, F0, TOP, plaster)                               # the engawa's end walls
for F in (NORTH, SOUTH, EAST, WEST):
    for w in wins[id(F)]:
        if w[:4] in holes[id(F)]:
            sash(F, w[0], w[1], w[2], w[3], w[4], bars=w[5])
# the engawa: the glass doors set back, the deck, its posts at the eave line, the shutter boxes
n_doors = max(4, int((ENG_U[1] - ENG_U[0]) / 1.8))
dw = (ENG_U[1] - ENG_U[0]) / n_doors
for k in range(n_doors):
    u0 = ENG_U[0] + k * dw
    sash(SOUTH, u0, u0 + dw, F0, TOP - 0.45, glass, bars=1, inset=ENG)
    fb(walls, SOUTH, u0, u0 + dw, -ENG - 1.6, -ENG - 1.55, F0, TOP - 0.45, shoji)                  # shoji screens behind
fb(fittings, SOUTH, ENG_U[0] - 0.1, ENG_U[1] + 0.1, -ENG, 0.05, F0 - 0.06, F0, deck)
u = ENG_U[0] + 0.05
while u < ENG_U[1]:
    fb(fittings, SOUTH, u, u + 0.005, -ENG, 0.05, F0, F0 + 0.003, dark)                             # board joints
    u += 0.15
for u in (ENG_U[0], (ENG_U[0] + ENG_U[1]) / 2, ENG_U[1]):
    fb(fittings, SOUTH, u - 0.06, u + 0.06, 0.0, 0.12, 0.0, TOP, post)                                # the eave-line posts
fb(fittings, SOUTH, ENG_U[0] - 0.1, ENG_U[1] + 0.1, 0.0, 0.12, TOP - 0.22, TOP, post)              # the beam over them
fb(fittings, SOUTH, ENG_U[0] - 0.4, ENG_U[1] + 0.4, 0.05, 0.6, 0.0, 0.25, stone)                    # the stepping stone (kutsunugi-ishi) row
for e in (ENG_U[0] - 0.75, ENG_U[1] + 0.1):
    fb(fittings, SOUTH, e, e + 0.65, -ENG, -ENG + 0.35, F0 + 0.1, TOP - 0.3, board)                  # tobukuro
# the genkan: sliding doors in wooden frames, the step, the lamp, a little canopy
sash(ENT, GEN_U[0], GEN_U[1], F0 - 0.4, F0 + 1.85, frosted, bars=3)
fb(fittings, ENT, GEN_U[0] - 0.5, GEN_U[1] + 0.5, 0.0, 0.8, 0.0, 0.16, stone)
fb(fittings, ENT, GEN_U[0] - 0.3, GEN_U[1] + 0.3, -0.2, 0.0, 0.0, F0 - 0.4, stone)
fb(fittings, ENT, GEN_U[1] + 0.2, GEN_U[1] + 0.35, 0.0, 0.08, F0 + 1.95, F0 + 2.15, lamp)
fb(fittings, ENT, GEN_U[0] - 0.6, GEN_U[1] + 0.6, 0.0, 1.0, F0 + 2.2, F0 + 2.28, board)
for e in (GEN_U[0] - 0.55, GEN_U[1] + 0.5):
    fb(fittings, ENT, e, e + 0.05, 0.0, 0.95, F0 + 2.0, F0 + 2.22, post)
# the floor and the top (closed)
walls.box((-W / 2 + 0.05, -D / 2 + 0.05, TOP - 0.05), (W / 2 - 0.05, D / 2 - 0.05, TOP), room)
# an air-conditioner unit and the propane cylinders at the back
a = NORTH(10.5, 0.0)
obox(fittings, Vector((a[0], a[1] + 0.25, 0.4)), (0.8, 0.28, 0.56), (1, 0, 0), (0, 1, 0), unit, 0.018)
for k in range(2):
    c = NORTH(12.3 + k * 0.45, 0.0)
    fittings.cylinder((c[0], c[1] + 0.25, 0.0), 0.18, 1.2, unit, segments=20)

# ----------------------------------------------------------------------------- the irimoya roof
RT = 0.16
xe, ye = W / 2 + EOV, D / 2 + EOV
ze = TOP - EOV * math.tan(PITCH) + 0.05
tp = math.tan(PITCH)
zr = ze + ye * tp                               # the ridge
yg = ye * 0.42                                  # the little gable's half width
xg = xe - (ye - yg)                             # where it stands (hips at 45 degrees in plan)
zg = ze + (ye - yg) * tp
rb = kz.MeshBuilder("irimoya", uv_scale=0.5)
re = kz.MeshBuilder("irimoya_ends", uv_scale=0.5)     # (the hipped ends: their tiles turned to run down the slope)


def slab(poly, mat, rb=rb):
    """A roof face (its top) and its underside RT below."""
    rb.quad([Vector(v) for v in poly], mat) if len(poly) == 4 else None
    if len(poly) != 4:
        vs = [rb.bm.verts.new(Vector(v)) for v in poly]
        f = rb.bm.faces.new(vs)
        f.material_index = rb._mat_index(mat)
    vs = [rb.bm.verts.new(Vector(v) - Vector((0, 0, RT))) for v in reversed(poly)]
    f = rb.bm.faces.new(vs)
    f.material_index = rb._mat_index(mat)


for s in (-1, 1):   # the long slopes (to the garden, to the back)
    poly = [(-xe, s * ye, ze), (xe, s * ye, ze), (xg, s * yg, zg), (xg, 0, zr), (-xg, 0, zr), (-xg, s * yg, zg)]
    slab(poly if s < 0 else list(reversed(poly)), roof)
for sx in (-1, 1):  # the hipped ends
    poly = [(sx * xe, -ye, ze), (sx * xe, ye, ze), (sx * xg, yg, zg), (sx * xg, -yg, zg)]
    slab(poly if sx > 0 else list(reversed(poly)), roof, re)
    # the little gable: dark boards, a ventilation lattice, its barge boards
    tri = [(sx * xg, -yg, zg), (sx * xg, yg, zg), (sx * xg, 0, zr)]
    vs = [rb.bm.verts.new(Vector(v)) for v in (tri if sx > 0 else [tri[1], tri[0], tri[2]])]
    f = rb.bm.faces.new(vs)
    f.material_index = rb._mat_index(board)
    for k in range(-3, 4):
        y = k * yg * 0.22
        h = (zr - zg) * (1 - abs(y) / yg) * 0.55
        fittings.box((sx * xg - 0.02, y - 0.015, zg + 0.15), (sx * xg + 0.02, y + 0.015, zg + 0.15 + h), post)
    for sy in (-1, 1):
        a, b = Vector((sx * (xg + 0.05), sy * (yg + 0.12), zg - 0.06)), Vector((sx * (xg + 0.05), 0, zr + 0.08))
        mid, d = (a + b) / 2, b - a
        fittings.oriented_box(mid, (0.06, d.length, 0.24), Matrix.Rotation(math.atan2(d.z, abs(d.y)) * (-sy), 3, "X"), post)
# eave boards, round-ended eave tiles, the ridge and the hips capped
for (p0, p1, out) in (((-xe, -ye), (xe, -ye), (0, -1)), ((-xe, ye), (xe, ye), (0, 1)), ((-xe, -ye), (-xe, ye), (-1, 0)), ((xe, -ye), (xe, ye), (1, 0))):
    L = math.dist(p0, p1)
    t = ((p1[0] - p0[0]) / L, (p1[1] - p0[1]) / L)
    c = ((p0[0] + p1[0]) / 2 + out[0] * 0.02, (p0[1] + p1[1]) / 2 + out[1] * 0.02)
    obox(fittings, (c[0], c[1], ze - RT - 0.09), (L + 0.04, 0.05, 0.2), (t[0], t[1], 0), (out[0], out[1], 0), board)
    nk = int(L / 0.3)
    for k in range(nk):
        q = (p0[0] + t[0] * (k + 0.5) * L / nk, p0[1] + t[1] * (k + 0.5) * L / nk)
        tip = Vector((q[0] + out[0] * 0.03, q[1] + out[1] * 0.03, ze + 0.02))
        rcyl(roofing, tip - Vector((out[0] * 0.13, out[1] * 0.13, -0.05)), tip, 0.075, roof, seg=12)
    # gutters along the long sides
    if out[1] != 0:
        rcyl(fittings, (p0[0] + 0.1, p0[1] + out[1] * 0.1, ze - 0.18), (p1[0] - 0.1, p1[1] + out[1] * 0.1, ze - 0.18), 0.06, gutter, seg=14)
        for x in (p0[0] + 0.3, p1[0] - 0.3):
            fittings.cylinder((x, p0[1] + out[1] * 0.1, 0.0), 0.035, ze - 0.2, gutter, segments=10)
rcyl(roofing, (-xg - 0.1, 0, zr + 0.08), (xg + 0.1, 0, zr + 0.08), 0.17, roof, seg=16)              # ridge
roofing.box((-xg - 0.05, -0.12, zr - 0.05), (xg + 0.05, 0.12, zr + 0.1), roof)
for sx in (-1, 1):
    for sy in (-1, 1):
        rcyl(roofing, (sx * xe, sy * ye, ze + 0.06), (sx * xg, sy * yg, zg + 0.06), 0.11, roof, seg=12)    # hips
        rcyl(roofing, (sx * xg, sy * yg, zg + 0.06), (sx * xg, 0, zr + 0.06), 0.09, roof, seg=12)          # along the gable's foot
    # the ridge ends' ornamental tile (onigawara): a raised block
    roofing.box((sx * xg - 0.08, -0.22, zr - 0.05), (sx * xg + 0.08, 0.22, zr + 0.42), roof)

for b in (walls, fittings, roofing, rb):
    b.finish(collection="HOUSE")
ends = re.finish(collection="HOUSE")
for l in ends.data.uv_layers.active.data:
    u, v = l.uv
    l.uv = (v, u)

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
