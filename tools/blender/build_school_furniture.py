"""Classroom furniture for Kazehana Junior High (docs/school_plan.md, docs/lessons_plan.md), each
its own quality-gated prop:

    blender --background --factory-startup --python tools/blender/build_school_furniture.py -- <id>

ids:
    school_desk    the new-JIS height-adjustable student desk, as in the school-furniture
                   makers' catalogues: a 650 x 450 mm honey wood-grain melamine top with a
                   brown edge band, a light-grey resin book box open to the student, an
                   ivory-grey tubular frame with telescoping legs (set for a 1st-year: top at
                   700 mm) standing on floor runners with brown end caps, a bag hook each side
(its chair, school_chair, is the shop's pipe chair recoloured: tools/blender/build_shop_props.py)

-> assets/props/<id>/<id>.gltf

Frame (Blender): origin on the floor under the middle; the student's side faces -Y (the desk's
book box opens towards -Y). The chair stands on that side, unturned (its sitter faces +Y).
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
COLL = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(COLL)

DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
# (as the new-JIS school desks of the makers' catalogues: an ivory-grey powder-coated frame, a
# moulded light-grey resin book box, a honey wood-grain melamine top with a brown edge band,
# brown end caps on the floor runners)
frame_paint = P("school_furniture_frame", (0.70, 0.68, 0.62), DETAIL, roughness=0.5, metallic=0.1, normal_strength=0.05, grain=0.08)
box_paint = P("school_desk_bookbox", (0.78, 0.78, 0.76), DETAIL, roughness=0.42, normal_strength=0.01, grain=0.015)
cap_brown = P("school_desk_cap", (0.22, 0.12, 0.06), DETAIL, roughness=0.5, normal_strength=0.05, grain=0.05)
collar_plastic = P("school_desk_collar", (0.20, 0.19, 0.18), DETAIL, roughness=0.45, normal_strength=0.05, grain=0.05)
# the wood: Megascans plywood (its seam-free left panel, mapped per board by `grain_uv`), tinted to
# a school desk's pale birch laminate and the chair's lacquered plywood
laminate = kz.surface_material("school_desk_laminate", "Plywood_Panels", tint=(1.22, 0.94, 0.60), desaturate=0.0)
laminate.node_tree.nodes["Principled BSDF"].inputs["Coat Weight"].default_value = 0.35
edge = P("school_desk_edge", (0.30, 0.18, 0.09), DETAIL, roughness=0.35, normal_strength=0.03, grain=0.04)
plywood = kz.surface_material("school_chair_plywood", "Plywood_Panels", tint=(1.12, 0.98, 0.80), desaturate=0.15)
plywood.node_tree.nodes["Principled BSDF"].inputs["Coat Weight"].default_value = 0.5
ply_edge = P("school_chair_ply_edge", (0.66, 0.52, 0.34), DETAIL, roughness=0.5, normal_strength=0.1, grain=0.2)
rubber = P("school_furniture_rubber", (0.035, 0.035, 0.035), DETAIL, roughness=0.85, normal_strength=0.2)
bolt = P("school_furniture_bolt", (0.70, 0.70, 0.68), DETAIL, roughness=0.25, metallic=0.95, normal_strength=0.05, grain=0.05)


# boards whose grain is mapped by hand: name -> (grain along "x" or "y", the board's size)
GRAIN = {}


def grain_uv(ob, along, size):
    """UVs for a wooden board: the texture's grain (its v) along the board, kept inside the
    plywood texture's left panel (u 0.02..0.46) so its seam never shows."""
    me = ob.data
    uv = me.uv_layers.new(name="UVMap") if not me.uv_layers else me.uv_layers[0]
    w, d = size
    for poly in me.polygons:
        for li in poly.loop_indices:
            co = me.vertices[me.loops[li].vertex_index].co
            # (the texture is a 2.44 x 1.22 m sheet: true scale)
            if along == "back":
                # (an upright board: grain across it, u up it)
                u, v = 0.05 + (co.z - 0.55) / 2.44, 0.1 + (co.x / w + 0.5) * w / 1.22
            elif along == "x":
                u, v = 0.05 + (co.y / d + 0.5) * d / 2.44, 0.1 + (co.x / w + 0.5) * w / 1.22
            else:
                u, v = 0.05 + (co.x / w + 0.5) * w / 2.44, 0.1 + (co.y / d + 0.5) * d / 1.22 + co.z / 1.22
            uv.data[li].uv = (min(max(u, 0.0), 0.47), v)


