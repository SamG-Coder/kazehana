"""House type 1: the modern two-storey gable house (docs/TOWN_PLAN.md), as one quality-gated asset.

    blender --background --factory-startup --python tools/blender/build_house_modern_gable.py

-> assets/buildings/house_modern_gable/house_modern_gable.gltf

Local frame (Blender): origin at ground level in the middle of the footprint, +X along the long
side (10 m), +Y north (7.5 m deep). The south (-Y) face is the garden side: living room sliding
doors, the balcony. The north (+Y) face has the entrance (genkan) under a porch roof. Placement
turns the house so the garden faces the plot's garden side.

What it is built from:
- a raised concrete foundation (0.45 m) with vents
- walls of ceramic siding: 455 mm courses with recessed joints (geometry), corner boards, a trim
  band between the storeys
- a 24-degree gable roof in kawara tiles (Megascans Japanese Roof Tiles), barge and fascia boards,
  ridge cap, half-round gutters and downpipes
- aluminium sash windows and sliding doors, shutter boxes over the big openings, glass that
  lights up at night (kz_glow), a dim room behind it
- a south balcony with railing and laundry poles, the genkan door with a porch roof and steps, an
  air-conditioner outdoor unit, a heat-pump water heater
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
# Colour schemes, as separate assets (each its own quality-gated id):
#   blender ... --python build_house_modern_gable.py -- <scheme>
SCHEMES = {
    # id suffix: siding, trim (and gutters), roof tint (multiplies the kawara scan), shutter, door
    "a": ((0.62, 0.57, 0.48), (0.12, 0.09, 0.07), None, (0.30, 0.27, 0.24), (0.22, 0.14, 0.08)),          # beige, brown trim, brown kawara
    "b": ((0.72, 0.72, 0.70), (0.05, 0.05, 0.05), (0.55, 0.57, 0.62), (0.20, 0.20, 0.20), (0.06, 0.06, 0.06)),  # off-white, black trim, silver-grey ibushi kawara
    "c": ((0.30, 0.30, 0.29), (0.06, 0.06, 0.06), (0.30, 0.31, 0.33), (0.18, 0.18, 0.18), (0.10, 0.07, 0.05)),  # mid grey siding, black trim, charcoal kawara
    "d": ((0.50, 0.42, 0.32), (0.20, 0.14, 0.09), (0.22, 0.16, 0.12), (0.32, 0.27, 0.22), (0.16, 0.10, 0.06)),  # warm greige siding, brown trim, dark brown kawara
}
SCHEME = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "a"
SIDING, TRIM, ROOF_TINT, SHUTTER, DOOR = SCHEMES[SCHEME]
# Layout: "n" = the genkan on the north (entrance) side, the usual plan; "s" = the genkan on the
# garden side at its east end, for plots whose road runs along the garden (south) side; "e" = the
# genkan on the east gable end, for plots whose road runs along a side
LAYOUT = sys.argv[sys.argv.index("--") + 2] if "--" in sys.argv and len(sys.argv) > sys.argv.index("--") + 2 else "n"
# House type: "gable" (type 1, 10 x 7.5 m) or "hip" (type 2, 9.5 x 8 m: a hipped roof, finer siding
# courses, the balcony over the west half) or "small" (type 3, 8 x 6.5 m: one storey, a gable roof,
# a laundry stand in the garden)
TYPE = sys.argv[sys.argv.index("--") + 3] if "--" in sys.argv and len(sys.argv) > sys.argv.index("--") + 3 else "gable"
ASSET_ID = ("house_modern_" + TYPE) + ("" if SCHEME == "a" else "_" + SCHEME) + ("_" + LAYOUT if LAYOUT != "n" else "")
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")

W, D = {"gable": (10.0, 7.5), "hip": (9.5, 8.0), "small": (8.0, 6.5)}[TYPE]   # footprint (siding face)
FLOORS = 1 if TYPE == "small" else 2
F0 = 0.45                  # top of the foundation (ground floor level)
FH = 2.9                   # storey height
F1 = F0 + FH               # first floor level
TOP = F0 + FLOORS * FH     # wall top (eaves)
PITCH = math.radians(24.0)
EAVE_OV, VERGE_OV = 0.6, 0.45
COURSE, JOINT, SIDE_T = (0.303 if TYPE == "hip" else 0.455), 0.007, 0.016
CORE_T = 0.15

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)

# ----------------------------------------------------------------------------- materials
DETAIL = "Precast_Concrete_Wall"
siding = kz.painted_material("house_siding_" + SCHEME, SIDING, DETAIL, roughness=0.7, normal_strength=0.35)
trim = kz.painted_material("house_trim_" + SCHEME, TRIM, DETAIL, roughness=0.55, normal_strength=0.2)
found = kz.painted_material("house_foundation", (0.36, 0.35, 0.33), DETAIL, roughness=0.85, normal_strength=0.8)
roof = kz.surface_material("house_roof_" + SCHEME, "Japanese_Roof_Tiles", tint=ROOF_TINT, desaturate=0.0 if ROOF_TINT is None else 0.85)
alu = kz.painted_material("house_aluminium", (0.09, 0.075, 0.065), DETAIL, roughness=0.35, metallic=0.8, normal_strength=0.1)
shutter = kz.painted_material("house_shutter_" + SCHEME, SHUTTER, DETAIL, roughness=0.45, metallic=0.6, normal_strength=0.15)
gutter = kz.painted_material("house_gutter_" + SCHEME, tuple(c * 0.9 for c in TRIM) if SCHEME != "c" else (0.5, 0.5, 0.48), DETAIL, roughness=0.4, normal_strength=0.1)
door = kz.painted_material("house_door_" + SCHEME, DOOR, "Wood_Planks", roughness=0.5, normal_strength=0.6)
unit = kz.painted_material("house_unit", (0.70, 0.70, 0.67), DETAIL, roughness=0.45, metallic=0.2, normal_strength=0.15)
dark = kz.painted_material("house_dark", (0.02, 0.02, 0.02), DETAIL, roughness=0.6, normal_strength=0.1)
room = kz.painted_material("house_room", (0.28, 0.24, 0.20), DETAIL, roughness=0.9, normal_strength=0.2)
curtain = kz.painted_material("house_curtain", (0.62, 0.58, 0.50), DETAIL, roughness=0.95, normal_strength=0.2)
glass = kz.plain_material("kz_glow_glass", (0.025, 0.03, 0.035), roughness=0.04)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)
lamp = kz.plain_material("kz_glow_porch_lamp", (0.85, 0.82, 0.75), roughness=0.3)

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


def obox(b, c, size, t, n, mat, bevel=0.0):
    """A box centred at c, size (along t, along n, up), t and n horizontal unit vectors; rounded
    edges of radius `bevel`."""
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
    """An air-conditioner / heat-pump outdoor unit standing with its back to a wall: base = the
    point on the wall face at ground level under the unit's centre; t along the wall; n outward."""
    t, n, base = Vector(t), Vector(n), Vector(base)
    up = Vector((0, 0, 1))
    obox(b, base + n * 0.22 + up * 0.04, (w + 0.2, 0.42, 0.08), t, n, found, 0.01)            # concrete pad
    for k in (-1, 1):
        obox(b, base + n * 0.2 + t * (k * w * 0.36) + up * 0.1, (0.05, d, 0.04), t, n, dark, 0.005)  # feet
    c = base + n * (0.06 + d / 2) + up * (0.12 + h / 2)
    obox(b, c, (w, d, h), t, n, unit, 0.018)                                                      # body
    obox(b, c + up * (h / 2 + 0.006), (w + 0.02, d + 0.02, 0.012), t, n, unit, 0.004)            # top lid
    front = c + n * (d / 2)
    fc = front + t * (-w * 0.17)
    rcyl(b, fc - n * 0.012, fc + n * 0.002, 0.2, dark, seg=28)                                    # fan opening
    rcyl(b, fc - n * 0.03, fc - n * 0.012, 0.07, unit, seg=16)                                    # fan hub
    for k in range(-7, 8):                                                                        # grille
        z = k * 0.026
        chord = 2 * (max(0.0, 0.2 ** 2 - z ** 2)) ** 0.5
        if chord > 0.02:
            obox(b, fc + up * z + n * 0.006, (chord, 0.006, 0.005), t, n, unit)
    for k in (-2, -1, 0, 1, 2):
        obox(b, fc + t * (k * 0.075) + n * 0.009, (0.006, 0.006, 0.4), t, n, unit)
    for k in range(10):                                                                           # side vents
        obox(b, c - t * (w / 2) + up * (-h / 2 + 0.08 + k * 0.042) + n * 0.0, (0.004, d * 0.8, 0.018), t, n, dark)
    obox(b, front + t * (w * 0.33) + up * 0.12, (0.18, 0.01, 0.12), t, n, unit, 0.004)            # valve cover
    return c, front


