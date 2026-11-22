"""The trading-card binder: a 4-ring "collection file" and its 9-pocket top-loading refill sheet.
Two quality-gated props (docs/ASSET_QUALITY.md), built procedurally with their textures:

    D:/Blender/blender.exe --background --factory-startup --python tools/binder/build_binder.py -- binder_ring4
    D:/Blender/blender.exe --background --factory-startup --python tools/binder/build_binder.py -- binder_sheet9

-> assets/props/<id>/<id>.gltf  (+ textures/, and layout.json with the dimensions the game uses)

Sizes follow the cards from the card-templates session (tools/cardgen/src/props.rs): 63 x 88 x
0.31 mm with 2.94 mm corners. A pocket is 66.5 x 92.5 mm inside, so a card has 1.75 mm play each
side. The sheet is A4 tall (297 mm) with ISO 838 four-hole punching (80 mm pitch, 6 mm holes,
12 mm from the edge). Pockets are open at the top; two cards fit back to back in one pocket.

Frame (Blender, Z up; the glTF exporter makes it +Y up):
- binder_ring4: lying open flat. The spine runs along Y, centred on x = 0; the back board (the
  right-hand page side) is +X, the front board -X. z = 0 is the underside of the boards. The four
  O-rings stand on the spine in the XZ plane.
- binder_sheet9: lying flat, front side up (+Z). x = 0 is the line through the hole centres, the
  pockets run to +X; y = 0 is the sheet's middle; the films sit at z = +-0.45 mm.
"""

import json
import math
import os
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PID = sys.argv[sys.argv.index("--") + 1]
OUT_DIR = os.path.join(ROOT, "assets", "props", PID)
SRC_TEX = os.path.join(ROOT, "assets", "cache", "binder_src", PID)
MM = 0.001

# ----------------------------------------------------------------------------- dimensions (mm)

def card_size():
    """The card (mm): read from the card-templates session's slab (tools/cardgen/src/props.rs),
    so the sleeves, the sheet and the binder all follow the card if it ever changes size."""
    import re
    vals = {"CARD_W": 63.0, "CARD_H": 88.0, "CARD_T": 0.31, "CORNER": 2.94}
    try:
        src = open(os.path.join(ROOT, "tools", "cardgen", "src", "props.rs"), encoding="utf-8").read()
        for k in vals:
            m = re.search(r"^const %s: f32 = ([0-9.]+);" % k, src, re.M)
            if m:
                vals[k] = float(m.group(1))
    except OSError:
        pass
    return vals["CARD_W"], vals["CARD_H"], vals["CARD_T"], vals["CORNER"]


CARD_W, CARD_H, CARD_T, CARD_CORNER = card_size()
# everything below follows the card
SIDE_PLAY = 1.75              # room each side of a card in its pocket
HEAD_ROOM = 4.5               # pocket height beyond the card (it seats 1.5 mm off the bottom weld)
POCKET_W, POCKET_H = CARD_W + 2 * SIDE_PLAY, CARD_H + HEAD_ROOM
SEAM = 2.4                    # weld between pockets
EDGE_MARGIN = 7.25            # weld above the top row and below the bottom row
SHEET_H = 3 * POCKET_H + 2 * SEAM + 2 * EDGE_MARGIN          # 297 mm (A4) for a 63 x 88 card
HOLE_D = 6.0
HOLE_EDGE = 12.0              # hole centre to the sheet's binding edge
# ISO 838 four-hole punching (80 mm pitch), centred; wider pitch if the sheet grows a lot
HOLE_PITCH = max(80.0, (SHEET_H - 57.0) / 3)
HOLES_Y = tuple((k - 1.5) * HOLE_PITCH for k in range(4))
STRIP = 10.0                  # the welded binding strip runs from the edge to x = +STRIP
OUTER = 3.0                   # outer border weld
TOP_MARGIN = EDGE_MARGIN
FILM_GAP = 2 * CARD_T + 0.68  # inside a pocket the films are this far apart (two cards + air)

POCKET_X0 = STRIP             # left seam of the first column starts here
SHEET_X0 = -HOLE_EDGE
SHEET_X1 = POCKET_X0 + 3 * (SEAM + POCKET_W) + OUTER
SHEET_W = SHEET_X1 - SHEET_X0

