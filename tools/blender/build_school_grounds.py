"""Kazehana Junior High's grounds (docs/school_plan.md): the gate, the fence, and the yard's and
sports ground's fittings, as one quality-gated level piece in the site's own frame.

    python tools/make_school_signs.py   (the name plate, first)
    blender --background --factory-startup --python tools/blender/build_school_grounds.py

-> assets/buildings/school_grounds/school_grounds.gltf

From the town plan (assets/town/town.json: the site's outline, its buildings and the gate):
- **Gate and fence:** the gate, with two granite-faced pillars, the school's bronze name plate,
  and the steel accordion gate (伸縮門) folded open against the west pillar. The fence is a green
  welded-mesh fence on a concrete curb right round the site, open at the gate.
- **The north side of the sports ground:** three flagpoles (the national flag, the prefecture's,
  the school's) on their base; the green steel assembly platform (朝礼台) with its steps, facing
  the ground; a drinking-water station (a concrete trough with upturned taps); benches.
- **The sports ground:** a 200 m track (four lanes, its white lines, the start and finish), two
  soccer goals inside it, the baseball backstop in the north-west corner, a row of iron bars
  (鉄棒), the long-jump pit with its runway.
- **Bikes:** a bike shed along the north strip east of the gate, its roof on steel posts, the
  racks.

Local frame (Blender): origin on the ground at the site's north-west corner, +Y towards the
sports ground (south), +X west (so the frame is the world's turned, not mirrored). The town places
it on the site (plots.rs).
"""

import json
import math
import os
import sys

import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fixture_kit as fk  # noqa: E402
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ASSET_ID = "school_grounds"
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")
SIGNS = os.path.join(ROOT, "assets", "cache", "signs")

town = json.load(open(os.path.join(ROOT, "assets", "town", "town.json"), encoding="utf8"))
site = next(s for s in town["sites"] if "Junior High" in s["name"])
bld = {b["name"]: b for b in site["buildings"]}
q = [Vector(p) for p in bld["school building"]["poly"]]
E = (q[1] - q[0]).normalized()
S = (q[3] - q[0]).normalized()
O = q[0] - E * 8.0 - S * 10.0


def local(p):
    """A plan point in the piece's frame. (+X runs *west*: a frame with +Y south and +Z up is a
    rotation of the world only if its +X is west; the town turns +Y to the site's south.)"""
    d = Vector(p) - O
    return Vector((-d.dot(E), d.dot(S)))


SITE = [local(p) for p in site["poly"]]
GATE = local(town["school_gate"])
GA = GATE.x
GROUND = [local(p) for p in bld["sports ground"]["poly"]]
GC = sum(GROUND, Vector((0.0, 0.0))) / 4          # the sports ground's middle