walls = kz.MeshBuilder("walls", uv_scale=0.5)
fittings = kz.MeshBuilder("fittings", uv_scale=1.0)
roofing = kz.MeshBuilder("roof", uv_scale=0.5)

# ----------------------------------------------------------------------------- openings
# per facade: (u0, u1, z0, z1, kind) with u along the facade from its left end seen from outside
# kinds: "slide" (floor-length sliding door, shutter box), "win" (window), "frost" (frosted),
#        "door" (genkan), "slide_up" (upstairs sliding door to the balcony, shutter box)
H1 = F1
SOUTH = [(0.8, 3.4, F0 + 0.05, F0 + 2.05, "slide"), (3.8, 6.4, F0 + 0.05, F0 + 2.05, "slide"), (7.4, 9.2, F0 + 0.75, F0 + 1.85, "win"),
         (1.4, 3.8, H1 + 0.05, H1 + 2.0, "slide_up"), (5.4, 7.8, H1 + 0.05, H1 + 2.0, "slide_up"), (8.4, 9.4, H1 + 0.9, H1 + 2.0, "win")]
NORTH = [(1.4, 2.4, F0 + 0.0, F0 + 2.1, "door"), (3.6, 4.6, F0 + 1.0, F0 + 1.8, "win"), (6.4, 7.2, F0 + 1.0, F0 + 1.7, "frost"),
         (1.4, 2.4, H1 + 0.9, H1 + 2.0, "win"), (4.9, 5.5, H1 + 1.1, H1 + 1.8, "frost"), (7.4, 8.4, H1 + 0.9, H1 + 2.0, "win")]