def obj(name, bm, mat, smooth=True):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    COLL.objects.link(ob)
    me.materials.append(mat)
    for p in me.polygons:
        p.use_smooth = smooth
    return ob


def box(name, x0, y0, z0, x1, y1, z1, mat, bevel=0.0):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co = Vector((x0 + (v.co.x + 0.5) * (x1 - x0), y0 + (v.co.y + 0.5) * (y1 - y0), z0 + (v.co.z + 0.5) * (z1 - z0)))
    if bevel > 0:
        bmesh.ops.bevel(bm, geom=list(bm.edges), offset=bevel, segments=2, affect="EDGES")
    return obj(name, bm, mat, smooth=bevel > 0)


def fillet(pts, radius, seg=6):
    """A polyline with its corners rounded (a bent tube's path)."""
    out = [pts[0]]
    for a, b, c in zip(pts, pts[1:], pts[2:]):
        d0, d1 = (b - a).normalized(), (c - b).normalized()
        ang = d0.angle(d1, 0.0)
        if ang < 1e-3:
            out.append(b)
            continue
        t = min(radius * math.tan(ang / 2), (b - a).length * 0.45, (c - b).length * 0.45)
        p0, p1 = b - d0 * t, b + d1 * t
        for k in range(seg + 1):
            s = k / seg
            # a quadratic Bezier through the corner (close to an arc at these angles)
            out.append(p0 * (1 - s) ** 2 + b * 2 * s * (1 - s) + p1 * s * s)
    out.append(pts[-1])
    return out


def tube(name, pts, r, mat, bend=0.04, seg=14, caps=True):
    """A round steel tube swept along a path, its corners bent to `bend` radius."""
    path = fillet([Vector(p) for p in pts], bend) if bend > 0 else [Vector(p) for p in pts]
    bm = bmesh.new()
    rings = []
    # a frame carried along the path (parallel transport) so the tube doesn't twist
    t0 = (path[1] - path[0]).normalized()
    n = t0.orthogonal().normalized()
    for i, p in enumerate(path):
        if i == 0:
            t = t0
        elif i == len(path) - 1:
            t = (path[i] - path[i - 1]).normalized()
        else:
            t = ((path[i] - path[i - 1]).normalized() + (path[i + 1] - path[i]).normalized()).normalized()
        n = (n - t * n.dot(t)).normalized()
        b = t.cross(n)
        rings.append([bm.verts.new(p + (n * math.cos(2 * math.pi * k / seg) + b * math.sin(2 * math.pi * k / seg)) * r) for k in range(seg)])
    for r0, r1 in zip(rings, rings[1:]):
        for k in range(seg):
            bm.faces.new((r0[k], r0[(k + 1) % seg], r1[(k + 1) % seg], r1[k]))
    if caps:
        bm.faces.new(list(reversed(rings[0])))
        bm.faces.new(rings[-1])
    return obj(name, bm, mat)


def cyl(name, c, r, h, mat, seg=16, axis="Z"):
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=seg, radius1=r, radius2=r, depth=h)
    rot = {"Z": Matrix.Identity(3), "X": Matrix.Rotation(math.pi / 2, 3, "Y"), "Y": Matrix.Rotation(math.pi / 2, 3, "X")}[axis]
    for v in bm.verts:
        v.co = rot @ v.co + Vector(c)
    bmesh.ops.bevel(bm, geom=[e for e in bm.edges if len(e.link_faces) == 2 and abs(e.calc_face_angle(0.0)) > 1.0], offset=min(r, h) * 0.15, segments=2, affect="EDGES")
    return obj(name, bm, mat)


