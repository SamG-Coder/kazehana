"""Build the 'Path to School' level in Blender (headless) and save assets/levels/path/path.blend.

    blender.exe --background --factory-startup --python tools/blender/build_path.py

The script is the source of the level: re-run it after changing the layout, or open the saved
.blend and edit by hand (then export with export_level.py). Blender axes: +X east, +Y north
(towards school), +Z up. Tsukumodō (home) is at the south end, y ~ -6; the school gate at
y ~ 206.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "assets", "levels", "path")
rng = random.Random(2026)

ROAD_W = 6.0          # asphalt width (x -3..3)
Y0, Y1 = -8.0, 212.0  # road extent


def mats():
    M = {}
    M["asphalt"] = kz.surface_material("asphalt", "Asphalt_Road")
    M["concrete"] = kz.surface_material("concrete", "Precast_Concrete_Wall")
    M["plaster"] = kz.surface_material("plaster", "Precast_Concrete_Wall", tint=(1.0, 0.95, 0.86))
    M["roof"] = kz.surface_material("roof_tiles", "Japanese_Roof_Tiles")
    M["wood"] = kz.surface_material("wood", "Wood_Planks")
    M["wood_dark"] = kz.surface_material("wood_dark", "Wooden_Planks")
    M["stone"] = kz.surface_material("stone_wall", "Mossy_Stone_Wall")
    M["shrine_stone"] = kz.surface_material("shrine_stone", "Japanese_Shrine_Stone_Floor")
    M["grass"] = kz.surface_material("grass", "Wet_Grass_Ground")
    M["mud"] = kz.surface_material("paddy_mud", "Soil_Mud")
    M["levee"] = kz.surface_material("levee", "Sparse_Grassy_Soil")
    M["riverbed"] = kz.surface_material("riverbed", "Mossy_Riverbed")
    M["ballast"] = kz.surface_material("ballast", "Icelandic_Gravel")
    M["forest"] = kz.surface_material("forest", "Mossy_Forest_Floor", tint=(0.55, 0.7, 0.5))
    M["water"] = kz.plain_material("water", (0.015, 0.03, 0.03), roughness=0.04)
    M["paint_white"] = kz.painted_material("paint_white", (0.85, 0.85, 0.82), "Asphalt_Road", roughness=0.55, normal_strength=0.3)
    M["vermilion"] = kz.painted_material("vermilion", (0.62, 0.08, 0.03), "Wood_Planks", roughness=0.5, normal_strength=0.6)
    M["black_paint"] = kz.painted_material("black_paint", (0.02, 0.02, 0.02), "Wood_Planks", roughness=0.45, normal_strength=0.5)
    M["yellow_paint"] = kz.painted_material("yellow_paint", (0.8, 0.6, 0.02), "Precast_Concrete_Wall", roughness=0.5, normal_strength=0.3)
    M["steel"] = kz.painted_material("steel", (0.55, 0.56, 0.58), "Precast_Concrete_Wall", roughness=0.35, metallic=1.0, normal_strength=0.15)
    M["galvanised"] = kz.painted_material("galvanised", (0.62, 0.63, 0.62), "Precast_Concrete_Wall", roughness=0.5, metallic=1.0, normal_strength=0.2)
    M["aluminium"] = kz.painted_material("aluminium", (0.75, 0.76, 0.77), "Precast_Concrete_Wall", roughness=0.3, metallic=1.0, normal_strength=0.1)
    M["ac_unit"] = kz.painted_material("ac_unit", (0.78, 0.78, 0.74), "Precast_Concrete_Wall", roughness=0.5, normal_strength=0.2)
    M["rubber"] = kz.painted_material("rubber", (0.03, 0.03, 0.03), "Asphalt_Road", roughness=0.9, normal_strength=0.3)
    # kz_glow_*: emission set by the game per frame (windows light up in the evening)
    M["window"] = kz.plain_material("kz_glow_window", (0.03, 0.035, 0.04), roughness=0.05, emission=(1.0, 0.78, 0.5), emission_strength=0.0)
    M["shop_glass"] = kz.plain_material("kz_neon_shopfront", (0.9, 0.95, 1.0), roughness=0.1, emission=(0.95, 0.97, 1.0), emission_strength=1.0)
    M["sign_teal"] = kz.plain_material("kz_neon_sign_teal", (0.1, 0.55, 0.5), roughness=0.3, emission=(0.1, 0.8, 0.7), emission_strength=1.0)
    M["sign_orange"] = kz.plain_material("kz_neon_sign_orange", (0.9, 0.45, 0.1), roughness=0.3, emission=(1.0, 0.55, 0.15), emission_strength=1.0)
    M["lamp"] = kz.plain_material("kz_lamp_head", (0.9, 0.92, 0.95), roughness=0.2, emission=(0.92, 0.95, 1.0), emission_strength=0.0)
    M["signal_red"] = kz.plain_material("kz_neon_signal_red", (0.6, 0.02, 0.02), roughness=0.3, emission=(1.0, 0.05, 0.02), emission_strength=0.3)
    return M


# ----------------------------------------------------------------------------- ground & road

def ground(M):
    g = kz.MeshBuilder("ground", uv_scale=0.5)
    # ground tiles around the road; the paddy and river areas are cut out
    for x0 in range(-160, 160, 20):
        for y0 in range(-60, 280, 20):
            x1, y1 = x0 + 20, y0 + 20
            if x0 >= -4 and x1 <= 4:
                continue  # road corridor (separate)
            if 72 <= y0 < 150 and (x0 >= 6 or x1 <= -6) and abs(x0 + 10) < 110:
                continue  # paddies
            if 150 <= y0 < 170:
                continue  # river band
            g.quad([(x0, y0, 0), (x1, y0, 0), (x1, y1, 0), (x0, y1, 0)], M["grass"])
    # verges between the gutters and the plots (x 3.5..4)
    for s in (-1, 1):
        x0, x1 = (3.5, 4.0) if s > 0 else (-4.0, -3.5)
        g.quad([(x0, Y0, 0), (x1, Y0, 0), (x1, 150, 0), (x0, 150, 0)], M["grass"])
        g.quad([(x0, 170, 0), (x1, 170, 0), (x1, Y1, 0), (x0, Y1, 0)], M["grass"])
    g.finish()

    r = kz.MeshBuilder("road", uv_scale=0.5)
    for (y0, y1) in ((Y0, 150.5), (169.5, Y1)):
        r.quad([(-3, y0, 0.0), (3, y0, 0.0), (3, y1, 0.0), (-3, y1, 0.0)], M["asphalt"])
        # concrete gutters either side (U-channel tops)
        r.box((-3.5, y0, -0.05), (-3.0, y1, 0.02), M["concrete"])
        r.box((3.0, y0, -0.05), (3.5, y1, 0.02), M["concrete"])
    # white edge lines (dashed in the residential part like a narrow local road)
    y = Y0
    while y < Y1:
        if not (150.5 < y < 169.5):
            for x in (-2.75, 2.65):
                r.box((x, y, 0.0), (x + 0.1, min(y + 4.0, Y1), 0.006), M["paint_white"])
        y += 4.0 if y < 70 else 4.0
    # 止まれ-style stop line before the level crossing
    r.box((-3.0, 172.2, 0.0), (0.0, 172.6, 0.006), M["paint_white"])
    r.finish()


def gutter_covers(A):
    # concrete gutter cover blocks along the road (asset), every 0.6 m near houses
    y = -6.0
    while y < 68.0:
        for x, rot in ((-3.25, 90), (3.25, 90)):
            kz.place(A["gutter"], (x, y, 0.0), rot)
        y += 0.56


# ----------------------------------------------------------------------------- houses

def house(M, cx, cy, w, d, floors, face, gate_gap=True):
    """A rural two-storey Japanese house: plaster walls on a dark wood base, kawara tile gable
    roof, aluminium sash windows, a sliding entrance with a small canopy, an AC unit outside.
    `face` = +1 if the front faces east (+X), -1 west."""
    b = kz.MeshBuilder(f"house_{cx:.0f}_{cy:.0f}", uv_scale=0.5)
    h = 2.8 * floors
    x0, x1, y0, y1 = cx - w / 2, cx + w / 2, cy - d / 2, cy + d / 2
    # foundation, walls
    b.box((x0 - 0.05, y0 - 0.05, 0.0), (x1 + 0.05, y1 + 0.05, 0.4), M["concrete"])
    b.box((x0, y0, 0.4), (x1, y1, 1.3), M["wood_dark"])
    b.box((x0, y0, 1.3), (x1, y1, h), M["plaster"])
    if floors > 1:
        b.box((x0 - 0.06, y0 - 0.06, 2.75), (x1 + 0.06, y1 + 0.06, 2.95), M["wood_dark"])  # floor band
    # gable roof, ridge along Y
    over = 0.7
    pitch = math.radians(rng.uniform(24, 30))
    half = w / 2 + over
    slab_len = half / math.cos(pitch)
    for s in (-1, 1):
        rot = Matrix.Rotation(s * pitch, 3, "Y")
        center = Vector((cx + s * half / 2, cy, h + half * math.tan(pitch) / 2 + 0.05))
        b.oriented_box(center, (slab_len + 0.05, d + 2 * over, 0.14), rot, M["roof"])
    ridge_z = h + half * math.tan(pitch) + 0.08
    b.box((cx - 0.18, y0 - over, ridge_z - 0.12), (cx + 0.18, y1 + over, ridge_z + 0.12), M["roof"])
    # gable end boards
    for yy in (y0 - 0.02, y1 + 0.02):
        b.box((x0, yy - 0.01, h), (x1, yy + 0.01, h + 0.15), M["wood_dark"])
    # windows on the street side and the gable ends
    fx = x1 if face > 0 else x0
    for f in range(floors):
        z = 0.95 + f * 2.8
        for k in (-0.28, 0.28):
            yy = cy + k * d
            b.box((fx - 0.04, yy - 0.75, z), (fx + 0.04, yy + 0.75, z + 1.15), M["window"])
            b.box((fx - 0.06 * face - 0.03, yy - 0.8, z - 0.05), (fx + 0.03, yy + 0.8, z), M["aluminium"])  # sill
            b.box((fx - 0.05, yy - 0.02, z), (fx + 0.05, yy + 0.02, z + 1.15), M["aluminium"])  # mullion
            # (outer frame)
            b.box((fx - 0.05, yy - 0.8, z + 1.15), (fx + 0.05, yy + 0.8, z + 1.2), M["aluminium"])
    for yy in (y0, y1):
        for f in range(floors):
            z = 1.0 + f * 2.8
            b.box((cx - 0.6, yy - 0.04, z), (cx + 0.6, yy + 0.04, z + 0.9), M["window"])
            b.box((cx - 0.65, yy - 0.05, z - 0.05), (cx + 0.65, yy + 0.05, z), M["aluminium"])
    # entrance: sliding door + canopy
    ey = cy - 0.1 * d
    b.box((fx - 0.05, ey - 0.85, 0.4), (fx + 0.05, ey + 0.85, 2.2), M["wood"])
    b.box((fx - 0.06, ey - 0.02, 0.4), (fx + 0.06, ey + 0.02, 2.2), M["aluminium"])
    b.box((fx, ey - 1.1, 2.35), (fx + face * 0.9, ey + 1.1, 2.45), M["roof"]) if face > 0 else \
        b.box((fx - 0.9, ey - 1.1, 2.35), (fx, ey + 1.1, 2.45), M["roof"])
    # step
    b.box((min(fx, fx + face * 0.8), ey - 0.9, 0.0), (max(fx, fx + face * 0.8), ey + 0.9, 0.2), M["concrete"])
    # AC outdoor unit on the side wall
    ay = y1 + 0.35
    b.box((cx + 0.5, ay - 0.15, 0.1), (cx + 1.3, ay + 0.15, 0.7), M["ac_unit"])
    b.box((cx + 0.6, ay + 0.151, 0.2), (cx + 1.0, ay + 0.16, 0.6), M["rubber"])
    b.finish()
    kz.box_marker("collider", (x0 - 0.1, y0 - 0.1, 0), (x1 + 0.1, y1 + 0.1, h), name="house")


def plot_wall(M, x, y0, y1, gate=None, height=1.2):
    """Concrete block wall along a plot front at x, from y0 to y1, with an optional gate gap."""
    b = kz.MeshBuilder(f"wall_{x:.0f}_{y0:.0f}", uv_scale=0.5)
    segs = [(y0, y1)] if gate is None else [(y0, gate - 1.0), (gate + 1.0, y1)]
    for a, c in segs:
        if c - a < 0.3:
            continue
        b.box((x - 0.08, a, 0.0), (x + 0.08, c, height), M["concrete"])
        b.box((x - 0.11, a, height), (x + 0.11, c, height + 0.06), M["concrete"])  # coping
        kz.box_marker("collider", (x - 0.15, a, 0), (x + 0.15, c, height), name="wall")
    b.finish()


def residential(M, A):
    # west side plots (fronts face east), east side plots (fronts face west)
    y = 0.0
    west = [(10.0, 9.0, 2), (9.0, 8.5, 1), (11.0, 9.5, 2), (8.5, 8.0, 2)]
    for i, (w, d, fl) in enumerate(west):
        cy = y + d / 2 + 1.0
        cx = -4.5 - 3.0 - w / 2
        if 44 <= cy <= 64:
            y += d + 3.0
            continue  # the konbini lot
        house(M, cx, cy, w, d, fl, +1)
        plot_wall(M, -4.3, y, y + d + 2.0, gate=cy - 0.1 * d)
        kz.place(A["mailbox_wood"], (-4.55, cy - 0.1 * d + 1.3, 0.8), -90)
        y += d + 3.0
    y = 2.0
    east = [(9.0, 9.0, 2), (10.0, 8.0, 2), (8.0, 8.0, 1), (10.0, 9.0, 2), (9.0, 8.5, 2), (9.5, 9.0, 2)]
    for i, (w, d, fl) in enumerate(east):
        cy = y + d / 2 + 1.0
        if cy > 70:
            break
        cx = 4.5 + 3.0 + w / 2
        house(M, cx, cy, w, d, fl, -1)
        plot_wall(M, 4.3, y, y + d + 2.0, gate=cy - 0.1 * d)
        kz.place(A["post_box"], (4.55, cy - 0.1 * d + 1.3, 0.9), 90)
        if i % 2 == 0:
            kz.place(A["street_box"], (4.45, y + 1.0, 0.3), 90)
        y += d + 3.0


def konbini(M):
    b = kz.MeshBuilder("konbini", uv_scale=0.5)
    x0, x1, y0, y1 = -24.0, -11.5, 46.0, 62.0
    b.box((x0, y0, 0), (x1, y1, 3.9), M["plaster"])
    b.box((x0 - 0.2, y0 - 0.2, 3.9), (x1 + 0.2, y1 + 0.2, 4.15), M["concrete"])  # parapet
    # glass shop front (always lit) with aluminium mullions
    b.box((x1 - 0.02, y0 + 1.0, 0.2), (x1 + 0.03, y1 - 1.0, 2.7), M["shop_glass"])
    yy = y0 + 1.0
    while yy <= y1 - 1.0:
        b.box((x1 + 0.02, yy - 0.04, 0.2), (x1 + 0.08, yy + 0.04, 2.7), M["aluminium"])
        yy += 2.0
    # fascia sign: generic stripes (no real brand)
    b.box((x1 + 0.02, y0 - 0.2, 2.9), (x1 + 0.1, y1 + 0.2, 3.25), M["sign_teal"])
    b.box((x1 + 0.02, y0 - 0.2, 3.25), (x1 + 0.1, y1 + 0.2, 3.45), M["sign_orange"])
    b.box((x1 + 0.02, y0 - 0.2, 3.45), (x1 + 0.1, y1 + 0.2, 3.75), M["sign_teal"])
    b.finish()
    kz.box_marker("collider", (x0, y0, 0), (x1 + 0.1, y1, 4), name="konbini")
    # car park with bay lines and wheel stops
    p = kz.MeshBuilder("konbini_parking", uv_scale=0.5)
    p.quad([(-11.5, 44.0, 0.005), (-4.0, 44.0, 0.005), (-4.0, 64.0, 0.005), (-11.5, 64.0, 0.005)], M["asphalt"])
    for k in range(7):
        yy = 46.0 + k * 2.6
        p.box((-11.0, yy, 0.005), (-6.0, yy + 0.1, 0.012), M["paint_white"])
        p.box((-10.4, yy + 0.8, 0.0), (-10.25, yy + 1.8, 0.12), M["concrete"])
    p.finish()
    kz.light((-9.0, 54.0, 2.6), (0.95, 0.97, 1.0), 0.02, radius=1.2, range_m=14.0, night_only=False, name="konbini_glow")


# ----------------------------------------------------------------------------- paddies

def paddies(M):
    for side in (-1, 1):
        y = 74.0
        while y < 148.0:
            y1 = min(y + 22.0, 148.0)
            xa, xb = (6.0, 100.0) if side > 0 else (-100.0, -6.0)
            if side < 0 and 100 <= y <= 124:
                # the shrine approach crosses the west paddies
                pass
            b = kz.MeshBuilder(f"paddy_{side}_{y:.0f}", uv_scale=0.5)
            b.quad([(xa, y, -0.35), (xb, y, -0.35), (xb, y1, -0.35), (xa, y1, -0.35)], M["mud"])
            b.quad([(xa, y, -0.12), (xb, y, -0.12), (xb, y1, -0.12), (xa, y1, -0.12)], M["water"])
            # levees around each field
            for (a, c) in ((y - 0.5, y), (y1, y1 + 0.5)):
                b.box((xa, a, -0.4), (xb, c, 0.12), M["levee"])
            edge = xa if side > 0 else xb
            b.box((edge - 0.5, y, -0.4), (edge, y1, 0.12) if side > 0 else (edge + 0.5, y1, 0.12), M["levee"]) if side > 0 else \
                b.box((edge, y, -0.4), (edge + 0.5, y1, 0.12), M["levee"])
            b.finish()
            y = y1 + 0.5
        # keep players on the road/levee strip
        x0, x1 = (6.0, 100.0) if side > 0 else (-100.0, -6.0)
        if side < 0:
            kz.box_marker("collider", (x0, 74.0, -1), (x1, 104.0, 2), name="paddy")
            kz.box_marker("collider", (x0, 116.0, -1), (x1, 148.5, 2), name="paddy")
            kz.box_marker("collider", (x0, 104.0, -1), (-44.0, 116.0, 2), name="paddy")
        else:
            kz.box_marker("collider", (x0, 74.0, -1), (x1, 148.5, 2), name="paddy")


# ----------------------------------------------------------------------------- shrine

def torii(b, M, x, y, scale=1.0, rot_axis="X"):
    """A vermilion torii straddling a path that runs along X (posts at y +- 1.35 s)."""
    s = scale
    for dy in (-1.35 * s, 1.35 * s):
        b.cylinder((x, y + dy, 0.0), 0.15 * s, 3.3 * s, M["vermilion"])
        b.cylinder((x, y + dy, 0.0), 0.2 * s, 0.35 * s, M["black_paint"])  # kamaki base
    b.box((x - 0.12 * s, y - 1.75 * s, 2.55 * s), (x + 0.12 * s, y + 1.75 * s, 2.75 * s), M["vermilion"])  # nuki
    b.box((x - 0.17 * s, y - 2.05 * s, 3.25 * s), (x + 0.17 * s, y + 2.05 * s, 3.45 * s), M["vermilion"])  # shimaki
    b.box((x - 0.22 * s, y - 2.25 * s, 3.45 * s), (x + 0.22 * s, y + 2.25 * s, 3.62 * s), M["black_paint"])  # kasagi
    # upturned kasagi ends
    for sgn in (-1, 1):
        b.box((x - 0.22 * s, y + sgn * 2.25 * s - (0.25 * s if sgn > 0 else 0), 3.5 * s),
              (x + 0.22 * s, y + sgn * 2.25 * s + (0.0 if sgn > 0 else 0.25 * s), 3.7 * s), M["black_paint"])


def shrine(M, A):
    # approach: stone path west from the road at y = 110, through a tunnel of torii
    b = kz.MeshBuilder("shrine_approach", uv_scale=0.5)
    b.box((-44.0, 106.5, -0.4), (-4.0, 113.5, 0.05), M["levee"])
    b.box((-44.0, 108.4, 0.0), (-4.0, 111.6, 0.07), M["shrine_stone"])
    x = -8.0
    while x > -34.0:
        torii(b, M, x, 110.0, 0.92)
        x -= 2.4
    b.finish()
    kz.box_marker("collider", (-44, 106.0, -1), (-4.5, 108.1, 2), name="approach_edge")
    kz.box_marker("collider", (-44, 111.9, -1), (-4.5, 114.0, 2), name="approach_edge")
    # raised shrine precinct, reached by the Megascans shrine stairs
    p = kz.MeshBuilder("shrine_precinct", uv_scale=0.5)
    p.box((-62.0, 98.0, -0.4), (-40.0, 122.0, 2.6), M["stone"])
    p.quad([(-62, 98, 2.6), (-40, 98, 2.6), (-40, 122, 2.6), (-62, 122, 2.6)], M["shrine_stone"])
    # haiden (worship hall): raised floor, posts, walls, offering box
    hx0, hx1, hy0, hy1 = -56.0, -48.0, 105.0, 115.0
    p.box((hx0, hy0, 2.6), (hx1, hy1, 3.3), M["wood_dark"])
    for px in (hx0 + 0.2, hx1 - 0.2):
        for py in (hy0 + 0.2, (hy0 + hy1) / 2, hy1 - 0.2):
            p.cylinder((px, py, 3.3), 0.14, 3.0, M["vermilion"])
    p.box((hx0 + 0.3, hy0 + 0.3, 3.3), (hx1 - 1.0, hy1 - 0.3, 6.1), M["wood"])
    p.box((hx1 - 0.9, hy0 + 2.0, 3.3), (hx1 - 0.3, hy1 - 2.0, 4.1), M["wood_dark"])  # saisen box
    # hip roof in kawara tiles
    over = 1.1
    pitch = math.radians(32)
    hw = (hx1 - hx0) / 2 + over
    cxh, cyh = (hx0 + hx1) / 2, (hy0 + hy1) / 2
    for sgn in (-1, 1):
        rot = Matrix.Rotation(sgn * pitch, 3, "Y")
        center = Vector((cxh + sgn * hw / 2, cyh, 6.3 + hw * math.tan(pitch) / 2))
        p.oriented_box(center, (hw / math.cos(pitch) + 0.1, (hy1 - hy0) + 2 * over, 0.2), rot, M["roof"])
    ridge = 6.3 + hw * math.tan(pitch)
    p.box((cxh - 0.25, hy0 - over, ridge - 0.15), (cxh + 0.25, hy1 + over, ridge + 0.25), M["black_paint"])
    p.finish()
    kz.box_marker("collider", (-62, 98, -1), (-46.0, 122, 3), name="precinct")
    kz.box_marker("collider", (hx0, hy0, 2.6), (hx1, hy1, 7), name="haiden")
    kz.place(A["stairs"], (-43.2, 110.0, -0.1), 90)
    for dy in (-3.2, 3.2):
        kz.place(A["lantern"], (-40.8, 110.0 + dy, 0.0), 90)
        kz.place(A["komainu"], (-38.6, 110.0 + dy * 0.75, 0.0), 90 if dy < 0 else 90)
    kz.light((-49.0, 110.0, 4.8), (1.0, 0.6, 0.25), 0.006, radius=0.2, range_m=10.0, name="shrine_lamp")


# ----------------------------------------------------------------------------- river & crossing

def river(M):
    b = kz.MeshBuilder("river", uv_scale=0.5)
    b.quad([(-160, 151.0, -2.2), (160, 151.0, -2.2), (160, 168.0, -2.2), (-160, 168.0, -2.2)], M["riverbed"])
    b.quad([(-160, 151.0, -1.5), (160, 151.0, -1.5), (160, 168.0, -1.5), (-160, 168.0, -1.5)], M["water"])
    for (y0, y1) in ((150.0, 151.0), (168.0, 169.0)):
        b.box((-160, y0, -2.3), (-3.6, y1, 0.0), M["concrete"])
        b.box((3.6, y0, -2.3), (160, y1, 0.0), M["concrete"])
    # bridge deck, kerbs, galvanised railings
    b.box((-3.6, 149.5, -0.45), (3.6, 169.5, 0.0), M["concrete"])
    b.quad([(-3.0, 149.5, 0.003), (3.0, 149.5, 0.003), (3.0, 169.5, 0.003), (-3.0, 169.5, 0.003)], M["asphalt"])
    for x0 in (-3.6, 3.0):
        b.box((x0, 149.5, 0.0), (x0 + 0.6, 169.5, 0.2), M["concrete"])
        xr = x0 + 0.3
        for yy in [149.6 + k * 2.0 for k in range(11)]:
            b.cylinder((xr, yy, 0.2), 0.04, 0.9, M["galvanised"], segments=8)
        for z in (0.55, 1.08):
            b.box((xr - 0.035, 149.6, z), (xr + 0.035, 169.4, z + 0.06), M["galvanised"])
    b.finish()
    kz.box_marker("collider", (-160, 150.0, -3), (-3.0, 169.0, 2), name="river")
    kz.box_marker("collider", (3.0, 150.0, -3), (160, 169.0, 2), name="river")


def level_crossing(M):
    b = kz.MeshBuilder("level_crossing", uv_scale=0.5)
    yc = 177.0
    b.box((-160, yc - 2.0, -0.3), (160, yc + 2.0, 0.08), M["ballast"])
    # sleepers and rails (the road crossing is a flat concrete panel)
    x = -159.0
    while x < 159.0:
        if abs(x) > 3.2:
            b.box((x - 0.1, yc - 1.25, 0.05), (x + 0.1, yc + 1.25, 0.16), M["wood_dark"])
        x += 0.62
    b.box((-3.2, yc - 1.6, 0.0), (3.2, yc + 1.6, 0.12), M["concrete"])
    for ry in (yc - 0.72, yc + 0.72):
        b.box((-160, ry - 0.04, 0.16), (160, ry + 0.04, 0.3), M["steel"])
    # crossing signals: black/yellow striped posts, red lamps, barrier arms
    for (sx, sy, arm_dir) in ((-3.9, yc - 2.6, 1), (3.9, yc + 2.6, -1)):
        for k in range(6):
            mat = M["black_paint"] if k % 2 == 0 else M["yellow_paint"]
            b.cylinder((sx, sy, k * 0.5), 0.09, 0.5, mat, segments=12)
        b.cylinder((sx, sy, 3.0), 0.06, 0.9, M["galvanised"], segments=8)
        b.box((sx - 0.5, sy - 0.08, 3.1), (sx + 0.5, sy + 0.08, 3.4), M["black_paint"])
        for dx in (-0.32, 0.32):
            b.cylinder((sx + dx, sy - 0.12 * arm_dir, 3.1), 0.13, 0.05, M["signal_red"], segments=16)
        # barrier arm (raised position, pointing up the post)
        for k in range(8):
            mat = M["yellow_paint"] if k % 2 == 0 else M["black_paint"]
            b.box((sx + arm_dir * 0.25 - 0.04, sy - 0.04, 1.0 + k * 0.45), (sx + arm_dir * 0.25 + 0.04, sy + 0.04, 1.45 + k * 0.45), mat)
    b.finish()
    kz.box_marker("collider", (-160, yc - 2.0, -1), (-3.3, yc + 2.0, 2), name="tracks")
    kz.box_marker("collider", (3.3, yc - 2.0, -1), (160, yc + 2.0, 2), name="tracks")


# ----------------------------------------------------------------------------- street furniture

def utility_poles(M):
    """Concrete poles with cross-arms and a transformer, wires between them (placeholders until
    the utility pole pack passes the gate)."""
    b = kz.MeshBuilder("utility_poles", uv_scale=0.5)
    tops = []
    y = -4.0
    while y < 206.0:
        if 149.0 < y < 170.0:
            y += 31.0
            continue
        x = 3.95
        b.cylinder((x, y, 0.0), 0.17, 10.5, M["concrete"], segments=14)
        b.box((x - 0.9, y - 0.05, 9.4), (x + 0.9, y + 0.05, 9.52), M["galvanised"])
        b.box((x - 0.7, y - 0.05, 8.6), (x + 0.7, y + 0.05, 8.7), M["galvanised"])
        for dx in (-0.75, 0.0, 0.75):
            b.cylinder((x + dx, y, 9.52), 0.04, 0.16, M["paint_white"], segments=8)
        if int(y) % 3 == 0:
            b.cylinder((x - 0.4, y, 7.2), 0.25, 0.8, M["galvanised"], segments=14)  # transformer
        # step bolts
        for k in range(12):
            z = 2.4 + k * 0.45
            b.box((x - 0.02, y - (0.22 if k % 2 else -0.17), z), (x + 0.02, y - (0.17 if k % 2 else -0.22), z + 0.03), M["galvanised"])
        tops.append((x, y))
        y += 31.0
    # wires with sag
    for (xa, ya), (xb, yb) in zip(tops, tops[1:]):
        if yb - ya > 40:
            continue
        for dx, z in ((-0.75, 9.55), (0.0, 9.55), (0.75, 9.55), (0.6, 8.65)):
            pts = [Vector((xa + dx, ya + (yb - ya) * t, z - 0.6 * 4 * t * (1 - t))) for t in (i / 8 for i in range(9))]
            for p0, p1 in zip(pts, pts[1:]):
                d = p1 - p0
                rot = d.to_track_quat("Y", "Z").to_matrix()
                b.oriented_box((p0 + p1) / 2, (0.025, d.length, 0.025), rot, M["rubber"])
    b.finish()
    for (x, y) in tops:
        kz.box_marker("collider", (x - 0.25, y - 0.25, 0), (x + 0.25, y + 0.25, 3), name="pole")


def street_lamps(M):
    b = kz.MeshBuilder("street_lamps", uv_scale=0.5)
    y = 6.0
    while y < 205.0:
        if 149.0 < y < 170.0:
            y += 28.0
            continue
        x = -3.8
        b.cylinder((x, y, 0.0), 0.07, 5.4, M["galvanised"], segments=10)
        b.box((x, y - 0.04, 5.3), (x + 1.3, y + 0.04, 5.38), M["galvanised"])
        b.box((x + 1.0, y - 0.14, 5.18), (x + 1.55, y + 0.14, 5.3), M["galvanised"])
        b.box((x + 1.05, y - 0.11, 5.16), (x + 1.5, y + 0.11, 5.18), M["lamp"])
        kz.light((x + 1.27, y, 5.05), (0.92, 0.95, 1.0), 0.05, radius=0.15, range_m=24.0, night_only=True,
                 spot_dir=(0, 0, -1), cone_deg=65.0, name="street_lamp")
        kz.box_marker("collider", (x - 0.12, y - 0.12, 0), (x + 0.12, y + 0.12, 3), name="lamp_post")
        y += 28.0
    b.finish()


def school_end(M):
    b = kz.MeshBuilder("school_wall", uv_scale=0.5)
    for (x0, x1) in ((-70.0, -4.0), (4.0, 70.0)):
        b.box((x0, 206.0, 0.0), (x1, 206.4, 2.0), M["concrete"])
        b.box((x0, 205.95, 2.0), (x1, 206.45, 2.08), M["concrete"])
        kz.box_marker("collider", (x0, 205.8, 0), (x1, 206.6, 2), name="school_wall")
    for x in (-4.6, 4.0):
        b.box((x, 205.6, 0.0), (x + 0.6, 207.0, 2.4), M["concrete"])
    b.finish()


def distant_hills(M):
    """A ring of forested hills (simple displaced mesh) beyond the town; aerial haze carries
    them into the sky."""
    import bmesh
    bm = bmesh.new()
    rings, segs = 6, 96
    verts = []
    for r in range(rings + 1):
        rad = 420.0 + r * 90.0
        row = []
        for s in range(segs):
            a = s / segs * math.tau
            h = 0.0 if r == 0 else (40 + 110 * (r / rings)) * (0.55 + 0.45 * math.sin(a * 3 + r) * math.sin(a * 7 + 1.3))
            h = max(h, 0.0) + (0 if r == 0 else 30 * rng.random())
            row.append(bm.verts.new((math.cos(a) * rad, 100 + math.sin(a) * rad, h - 2.0)))
        verts.append(row)
    for r in range(rings):
        for s in range(segs):
            a, b2 = verts[r][s], verts[r][(s + 1) % segs]
            c, d = verts[r + 1][(s + 1) % segs], verts[r + 1][s]
            bm.faces.new((a, b2, c, d))
    bm.normal_update()
    kz.world_uv(bm, 0.02)
    me = bpy.data.meshes.new("far_hills")
    bm.to_mesh(me)
    bm.free()
    me.materials.append(M["forest"])
    ob = bpy.data.objects.new("far_hills", me)
    kz.coll("LEVEL").objects.link(ob)


# ----------------------------------------------------------------------------- markers

def markers():
    kz.marker("spawn", (0.0, -2.0, 0.0), spawn="from_home", yaw_deg=0.0)
    kz.marker("spawn", (0.0, 202.0, 0.0), spawn="from_school", yaw_deg=180.0)
    kz.box_marker("exit", (-4.0, -14.0, 0.0), (4.0, -5.0, 3.0), to="home", spawn="from_path")
    kz.box_marker("exit", (-4.0, 205.0, 0.0), (4.0, 214.0, 3.0), to="school", spawn="from_path")
    kz.marker("npc_spot", (1.4, 100.0, 0.0), spot="Path", yaw_deg=0.0)
    # keep the player in the town strip
    kz.box_marker("collider", (-160, -16.0, 0), (160, -14.5, 3), name="south_edge")
    kz.box_marker("collider", (-71, -16.0, 0), (-69.5, 214.0, 3), name="west_edge")
    kz.box_marker("collider", (69.5, -16.0, 0), (71, 214.0, 3), name="east_edge")


def main():
    kz.reset_scene()
    scene = bpy.context.scene
    scene["kz_name"] = "Path to School"
    scene["kz_id"] = "level_path"
    scene["kz_fog"] = 1.0
    M = mats()
    A = {
        "lantern": kz.asset("japanese_stone_lantern", "Japanese_Stone_Lantern"),
        "komainu": kz.asset("japanese_komainu", "Japanese_Komainu_Statue"),
        "stairs": kz.asset("japanese_shrine_stairs", "Japanese_Shrine_Stairs"),
        "post_box": kz.asset("japanese_post_box", "Japanese_Mailbox"),
        "mailbox_wood": kz.asset("japanese_wooden_mailbox", "Japanese_Wooden_Mailbox"),
        "street_box": kz.asset("street_box_metal", "Urban_Japanese_Street_Box_Metal"),
        "gutter": kz.asset("street_gutter_block", "Urban_Japanese_Street_Block_Concrete"),
        "bollard": kz.asset("street_bollard_metal", "Urban_Japanese_Street_Bollard_Metal"),
    }
    ground(M)
    gutter_covers(A)
    residential(M, A)
    konbini(M)
    paddies(M)
    shrine(M, A)
    river(M)
    level_crossing(M)
    utility_poles(M)
    street_lamps(M)
    school_end(M)
    distant_hills(M)
    for y in (172.0, 182.0):
        for x in (-3.7, 3.7):
            kz.place(A["bollard"], (x, y, 0.0))
    markers()
    os.makedirs(OUT, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "path.blend"))
    print("BUILT", os.path.join(OUT, "path.blend"))
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import export_level
    export_level.main(OUT)


main()