if TYPE == "hip":
    SOUTH = [(0.7, 3.3, F0 + 0.05, F0 + 2.05, "slide"), (3.7, 6.3, F0 + 0.05, F0 + 2.05, "slide"), (7.0, 8.7, F0 + 0.75, F0 + 1.85, "win"),
             (0.8, 3.2, H1 + 0.05, H1 + 2.0, "slide_up"), (3.6, 6.0, H1 + 0.05, H1 + 2.0, "slide_up"), (7.2, 8.6, H1 + 0.9, H1 + 2.0, "win")]
    NORTH = [(1.3, 2.3, F0 + 0.0, F0 + 2.1, "door"), (3.4, 4.6, F0 + 1.0, F0 + 1.8, "win"), (6.0, 6.8, F0 + 1.0, F0 + 1.7, "frost"),
             (1.2, 2.4, H1 + 0.9, H1 + 2.0, "win"), (4.6, 5.2, H1 + 1.1, H1 + 1.8, "frost"), (6.8, 8.2, H1 + 0.9, H1 + 2.0, "win")]
EAST = [(2.8, 4.6, F0 + 0.75, F0 + 1.95, "win"), (3.0, 4.5, H1 + 0.9, H1 + 2.0, "win")]
EAST_DOOR = (5.2, 6.2)
if TYPE == "small":
    SOUTH = [(0.6, 3.2, F0 + 0.05, F0 + 2.05, "slide"), (3.6, 6.2, F0 + 0.05, F0 + 2.05, "slide"), (6.7, 7.6, F0 + 0.75, F0 + 1.85, "win")]
    NORTH = [(1.2, 2.2, F0 + 0.0, F0 + 2.1, "door"), (3.2, 4.2, F0 + 1.0, F0 + 1.8, "win"), (5.4, 6.2, F0 + 1.0, F0 + 1.7, "frost")]
    EAST = [(1.8, 3.4, F0 + 0.75, F0 + 1.95, "win")]
    EAST_DOOR = (4.2, 5.2)
if LAYOUT == "e":
    EAST = [o for o in EAST] + [(EAST_DOOR[0], EAST_DOOR[1], F0 + 0.0, F0 + 2.1, "door")]
    NORTH = [(o[0], o[1], F0 + 1.0, F0 + 1.8, "win") if o[4] == "door" else o for o in NORTH]
if LAYOUT == "s":
    SOUTH = [o for o in SOUTH if not (o[4] == "win" and o[2] < H1)] + [{"gable": (8.0, 9.0, F0 + 0.0, F0 + 2.1, "door"), "hip": (7.4, 8.4, F0 + 0.0, F0 + 2.1, "door"), "small": (6.7, 7.6, F0 + 0.0, F0 + 2.1, "door")}[TYPE]]
    NORTH = [(o[0], o[1], F0 + 1.0, F0 + 1.8, "win") if o[4] == "door" else o for o in NORTH]
WEST = [(3.2, 4.2, F0 + 0.9, F0 + 1.9, "frost"), (2.2, 3.7, H1 + 0.9, H1 + 2.0, "win")] if TYPE != "small" else [(2.6, 3.4, F0 + 0.9, F0 + 1.9, "frost")]

# facade frames: origin (left end, seen from outside), unit along u, outward normal
FACADES = {
    "south": (Vector((-W / 2, -D / 2, 0)), Vector((1, 0, 0)), Vector((0, -1, 0)), W, SOUTH),
    "north": (Vector((W / 2, D / 2, 0)), Vector((-1, 0, 0)), Vector((0, 1, 0)), W, NORTH),
    "east": (Vector((W / 2, -D / 2, 0)), Vector((0, 1, 0)), Vector((1, 0, 0)), D, EAST),
    "west": (Vector((-W / 2, D / 2, 0)), Vector((0, -1, 0)), Vector((-1, 0, 0)), D, WEST),
}


def fbox(b, o, eu, en, u0, u1, z0, z1, d0, d1, mat):
    """A box in a facade frame: u0..u1 along it, z0..z1 up, d0..d1 out from the siding face."""
    pts = [o + eu * u + en * d for u in (u0, u1) for d in (d0, d1)]
    mn = Vector((min(p.x for p in pts), min(p.y for p in pts), z0))
    mx = Vector((max(p.x for p in pts), max(p.y for p in pts), z1))
    b.box(mn, mx, mat)


def free_rects(length, z0, z1, ops):
    """The facade between z0 and z1 minus the openings, as rectangles (vertical strips)."""
    xs = sorted({0.0, length} | {o[0] for o in ops} | {o[1] for o in ops})
    out = []
    for a, b in zip(xs[:-1], xs[1:]):
        if b - a < 1e-4:
            continue
        mid = (a + b) / 2
        blocked = sorted((o[2], o[3]) for o in ops if o[0] < mid < o[1])
        z = z0
        for bz0, bz1 in blocked:
            if bz0 > z + 1e-4:
                out.append((a, b, z, min(bz0, z1)))
            z = max(z, bz1)
        if z < z1 - 1e-4:
            out.append((a, b, z, z1))
    return out