kz.reset_scene()
bpy.context.scene.collection.children.link(bpy.data.collections.new("GROUNDS"))
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
concrete = kz.surface_material("school_concrete", "Precast_Concrete_Wall", tint=(0.92, 0.88, 0.78), desaturate=0.55)
curb = P("grounds_curb", (0.55, 0.54, 0.50), DETAIL, roughness=0.85, normal_strength=0.7)
granite = kz.surface_material("grounds_granite", "Japanese_Shrine_Stone_Floor", tint=(0.55, 0.53, 0.52), desaturate=0.6)
fence = P("school_fence", (0.20, 0.36, 0.24), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.1)
steel = P("grounds_steel", (0.62, 0.63, 0.62), DETAIL, roughness=0.35, metallic=0.85, normal_strength=0.05, grain=0.05)
green = P("grounds_green_steel", (0.16, 0.42, 0.30), DETAIL, roughness=0.5, metallic=0.4, normal_strength=0.1)
white = P("grounds_white", (0.92, 0.92, 0.90), DETAIL, roughness=0.5, normal_strength=0.05, grain=0.05)
chalk = P("grounds_chalk_line", (0.93, 0.93, 0.90), "Concrete_Floor-f4b85cb0", roughness=0.95, normal_strength=0.3, grain=0.4)
wood = P("grounds_bench_wood", (0.45, 0.32, 0.20), "Wood_Planks", roughness=0.6, normal_strength=0.4)
sand = kz.surface_material("grounds_sand", "Beach_Sand", tint=(0.85, 0.80, 0.70))
roof = P("grounds_shed_roof", (0.55, 0.60, 0.58), DETAIL, roughness=0.45, metallic=0.5, normal_strength=0.2)
dark = P("grounds_dark", (0.05, 0.05, 0.05), DETAIL, roughness=0.6, normal_strength=0.1)
_, _n, _ = kz.fab_textures(DETAIL)
name_mat = bpy.data.materials.new("grounds_name_plate")
name_mat.use_nodes = True
_t = name_mat.node_tree.nodes.new("ShaderNodeTexImage")
_t.image = kz._image(os.path.join(SIGNS, "school_name.png"), "sRGB")
name_mat.node_tree.links.new(_t.outputs["Color"], name_mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"])
_tn = name_mat.node_tree.nodes.new("ShaderNodeTexImage")
_tn.image = kz._image(_n, "Non-Color")
_nm = name_mat.node_tree.nodes.new("ShaderNodeNormalMap")
_nm.inputs["Strength"].default_value = 0.1
name_mat.node_tree.links.new(_tn.outputs["Color"], _nm.inputs["Color"])
name_mat.node_tree.links.new(_nm.outputs["Normal"], name_mat.node_tree.nodes["Principled BSDF"].inputs["Normal"])
name_mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.25

b = kz.MeshBuilder("grounds", uv_scale=1.0)
fit = kz.MeshBuilder("fittings", uv_scale=1.0)
UV = []
V3 = lambda v, z=0.0: Vector((v.x, v.y, z))

# ---- the fence: right round the site on a concrete curb, open at the gate
FH = 1.8
GAP = (GA - 4.6, GA + 4.6)                          # (x: the gate's place along the north edge)
edges = []
for i in range(len(SITE)):
    a, c = SITE[i], SITE[(i + 1) % len(SITE)]
    edges.append((a, c))
for (a, c) in edges:
    segs = [(a, c)]
    d = c - a
    if abs(d.y) < 0.2 * abs(d.x) and min(a.y, c.y) < 5.0:        # the north edge: the gate's gap
        t0 = (GAP[0] - a.x) / d.x
        t1 = (GAP[1] - a.x) / d.x
        t0, t1 = sorted((t0, t1))
        segs = [(a, a + d * t0), (a + d * t1, c)]
    for (p0, p1) in segs:
        L = (p1 - p0).length
        if L < 0.5:
            continue
        ang = math.atan2((p1 - p0).y, (p1 - p0).x)
        mid = (p0 + p1) / 2
        R = Matrix.Rotation(ang, 3, "Z")
        b.oriented_box(V3(mid, 0.1), (L, 0.2, 0.3), R, curb)
        n = max(1, int(L / 2.5))
        for k in range(n + 1):
            p = p0 + (p1 - p0) * (k / n)
            fit.box((p.x - 0.03, p.y - 0.03, 0.2), (p.x + 0.03, p.y + 0.03, 0.25 + FH), fence)
        for z in (0.3, 0.25 + FH * 0.5, 0.25 + FH):
            fk.tube(fit, [V3(p0, z), V3(p1, z)], 0.018, fence, seg=6)
        nb = int(L / 0.15)
        for k in range(nb):
            p = p0 + (p1 - p0) * ((k + 0.5) / nb)
            fit.box((p.x - 0.004, p.y - 0.004, 0.3), (p.x + 0.004, p.y + 0.004, 0.25 + FH), fence)

# ---- the gate: pillars, the name plate, the accordion gate folded open
for x in (GAP[0] - 0.35, GAP[1] + 0.35):
    b.box((x - 0.35, -0.35, 0.0), (x + 0.35, 0.35, 1.9), granite)
    b.box((x - 0.4, -0.4, 1.9), (x + 0.4, 0.4, 2.0), granite)
px = GAP[1] + 0.35
# (the plate lies across the pillar's road face, rotated to run downwards: written top to bottom
# as on a Japanese school's gate)
pz0, pz1 = 0.25, 1.75
UV.append(([(px + 0.27, -0.352, pz0), (px - 0.27, -0.352, pz0), (px - 0.27, -0.352, pz1), (px + 0.27, -0.352, pz1)],
           [(0.0, 1.0), (0.0, 1 - 288 / 1024), (1.0, 1 - 288 / 1024), (1.0, 1.0)], name_mat))
fit.box((px - 0.3, -0.36, pz0 - 0.03), (px + 0.3, -0.351, pz1 + 0.03), dark)
# the accordion gate folded against the west pillar, on its wheels, its rail across the gap
x0 = GAP[0] + 0.1
for k in range(14):
    x = x0 + k * 0.06
    for s_ in (-1, 1):
        fk.tube(fit, [(x, 0.0, 0.15), (x + 0.03, 0.0 + s_ * 0.0, 1.35)], 0.012, steel, seg=6, caps=False)
    fk.tube(fit, [(x, -0.05, 0.15), (x + 0.03, 0.05, 1.35)], 0.008, steel, seg=6, caps=False)
fk.tube(fit, [(x0, 0.0, 1.38), (x0 + 0.9, 0.0, 1.38)], 0.02, steel, seg=8)
fk.tube(fit, [(x0, 0.0, 0.15), (x0 + 0.9, 0.0, 0.15)], 0.02, steel, seg=8)
for x in (x0 + 0.1, x0 + 0.8):
    fk.cyl(fit, (x, -0.03, 0.06), 0.06, 0.06, dark, seg=12, axis=(0, 1, 0))
b.box((GAP[0], -0.04, 0.0), (GAP[1], 0.04, 0.01), steel)

# ---- the north side of the ground: flagpoles, the assembly platform, the drinking taps, benches
NB = min(p.y for p in GROUND)                     # the ground's north edge
FX = GC.x - 22.0
b.box((FX - 2.5, NB - 6.0, 0.0), (FX + 2.5, NB - 4.6, 0.25), concrete)
for k, x in enumerate((FX - 1.6, FX, FX + 1.6)):
    h = 10.0 if k == 1 else 9.0
    fit.cylinder((x, NB - 5.3, 0.25), 0.06, h, steel, segments=16)
    fit.cylinder((x, NB - 5.3, 0.25 + h), 0.09, 0.12, steel, segments=16)
    fk.tube(fit, [(x + 0.08, NB - 5.3, 1.2), (x + 0.08, NB - 5.3, h + 0.2)], 0.004, white, seg=4)
# the assembly platform (朝礼台): a green steel deck on legs, steps up behind it, a rail
PX, PY = GC.x, NB - 2.0
fk.rbox(fit, (PX - 1.2, PY - 1.2, 1.15), (PX + 1.2, PY + 1.2, 1.25), green, r=0.02)
for sx in (-1, 1):
    for sy in (-1, 1):
        fit.box((PX + sx * 1.1 - 0.05, PY + sy * 1.1 - 0.05, 0.0), (PX + sx * 1.1 + 0.05, PY + sy * 1.1 + 0.05, 1.15), green)
for k in range(4):
    z = 0.29 * (k + 1)
    y0 = PY - 1.2 - (4 - k) * 0.28
    fit.box((PX - 0.5, y0, z - 0.04), (PX + 0.5, y0 + 0.28, z), green)
for sx in (-1, 1):
    fk.tube(fit, [(PX + sx * 0.5, PY - 2.4, 0.9), (PX + sx * 0.5, PY - 1.2, 2.1), (PX + sx * 0.5, PY - 1.0, 2.1)], 0.025, green, seg=8)
for (a, c) in (((PX - 1.2, PY + 1.2), (PX + 1.2, PY + 1.2)), ((PX - 1.2, PY - 1.2), (PX - 1.2, PY + 1.2)), ((PX + 1.2, PY - 1.2), (PX + 1.2, PY + 1.2))):
    fk.tube(fit, [(a[0], a[1], 2.05), (c[0], c[1], 2.05)], 0.025, green, seg=8)
    for t in (0.0, 1.0):
        p = Vector(a).lerp(Vector(c), t)
        fk.tube(fit, [(p.x, p.y, 1.25), (p.x, p.y, 2.05)], 0.02, green, seg=6)
# the drinking-water station: a long concrete trough with upturned taps
WX = GC.x - 45.0
fk.rbox(fit, (WX - 2.0, NB - 3.6, 0.0), (WX + 2.0, NB - 3.0, 0.75), concrete, r=0.03)
fk.basin_bowl(fit, (WX, NB - 3.3, 0.75), 3.7, 0.4, 0.12, steel, e_top=12, e_bottom=6, flange=0.01, n=40)
for k in range(6):
    x = WX - 1.6 + k * 0.64
    fk.cyl(fit, (x, NB - 3.52, 0.75), 0.02, 0.1, steel, seg=10)
    fk.tube(fit, [(x, NB - 3.52, 0.85), (x, NB - 3.42, 0.9), (x, NB - 3.36, 0.88)], 0.009, steel, seg=8)
# benches along the ground's north edge
for x in (GC.x - 60, GC.x - 35, GC.x + 30, GC.x + 55):
    fk.rbox(fit, (x - 1.0, NB - 1.6, 0.42), (x + 1.0, NB - 1.25, 0.47), wood, r=0.01)
    for sx in (-0.85, 0.85):
        fit.box((x + sx - 0.04, NB - 1.55, 0.0), (x + sx + 0.04, NB - 1.3, 0.42), concrete)

# ---- the sports ground: its edge, the track, goals, the backstop, iron bars, the jump pit
for i in range(4):
    a, c = GROUND[i], GROUND[(i + 1) % 4]
    L = (c - a).length
    b.oriented_box(V3((a + c) / 2, 0.03), (L, 0.15, 0.08), Matrix.Rotation(math.atan2((c - a).y, (c - a).x), 3, "Z"), curb)
STRAIGHT = 40.0
R0 = 16.0
LANE = 1.25


def oval_point(t, r):
    """A point round the track at distance t (0..1 of the way) at radius r: the straights
    along x."""
    L = 2 * STRAIGHT + 2 * math.pi * r
    s_ = t * L
    if s_ < STRAIGHT:
        return Vector((GC.x - STRAIGHT / 2 + s_, GC.y - r))
    s_ -= STRAIGHT
    if s_ < math.pi * r:
        a = -math.pi / 2 + s_ / r
        return Vector((GC.x + STRAIGHT / 2 + math.cos(a) * r, GC.y + math.sin(a) * r))
    s_ -= math.pi * r
    if s_ < STRAIGHT:
        return Vector((GC.x + STRAIGHT / 2 - s_, GC.y + r))
    s_ -= STRAIGHT
    a = math.pi / 2 + s_ / r
    return Vector((GC.x - STRAIGHT / 2 + math.cos(a) * r, GC.y + math.sin(a) * r))


for k in range(5):
    r = R0 + k * LANE
    n = 240
    for i in range(n):
        p0, p1 = oval_point(i / n, r), oval_point((i + 1) / n, r)
        L = (p1 - p0).length
        fit.oriented_box(V3((p0 + p1) / 2, 0.004), (L + 0.01, 0.05, 0.01), Matrix.Rotation(math.atan2((p1 - p0).y, (p1 - p0).x), 3, "Z"), chalk)
for x in (GC.x - STRAIGHT / 2 + 1.0, GC.x + STRAIGHT / 2 - 1.0):          # the start and finish lines
    fit.box((x - 0.025, GC.y - R0 - 4 * LANE, 0.0), (x + 0.025, GC.y - R0, 0.009), chalk)
for sx in (-1, 1):                                                       # the soccer goals
    gx = GC.x + sx * (STRAIGHT / 2 + R0 - 2.0)
    for y in (GC.y - 3.66, GC.y + 3.66):
        fk.tube(fit, [(gx, y, 0.0), (gx, y, 2.44)], 0.06, white, seg=12)
        fk.tube(fit, [(gx, y, 2.44), (gx + sx * 1.0, y, 2.0), (gx + sx * 2.0, y, 0.0)], 0.03, white, seg=8)
        fk.tube(fit, [(gx, y, 0.0), (gx + sx * 2.0, y, 0.0)], 0.03, white, seg=8)
    fk.tube(fit, [(gx, GC.y - 3.66, 2.44), (gx, GC.y + 3.66, 2.44)], 0.06, white, seg=12)
    fk.tube(fit, [(gx + sx * 2.0, GC.y - 3.66, 0.0), (gx + sx * 2.0, GC.y + 3.66, 0.0)], 0.03, white, seg=8)
    fk.tube(fit, [(gx + sx * 1.0, GC.y - 3.66, 2.0), (gx + sx * 1.0, GC.y + 3.66, 2.0)], 0.03, white, seg=8)
    for k in range(25):                                                   # its net, as cords
        y = GC.y - 3.66 + k * 7.32 / 24
        fk.tube(fit, [(gx, y, 2.44), (gx + sx * 1.0, y, 2.0), (gx + sx * 2.0, y, 0.0)], 0.004, white, seg=4, caps=False)
    for k in range(9):
        t = k / 8
        fk.tube(fit, [(gx + sx * 1.0 * t, GC.y - 3.66, 2.44 - 0.44 * t), (gx + sx * 1.0 * t, GC.y + 3.66, 2.44 - 0.44 * t)], 0.004, white, seg=4, caps=False)
# the baseball backstop in the north-west corner: three panels of net on tall posts
BX, BY = min(p.x for p in GROUND) + 14.0, NB + 10.0
panels = [(-50, 6.0), (0, 8.0), (50, 6.0)]
for (deg, w) in panels:
    centre = Vector((BX, BY)) + Vector((math.sin(math.radians(deg)), -math.cos(math.radians(deg)))) * 6.0
    t = Vector((math.cos(math.radians(deg)), math.sin(math.radians(deg))))
    p0, p1 = centre - t * w / 2, centre + t * w / 2
    for p in (p0, p1):
        fit.cylinder((p.x, p.y, 0.0), 0.08, 8.0, green, segments=12)
    for z in (0.3, 2.5, 5.0, 8.0):
        fk.tube(fit, [V3(p0, z), V3(p1, z)], 0.04, green, seg=8)
    nb = int(w / 0.3)
    for k in range(nb + 1):
        p = p0 + (p1 - p0) * (k / nb)
        fit.box((p.x - 0.006, p.y - 0.006, 0.3), (p.x + 0.006, p.y + 0.006, 8.0), green)
    for k in range(26):
        z = 0.3 + k * 0.3
        fk.tube(fit, [V3(p0, z), V3(p1, z)], 0.005, green, seg=4, caps=False)
# the iron bars (鉄棒): five bars of rising heights on posts, at the ground's east edge
IX, IY = max(p.x for p in GROUND) - 8.0, NB + 4.0
heights = [0.9, 1.1, 1.3, 1.5, 1.8]
for k, h in enumerate(heights):
    y = IY + k * 1.6
    fk.tube(fit, [(IX, y, 0.0), (IX, y, h + 0.1)], 0.05, steel, seg=12)
    if k == len(heights) - 1:
        fk.tube(fit, [(IX, y + 1.6, 0.0), (IX, y + 1.6, h + 0.1)], 0.05, steel, seg=12)
    fk.tube(fit, [(IX, y, h), (IX, y + 1.6, h)], 0.017, steel, seg=10)
# the long-jump pit and its runway, by the south edge
JX, JY = GC.x + 40.0, max(p.y for p in GROUND) - 8.0
b.box((JX, JY - 1.4, -0.05), (JX + 8.0, JY + 1.4, 0.02), sand)
for (y0, y1) in ((JY - 1.5, JY - 1.4), (JY + 1.4, JY + 1.5)):
    b.box((JX - 0.1, y0, 0.0), (JX + 8.1, y1, 0.08), concrete)
for (x0, x1) in ((JX - 0.1, JX), (JX + 8.0, JX + 8.1)):
    b.box((x0, JY - 1.5, 0.0), (x1, JY + 1.5, 0.08), concrete)
fit.box((JX - 1.2, JY - 0.6, 0.0), (JX - 1.0, JY + 0.6, 0.01), white)                       # the take-off board
for y in (JY - 0.6, JY + 0.6):
    fit.box((JX - 40.0, y - 0.025, 0.0), (JX - 1.0, y + 0.025, 0.008), chalk)

# ---- the bike shed east of the gate, along the north strip
SX0, SX1 = GA + 12.0, GA + 34.0
SY0, SY1 = 8.5, 13.0
for x in [SX0 + k * (SX1 - SX0) / 5 for k in range(6)]:
    for (y, h) in ((SY0, 2.6), (SY1, 2.2)):
        fit.box((x - 0.05, y - 0.05, 0.0), (x + 0.05, y + 0.05, h), green)
fit.oriented_box(Vector(((SX0 + SX1) / 2, (SY0 + SY1) / 2, 2.45)), (SX1 - SX0 + 0.6, math.hypot(SY1 - SY0 + 0.6, 0.4), 0.04), Matrix.Rotation(-math.atan2(0.4, SY1 - SY0), 3, "X"), roof)
for k in range(int((SX1 - SX0) / 0.25)):
    x = SX0 + 0.12 + k * 0.25
    fit.oriented_box(Vector((x, (SY0 + SY1) / 2, 2.48)), (0.03, math.hypot(SY1 - SY0 + 0.6, 0.4), 0.03), Matrix.Rotation(-math.atan2(0.4, SY1 - SY0), 3, "X"), roof)
b.box((SX0 - 0.3, SY0 - 0.3, 0.0), (SX1 + 0.3, SY1 + 0.3, 0.05), concrete)
for k in range(int((SX1 - SX0) / 0.6)):                                    # the racks
    x = SX0 + 0.4 + k * 0.6
    fk.tube(fit, [(x, SY1 - 0.3, 0.05), (x, SY1 - 0.3, 0.45), (x, SY1 - 0.9, 0.45), (x, SY1 - 0.9, 0.05)], 0.015, steel, seg=6, caps=False)

# ---- finish
import bmesh  # noqa: E402

bm = bmesh.new()
uvl = bm.loops.layers.uv.new()
mats = []
for corners, uvs, mat in [u for u in UV if u]:
    if mat not in mats:
        mats.append(mat)
    f = bm.faces.new([bm.verts.new(c) for c in corners])
    f.material_index = mats.index(mat)
    for lp, uv in zip(f.loops, uvs):
        lp[uvl].uv = uv
me = bpy.data.meshes.new("plates")
bm.to_mesh(me)
bm.free()
for m in mats:
    me.materials.append(m)
kz.coll("GROUNDS").objects.link(bpy.data.objects.new("plates", me))
for bb in (b, fit):
    bb.finish(collection="GROUNDS")
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT, "gate at", round(GA, 1))