RING_R = 13.7                 # O-ring centre-line radius (25 mm inside)
RING_WIRE = 1.25
SPINE_W = 40.0
OVERHANG = 9.0                # the boards stand this far past the sheets
BOARD_W, BOARD_H, BOARD_T = SHEET_X1 + OVERHANG + 1.5, SHEET_H + 2 * OVERHANG, 2.6
HINGE_GAP = 2.5
MECH_W, MECH_H, MECH_L = 18.0, 5.5, BOARD_H - 43.0
RING_CZ = BOARD_T + MECH_H + RING_R - 3.2   # ring centre height: its foot is inside the plate


def pocket_rect(col, row):
    """(x0, x1, y0, y1) of a pocket's inside, mm; row 0 is the top row."""
    x0 = POCKET_X0 + SEAM + col * (POCKET_W + SEAM)
    ytop = SHEET_H / 2 - TOP_MARGIN - row * (POCKET_H + SEAM)
    return x0, x0 + POCKET_W, ytop - POCKET_H, ytop


# ----------------------------------------------------------------------------- texture helpers

def to_srgb(lin):
    lin = np.clip(lin, 0.0, 1.0)
    return np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(lin, 1 / 2.4) - 0.055)


def save_png(name, rgb, srgb):
    """rgb: (H, W, 3) floats, row 0 at the bottom (Blender's pixel order, v = 0)."""
    os.makedirs(SRC_TEX, exist_ok=True)
    path = os.path.join(SRC_TEX, name + ".png")
    h, w = rgb.shape[:2]
    img = bpy.data.images.new(name, w, h, alpha=False)
    img.colorspace_settings.name = "sRGB" if srgb else "Non-Color"
    px = np.concatenate([np.clip(rgb, 0, 1), np.ones((h, w, 1), np.float32)], -1).astype(np.float32)
    img.pixels.foreach_set(px.ravel())
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)
    return path


def value_noise(n, cells, rng, tile=True):
    """Smooth value noise, (n, n), `cells` lattice cells across, tileable."""
    g = rng.random((cells, cells)).astype(np.float32)
    t = np.arange(n, dtype=np.float32) * cells / n
    i0 = np.floor(t).astype(int)
    f = t - i0
    f = f * f * (3 - 2 * f)
    i1 = (i0 + 1) % cells
    a = g[i0][:, i0] * (1 - f)[None, :] + g[i0][:, i1] * f[None, :]
    b = g[i1][:, i0] * (1 - f)[None, :] + g[i1][:, i1] * f[None, :]
    return a * (1 - f)[:, None] + b * f[:, None]


def fbm(n, base, octaves, rng, gain=0.5):
    out = np.zeros((n, n), np.float32)
    amp, tot = 1.0, 0.0
    for o in range(octaves):
        out += amp * value_noise(n, base * 2 ** o, rng)
        tot += amp
        amp *= gain
    return out / tot


def worley(n, cell_px, rng):
    """F1, F2 (in cell units) of jittered points, tileable."""
    g = int(round(n / cell_px))
    pts = rng.random((g, g, 2)).astype(np.float32)
    ys, xs = np.mgrid[0:n, 0:n].astype(np.float32) * (g / n)
    cy, cx = np.floor(ys).astype(int), np.floor(xs).astype(int)
    fy, fx = ys - cy, xs - cx
    d = []
    ids = []
    for oy in (-1, 0, 1):
        for ox in (-1, 0, 1):
            jy, jx = (cy + oy) % g, (cx + ox) % g
            p = pts[jy, jx]
            dy = oy + p[..., 0] - fy
            dx = ox + p[..., 1] - fx
            d.append(np.sqrt(dx * dx + dy * dy))
            ids.append(jy * g + jx)
    d = np.stack(d)
    ids = np.stack(ids)
    order = np.argsort(d, axis=0)
    f1 = np.take_along_axis(d, order[0:1], 0)[0]
    f2 = np.take_along_axis(d, order[1:2], 0)[0]
    id1 = np.take_along_axis(ids, order[0:1], 0)[0]
    return f1, f2, id1