for name, (o, eu, en, length, ops) in FACADES.items():
    # wall core (behind the siding) and the siding courses, around the openings
    for (a, b, z0, z1) in free_rects(length, F0, TOP, ops):
        fbox(walls, o, eu, en, a, b, z0, z1, -CORE_T, 0.0, room)
        # courses: each 455 mm, the joint recessed (the core shows through it)
        k0 = int(math.floor((z0 - F0) / COURSE))
        k1 = int(math.ceil((z1 - F0) / COURSE))
        for k in range(k0, k1):
            c0 = max(z0, F0 + k * COURSE + JOINT)
            c1 = min(z1, F0 + (k + 1) * COURSE)
            if c1 - c0 > 0.003:
                fbox(walls, o, eu, en, a, b, c0, c1, 0.0, SIDE_T, siding)
    # trim band between the storeys, corner boards
    if FLOORS > 1:
        fbox(fittings, o, eu, en, -0.03, length + 0.03, F1 - 0.12, F1 + 0.04, 0.0, SIDE_T + 0.022, trim)
    for u in (0.0, length - 0.1):
        fbox(fittings, o, eu, en, u, u + 0.1, F0, TOP, 0.0, SIDE_T + 0.02, trim)

    # openings: frame, glass, mullions, sills; the room behind; shutter boxes; the door
    for (u0, u1, z0, z1, kind) in ops:
        FR = 0.045
        if kind == "door":
            fbox(fittings, o, eu, en, u0, u1, z0, z1, -0.05, -0.02, door)
            fbox(fittings, o, eu, en, u0 - 0.06, u0, z0, z1 + 0.06, -0.06, SIDE_T + 0.01, alu)
            fbox(fittings, o, eu, en, u1, u1 + 0.06, z0, z1 + 0.06, -0.06, SIDE_T + 0.01, alu)
            fbox(fittings, o, eu, en, u0 - 0.06, u1 + 0.06, z1, z1 + 0.06, -0.06, SIDE_T + 0.01, alu)
            # a narrow glass strip and the handle
            fbox(fittings, o, eu, en, u0 + 0.15, u0 + 0.28, z0 + 0.4, z1 - 0.3, -0.018, -0.012, frosted)
            fbox(fittings, o, eu, en, u1 - 0.16, u1 - 0.13, z0 + 0.9, z0 + 1.3, -0.02, 0.03, alu)
            continue
        g = frosted if kind == "frost" else glass
        # the room behind (so the glass shows a dim interior, curtains drawn part way)
        fbox(walls, o, eu, en, u0, u1, z0, z1, -0.9, -0.85, room)
        if kind in ("slide", "slide_up"):
            fbox(walls, o, eu, en, u0, u0 + (u1 - u0) * 0.22, z0, z1, -0.3, -0.28, curtain)
            fbox(walls, o, eu, en, u1 - (u1 - u0) * 0.18, u1, z0, z1, -0.3, -0.28, curtain)
        # reveal (the opening's sides through the wall)
        for (a0, a1) in ((u0 - 0.002, u0), (u1, u1 + 0.002)):
            fbox(walls, o, eu, en, a0, a1, z0, z1, -CORE_T, SIDE_T, room)
        # glass, two panes for sliding units
        fbox(fittings, o, eu, en, u0, u1, z0, z1, -0.06, -0.055, g)
        # frame and mullions
        fbox(fittings, o, eu, en, u0, u1, z0, z0 + FR, -0.08, 0.0, alu)
        fbox(fittings, o, eu, en, u0, u1, z1 - FR, z1, -0.08, 0.0, alu)
        fbox(fittings, o, eu, en, u0, u0 + FR, z0, z1, -0.08, 0.0, alu)
        fbox(fittings, o, eu, en, u1 - FR, u1, z0, z1, -0.08, 0.0, alu)
        if (u1 - u0) > 0.9:
            m = (u0 + u1) / 2
            fbox(fittings, o, eu, en, m - 0.03, m + 0.03, z0, z1, -0.075, -0.01, alu)
        if kind in ("win", "frost"):
            fbox(fittings, o, eu, en, u0 - 0.04, u1 + 0.04, z0 - 0.04, z0, 0.0, SIDE_T + 0.05, alu)  # sill
        if kind in ("slide", "slide_up"):
            # the shutter box (amado / shutter case) over the opening, its guide rails down the sides
            fbox(fittings, o, eu, en, u0 - 0.05, u1 + 0.05, z1, z1 + 0.28, 0.0, 0.22, shutter)
            for a in (u0 - 0.05, u1):
                fbox(fittings, o, eu, en, a, a + 0.05, z0, z1, 0.0, 0.08, shutter)

    # foundation (a little behind the siding face), with vents every ~2.5 m
    fbox(walls, o, eu, en, -0.03, length + 0.03, -0.15, F0, -CORE_T, -0.01, found)
    n = max(1, int(length / 2.5))
    for k in range(n):
        u = (k + 0.5) * length / n
        fbox(fittings, o, eu, en, u - 0.2, u + 0.2, 0.15, 0.27, -0.012, 0.0, dark)

# floor slab / ceiling inside (the interior seen through the glass has a floor and a ceiling)
walls.box((-W / 2 + 0.01, -D / 2 + 0.01, F0 - 0.05), (W / 2 - 0.01, D / 2 - 0.01, F0), room)
if FLOORS > 1:
    walls.box((-W / 2 + 0.01, -D / 2 + 0.01, F1 - 0.05), (W / 2 - 0.01, D / 2 - 0.01, F1), room)
walls.box((-W / 2 + 0.01, -D / 2 + 0.01, TOP - 0.05), (W / 2 - 0.01, D / 2 - 0.01, TOP), room)

