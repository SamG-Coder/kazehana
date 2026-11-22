"""Kazehana Junior High's pool (docs/school_plan.md), as one quality-gated building.

    python tools/make_school_textures.py && python tools/make_school_signs.py   (first)
    blender --background --factory-startup --python tools/blender/build_school_pool.py

-> assets/buildings/school_pool/school_pool.gltf

A Japanese school's outdoor pool, raised on its concrete base:
- **The pool:** 25 m and six lanes, its tank 1.1 m deep and painted sky blue (as Japanese school
  pools are: a smooth pool paint, not tiles) with the lanes' narrow navy lines and their T-ends,
  the end walls' targets, the painted coping round the edge.
- **In and over the water:** numbered starting platforms at the west end (a stainless frame,
  its non-slip top sloping to the water, as on school pools), the lane ropes (strings
  of small disc floats, red for 5 m at each end, blue and white between),
  steel ladders at the east end, the water.
- **The deck:** painted green (non-slip), and fenced, with steps up on the north side and their
  handrails, and the pool clock on the fence.
- **The changing block:** at the west end, boys' and girls' rooms with their plates.

Local frame (Blender): origin at ground level in the middle of the pool's footprint, +X east
(along the lanes), +Y south.
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
ASSET_ID = "school_pool"
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")
TEX = os.path.join(ROOT, "assets", "cache", "school")
SIGNS = os.path.join(ROOT, "assets", "cache", "signs")

LX, LY = 12.5, 6.5            # the tank: x -12.5..12.5 (25 m), y -6.5..6.5 (six lanes of 2 m)
DZ = 1.2                       # the deck's top
DEPTH = 1.1
DX0, DX1, DY0, DY1 = -14.0, 14.0, -8.0, 8.0     # the deck
CB = (-19.0, -14.4, -8.0, 8.0)                  # the changing block

kz.reset_scene()
bpy.context.scene.collection.children.link(bpy.data.collections.new("POOL"))
DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
concrete = kz.surface_material("school_concrete", "Precast_Concrete_Wall", tint=(0.92, 0.88, 0.78), desaturate=0.55)
plinth = P("pool_plinth", (0.42, 0.41, 0.39), DETAIL, roughness=0.85, normal_strength=0.8)
lane_m = P("pool_lane_paint", (0.02, 0.05, 0.16), DETAIL, roughness=0.3, normal_strength=0.1, grain=0.05)
steel = P("pool_steel", (0.70, 0.71, 0.72), DETAIL, roughness=0.25, metallic=0.95, normal_strength=0.05, grain=0.05)
fence = P("school_fence", (0.20, 0.36, 0.24), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.1)
white = P("pool_white", (0.90, 0.90, 0.88), DETAIL, roughness=0.35, normal_strength=0.05, grain=0.05)
grate = P("pool_gutter_grate", (0.12, 0.13, 0.13), DETAIL, roughness=0.6, metallic=0.4, normal_strength=0.2)
door_m = P("pool_door", (0.40, 0.46, 0.44), DETAIL, roughness=0.45, metallic=0.5, normal_strength=0.08)
float_cols = [P("pool_float_red", (0.72, 0.07, 0.05), DETAIL, roughness=0.55, normal_strength=0.1),
              P("pool_float_white", (0.88, 0.88, 0.86), DETAIL, roughness=0.55, normal_strength=0.1),
              P("pool_float_blue", (0.05, 0.16, 0.58), DETAIL, roughness=0.55, normal_strength=0.1)]
water = kz.plain_material("pool_water_clear_glass", (0.55, 0.80, 0.86), roughness=0.05)
# the pool's water: one upward face, shaded by the renderer as water (Akari MAT_WATER: refraction,
# reflection, ripples, depth colour and the sun's caustics on the tiles)
water_surface = kz.plain_material("pool_water_surface", (1.0, 1.0, 1.0), roughness=0.0)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)


def tex_material(name, stem, normal_strength=0.8):
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


def image_material(name, path):
    _, n, _ = kz.fab_textures(DETAIL)
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
    nm.inputs["Strength"].default_value = 0.02
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    bsdf.inputs["Roughness"].default_value = 0.3
    return m


# (reference photos of Japanese school pools: the tank painted a light sky blue, the deck a green
# non-slip coat on concrete, the coping a light concrete)
# (the tank: tools/make_pool_textures.py, 8 m a tile; the deck and coping: the Fab scan Concrete
# Floor, 2 m a tile, made ready by tools/prep_fab_textures.py)
pool_tile = tex_material("pool_tank_paint", "pool_paint", normal_strength=0.3)
# the deck: artificial turf (人工芝, as on many school pool decks): the renderer's lawn stands its
# blades on this face in its colour (Akari MAT_LAWN: shaders/lawn.glsl)
deck_tile = kz.plain_material("pool_deck_lawn_surface", (0.11, 0.30, 0.10), roughness=0.9)
coping_m = tex_material("pool_coping", "fab_pool_coping", normal_strength=0.8)
plates = image_material("pool_plates", os.path.join(SIGNS, "school_plates.png"))
clock_mat = image_material("pool_clock_face", os.path.join(SIGNS, "school_clock.png"))
import json  # noqa: E402

cells = json.load(open(os.path.join(SIGNS, "school_plates.json"), encoding="utf8"))

b = kz.MeshBuilder("pool", uv_scale=1.0)
fit = kz.MeshBuilder("fittings", uv_scale=1.0)
tank = kz.MeshBuilder("tank", uv_scale=1 / 8)      # the pool paint (8 m a texture)
deck = kz.MeshBuilder("deck", uv_scale=0.5)        # the deck's coat (2 m a texture)
coping = kz.MeshBuilder("coping", uv_scale=0.5)    # the pool's edge (2 m a texture)
SX0, SX1 = -10.0, -7.6                            # the steps up, along x
DOORS = []
uv_faces = []

# ---- the base and the deck (round the tank), the tank's walls and floor
strips = [(DX0, DX1, DY0, -LY), (DX0, DX1, LY, DY1), (DX0, -LX, -LY, LY), (LX, DX1, -LY, LY)]
for (x0, x1, y0, y1) in strips:
    b.box((x0, y0, 0.0), (x1, y1, DZ - 0.03), concrete)
    deck.box((x0, y0, DZ - 0.03), (x1, y1, DZ), deck_tile)
b.box((-LX, -LY, 0.0), (LX, LY, DZ - DEPTH), concrete)
tank.box((-LX, -LY, DZ - DEPTH), (LX, LY, DZ - DEPTH + 0.02), pool_tile)
for (x0, x1, y0, y1) in ((-LX, LX, -LY, -LY + 0.02), (-LX, LX, LY - 0.02, LY), (-LX, -LX + 0.02, -LY, LY), (LX - 0.02, LX, -LY, LY)):
    tank.box((x0, y0, DZ - DEPTH), (x1, y1, DZ - 0.03), pool_tile)
# the coping round the edge: light concrete, a little proud of the deck, its lip rounded
for (x0, x1, y0, y1) in ((-LX - 0.35, LX + 0.35, -LY - 0.35, -LY), (-LX - 0.35, LX + 0.35, LY, LY + 0.35), (-LX - 0.35, -LX, -LY, LY), (LX, LX + 0.35, -LY, LY)):
    coping.box((x0, y0, DZ - 0.002), (x1, y1, DZ + 0.012), coping_m)
for (a, c) in (((-LX, -LY), (LX, -LY)), ((-LX, LY), (LX, LY)), ((-LX, -LY), (-LX, LY)), ((LX, -LY), (LX, LY))):
    fk.tube(coping, [(a[0], a[1], DZ - 0.016), (c[0], c[1], DZ - 0.016)], 0.028, coping_m, seg=10)
# the lanes' lines on the floor and their targets on the end walls
for k in range(6):
    y = -LY + 0.5 + 1.0 + 2.0 * k
    fit.box((-LX + 2.0, y - 0.1, DZ - DEPTH + 0.02), (LX - 2.0, y + 0.1, DZ - DEPTH + 0.023), lane_m)
    for x in (-LX + 2.0, LX - 2.0):
        fit.box((x - 0.1, y - 0.5, DZ - DEPTH + 0.02), (x + 0.1, y + 0.5, DZ - DEPTH + 0.023), lane_m)
    for (x, s) in ((-LX + 0.02, 1), (LX - 0.02, -1)):
        xs = (x, x + s * 0.005)
        fit.box((min(xs), y - 0.1, DZ - DEPTH + 0.1), (max(xs), y + 0.1, DZ - 0.35), lane_m)
        fit.box((min(xs), y - 0.25, DZ - 0.6), (max(xs), y + 0.25, DZ - 0.45), lane_m)
# the water
WZ = DZ - 0.17
# the waterline round the tank's walls: a band of wet, darker paint just above the water and the
# faint grey tide mark where it laps
wet = P("pool_waterline", (0.16, 0.36, 0.55), DETAIL, roughness=0.12, normal_strength=0.05, grain=0.02)
tide = P("pool_tide_mark", (0.30, 0.38, 0.42), DETAIL, roughness=0.5, normal_strength=0.1, grain=0.08)
for (x0, x1, y0, y1) in ((-LX, LX, -LY + 0.02, -LY + 0.021), (-LX, LX, LY - 0.021, LY - 0.02), (-LX + 0.02, -LX + 0.021, -LY, LY), (LX - 0.021, LX - 0.02, -LY, LY)):
    fit.box((x0, y0, WZ - 0.01), (x1, y1, WZ + 0.035), wet)
    fit.box((x0, y0, WZ + 0.035), (x1, y1, WZ + 0.045), tide)
uv_faces.append(([(-LX + 0.02, -LY + 0.02, WZ), (LX - 0.02, -LY + 0.02, WZ), (LX - 0.02, LY - 0.02, WZ), (-LX + 0.02, LY - 0.02, WZ)], [(0, 0), (1, 0), (1, 1), (0, 1)], water_surface))
# ---- the starting platforms at the west end (as on Japanese school pools: a stainless tube frame
# standing on the coping and the deck, a non-slip top sloping a little down to the water, a
# backstroke grip bar under its front, the lane's number on a plate facing the deck)
top_m = P("pool_start_top", (0.10, 0.36, 0.46), DETAIL, roughness=0.85, normal_strength=1.0, grain=0.2)
X_F, X_R = -LX + 0.03, -LX - 0.52           # the top's front (over the wall) and back
Z_F, Z_R = DZ + 0.43, DZ + 0.48             # its height there
tilt = math.atan2(Z_R - Z_F, X_F - X_R)
for k in range(6):
    y = -LY + 1.5 + 2.0 * k
    # the top: a 50 cm square slab, its front edge flush with the wall
    fit.oriented_box(Vector(((X_F + X_R) / 2, y, (Z_F + Z_R) / 2)), (X_F - X_R, 0.5, 0.035), Matrix.Rotation(tilt, 3, "Y"), top_m)
    for sy in (-1, 1):
        yy = y + sy * 0.21
        # each side's frame: the back leg on the deck, the front leg on the coping, a rail under
        # the top joining them, and a brace low down
        fk.tube(fit, [(X_R + 0.04, yy, DZ), (X_R + 0.04, yy, Z_R - 0.03), (X_F - 0.08, yy, Z_F - 0.03), (X_F - 0.08, yy, DZ)], 0.021, steel, seg=12)
        fk.tube(fit, [(X_R + 0.04, yy, DZ + 0.12), (X_F - 0.08, yy, DZ + 0.12)], 0.016, steel, seg=10)
        for xf in (X_R + 0.04, X_F - 0.08):
            fk.cyl(fit, (xf, yy, DZ), 0.04, 0.008, steel, seg=16)                     # the leg's base plate
    # the backstroke grip: a bar across the front, just under the top
    fk.tube(fit, [(X_F - 0.02, y - 0.24, Z_F - 0.08), (X_F - 0.02, y + 0.24, Z_F - 0.08)], 0.016, steel, seg=10)
    for sy in (-1, 1):
        fk.tube(fit, [(X_F - 0.08, y + sy * 0.21, Z_F - 0.08), (X_F - 0.02, y + sy * 0.24, Z_F - 0.08)], 0.014, steel, seg=8)
    # the number plate between the back legs, facing the deck
    fit.box((X_R + 0.02, y - 0.13, DZ + 0.2), (X_R + 0.035, y + 0.13, DZ + 0.4), white)
    u0, v0, u1, v1 = cells[str(k + 1)]
    x = X_R + 0.019
    uv_faces.append(([(x, y + 0.11, DZ + 0.22), (x, y - 0.11, DZ + 0.22), (x, y - 0.11, DZ + 0.38), (x, y + 0.11, DZ + 0.38)], [(u0, v0), (u1, v0), (u1, v1), (u0, v1)], plates))
for k in range(1, 6):
    y = -LY + 0.5 + 2.0 * k
    # (the rope itself and its floats are drawn by the town, afloat: each float riding the
    # water where it is, the rope sagging between: crates/town/src/lanes.rs; here, its fixings)
    WZ0 = DZ - 0.17
    for xw, sx in ((-LX + 0.02, 1), (LX - 0.02, -1)):
        fit.box((min(xw, xw + sx * 0.008), y - 0.04, WZ0 - 0.04), (max(xw, xw + sx * 0.008), y + 0.04, WZ0 + 0.04), steel)  # the eye plate
        fk.tube(fit, [(xw + sx * 0.008, y, WZ0), (xw + sx * 0.12, y, WZ0)], 0.008, steel, seg=8)  # the hook and tensioner

# the ladders at the east end: rails up from the deck, over the coping, down to the floor
for y in (-LY + 0.4, LY - 0.4):
    for sy in (-0.25, 0.25):
        fk.tube(fit, [(LX + 0.35, y + sy, DZ), (LX + 0.35, y + sy, DZ + 0.75), (LX - 0.1, y + sy, DZ + 0.75), (LX - 0.1, y + sy, DZ - DEPTH + 0.02)], 0.022, steel, seg=12)
        fk.cyl(fit, (LX + 0.35, y + sy, DZ), 0.045, 0.012, steel, seg=16)              # its deck anchor's flange
        fk.cyl(fit, (LX - 0.1, y + sy, DZ - DEPTH + 0.02), 0.04, 0.03, steel, seg=16)   # its foot on the floor
    for z in (DZ - 0.25, DZ - 0.55, DZ - 0.85):
        fk.rbox(fit, (LX - 0.16, y - 0.2, z - 0.02), (LX - 0.04, y + 0.2, z + 0.02), steel, r=0.008, seg=1)

# ---- the lane ropes' floats and rope, as the town strings them (crates/town/src/lanes.rs): a
# plain white rope, and on it separate floats a hand apart, red for 5 m from each end, blue and
# white between in fours (as on a school pool's course rope); lanes.json says where they run
rope_m = P("pool_lane_rope", (0.90, 0.89, 0.85), DETAIL, roughness=0.85, normal_strength=0.3, grain=0.08)
FLOAT_LEN, FLOAT_PITCH = 0.11, 0.26
float_builders = []
for col, mat in (("red", float_cols[0]), ("white", float_cols[1]), ("blue", float_cols[2])):
    fb_ = kz.MeshBuilder("kzfloat_" + col, uv_scale=1.0)
    # (a float: a short barrel with rounded shoulders, the rope through its middle)
    fk.cyl(fb_, (-0.04, 0.0, 0.0), 0.042, 0.08, mat, seg=16, axis=(1, 0, 0))
    for (x0, sx) in ((-0.04, -1), (0.04, 1)):
        fk.cyl(fb_, (x0, 0.0, 0.0), 0.036, sx * 0.01, mat, seg=16, axis=(1, 0, 0))
        fk.cyl(fb_, (x0 + sx * 0.01, 0.0, 0.0), 0.026, sx * 0.005, mat, seg=16, axis=(1, 0, 0))
    float_builders.append(fb_)
ropeb = kz.MeshBuilder("kzrope", uv_scale=1.0)
fk.tube(ropeb, [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)], 0.006, rope_m, seg=8, caps=False)
os.makedirs(os.path.dirname(OUT), exist_ok=True)
with open(os.path.join(os.path.dirname(OUT), "lanes.json"), "w", encoding="utf8") as f:
    json.dump({"water_z": DZ - 0.17, "pitch": FLOAT_PITCH, "float_len": FLOAT_LEN, "red_m": 5.0, "group": 4,
               "ropes": [{"y": -LY + 0.5 + 2.0 * k, "x0": -LX + 0.13, "x1": LX - 0.13} for k in range(1, 6)]}, f, indent=1)

# ---- the base's sides, the steps up (north side)
for (x0, x1, y0, y1) in ((DX0, DX1, DY0 - 0.01, DY0), (DX0, DX1, DY1, DY1 + 0.01)):
    b.box((x0, y0, 0.0), (x1, y1, 0.3), plinth)
n_steps = 7
for k in range(n_steps):
    z = DZ * (k + 1) / n_steps
    y1 = DY0 - (n_steps - 1 - k) * 0.3
    fit.box((SX0, DY0 - n_steps * 0.3, 0.0), (SX1, y1, z), concrete) if k == 0 else fit.box((SX0, y1 - 0.3, 0.0), (SX1, y1, z), concrete)
    fit.box((SX0, y1 - 0.3, z - 0.01), (SX1, y1 - 0.24, z + 0.004), grate)
for x in (SX0 + 0.05, SX1 - 0.05):
    # (the handrail up the steps carries on over the deck and turns down into it: no loose end)
    fk.tube(fit, [(x, DY0 - n_steps * 0.3 - 0.1, 0.9), (x, DY0, DZ + 0.9), (x, DY0 + 0.6, DZ + 0.9), (x, DY0 + 0.6, DZ)], 0.022, steel, seg=10)
    fk.cyl(fit, (x, DY0 + 0.6, DZ), 0.045, 0.012, steel, seg=16)
    for t in (0.0, 0.5, 1.0):
        yy = DY0 - n_steps * 0.3 + t * n_steps * 0.3
        fk.tube(fit, [(x, yy, DZ * t), (x, yy, DZ * t + 0.9)], 0.02, steel, seg=8)
# ---- the fence round the deck (open at the steps), its posts, the pool clock on a post
FH = 1.8
segs = [((DX0, DY0), (SX0, DY0)), ((SX1, DY0), (DX1, DY0)), ((DX1, DY0), (DX1, DY1)), ((DX1, DY1), (DX0, DY1)), ((DX0, DY1), (DX0, DY0))]
for (a, c) in segs:
    a, c = Vector(a + (DZ,)), Vector(c + (DZ,))
    L = (c - a).length
    n = max(1, int(L / 2.0))
    for k in range(n + 1):
        p = a + (c - a) * (k / n)
        fit.box((p.x - 0.035, p.y - 0.035, DZ), (p.x + 0.035, p.y + 0.035, DZ + FH + 0.05), fence)
    for z in (0.1, FH * 0.5, FH):
        fk.tube(fit, [a + Vector((0, 0, z)), c + Vector((0, 0, z))], 0.02, fence, seg=8)
    nb = int(L / 0.15)
    for k in range(nb):
        p = a + (c - a) * ((k + 0.5) / nb)
        fit.box((p.x - 0.004, p.y - 0.004, DZ + 0.1), (p.x + 0.004, p.y + 0.004, DZ + FH), fence)
cx, cy, cz, r = DX1 - 0.02, 0.0, DZ + FH + 0.55, 0.3
fit.box((DX1 - 0.04, -0.05, DZ), (DX1 + 0.04, 0.05, cz), fence)
fit.box((cx - 0.02, -0.34, cz - 0.34), (cx + 0.06, 0.34, cz + 0.34), white)
for k in range(32):
    a0, a1 = 2 * math.pi * k / 32, 2 * math.pi * (k + 1) / 32
    p0 = (cx - 0.021, cy + math.cos(a0) * r, cz + math.sin(a0) * r)
    p1 = (cx - 0.021, cy + math.cos(a1) * r, cz + math.sin(a1) * r)
    uv_faces.append(([(cx - 0.021, cy, cz), p0, p1], [(0.5, 0.5), (0.5 + 0.5 * math.cos(a0), 0.5 + 0.5 * math.sin(a0)), (0.5 + 0.5 * math.cos(a1), 0.5 + 0.5 * math.sin(a1))], clock_mat))

# ---- the changing block: boys' and girls' rooms, doors on the north, a flat roof, the plates
x0, x1, y0, y1 = CB
H = 3.0
for (a0, a1, c0, c1) in ((x0, x1, y0, y0 + 0.2), (x0, x1, y1 - 0.2, y1), (x0, x0 + 0.2, y0, y1), (x1 - 0.2, x1, y0, y1)):
    b.box((a0, c0, 0.0), (a1, c1, H), concrete)
b.box((x0 - 0.3, y0 - 0.3, H), (x1 + 0.3, y1 + 0.3, H + 0.25), concrete)
b.box((x0 + 0.2, -0.1, 0.0), (x1 - 0.2, 0.1, H), concrete)
coping.box((x0 + 0.2, y0 + 0.2, 0.0), (x1 - 0.2, y1 - 0.2, 0.05), coping_m)   # (the changing rooms: a concrete floor)
# doors (one each, on the west side of the north end of each room) and high windows
for (yc, name) in ((-4.0, "男子更衣室"), (4.0, "女子更衣室")):
    # its door on the block's west face; the plate over it
    b.box((x0 - 0.01, yc - 0.5, 0.0), (x0, yc + 0.5, 2.0), door_m)
    fit.box((x0 - 0.05, yc + 0.35, 0.9), (x0 - 0.01, yc + 0.42, 1.2), steel)
    u0, v0, u1, v1 = cells[name]
    uv_faces.append(([(x0 - 0.02, yc + 0.55, 2.15), (x0 - 0.02, yc - 0.55, 2.15), (x0 - 0.02, yc - 0.55, 2.43), (x0 - 0.02, yc + 0.55, 2.43)], [(u0, v0), (u1, v0), (u1, v1), (u0, v1)], plates))
    for yy in (yc - 2.5, yc + 1.5):
        fit.box((x1 + 0.001, yy, 2.1), (x1 + 0.01, yy + 1.0, 2.6), frosted)
# the plate on the fence by the steps: プール
u0, v0, u1, v1 = cells["プール"]
uv_faces.append(([(SX0 - 1.3, DY0 - 0.04, DZ + 1.2), (SX0 - 0.1, DY0 - 0.04, DZ + 1.2), (SX0 - 0.1, DY0 - 0.04, DZ + 1.5), (SX0 - 1.3, DY0 - 0.04, DZ + 1.5)], [(u0, v0), (u1, v0), (u1, v1), (u0, v1)], plates))
fit.box((SX0 - 1.32, DY0 - 0.035, DZ + 1.18), (SX0 - 0.08, DY0 - 0.02, DZ + 1.52), white)   # (its plain back)

# ---- finish
bm = bmesh.new()
uvl = bm.loops.layers.uv.new()
mats = []
for corners, uvs, mat in uv_faces:
    if mat not in mats:
        mats.append(mat)
    f = bm.faces.new([bm.verts.new(c) for c in corners])
    f.material_index = mats.index(mat)
    for lp, uv in zip(f.loops, uvs):
        lp[uvl].uv = uv
me = bpy.data.meshes.new("faces")
bm.to_mesh(me)
bm.free()
for m in mats:
    me.materials.append(m)
kz.coll("POOL").objects.link(bpy.data.objects.new("faces", me))
for bb in (b, fit, tank, deck, coping, ropeb, *float_builders):
    bb.finish(collection="POOL")
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
