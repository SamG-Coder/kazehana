"""Tsukumodō's shop fittings, each its own quality-gated prop (built empty: no stock, no cards;
those are their own assets, approved one at a time).

    blender --background --factory-startup --python tools/blender/build_shop_props.py -- <id>

ids:
    shop_showcase      low glass showcase (1.2 x 0.5 x 0.95 m): the singles case, three make the counter run
    shop_tower_case    tall glass tower case (0.45 x 0.45 x 1.8 m)
    shop_counter       the till counter (1.2 x 0.6 x 0.95 m): wood-panelled front, laminate top
    shop_wall_shelf    wall shelving module (0.9 x 0.45 x 1.8 m): pegboard back, four shelves with price rails
    shop_battle_table  play table (1.8 x 0.9 x 0.72 m): laminate top, steel legs
    shop_pipe_chair    folding steel pipe chair with vinyl seat and back
    shop_tv            43-inch TV on a tilting corner wall arm

-> assets/props/<id>/<id>.gltf

Frame (Blender): origin on the floor under the middle of the footprint, +Y the customer (front)
side, -Y the back (against the wall, or the staff side). The TV's origin is the bottom of its
wall plate; it is placed at mounting height.
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
PID = sys.argv[sys.argv.index("--") + 1]
OUT = os.path.join(ROOT, "assets", "props", PID, PID + ".gltf")

kz.reset_scene()
_c = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(_c)

DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
alu = P("prop_aluminium", (0.55, 0.55, 0.54), DETAIL, roughness=0.3, metallic=0.85, normal_strength=0.1, grain=0.1)
chrome = P("prop_chrome", (0.62, 0.62, 0.62), DETAIL, roughness=0.15, metallic=1.0, normal_strength=0.05, grain=0.05)
melamine = P("prop_white_melamine", (0.78, 0.78, 0.76), DETAIL, roughness=0.45, normal_strength=0.08, grain=0.12)
kick = P("prop_kick_plate", (0.06, 0.06, 0.06), DETAIL, roughness=0.6, normal_strength=0.2)
steel = P("prop_shelf_steel", (0.74, 0.74, 0.72), DETAIL, roughness=0.45, metallic=0.5, normal_strength=0.1, grain=0.1)
laminate = P("prop_laminate_top", (0.70, 0.69, 0.65), DETAIL, roughness=0.35, normal_strength=0.06, grain=0.12)
edge = P("prop_edge_band", (0.12, 0.11, 0.10), DETAIL, roughness=0.4, normal_strength=0.1)
oak = P("prop_table_oak", (0.46, 0.31, 0.17), "Wood_Planks", roughness=0.45, normal_strength=0.5)
woodpanel = P("prop_counter_wood", (0.36, 0.22, 0.11), "Wood_Planks", roughness=0.45, normal_strength=0.6)
vinyl = P("prop_seat_vinyl", (0.08, 0.10, 0.22), "Wood_Planks", roughness=0.4, normal_strength=0.15, grain=0.15)
rubber = P("prop_rubber", (0.03, 0.03, 0.03), DETAIL, roughness=0.8, normal_strength=0.2)
plastic = P("prop_black_plastic", (0.025, 0.025, 0.027), DETAIL, roughness=0.35, normal_strength=0.08, grain=0.05)
price = P("prop_price_rail", (0.82, 0.82, 0.80), DETAIL, roughness=0.4, normal_strength=0.05, grain=0.05)
glass = kz.plain_material("prop_clear_glass", (0.9, 0.95, 0.93), roughness=0.02)
led = kz.plain_material("kz_neon_case_led", (0.95, 0.95, 0.93), roughness=0.3, emission=(1.0, 0.98, 0.95), emission_strength=1.0)
screen = kz.plain_material("prop_tv_glass", (0.012, 0.013, 0.015), roughness=0.05)


def pegboard_material():
    """Pegboard: off-white hardboard with 5 mm holes on a 25 mm grid (a generated texture, 1 m
    per repeat), the scan's normal for the board's grain."""
    import numpy as np
    path = os.path.join(kz.PAINT_CACHE, "prop_pegboard.png")
    if not os.path.exists(path):
        os.makedirs(kz.PAINT_CACHE, exist_ok=True)
        n = 1024
        y, x = np.mgrid[0:n, 0:n].astype(np.float32)
        pitch = n / 40.0
        dx = (x % pitch) - pitch / 2
        dy = (y % pitch) - pitch / 2
        r = np.sqrt(dx * dx + dy * dy)
        hole = np.clip((r - 2.6) / 1.2, 0.0, 1.0)  # 0 in the hole, 1 on the board
        rng = np.random.default_rng(4)
        grain = 1.0 + (rng.random((n, n)).astype(np.float32) - 0.5) * 0.05
        board = np.array([0.66, 0.64, 0.58], np.float32)
        lin = board[None, None, :] * (grain * hole)[..., None] + np.array([0.02, 0.02, 0.02], np.float32) * (1 - hole)[..., None]
        srgb = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(np.clip(lin, 0, None), 1 / 2.4) - 0.055)
        out = np.concatenate([np.clip(srgb, 0, 1), np.ones((n, n, 1), np.float32)], axis=-1)
        img = bpy.data.images.new("prop_pegboard", n, n, alpha=True)
        img.pixels.foreach_set(out[::-1].ravel())
        img.filepath_raw = path
        img.file_format = "PNG"
        img.save()
        bpy.data.images.remove(img)
    _, nrm, orm = kz.fab_textures(DETAIL)
    m = bpy.data.materials.new("prop_pegboard")
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(path, "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(nrm, "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = 0.1
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(orm, "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    return m


b = kz.MeshBuilder(PID, uv_scale=1.0)


def _set_mat(verts, mat):
    idx = b._mat_index(mat)
    for f in {f for v in verts if v.is_valid for f in v.link_faces}:
        f.material_index = idx


def box(x0, y0, z0, x1, y1, z1, mat):
    b.box((min(x0, x1), min(y0, y1), min(z0, z1)), (max(x0, x1), max(y0, y1), max(z0, z1)), mat)


def rbox(c, size, mat, bevel=0.004, rot=None):
    """A box with rounded edges, centred at c (size along x, y, z; optional rotation matrix)."""
    res = bmesh.ops.create_cube(b.bm, size=1.0)
    verts = res["verts"]
    m = Matrix.Translation(Vector(c)) @ (rot.to_4x4() if rot else Matrix.Identity(4)) @ Matrix.Diagonal((*size, 1.0)).to_4x4()
    bmesh.ops.transform(b.bm, matrix=m, verts=verts)
    if bevel > 0:
        edges = list({e for v in verts for e in v.link_edges})
        out = bmesh.ops.bevel(b.bm, geom=edges, offset=bevel, segments=3, profile=0.5, affect="EDGES")
        verts = list(verts) + list(out["verts"])
    _set_mat(verts, mat)


def tube(p0, p1, r, mat, seg=14):
    p0, p1 = Vector(p0), Vector(p1)
    d = p1 - p0
    res = bmesh.ops.create_cone(b.bm, cap_ends=True, segments=seg, radius1=r, radius2=r, depth=d.length)
    rot = Vector((0, 0, 1)).rotation_difference(d.normalized()).to_matrix().to_4x4()
    bmesh.ops.transform(b.bm, matrix=Matrix.Translation((p0 + p1) / 2) @ rot, verts=res["verts"])
    _set_mat(res["verts"], mat)


def glass_case(x0, x1, y0, y1, z0, z1, shelves, back_doors=True):
    """A glass box on a frame: aluminium corner posts and rails, glass on all sides and the top,
    glass shelves on small brackets, an LED strip under the front top rail."""
    F = 0.022
    for (x, y) in ((x0, y0), (x1, y0), (x0, y1), (x1, y1)):
        rbox(((x + (F / 2 if x == x0 else -F / 2)), (y + (F / 2 if y == y0 else -F / 2)), (z0 + z1) / 2), (F, F, z1 - z0), alu, 0.003)
    for z in (z0 + F / 2, z1 - F / 2):
        for (y,) in ((y0 + F / 2,), (y1 - F / 2,)):
            rbox(((x0 + x1) / 2, y, z), (x1 - x0, F, F), alu, 0.003)
        for x in (x0 + F / 2, x1 - F / 2):
            rbox((x, (y0 + y1) / 2, z), (F, y1 - y0, F), alu, 0.003)
    g = 0.005
    box(x0 + F, y1 - F / 2 - g, z0 + F, x1 - F, y1 - F / 2 + g, z1 - F, glass)          # front
    for x in (x0 + F / 2, x1 - F / 2):
        box(x - g, y0 + F, z0 + F, x + g, y1 - F, z1 - F, glass)                        # sides
    box(x0 + F, y0 + F, z1 - F / 2 - g, x1 - F, y1 - F, z1 - F / 2 + g, glass)          # top
    if back_doors:
        # two sliding glass panes on the staff side, overlapping, with a lock
        xm = (x0 + x1) / 2
        box(x0 + F, y0 + F / 2 - g, z0 + F, xm + 0.04, y0 + F / 2 + g, z1 - F, glass)
        box(xm - 0.04, y0 + F / 2 + 0.012 - g, z0 + F, x1 - F, y0 + F / 2 + 0.012 + g, z1 - F, glass)
        rbox((xm, y0 + 0.004, z0 + 0.1), (0.03, 0.012, 0.04), chrome, 0.003)
    else:
        box(x0 + F, y0 + F / 2 - g, z0 + F, x1 - F, y0 + F / 2 + g, z1 - F, glass)
    for z in shelves:
        box(x0 + F + 0.004, y0 + F + 0.004, z - 0.004, x1 - F - 0.004, y1 - F - 0.004, z + 0.004, glass)
        for (x, y) in ((x0 + F, y0 + F), (x1 - F, y0 + F), (x0 + F, y1 - F), (x1 - F, y1 - F)):
            box(x - 0.012 if x > x0 + F else x, y - 0.012 if y > y0 + F else y, z - 0.014, x + 0.012 if x == x0 + F else x, y + 0.012 if y == y0 + F else y, z - 0.004, chrome)
    # the LED strip lighting the case from the front top rail
    box(x0 + 0.05, y1 - F - 0.012, z1 - F - 0.008, x1 - 0.05, y1 - F - 0.002, z1 - F, led)
    box(x0 + 0.04, y1 - F - 0.02, z1 - F - 0.012, x1 - 0.04, y1 - F, z1 - F - 0.008, alu)


# ----------------------------------------------------------------------------- the props
if PID == "shop_showcase":
    L, Dp = 1.2, 0.5
    x0, x1, y0, y1 = -L / 2, L / 2, -Dp / 2, Dp / 2
    rbox((0, 0.0, 0.04), (L - 0.06, Dp - 0.06, 0.08), kick, 0.004)               # plinth (recessed)
    rbox((0, 0, 0.19), (L, Dp, 0.22), melamine, 0.006)                            # base cabinet
    # staff side: two sliding doors with finger pulls, a lock
    for k, x in enumerate((-L / 4, L / 4)):
        box(x - L / 4 + 0.01, y0 - 0.006 - 0.008 * k, 0.1, x + L / 4 - 0.01, y0 - 0.002 - 0.008 * k, 0.28, melamine)
        box(x + (0.2 if k == 0 else -0.2) - 0.02, y0 - 0.016, 0.17, x + (0.2 if k == 0 else -0.2) + 0.02, y0 - 0.012, 0.22, chrome)
    rbox((0, y0 - 0.012, 0.26), (0.025, 0.01, 0.025), chrome, 0.003)
    box(x0, y0, 0.3, x1, y1, 0.31, melamine)                                       # deck
    glass_case(x0, x1, y0, y1, 0.31, 0.95, [0.62])

elif PID == "shop_tower_case":
    S = 0.45
    x0, x1, y0, y1 = -S / 2, S / 2, -S / 2, S / 2
    rbox((0, 0, 0.05), (S - 0.04, S - 0.04, 0.1), kick, 0.004)
    rbox((0, 0, 0.3), (S, S, 0.4), melamine, 0.006)
    box(x0 + 0.02, y1, 0.12, x1 - 0.02, y1 + 0.004, 0.48, melamine)                # cabinet door
    rbox((x1 - 0.05, y1 + 0.01, 0.42), (0.015, 0.01, 0.06), chrome, 0.003)
    box(x0, y0, 0.5, x1, y1, 0.51, melamine)
    glass_case(x0, x1, y0, y1, 0.51, 1.75, [0.82, 1.12, 1.42], back_doors=False)
    # the front pane is a hinged door: its lock
    rbox((x1 - 0.035, y1 + 0.006, 0.6), (0.02, 0.012, 0.03), chrome, 0.003)
    rbox((0, 0, 1.775), (S + 0.01, S + 0.01, 0.05), melamine, 0.006)              # top cap
    box(-0.08, -0.08, 1.749, 0.08, 0.08, 1.751, led)                               # downlight

elif PID == "shop_counter":
    L, Dp, H = 1.2, 0.6, 0.95
    y0, y1 = -Dp / 2, Dp / 2
    rbox((0, 0.02, 0.05), (L - 0.04, Dp - 0.06, 0.1), kick, 0.004)
    # the customer side: vertical wood panelling between two posts
    box(-L / 2, y1 - 0.03, 0.1, L / 2, y1 - 0.02, H - 0.035, woodpanel)
    n = 10
    for k in range(n + 1):
        x = -L / 2 + 0.01 + k * (L - 0.02) / n
        box(x - 0.004, y1 - 0.02, 0.1, x + 0.004, y1 - 0.012, H - 0.035, woodpanel)
    for x in (-L / 2, L / 2 - 0.03):
        box(x, y0, 0.1, x + 0.03, y1, H - 0.035, melamine)                         # end panels
    # the staff side: two open shelves and a cupboard
    for z in (0.12, 0.45):
        box(-L / 2 + 0.03, y0 + 0.02, z, L / 2 - 0.03, y1 - 0.03, z + 0.018, melamine)
    box(-L / 2 + 0.03, y0 + 0.02, 0.1, -L / 2 + 0.05, y1 - 0.03, H - 0.035, melamine)
    box(0.1, y0, 0.1, L / 2 - 0.03, y0 + 0.018, 0.4, melamine)                     # cupboard door
    rbox((0.15, y0 - 0.008, 0.33), (0.012, 0.012, 0.08), chrome, 0.003)
    # the top: laminate with dark edge banding, a little overhang
    box(-L / 2 - 0.02, y0 - 0.02, H - 0.035, L / 2 + 0.02, y1 + 0.04, H - 0.005, laminate)
    for (a, bb, c, d) in ((-L / 2 - 0.022, L / 2 + 0.022, y1 + 0.04, y1 + 0.042), (-L / 2 - 0.022, L / 2 + 0.022, y0 - 0.022, y0 - 0.02)):
        box(a, c, H - 0.036, bb, d, H - 0.004, edge)
    for x in (-L / 2 - 0.022, L / 2 + 0.02):
        box(x, y0 - 0.02, H - 0.036, x + 0.002, y1 + 0.04, H - 0.004, edge)

elif PID == "shop_wall_shelf":
    Wd, Dp, H = 0.9, 0.45, 1.8
    y0 = -Dp / 2
    peg = pegboard_material()
    # uprights with slots, the pegboard back between them, the base and its kick plate
    for x in (-Wd / 2, Wd / 2 - 0.04):
        box(x, y0, 0.0, x + 0.04, y0 + 0.04, H, steel)
        for k in range(int((H - 0.2) / 0.05)):
            z = 0.2 + k * 0.05
            box(x + 0.015, y0 + 0.04, z, x + 0.025, y0 + 0.0405, z + 0.022, kick)
    box(-Wd / 2 + 0.04, y0 + 0.012, 0.12, Wd / 2 - 0.04, y0 + 0.018, H - 0.02, peg)
    box(-Wd / 2, y0, 0.0, Wd / 2, Dp / 2 - 0.02, 0.1, steel)                     # base deck
    box(-Wd / 2, Dp / 2 - 0.02, 0.0, Wd / 2, Dp / 2, 0.1, kick)
    box(-Wd / 2, y0, H - 0.02, Wd / 2, y0 + 0.1, H, steel)                      # header
    for z in (0.5, 0.85, 1.2, 1.55):
        d = 0.4 if z < 1.0 else 0.35
        box(-Wd / 2 + 0.04, y0 + 0.04, z - 0.02, Wd / 2 - 0.04, y0 + 0.04 + d, z, steel)
        box(-Wd / 2 + 0.04, y0 + 0.04 + d, z - 0.035, Wd / 2 - 0.04, y0 + 0.052 + d, z + 0.005, price)
        for x in (-Wd / 2 + 0.042, Wd / 2 - 0.052):                              # brackets
            box(x, y0 + 0.04, z - 0.06, x + 0.01, y0 + 0.04 + d * 0.8, z - 0.02, steel)

elif PID == "shop_battle_table":
    # a solid oak table: a 30 mm top with rounded edges, an apron, four square legs, felt feet
    L, Dp, H = 1.8, 0.9, 0.72
    rbox((0, 0, H - 0.015), (L, Dp, 0.03), oak, 0.006)
    for y in (-Dp / 2 + 0.07, Dp / 2 - 0.07):
        rbox((0, y, H - 0.075), (L - 0.2, 0.022, 0.09), oak, 0.003)
    for x in (-L / 2 + 0.07, L / 2 - 0.07):
        rbox((x, 0, H - 0.075), (0.022, Dp - 0.2, 0.09), oak, 0.003)
        for y in (-Dp / 2 + 0.07, Dp / 2 - 0.07):
            rbox((x, y, (H - 0.03) / 2), (0.055, 0.055, H - 0.03), oak, 0.004)
            rbox((x, y, 0.002), (0.05, 0.05, 0.004), rubber, 0.0)

elif PID == "shop_pipe_chair":
    # a stacking chair: four chrome legs (the back pair rising into the back posts), a padded
    # seat on the frame, a padded back panel between the posts
    R = 0.011
    SZ = 0.44
    SW, SD = 0.42, 0.40
    for sx in (-1, 1):
        x = sx * (SW / 2 - 0.01)
        tube((x * 1.04, SD / 2 - 0.01, 0.0), (x, SD / 2 - 0.03, SZ - 0.03), R, chrome)        # front leg
        tube((x * 1.04, -SD / 2 - 0.02, 0.0), (x, -SD / 2 + 0.02, SZ - 0.03), R, chrome)      # back leg
        tube((x, -SD / 2 + 0.02, SZ - 0.03), (x, -SD / 2 - 0.03, 0.86), R, chrome)           # back post
        tube((x, -SD / 2 + 0.02, SZ - 0.03), (x, SD / 2 - 0.03, SZ - 0.03), R, chrome)        # seat rail
        tube((x * 1.04, SD / 2 - 0.01, 0.0), (x * 1.04, SD / 2 - 0.01, 0.012), R * 1.3, rubber)
        tube((x * 1.04, -SD / 2 - 0.02, 0.0), (x * 1.04, -SD / 2 - 0.02, 0.012), R * 1.3, rubber)
    for y in (SD / 2 - 0.03, -SD / 2 + 0.02):
        tube((-(SW / 2 - 0.01), y, SZ - 0.03), ((SW / 2 - 0.01), y, SZ - 0.03), R * 0.9, chrome)
    rbox((0, 0, SZ - 0.012), (SW - 0.02, SD - 0.02, 0.012), rubber, 0.003)                     # seat pan
    rbox((0, 0.005, SZ + 0.02), (SW, SD, 0.05), vinyl, 0.018)                                  # cushion
    # the back: a padded panel between the posts, leaning back with them
    ang = math.atan2(0.05, 0.86 - SZ)
    rot = Matrix.Rotation(ang, 3, "X")
    yb = -SD / 2 - 0.005
    rbox((0, yb - 0.012, 0.73), (SW - 0.035, 0.035, 0.2), vinyl, 0.012, rot)
    rbox((0, yb - 0.032, 0.73), (SW - 0.05, 0.008, 0.18), rubber, 0.0, rot)

elif PID == "shop_tv":
    # 43": 0.97 x 0.57 m, 6 cm deep; on a tilting arm from a corner wall plate (the plate's bottom
    # is the origin; the arm reaches 0.35 m out, the screen tilts 10 degrees down)
    TW, TH, TD = 0.97, 0.57, 0.06
    ZA = 0.3  # the arm's height above the origin (the screen's bottom edge sits at the origin)
    rbox((0, -0.005, ZA), (0.12, 0.01, 0.3), plastic, 0.004)                     # wall plate
    for z in (ZA - 0.11, ZA + 0.11):
        tube((0, -0.01, z), (0, -0.014, z), 0.007, chrome, seg=10)                  # screws
    tube((0, 0.0, ZA), (0, 0.18, ZA), 0.022, plastic)                          # arm, two links
    tube((0, 0.18, ZA), (0, 0.33, ZA), 0.018, plastic)
    rbox((0, 0.33, ZA), (0.06, 0.04, 0.08), plastic, 0.006)                      # tilt head
    tilt = Matrix.Rotation(math.radians(-10.0), 3, "X")
    c = Vector((0, 0.38 + TD / 2, ZA))
    rbox(c, (0.3, 0.02, 0.22), plastic, 0.004, tilt)                               # VESA bracket
    tv_c = c + tilt @ Vector((0, 0.02 + TD / 2, 0))
    rbox(tv_c, (TW, TD, TH), plastic, 0.01, tilt)                                  # body
    rbox(tv_c + tilt @ Vector((0, -TD / 2 - 0.005, -0.05)), (0.5, 0.012, 0.3), plastic, 0.004, tilt)  # rear bulge
    rbox(tv_c + tilt @ Vector((0, TD / 2 + 0.0005, 0.006)), (TW - 0.024, 0.002, TH - 0.04), screen, 0.0, tilt)
    rbox(tv_c + tilt @ Vector((0, TD / 2 + 0.001, -TH / 2 + 0.009)), (0.03, 0.002, 0.006), chrome, 0.0, tilt)  # logo
    rbox(tv_c + tilt @ Vector((0.4, TD / 2 + 0.001, -TH / 2 + 0.008)), (0.004, 0.002, 0.004), led, 0.0, tilt)  # standby light
else:
    raise SystemExit("unknown prop " + PID)

b.finish(collection="PROP")
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