# ----------------------------------------------------------------------------- roof
if TYPE in ("gable", "small"):
    rise = (D / 2) * math.tan(PITCH)
    RT = 0.14  # roof thickness
    run = D / 2 + EAVE_OV
    slope_len = run / math.cos(PITCH)
    roof_w = W + 2 * VERGE_OV
    z_eave = TOP - EAVE_OV * math.tan(PITCH)
    for s in (-1, 1):  # south (-1), north (+1)
        ang = PITCH if s < 0 else -PITCH
        rot = Matrix.Rotation(ang, 3, "X")
        nrm = rot @ Vector((0, 0, 1))
        # centre of the slab's top surface, then down half the thickness
        cy = s * run / 2
        cz = z_eave + (TOP + rise - z_eave) / 2 + 0.04
        c = Vector((0, cy, cz)) - nrm * (RT / 2)
        roofing.oriented_box(c, (roof_w, slope_len, RT), rot, roof)
        # fascia board along the eave, the gutter in front of it, the soffit underneath
        ye = s * run
        # fascia board, tucked under the roof slab
        fittings.box((-roof_w / 2, min(ye, ye - s * 0.035), z_eave - RT - 0.2), (roof_w / 2, max(ye, ye - s * 0.035), z_eave - RT + 0.01), trim)
        # the eave board the tile row sits on (closes the eave: no light between the round tiles)
        roofing.box((-roof_w / 2, min(ye - s * 0.06, ye + s * 0.03), z_eave - RT - 0.02), (roof_w / 2, max(ye - s * 0.06, ye + s * 0.03), z_eave + 0.04), roof)
        # eave tiles (nokigawara): a row of rounded tile ends along the eave, every 0.3 m
        slope_dir = Vector((0, s * math.cos(PITCH), -math.sin(PITCH)))
        nx = int(roof_w / 0.3)
        for k in range(nx):
            x = -roof_w / 2 + (k + 0.5) * roof_w / nx
            tip = Vector((x, ye + s * 0.02, z_eave + 0.02))
            rcyl(roofing, tip - slope_dir * 0.12, tip, 0.07, roof, seg=14)
            rcyl(roofing, tip - slope_dir * 0.01, tip + slope_dir * 0.012, 0.085, roof, seg=14)  # the round face
        # round gutter on brackets, just under the eave tiles
        gy = ye + s * 0.08
        rcyl(fittings, (-roof_w / 2 + 0.05, gy, z_eave - 0.14), (roof_w / 2 - 0.05, gy, z_eave - 0.14), 0.06, gutter, seg=16)
        for k in range(int(roof_w / 0.9) + 1):
            x = -roof_w / 2 + 0.1 + k * (roof_w - 0.2) / max(1, int(roof_w / 0.9))
            fittings.box((x - 0.012, min(ye - s * 0.035, gy + s * 0.07), z_eave - 0.21), (x + 0.012, max(ye - s * 0.035, gy + s * 0.07), z_eave - 0.19), alu)
        sz = TOP - 0.02
        fittings.box((-roof_w / 2, min(s * D / 2, ye), sz - 0.02), (roof_w / 2, max(s * D / 2, ye), sz), trim)
        # downpipes at both ends of the gutter
        for x in (-W / 2 + 0.25, W / 2 - 0.25):
            fittings.cylinder((x, s * (D / 2 + 0.09), 0.0), 0.032, z_eave - 0.55, gutter, segments=12)
            # the swan neck from the gutter back to the wall, and clips holding the pipe
            rcyl(fittings, (x, gy, z_eave - 0.18), (x, gy, z_eave - 0.3), 0.035, gutter)
            rcyl(fittings, (x, gy, z_eave - 0.3), (x, s * (D / 2 + 0.09), z_eave - 0.55), 0.032, gutter)
            for z in [z for z in (1.0, 3.0, 5.0) if z < z_eave - 0.7]:
                fittings.box((x - 0.045, min(s * D / 2, s * (D / 2 + 0.13)), z), (x + 0.045, max(s * D / 2, s * (D / 2 + 0.13)), z + 0.03), alu)
    # gable triangles (east, west) in siding, and barge boards along the verges
    for sx in (-1, 1):
        x = sx * W / 2
        tri = [Vector((x, -D / 2, TOP)), Vector((x, D / 2, TOP)), Vector((x, 0, TOP + rise))]
        if sx < 0:
            tri = [tri[1], tri[0], tri[2]]
        walls.quad(tri, siding)
        for s in (-1, 1):
            a = Vector((sx * (roof_w / 2), s * run, z_eave + 0.02))
            b = Vector((sx * (roof_w / 2), 0, TOP + rise + 0.06))
            mid = (a + b) / 2
            d = (b - a)
            rot = Matrix.Rotation(math.atan2(d.z, abs(d.y)) * (1 if s < 0 else -1), 3, "X")
            fittings.oriented_box(mid - Vector((0, 0, 0.08)), (0.04, d.length, 0.22), rot, trim)
    # ridge cap
    roofing.box((-roof_w / 2, -0.16, TOP + rise - 0.02), (roof_w / 2, 0.16, TOP + rise + 0.16), roof)