def normal_from_height(h, strength):
    """Tangent-space normal map (OpenGL / glTF: +Y = +v) from a height field in pixels."""
    dx = (np.roll(h, -1, 1) - np.roll(h, 1, 1)) * 0.5
    dy = (np.roll(h, -1, 0) - np.roll(h, 1, 0)) * 0.5
    n = np.stack([-dx * strength, -dy * strength, np.ones_like(h)], -1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    return n * 0.5 + 0.5


def smoothstep(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0, 1)
    return t * t * (3 - 2 * t)


# ----------------------------------------------------------------------------- materials

def pbr_material(name, base, normal, orm, normal_strength=1.0, metallic_from_orm=True, double_sided=False):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    m.use_backface_culling = not double_sided
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = bpy.data.images.load(base)
    tb.image.colorspace_settings.name = "sRGB"
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = bpy.data.images.load(normal)
    tn.image.colorspace_settings.name = "Non-Color"
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = normal_strength
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = bpy.data.images.load(orm)
    to.image.colorspace_settings.name = "Non-Color"
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    if metallic_from_orm:
        nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
    else:
        bsdf.inputs["Metallic"].default_value = 0.0
    # glTF occlusion from the ORM red channel
    grp = bpy.data.node_groups.get("glTF Material Output")
    if grp is None:
        grp = bpy.data.node_groups.new("glTF Material Output", "ShaderNodeTree")
        grp.interface.new_socket("Occlusion", in_out="INPUT", socket_type="NodeSocketFloat")
    gn = nt.nodes.new("ShaderNodeGroup")
    gn.node_tree = grp
    nt.links.new(sep.outputs["Red"], gn.inputs["Occlusion"])
    return m


# ----------------------------------------------------------------------------- leatherette

def leatherette_textures(n=2048, span_mm=320.0):
    """PU leatherette cover: pebble grain, the board's stitched border and the spine. The texture
    spans span_mm square: a cover board takes u in [0, BOARD_W/span] (the hinge edge at u = 0),
    the spine u in [SPINE_U0, SPINE_U0 + SPINE_W/span]. Colour: a deep navy (the first binder;
    other colours and brands come with the story)."""
    rng = np.random.default_rng(7)
    px_mm = n / span_mm
    f1, f2, cid = worley(n, 0.95 * px_mm, rng)
    cell_h = (np.random.default_rng(3).random(cid.max() + 1).astype(np.float32))[cid]
    edge = f2 - f1                                       # 0 on a crease
    pebble = smoothstep(0.0, 0.32, edge) * (0.75 + 0.25 * cell_h)
    pebble = pebble - 0.12 * f1 ** 2                     # domed tops
    fine = fbm(n, 64, 3, rng)
    wrinkle = fbm(n, 6, 4, rng)
    h = pebble * 1.0 + fine * 0.25 + (wrinkle - 0.5) * 2.5
    crease = 1.0 - smoothstep(0.0, 0.16, edge)

    # stitched border on the board, 5 mm in; the needle line sinks into a channel
    v_mm = (np.arange(n, dtype=np.float32) + 0.5) / px_mm      # rows: v
    u_mm = (np.arange(n, dtype=np.float32) + 0.5) / px_mm      # cols: u
    U, V = np.meshgrid(u_mm, v_mm)
    inset = 5.0
    bx0, bx1, by0, by1 = inset, BOARD_W - inset, inset, BOARD_H - inset
    on_board = (U < BOARD_W) & (V < BOARD_H)
    # distance to the stitch rectangle's outline, and the position along it (for the dashes)
    dxl, dxr, dyb, dyt = np.abs(U - bx0), np.abs(U - bx1), np.abs(V - by0), np.abs(V - by1)
    inside_x = (U > bx0 - 1) & (U < bx1 + 1)
    inside_y = (V > by0 - 1) & (V < by1 + 1)
    d_vert = np.where(inside_y, np.minimum(dxl, dxr), 99.0)
    d_horz = np.where(inside_x, np.minimum(dyb, dyt), 99.0)
    d_line = np.minimum(d_vert, d_horz)
    along = np.where(d_vert < d_horz, V, U)
    stitch_len, stitch_gap = 3.2, 1.3
    ph = np.mod(along, stitch_len + stitch_gap)
    in_dash = smoothstep(0.0, 0.35, ph) * (1 - smoothstep(stitch_len - 0.35, stitch_len, ph))
    thread_w = 0.5
    thread = (1 - smoothstep(thread_w * 0.55, thread_w, d_line)) * in_dash * on_board
    channel = (1 - smoothstep(0.4, 1.4, d_line)) * on_board
    # the twist of the thread (diagonal ridges)
    twist = 0.5 + 0.5 * np.sin((along + d_line) * 2 * math.pi / 0.55)
    h = h - channel * 1.6 + thread * (2.6 + 0.4 * twist)
    # the board's edge rolls over: soften the grain within 1.5 mm of the board edge
    d_edge = np.minimum(np.minimum(U, BOARD_W - U), np.minimum(V, BOARD_H - V))
    roll = smoothstep(0.0, 1.5, d_edge)
    h = h * np.where(on_board, 0.5 + 0.5 * roll, 1.0)

    navy = np.array([0.018, 0.028, 0.062], np.float32)    # linear
    thread_col = np.array([0.060, 0.072, 0.120], np.float32)
    var = 0.92 + 0.12 * (wrinkle - 0.5) + 0.05 * (fine - 0.5)
    col = navy[None, None, :] * (var * (1.0 - 0.28 * crease))[..., None]
    col = col * (1 - 0.35 * channel)[..., None]
    col = col * (1 - thread[..., None]) + thread_col[None, None, :] * (0.9 + 0.2 * twist)[..., None] * thread[..., None]

    rough = 0.52 + 0.14 * crease - 0.06 * (pebble - 0.5) + 0.05 * (wrinkle - 0.5)
    rough = rough * (1 - thread) + 0.82 * thread
    ao = 1.0 - 0.25 * crease - 0.35 * channel * (1 - thread)
    orm = np.stack([ao, np.clip(rough, 0.3, 0.95), np.zeros_like(ao)], -1)
    nrm = normal_from_height(h, 0.55)
    return (save_png("binder_cover_base", to_srgb(col), True),
            save_png("binder_cover_normal", nrm, False),
            save_png("binder_cover_orm", orm, False))


def lining_textures(n=2048):
    """The inside lining: smooth black PVC with a fine calf grain (cards read best on black)."""
    rng = np.random.default_rng(11)
    f1, f2, _ = worley(n, 4.0, rng)
    edge = f2 - f1
    fine = fbm(n, 96, 3, rng)
    soft = fbm(n, 8, 4, rng)
    h = smoothstep(0, 0.4, edge) * 0.35 + fine * 0.2 + (soft - 0.5) * 1.2
    col = np.array([0.022, 0.022, 0.024], np.float32)[None, None, :] * (0.95 + 0.1 * (soft - 0.5) - 0.06 * (1 - smoothstep(0, 0.2, edge)))[..., None]
    rough = 0.42 + 0.08 * (1 - smoothstep(0, 0.25, edge)) + 0.05 * (soft - 0.5)
    orm = np.stack([np.ones_like(rough) * (1.0 - 0.06 * (1 - smoothstep(0, 0.2, edge))), rough, np.zeros_like(rough)], -1)
    return (save_png("binder_lining_base", to_srgb(col), True),
            save_png("binder_lining_normal", normal_from_height(h, 0.35), False),
            save_png("binder_lining_orm", orm, False))


def metal_textures(n=2048):
    """Nickel-plated steel for the ring mechanism: polished, with faint handling marks."""
    rng = np.random.default_rng(5)
    fine = fbm(n, 128, 2, rng)
    blotch = fbm(n, 6, 4, rng)
    # long faint scratches along u (the rings are unwrapped along their length)
    scratch = np.zeros((n, n), np.float32)
    for _ in range(90):
        r = rng.integers(0, n)
        w = rng.random() * 0.6 + 0.2
        scratch[r, :] += w * (0.5 + 0.5 * np.sin(np.linspace(0, rng.random() * 20, n)))
    scratch = np.clip(scratch, 0, 1)
    h = fine * 0.3 + scratch * 0.4
    col = np.array([0.62, 0.60, 0.56], np.float32)[None, None, :] * (0.97 + 0.06 * (blotch - 0.5))[..., None]
    rough = 0.16 + 0.08 * blotch + 0.1 * scratch
    orm = np.stack([np.ones_like(rough), rough, np.ones_like(rough)], -1)
    return (save_png("binder_metal_base", to_srgb(col), True),
            save_png("binder_metal_normal", normal_from_height(h, 0.4), False),
            save_png("binder_metal_orm", orm, False))


# ----------------------------------------------------------------------------- sheet film

def film_textures(n=2048):
    """The polypropylene film of the sheet (the same texture on both films). Plain clear film with
    a faint orange-peel, knurled welds along the seams and round the holes, and light handling
    scuffs. UV: u across the sheet (SHEET_X0..SHEET_X1), v up it (-H/2..H/2)."""
    rng = np.random.default_rng(21)
    u_mm = SHEET_X0 + (np.arange(n, dtype=np.float32) + 0.5) / n * SHEET_W
    v_mm = -SHEET_H / 2 + (np.arange(n, dtype=np.float32) + 0.5) / n * SHEET_H
    X, Y = np.meshgrid(u_mm, v_mm)
    # weld mask: the binding strip, the seams between pockets, the outer border
    weld = np.zeros((n, n), np.float32)
    inside = np.zeros((n, n), bool)
    for c in range(3):
        for r in range(3):
            x0, x1, y0, y1 = pocket_rect(c, r)
            inside |= (X > x0) & (X < x1) & (Y > y0) & (Y < y1)
    weld = (~inside).astype(np.float32)
    weld *= (X > STRIP - 0.2).astype(np.float32) * 1.0 + (X <= STRIP - 0.2) * 0.0  # the strip itself is smooth
    # knurl: a fine diamond pattern on the welds
    k = 0.9
    knurl = (np.sin((X + Y) * 2 * math.pi / k) * np.sin((X - Y) * 2 * math.pi / k)) * 0.5 + 0.5
    # reinforcement rings round the holes (embossed)
    ring = np.zeros((n, n), np.float32)
    for hy in HOLES_Y:
        d = np.sqrt(X ** 2 + (Y - hy) ** 2)
        ring += smoothstep(HOLE_D / 2 + 0.2, HOLE_D / 2 + 0.6, d) * (1 - smoothstep(HOLE_D / 2 + 1.6, HOLE_D / 2 + 2.0, d))
    peel = fbm(n, 48, 3, rng)
    wave = fbm(n, 5, 3, rng)
    # scuffs: short random strokes
    scuff = np.zeros((n, n), np.float32)
    srng = np.random.default_rng(4)
    for _ in range(260):
        cx, cy = srng.integers(0, n, 2)
        ang = srng.random() * math.pi
        ln = srng.integers(8, 60)
        for t in range(ln):
            x = int(cx + math.cos(ang) * t) % n
            y = int(cy + math.sin(ang) * t) % n
            scuff[y, x] = max(scuff[y, x], 0.5 + 0.5 * srng.random())
    h = peel * 0.25 + (wave - 0.5) * 3.0 + weld * knurl * 1.6 + ring * 1.2 + scuff * 0.25
    nrm = normal_from_height(h, 0.35)
    col = np.ones((n, n, 3), np.float32) * 0.96
    col *= (1 - 0.06 * weld)[..., None]
    rough = 0.06 + 0.25 * weld * knurl + 0.2 * scuff
    orm = np.stack([np.ones_like(rough), rough, np.zeros_like(rough)], -1)
    return (save_png("sleeve_film_base", to_srgb(col), True),
            save_png("sleeve_film_normal", nrm, False),
            save_png("sleeve_film_orm", orm, False))


# ----------------------------------------------------------------------------- geometry helpers

def new_obj(name, bm, mats, origin=(0, 0, 0)):
    me = bpy.data.meshes.new(name)
    # (tangents are only exported for triangles and quads)
    bmesh.ops.triangulate(bm, faces=[f for f in bm.faces if len(f.verts) > 4])
    bm.to_mesh(me)
    bm.free()
    for m in mats:
        me.materials.append(m)
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = origin
    for p in me.polygons:
        p.use_smooth = True
    return ob


def rounded_rect(w, h, r, seg=8):
    """CCW outline (mm), centred."""
    pts = []
    for cx, cy, a0 in ((w / 2 - r, -h / 2 + r, -90), (w / 2 - r, h / 2 - r, 0), (-w / 2 + r, h / 2 - r, 90), (-w / 2 + r, -h / 2 + r, 180)):
        for k in range(seg + 1):
            a = math.radians(a0 + 90 * k / seg)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def board(bm, outline, z0, z1, mat_index, uv_fn, bevel=0.7):
    """A slab from a CCW outline (mm), with a rounded rim (bevel rings) and planar UVs."""
    uv = bm.loops.layers.uv.verify()
    rings = []
    steps = 4
    for k in range(steps + 1):
        a = k / steps * math.pi
        dz = (1 - math.cos(a)) / 2
        inset = bevel * math.sin(a)
        z = z0 + (z1 - z0) * dz
        ring = []
        n = len(outline)
        for i, (x, y) in enumerate(outline):
            px, py = outline[i - 1]
            nx, ny = outline[(i + 1) % n]
            tx, ty = nx - px, ny - py
            l = math.hypot(tx, ty) or 1
            ox, oy = ty / l, -tx / l           # outward normal for a CCW outline
            ring.append(bm.verts.new(((x - ox * (bevel - inset)) * MM, (y - oy * (bevel - inset)) * MM, z * MM)))
        rings.append(ring)
    faces = []
    for a, b in zip(rings[:-1], rings[1:]):
        n = len(a)
        for i in range(n):
            faces.append(bm.faces.new((a[i], a[(i + 1) % n], b[(i + 1) % n], b[i])))
    bottom = bm.faces.new(list(reversed(rings[0])))
    top = bm.faces.new(rings[-1])
    faces += [bottom, top]
    for f in faces:
        f.material_index = mat_index
        for lp in f.loops:
            lp[uv].uv = uv_fn(lp.vert.co.x / MM, lp.vert.co.y / MM)
    return faces


# ----------------------------------------------------------------------------- binder_ring4

SPINE_U0 = 0.80


def build_binder():
    cb, cn, co = leatherette_textures()
    lb, ln, lo = lining_textures()
    mb, mn, mo = metal_textures()
    cover = pbr_material("binder_cover", cb, cn, co, metallic_from_orm=False)
    lining = pbr_material("binder_lining", lb, ln, lo, metallic_from_orm=False)
    metal = pbr_material("binder_ring_metal", mb, mn, mo)
    span = 320.0
    out = rounded_rect(BOARD_W, BOARD_H, 6.0)
    # boards: local frame with the hinge edge at x = 0 and the board running +x
    hinge_x = SPINE_W / 2 + HINGE_GAP

    def board_obj(name, side):
        bm = bmesh.new()
        ol = [(x + BOARD_W / 2, y) for x, y in out]
        # u from the hinge edge; the front board is mirrored so its stitching matches
        board(bm, ol, 0.0, BOARD_T, 0, lambda x, y: (x / span, (y + BOARD_H / 2) / span))
        # the lining sheet inside, 6 mm in from the edges
        lin = [(x + BOARD_W / 2, y) for x, y in rounded_rect(BOARD_W - 12, BOARD_H - 12, 3.0)]
        board(bm, lin, BOARD_T, BOARD_T + 0.35, 1, lambda x, y: (x / 300.0, y / 300.0), bevel=0.15)
        # the hinge: a thin strip of the cover material from the board to the spine
        uv = bm.loops.layers.uv.verify()
        hz0, hz1 = 0.0, 1.0
        x0, x1 = -HINGE_GAP - 0.5, 0.8
        y0, y1 = -BOARD_H / 2 + 6, BOARD_H / 2 - 6
        vs = [bm.verts.new((x * MM, y * MM, z * MM)) for (x, y, z) in
              [(x0, y0, hz0), (x1, y0, hz0), (x1, y1, hz0), (x0, y1, hz0), (x0, y0, hz1), (x1, y0, hz1), (x1, y1, hz1), (x0, y1, hz1)]]
        for q in ((0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (2, 3, 7, 6), (1, 2, 6, 5), (3, 0, 4, 7)):
            f = bm.faces.new([vs[i] for i in q])
            for lp in f.loops:
                lp[uv].uv = (SPINE_U0 + 0.13 + (lp.vert.co.x / MM) / span * 4, (lp.vert.co.y / MM + BOARD_H / 2) / span)
        if side < 0:
            bmesh.ops.scale(bm, vec=(-1, 1, 1), verts=bm.verts)
            bmesh.ops.reverse_faces(bm, faces=bm.faces)
        return new_obj(name, bm, [cover, lining], origin=(side * hinge_x * MM, 0, 0))

    board_obj("cover_back", 1)
    board_obj("cover_front", -1)

    # spine
    bm = bmesh.new()
    board(bm, rounded_rect(SPINE_W, BOARD_H, 1.0, 2), 0.0, BOARD_T, 0,
          lambda x, y: (SPINE_U0 + (x + SPINE_W / 2) / span, (y + BOARD_H / 2) / span), bevel=0.6)
    new_obj("spine", bm, [cover])

    # ring mechanism: an arched steel cover along the spine, end rivets, four O-rings
    bm = bmesh.new()
    uv = bm.loops.layers.uv.verify()
    seg = 16
    prof = []
    for k in range(seg + 1):
        a = math.pi * k / seg
        prof.append((-math.cos(a) * MECH_W / 2, BOARD_T + 0.2 + math.sin(a) * MECH_H * (0.55 + 0.45 * math.sin(a) ** 0.3)))
    rows = []
    for y in (-MECH_L / 2, MECH_L / 2):
        rows.append([bm.verts.new((x * MM, y * MM, z * MM)) for x, z in prof])
    for i in range(seg):
        f = bm.faces.new((rows[0][i], rows[0][i + 1], rows[1][i + 1], rows[1][i]))
        for lp, (uu, vv) in zip(f.loops, ((i / seg, 0), ((i + 1) / seg, 0), ((i + 1) / seg, 1), (i / seg, 1))):
            lp[uv].uv = (uu * 0.2, vv)
    for r in rows:
        f = bm.faces.new(r if r is rows[1] else list(reversed(r)))
        for lp in f.loops:
            lp[uv].uv = (0.5 + lp.vert.co.x / MM / 40, lp.vert.co.z / MM / 40)
    # O-rings: tori in the XZ plane
    rseg, wseg = 48, 10
    for hy in HOLES_Y:
        grid = []
        for i in range(rseg):
            a = 2 * math.pi * i / rseg
            cx, cz = math.cos(a) * RING_R, RING_CZ + math.sin(a) * RING_R
            row = []
            for j in range(wseg):
                b = 2 * math.pi * j / wseg
                rr = RING_R + math.cos(b) * RING_WIRE
                row.append(bm.verts.new((math.cos(a) * rr * MM, (hy + math.sin(b) * RING_WIRE) * MM, (RING_CZ + math.sin(a) * rr) * MM)))
            grid.append(row)
        for i in range(rseg):
            for j in range(wseg):
                a, b = grid[i], grid[(i + 1) % rseg]
                f = bm.faces.new((a[j], b[j], b[(j + 1) % wseg], a[(j + 1) % wseg]))
                for lp, (uu, vv) in zip(f.loops, ((i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1))):
                    lp[uv].uv = (0.25 + uu / rseg * 0.75, vv / wseg)
        # the split where the two ring halves meet, and the rivets holding the ring's feet
    for y in (-MECH_L / 2 + 8, MECH_L / 2 - 8):
        res = bmesh.ops.create_cone(bm, cap_ends=True, segments=16, radius1=2.2 * MM, radius2=1.6 * MM, depth=1.2 * MM)
        bmesh.ops.translate(bm, vec=(0, y * MM, (BOARD_T + MECH_H * 0.98 + 0.4) * MM), verts=res["verts"])
        for f in {f for v in res["verts"] for f in v.link_faces}:
            for lp in f.loops:
                lp[uv].uv = (0.9 + lp.vert.co.x * 10, 0.5 + lp.vert.co.y * 10 % 0.1)
    new_obj("ring_mechanism", bm, [metal])

    return {
        "card_mm": [CARD_W, CARD_H, CARD_T], "card_corner_mm": CARD_CORNER, "seat_gap_mm": 1.5,
        "spine_w_mm": SPINE_W, "hinge_gap_mm": HINGE_GAP,
        "board_mm": [BOARD_W, BOARD_H, BOARD_T],
        "ring_r_mm": RING_R, "ring_wire_mm": RING_WIRE, "ring_centre_z_mm": RING_CZ, "rings_y_mm": list(HOLES_Y),
        "mechanism_mm": [MECH_W, MECH_H, MECH_L],
    }


# ----------------------------------------------------------------------------- binder_sheet9

def build_sheet():
    fb, fn, fo = film_textures()
    film = pbr_material("sleeve_film_clear_glass", fb, fn, fo, metallic_from_orm=False, double_sided=True)

    def uvf(x, y):
        return ((x - SHEET_X0) / SHEET_W, (y + SHEET_H / 2) / SHEET_H)

    # grid lines across (x) and up (y): every seam edge, a ramp 1.6 mm into each pocket, the
    # binding strip and the hole squares
    ramp = 1.6
    xs = {SHEET_X0, -HOLE_EDGE / 2, HOLE_EDGE / 2, STRIP, SHEET_X1}
    for c in range(3):
        x0, x1, _, _ = pocket_rect(c, 0)
        xs |= {x0, x0 + ramp, (x0 + x1) / 2, x1 - ramp, x1}
    ys = {-SHEET_H / 2, SHEET_H / 2}
    for r in range(3):
        _, _, y0, y1 = pocket_rect(0, r)
        ys |= {y0, y0 + ramp, (y0 + y1) / 2, y1 - ramp - 0.5, y1 - 0.5, y1}
    for hy in HOLES_Y:
        ys |= {hy - HOLE_EDGE / 2, hy + HOLE_EDGE / 2}
    xs, ys = sorted(xs), sorted(ys)

    def puff(x, y):
        """0 on welds, 1 inside a pocket (smooth ramp in from the seams)."""
        for c in range(3):
            for r in range(3):
                x0, x1, y0, y1 = pocket_rect(c, r)
                if x0 <= x <= x1 and y0 <= y <= y1:
                    fx = min(x - x0, x1 - x) / ramp
                    fy = min(y - y0, y1 - y) / ramp
                    t = max(0.0, min(1.0, min(fx, fy)))
                    return t * t * (3 - 2 * t)
        return 0.0

    def in_slit(xm, ym):
        """The front film is cut open along the top of each pocket (0.5 mm under the top weld)."""
        for c in range(3):
            for r in range(3):
                x0, x1, y0, y1 = pocket_rect(c, r)
                if x0 < xm < x1 and y1 - 0.5 < ym < y1:
                    return True
        return False

    def hole_cell(xm, ym):
        for hy in HOLES_Y:
            if abs(xm) < HOLE_EDGE / 2 and abs(ym - hy) < HOLE_EDGE / 2:
                return hy
        return None

    objs = []
    for name, side in (("film_front", 1.0), ("film_back", -1.0)):
        bm = bmesh.new()
        uv = bm.loops.layers.uv.verify()
        vid = {}

        def v(x, y):
            key = (round(x, 4), round(y, 4))
            if key not in vid:
                z = side * (0.04 + (FILM_GAP / 2 - 0.04) * puff(x, y))
                vid[key] = bm.verts.new((x * MM, y * MM, z * MM))
            return vid[key]

        def face(vs):
            if side < 0:
                vs = list(reversed(vs))
            f = bm.faces.new(vs)
            f.material_index = 0
            for lp in f.loops:
                lp[uv].uv = uvf(lp.vert.co.x / MM, lp.vert.co.y / MM)

        for i in range(len(xs) - 1):
            for j in range(len(ys) - 1):
                xm, ym = (xs[i] + xs[i + 1]) / 2, (ys[j] + ys[j + 1]) / 2
                if side > 0 and in_slit(xm, ym):
                    continue
                hy = hole_cell(xm, ym)
                if hy is not None:
                    continue
                face([v(xs[i], ys[j]), v(xs[i + 1], ys[j]), v(xs[i + 1], ys[j + 1]), v(xs[i], ys[j + 1])])
        # the punched holes: an annulus from each hole to its square
        n = 32
        h = HOLE_EDGE / 2
        for hy in HOLES_Y:
            inner, outer = [], []
            for k in range(n):
                a = 2 * math.pi * k / n
                inner.append(v(math.cos(a) * HOLE_D / 2, hy + math.sin(a) * HOLE_D / 2))
                c, s = math.cos(a), math.sin(a)
                t = h / max(abs(c), abs(s))
                outer.append(v(c * t, hy + s * t))
            for k in range(n):
                face([inner[k], outer[k], outer[(k + 1) % n], inner[(k + 1) % n]])
        objs.append(new_obj(name, bm, [film]))
    return {
        "card_mm": [CARD_W, CARD_H, CARD_T], "card_corner_mm": CARD_CORNER, "seat_gap_mm": 1.5,
        "sheet_mm": [SHEET_X0, SHEET_X1, SHEET_H],
        "film_gap_mm": FILM_GAP,
        "hole_d_mm": HOLE_D, "holes_y_mm": list(HOLES_Y),
        "pockets_mm": [[list(pocket_rect(c, r)) for c in range(3)] for r in range(3)],
        "pocket_order": "pockets_mm[row][col], row 0 at the top; [x0, x1, y0, y1]; the opening is the top edge (y1)",
    }


# ----------------------------------------------------------------------------- main

bpy.ops.wm.read_factory_settings(use_empty=True)
if PID == "binder_ring4":
    layout = build_binder()
elif PID == "binder_sheet9":
    layout = build_sheet()
else:
    raise SystemExit("unknown id " + PID)

os.makedirs(OUT_DIR, exist_ok=True)
out = os.path.join(OUT_DIR, PID + ".gltf")
bpy.ops.export_scene.gltf(filepath=out, export_format="GLTF_SEPARATE", export_texture_dir="textures",
                          export_yup=True, export_tangents=True, export_apply=True)
layout["frame"] = "glTF (+Y up): metres; Blender x -> x, Blender y -> -z, Blender z -> y. Values here are Blender mm."
with open(os.path.join(OUT_DIR, "layout.json"), "w") as f:
    json.dump(layout, f, indent=1)
print("wrote", out)