def rounded_slab(name, w, d, t, rc, z, mat, edge_mat=None, dish=0.0, y=0.0, seg=6):
    """A flat board with rounded corners (a desk top, a chair seat), its top `dish` lower in the
    middle; the rim a separate material if given."""
    bm = bmesh.new()
    outline = []
    for cx, cy, a0 in ((w / 2 - rc, d / 2 - rc, 0), (-w / 2 + rc, d / 2 - rc, 90), (-w / 2 + rc, -d / 2 + rc, 180), (w / 2 - rc, -d / 2 + rc, 270)):
        for k in range(seg + 1):
            a = math.radians(a0 + 90 * k / seg)
            outline.append((cx + rc * math.cos(a), cy + rc * math.sin(a)))
    # a grid of rings inwards so the top can dish
    rings = 4
    top, bot = [], []
    for j in range(rings + 1):
        s = 1 - j / (rings + 0.6)
        dip = dish * (1 - s * s)
        top.append([bm.verts.new((x * s, y + yy * s, z + t - dip)) for x, yy in outline])
        bot.append([bm.verts.new((x * s, y + yy * s, z - dip * 0.6)) for x, yy in outline])
    n = len(outline)
    for j in range(rings):
        for k in range(n):
            bm.faces.new((top[j][k], top[j][(k + 1) % n], top[j + 1][(k + 1) % n], top[j + 1][k]))
            bm.faces.new((bot[j][k], bot[j + 1][k], bot[j + 1][(k + 1) % n], bot[j][(k + 1) % n]))
    bm.faces.new(top[-1])
    bm.faces.new(list(reversed(bot[-1])))
    rim = []
    for k in range(n):
        rim.append(bm.faces.new((top[0][k], bot[0][k], bot[0][(k + 1) % n], top[0][(k + 1) % n])))
    ob = obj(name, bm, mat)
    if edge_mat is not None:
        ob.data.materials.append(edge_mat)
        for p in ob.data.polygons:
            # (the rim's faces stand upright)
            if abs(p.normal.z) < 0.3:
                p.material_index = 1
    return ob