else:
    # a hipped roof: four slopes over the walls, eaves all round, the ridge along the long axis
    RT = 0.14
    OV = EAVE_OV
    Wr, Dr = W / 2 + OV, D / 2 + OV
    # (the slab's underside meets the wall tops; its top surface is a thickness higher)
    z_eave = TOP - OV * math.tan(PITCH) + RT / math.cos(PITCH)
    xr = Wr - Dr                                   # half the ridge's length
    zr = z_eave + Dr * math.tan(PITCH)              # ridge height
    hb = kz.MeshBuilder("hiproof", uv_scale=0.5)
    top = {
        "s": [(-Wr, -Dr, z_eave), (Wr, -Dr, z_eave), (xr, 0, zr), (-xr, 0, zr)],
        "n": [(Wr, Dr, z_eave), (-Wr, Dr, z_eave), (-xr, 0, zr), (xr, 0, zr)],
        "e": [(Wr, -Dr, z_eave), (Wr, Dr, z_eave), (xr, 0, zr)],
        "w": [(-Wr, Dr, z_eave), (-Wr, -Dr, z_eave), (-xr, 0, zr)],
    }
    for face in top.values():
        hb.quad([Vector(v) for v in face], roof)
        hb.quad([Vector(v) - Vector((0, 0, RT)) for v in reversed(face)], roof)   # the soffit side
    # the slab's edge round the eaves
    ring = [(-Wr, -Dr), (Wr, -Dr), (Wr, Dr), (-Wr, Dr)]
    for k in range(4):
        a, b = ring[k], ring[(k + 1) % 4]
        hb.quad([Vector((a[0], a[1], z_eave - RT)), Vector((b[0], b[1], z_eave - RT)), Vector((b[0], b[1], z_eave)), Vector((a[0], a[1], z_eave))], roof)
    hob = hb.finish(collection="HOUSE")
    # tiles run down each slope: UVs in each face's own frame (u along the eave, v up the slope)
    me = hob.data
    uvl = me.uv_layers.active
    for poly in me.polygons:
        n = poly.normal
        h = Vector((n.x, n.y, 0))
        if h.length < 1e-4:
            continue
        h.normalize()
        along = Vector((-h.y, h.x, 0))
        up = along.cross(n).normalized()
        for li in poly.loop_indices:
            co = me.vertices[me.loops[li].vertex_index].co
            uvl.data[li].uv = (co.dot(along) * 0.5, co.dot(up) * 0.5)
    # ridge and hip caps (rounded, as on a kawara roof)
    rcyl(roofing, (-xr - 0.05, 0, zr + 0.06), (xr + 0.05, 0, zr + 0.06), 0.14, roof, seg=14)
    for (cx, cy) in ((Wr, -Dr), (Wr, Dr), (-Wr, Dr), (-Wr, -Dr)):
        ex = xr if cx > 0 else -xr
        rcyl(roofing, (cx, cy, z_eave + 0.05), (ex, 0, zr + 0.06), 0.1, roof, seg=12)
    # eaves all round: fascia, the eave tile row, a round gutter on brackets
    sides = [  # (start, end, outward)
        (Vector((-Wr, -Dr, 0)), Vector((Wr, -Dr, 0)), Vector((0, -1, 0))),
        (Vector((Wr, Dr, 0)), Vector((-Wr, Dr, 0)), Vector((0, 1, 0))),
        (Vector((Wr, -Dr, 0)), Vector((Wr, Dr, 0)), Vector((1, 0, 0))),
        (Vector((-Wr, Dr, 0)), Vector((-Wr, -Dr, 0)), Vector((-1, 0, 0))),
    ]
    for a, b, out in sides:
        d = (b - a)
        L = d.length
        t = d / L
        slope_dir = out * math.cos(PITCH) + Vector((0, 0, -math.sin(PITCH)))
        mid = (a + b) / 2
        obox(fittings, mid - out * 0.02 + Vector((0, 0, z_eave - RT - 0.09)), (L, 0.035, 0.2), t, out, trim)
        n_t = int(L / 0.3)
        for k in range(1, n_t):
            p = a + t * (k * L / n_t) + out * 0.02 + Vector((0, 0, z_eave + 0.02))
            rcyl(roofing, p - slope_dir * 0.12, p, 0.07, roof, seg=14)
            rcyl(roofing, p - slope_dir * 0.01, p + slope_dir * 0.012, 0.085, roof, seg=14)
        g0 = a + out * 0.08 + t * 0.15 + Vector((0, 0, z_eave - 0.14))
        g1 = b + out * 0.08 - t * 0.15 + Vector((0, 0, z_eave - 0.14))
        rcyl(fittings, g0, g1, 0.06, gutter, seg=16)
        for k in range(int(L / 0.9) + 1):
            p = a + t * (0.2 + k * (L - 0.4) / max(1, int(L / 0.9))) + Vector((0, 0, z_eave - 0.2))
            obox(fittings, p + out * 0.02, (0.024, 0.14, 0.02), t, out, alu)
    # downpipes at the four corners, down the walls
    for (sx, sy_) in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
        x, y = sx * (W / 2 - 0.25), sy_ * (D / 2 + 0.09)
        fittings.cylinder((x, y, 0.0), 0.032, z_eave - 0.55, gutter, segments=12)
        gy = sy_ * (Dr + 0.08)
        rcyl(fittings, (x, gy, z_eave - 0.18), (x, gy, z_eave - 0.3), 0.035, gutter)
        rcyl(fittings, (x, gy, z_eave - 0.3), (x, y, z_eave - 0.55), 0.032, gutter)
        for z in [z for z in (1.0, 3.0, 5.0) if z < z_eave - 0.7]:
            fittings.box((x - 0.045, min(sy_ * D / 2, sy_ * (D / 2 + 0.13)), z), (x + 0.045, max(sy_ * D / 2, sy_ * (D / 2 + 0.13)), z + 0.03), alu)

# ----------------------------------------------------------------------------- balcony (south)
if FLOORS > 1:
    if TYPE == "gable":
        BX0, BX1, BD = -W / 2 + 0.6, (W / 2 - 1.8) if LAYOUT == "n" else 2.9, 1.0
    else:
        BX0, BX1, BD = -W / 2 + 0.4, -W / 2 + 6.4, 1.1
    y0 = -D / 2
    fittings.box((BX0, y0 - BD, F1 - 0.18), (BX1, y0, F1 - 0.02), unit)            # floor slab
    for (xa, xb, ya, yb) in ((BX0, BX1, y0 - BD, y0 - BD + 0.04), (BX0, BX0 + 0.04, y0 - BD, y0), (BX1 - 0.04, BX1, y0 - BD, y0)):
        fittings.box((xa, ya, F1 - 0.02), (xb, yb, F1 + 0.55), siding)              # solid lower panel
        fittings.box((xa - 0.01, ya - 0.01, F1 + 1.05), (xb + 0.01, yb + 0.01, F1 + 1.1), alu)  # top rail
        L = max(xb - xa, yb - ya)
        nb = int(L / 0.11)
        for k in range(nb + 1):                                                        # bars
            t = k / max(nb, 1)
            px = xa + (xb - xa) * t if xb - xa > yb - ya else (xa + xb) / 2
            py = ya + (yb - ya) * t if yb - ya > xb - xa else (ya + yb) / 2
            fittings.box((px - 0.01, py - 0.01, F1 + 0.55), (px + 0.01, py + 0.01, F1 + 1.05), alu)
    # laundry pole holders, fixed to the railing's top rail (monohoshi kanamono): a short post up from
    # the rail with an arm leaning in towards the house, two cradles; the poles rest in them just
    # above the railing
    RAIL_TOP = F1 + 1.1
    ry = y0 - BD + 0.02
    for x in (BX0 + 0.35, BX1 - 0.35):
        fittings.box((x - 0.018, ry - 0.018, RAIL_TOP), (x + 0.018, ry + 0.018, RAIL_TOP + 0.22), alu)       # post
        fittings.box((x - 0.016, ry, RAIL_TOP + 0.2), (x + 0.016, ry + 0.42, RAIL_TOP + 0.235), alu)          # arm, inwards
        for cy in (ry + 0.12, ry + 0.38):
            fittings.box((x - 0.03, cy - 0.03, RAIL_TOP + 0.235), (x + 0.03, cy + 0.03, RAIL_TOP + 0.27), alu)  # cradle
    for cy in (ry + 0.12, ry + 0.38):
        fittings.box((BX0 + 0.2, cy - 0.016, RAIL_TOP + 0.25), (BX1 - 0.2, cy + 0.016, RAIL_TOP + 0.282), unit)
else:
    # a laundry stand (monohoshi-dai) in the garden: two weighted posts, two poles
    ly = -D / 2 - 1.7
    lx0 = -1.2 if LAYOUT == "s" else 0.0   # (clear of the steps when the door is on this side)
    TOPZ = 1.7
    for x in (lx0 - 1.4, lx0 + 1.4):
        # the weight: a round concrete block, bevelled top, a steel socket for the post
        rcyl(fittings, (x, ly, 0.0), (x, ly, 0.09), 0.23, found, seg=28)
        rcyl(fittings, (x, ly, 0.09), (x, ly, 0.12), 0.2, found, seg=28)
        rcyl(fittings, (x, ly, 0.12), (x, ly, 0.24), 0.045, alu, seg=16)
        # the post: a wider lower tube, a narrower upper tube sliding in it, a clamp knob at the joint
        rcyl(fittings, (x, ly, 0.12), (x, ly, 0.95), 0.03, alu, seg=16)
        rcyl(fittings, (x, ly, 0.9), (x, ly, TOPZ - 0.05), 0.024, alu, seg=16)
        rcyl(fittings, (x, ly, 0.93), (x, ly, 0.99), 0.038, dark, seg=16)                       # clamp ring
        rcyl(fittings, (x, ly - 0.035, 0.96), (x, ly - 0.075, 0.96), 0.012, dark, seg=10)       # knob
        rcyl(fittings, (x, ly - 0.075, 0.96), (x, ly - 0.085, 0.96), 0.02, dark, seg=12)
        # the head: two arms angled up and out in a V, a cradle (a U of tube) at each end
        rcyl(fittings, (x, ly, TOPZ - 0.07), (x, ly, TOPZ - 0.01), 0.032, dark, seg=16)         # head block
        for sy in (-1.0, 1.0):
            tip = Vector((x, ly + sy * 0.3, TOPZ + 0.06))
            rcyl(fittings, (x, ly, TOPZ - 0.04), tip, 0.014, alu, seg=12)
            for dy in (-0.035, 0.035):
                rcyl(fittings, tip + Vector((0, dy, 0)), tip + Vector((0, dy, 0.07)), 0.007, alu, seg=8)
            rcyl(fittings, tip + Vector((0, -0.035, 0)), tip + Vector((0, 0.035, 0)), 0.008, alu, seg=8)
    # the poles: round, resting in the cradles, a cap on each end
    for sy in (-1.0, 1.0):
        y = ly + sy * 0.3
        z = TOPZ + 0.06 + 0.02
        rcyl(fittings, (lx0 - 1.8, y, z), (lx0 + 1.8, y, z), 0.016, unit, seg=16)
        for e in (-1.0, 1.0):
            rcyl(fittings, (lx0 + e * 1.8, y, z), (lx0 + e * 1.83, y, z), 0.019, dark, seg=16)