def bent_panel(name, w, h, t, radius, centre, mat):
    """A curved plywood panel (the chair's back): curved round a vertical axis behind it."""
    bm = bmesh.new()
    nx, nz = 16, 4
    grid = {}
    for side, off in (("f", 0.0), ("b", t)):
        for i in range(nx + 1):
            a = (i / nx - 0.5) * (w / radius)
            for j in range(nz + 1):
                x = math.sin(a) * (radius + off)
                # (the ends come forwards, to -Y: the back wraps round the sitter)
                y = centre.y + (math.cos(a) * (radius + off) - radius)
                # (rounded top corners: pull the top row down at the ends)
                zt = h / 2 - 0.02 * (abs(i / nx - 0.5) * 2) ** 6
                z = -h / 2 + (zt + h / 2) * j / nz
                grid[(side, i, j)] = bm.verts.new((centre.x + x, y + off, centre.z + z))
    for i in range(nx):
        for j in range(nz):
            bm.faces.new((grid[("f", i, j)], grid[("f", i + 1, j)], grid[("f", i + 1, j + 1)], grid[("f", i, j + 1)]))
            bm.faces.new((grid[("b", i, j)], grid[("b", i, j + 1)], grid[("b", i + 1, j + 1)], grid[("b", i + 1, j)]))
    for i in range(nx):
        for j in (0, nz):
            a, b = (grid[("f", i, j)], grid[("f", i + 1, j)]), (grid[("b", i, j)], grid[("b", i + 1, j)])
            f = bm.faces.new((a[0], b[0], b[1], a[1]) if j == 0 else (a[0], a[1], b[1], b[0]))
    for i in (0, nx):
        for j in range(nz):
            a, b = (grid[("f", i, j)], grid[("f", i, j + 1)]), (grid[("b", i, j)], grid[("b", i, j + 1)])
            bm.faces.new((a[0], a[1], b[1], b[0]) if i == 0 else (a[0], b[0], b[1], a[1]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return obj(name, bm, mat)


R_OUT, R_IN = 0.0135, 0.0115   # the frame's outer tube and the telescoping inner one
FOOT = 0.012

if PID == "school_desk":
    # the new-JIS height-adjustable student desk (size 5, a junior high's: top at 700 mm)
    W, D, TOP = 0.65, 0.45, 0.70
    T = 0.018
    UT = TOP - T                                   # the underside of the top
    # the top: wood-grain melamine on a core, a brown resin edge band round it
    GRAIN["desk_top"] = ("x", (W, D))
    rounded_slab("desk_top", W, D, T, 0.015, UT, laminate, edge)
    LX, LY = W / 2 - 0.04, D / 2 - 0.05            # the legs, in from the corners
    RUN = 0.019                                    # the floor runners' radius (their centre height)
    for sx in (-1, 1):
        x = sx * LX
        # each side: the upper frame (outer tube) an upside-down U under the top, its legs
        # running down to the height adjusters
        tube(f"desk_side_{sx}", [(x, -LY, 0.40), (x, -LY, UT - R_OUT), (x, LY, UT - R_OUT), (x, LY, 0.40)], R_OUT, frame_paint, bend=0.035)
        for sy in (-1, 1):
            y = sy * LY
            # the inner tube telescoping out to the runner, a plastic collar and a height bolt
            tube(f"desk_leg_{sx}_{sy}", [(x, y, RUN * 1.6), (x, y, 0.44)], R_IN, frame_paint, bend=0)
            cyl(f"desk_collar_{sx}_{sy}", (x, y, 0.392), R_OUT + 0.0025, 0.022, collar_plastic)
            cyl(f"desk_bolt_{sx}_{sy}", (x + sx * (R_OUT + 0.004), y, 0.42), 0.006, 0.007, bolt, seg=6, axis="X")
        # the floor runner the legs stand on, front to back, its ends capped in brown
        ry = LY + 0.045
        tube(f"desk_runner_{sx}", [(x, -ry, RUN), (x, ry, RUN)], RUN * 0.9, frame_paint, bend=0)
        for sy in (-1, 1):
            cyl(f"desk_cap_{sx}_{sy}", (x, sy * (ry + 0.012), RUN), RUN, 0.026, cap_brown, axis="Y")
        # the bag hook: a bent rod on the side rail, open forwards
        hx = x + sx * (R_OUT + 0.004)
        tube(f"desk_hook_{sx}", [(hx, 0.02, UT - 0.02), (hx + sx * 0.012, 0.02, UT - 0.05), (hx + sx * 0.02, 0.02, UT - 0.085), (hx + sx * 0.03, -0.012, UT - 0.095), (hx + sx * 0.032, -0.02, UT - 0.07)], 0.0028, frame_paint, bend=0.01, seg=8)
    # the stretcher joining the sides low at the back (the front is left open for your knees),
    # and a rail under the top at the back
    tube("desk_back_rail", [(-LX, LY, 0.17), (LX, LY, 0.17)], R_IN * 0.95, frame_paint, bend=0)
    tube("desk_top_rail", [(-LX, LY - 0.002, UT - R_OUT * 2.2), (LX, LY - 0.002, UT - R_OUT * 2.2)], R_IN * 0.8, frame_paint, bend=0)
    # the book box: a moulded resin tray hung under the top between the side frames, open
    # towards you (-Y) above a low rounded lip, its corners and bottom edges rounded
    bx0, bx1 = -LX + R_OUT + 0.004, LX - R_OUT - 0.004
    by0, by1 = -D / 2 + 0.03, LY + 0.005
    bz0, bz1 = UT - 0.115, UT - 0.004
    th = 0.004
    box("desk_box_floor", bx0, by0, bz0, bx1, by1, bz0 + th, box_paint, bevel=0.0018)
    box("desk_box_back", bx0, by1 - th, bz0, bx1, by1, bz1, box_paint, bevel=0.0018)
    for sx, xx in ((-1, bx0), (1, bx1 - th)):
        box(f"desk_box_side_{sx}", xx, by0, bz0, xx + th, by1, bz1, box_paint, bevel=0.0018)
    # (the rounded bottom edges, inside the corners)
    tube("desk_box_round_back", [(bx0, by1 - th, bz0 + th), (bx1, by1 - th, bz0 + th)], 0.008, box_paint, bend=0, seg=10)
    for sx, xx in ((-1, bx0 + th), (1, bx1 - th)):
        tube(f"desk_box_round_{sx}", [(xx, by0, bz0 + th), (xx, by1, bz0 + th)], 0.008, box_paint, bend=0, seg=10)
    # the lip: a rounded bead along the open front
    tube("desk_box_lip", [(bx0 + 0.002, by0 + 0.006, bz0 + 0.012), (bx1 - 0.002, by0 + 0.006, bz0 + 0.012)], 0.008, box_paint, bend=0, seg=12)

else:
    raise SystemExit(f"unknown prop {PID}")

# one mesh for the prop (its parts joined), world-projected UVs for the painted surfaces; the
# whole standing on z = 0
for o in COLL.objects:
    o.data.transform(o.matrix_world)
    o.matrix_world = Matrix.Identity(4)
    if o.name in GRAIN:
        grain_uv(o, *GRAIN[o.name])
    else:
        bm = bmesh.new()
        bm.from_mesh(o.data)
        kz.world_uv(bm, 2.0)
        bm.to_mesh(o.data)
        bm.free()
bpy.ops.object.select_all(action="DESELECT")
parts = list(COLL.objects)
for o in parts:
    o.select_set(True)
bpy.context.view_layer.objects.active = parts[0]
bpy.ops.object.join()
body = bpy.context.view_layer.objects.active
body.name = PID
zmin = min(v.co.z for v in body.data.vertices)
body.data.transform(Matrix.Translation((0, 0, -zmin)))

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