# ----------------------------------------------------------------------------- genkan
# the door's centre on its facade, the facade's outward normal and its along-direction
if LAYOUT == "n":
    dc, nv, tv = Vector((W / 2 - {"gable": 1.9, "hip": 1.8, "small": 1.7}[TYPE], D / 2, 0)), Vector((0, 1, 0)), Vector((-1, 0, 0))
elif LAYOUT == "s":
    dc, nv, tv = Vector((-W / 2 + {"gable": 8.5, "hip": 7.9, "small": 7.15}[TYPE], -D / 2, 0)), Vector((0, -1, 0)), Vector((1, 0, 0))
else:
    dc, nv, tv = Vector((W / 2, -D / 2 + (EAST_DOOR[0] + EAST_DOOR[1]) / 2, 0)), Vector((1, 0, 0)), Vector((0, 1, 0))


def pbox(a0, a1, d0, d1, z0, z1, mat):
    """A box a0..a1 along the facade from the door's centre, d0..d1 out from the facade."""
    pts = [dc + tv * a + nv * d for a in (a0, a1) for d in (d0, d1)]
    fittings.box((min(q.x for q in pts), min(q.y for q in pts), z0), (max(q.x for q in pts), max(q.y for q in pts), z1), mat)


pbox(-1.0, 1.0, 0.0, 1.0, F0 + 2.4, F0 + 2.5, unit)               # porch roof
pbox(-1.0, 1.0, 0.98, 1.02, F0 + 2.3, F0 + 2.5, alu)
pbox(-1.2, 1.2, 0.0, 1.1, 0.0, F0 - 0.15, found)                  # landing
pbox(-0.9, 0.9, 1.1, 1.45, 0.0, (F0 - 0.15) / 2, found)           # step
pbox(0.65, 0.8, 0.02, 0.1, F0 + 1.9, F0 + 2.1, lamp)              # porch lamp
pbox(0.85, 1.1, 0.0, 0.03, F0 + 1.3, F0 + 1.45, alu)              # intercom

# ----------------------------------------------------------------------------- services
# air-conditioner outdoor unit on the east side; its insulated pipes run into a pipe cover that
# climbs the wall to the upstairs room
E, Y_ = Vector((1, 0, 0)), Vector((0, 1, 0))
base = Vector((W / 2 + SIDE_T, -D / 2 + 1.3, 0))
c, front = outdoor_unit(fittings, base, Y_, E)
cover_y = -D / 2 + 1.95
CTOP = F1 + 0.6 if FLOORS > 1 else TOP - 0.6
for (z0, z1) in ((0.35, 1.3), (1.3, 2.4), (2.4, CTOP)) if FLOORS > 1 else ((0.35, 1.3), (1.3, CTOP)):
    obox(fittings, Vector((W / 2 + SIDE_T + 0.045, cover_y, (z0 + z1) / 2)), (0.1, 0.08, z1 - z0 - 0.01), Y_, E, unit, 0.01)
    obox(fittings, Vector((W / 2 + SIDE_T + 0.045, cover_y, z1)), (0.115, 0.095, 0.03), Y_, E, unit, 0.006)   # joint
obox(fittings, Vector((W / 2 + SIDE_T + 0.045, cover_y, 0.3)), (0.12, 0.1, 0.12), Y_, E, unit, 0.02)          # elbow
rcyl(fittings, (W / 2 + SIDE_T + 0.06, -D / 2 + 1.7, 0.3), (W / 2 + SIDE_T + 0.06, cover_y - 0.05, 0.3), 0.022, curtain)
rcyl(fittings, (W / 2 + SIDE_T + 0.1, -D / 2 + 1.7, 0.27), (W / 2 + SIDE_T + 0.1, cover_y - 0.05, 0.27), 0.016, curtain)
obox(fittings, Vector((W / 2 + SIDE_T + 0.03, cover_y, CTOP + 0.02)), (0.14, 0.06, 0.1), Y_, E, unit, 0.015)    # wall cap

# heat-pump water heater on the west side: the tank, its heat-pump unit, pipes into the wall
Wv = Vector((-1, 0, 0))
tank = Vector((-W / 2 - SIDE_T - 0.36, D / 2 - 1.3, 0.0))
obox(fittings, tank + Vector((0, 0, 0.04)), (0.7, 0.7, 0.08), -Y_, Wv, found, 0.01)                         # pad
obox(fittings, tank + Vector((0, 0, 0.08 + 0.9)), (0.62, 0.62, 1.8), -Y_, Wv, unit, 0.05)                    # tank
obox(fittings, tank + Vector((-0.315, 0, 1.25)), (0.2, 0.01, 0.12), -Y_, Wv, dark, 0.004)                   # control panel
obox(fittings, tank + Vector((-0.315, 0, 0.5)), (0.36, 0.01, 0.25), -Y_, Wv, unit, 0.01)                    # pipe hatch
for k in range(3):
    rcyl(fittings, tank + Vector((0.31, -0.15 + 0.15 * k, 0.25)), Vector((-W / 2 - SIDE_T, D / 2 - 1.45 + 0.15 * k, 0.25)), 0.018, unit)
outdoor_unit(fittings, Vector((-W / 2 - SIDE_T, D / 2 - 2.5, 0)), -Y_, Wv, w=0.72, h=0.6)

for b in (walls, fittings, roofing):
    b.finish(collection="HOUSE")

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
