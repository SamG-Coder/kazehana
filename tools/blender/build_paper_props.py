"""Paper things that carry information in the world (docs/life_plan.md: no menus), each its own
quality-gated prop:

    blender --background --factory-startup --python tools/blender/build_paper_props.py -- <id>

ids:
    wall_calendar   a Japanese wall calendar, B4 (364 x 515 mm): the month page, the sheets behind
                    it, a cardboard back, a twin-wire binding with its hanging loop, on a nail
    journal         your journal, closed: an A5 (148 x 210 mm) cloth-bound notebook with a label
    journal_open    the same, open on its spread: two pages curving into the gutter

-> assets/props/<id>/<id>.gltf

The faces that show a page are their own nodes, `kzpage_calendar` and `kzpage_journal`, UV-mapped
0..1 over the page image (pages::calendar: 1024 x 1448; pages::journal: 2048 x 1448 for the
spread), so the game can put the page it draws on them. Here they carry a sample page
(assets/cache/pages, from `cargo run -p pages --example preview`).

Frame (Blender): the calendar's origin is the bottom of its board, its face towards -Y (hung on a
wall at its height); the journal's origin
is under the middle of the book, lying on a desk, its top edge towards +Y.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PID = sys.argv[sys.argv.index("--") + 1]
OUT = os.path.join(ROOT, "assets", "props", PID, PID + ".gltf")
PAGES = os.path.join(ROOT, "assets", "cache", "pages")

kz.reset_scene()
COLL = bpy.data.collections.new("PROP")
bpy.context.scene.collection.children.link(COLL)


DETAIL = "Precast_Concrete_Wall"


def image_material(name, path, roughness=0.85):
    """A printed page: the page image, with a faint paper grain in the normals."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = roughness
    t = nt.nodes.new("ShaderNodeTexImage")
    t.image = bpy.data.images.load(path, check_existing=True)
    nt.links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
    _, n, _ = kz.fab_textures(DETAIL)
    if n:
        tn = nt.nodes.new("ShaderNodeTexImage")
        tn.image = bpy.data.images.load(n, check_existing=True)
        tn.image.colorspace_settings.name = "Non-Color"
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nm.inputs["Strength"].default_value = 0.05
        nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    return m


P = kz.painted_material
paper_edge = P("paper_edge", (0.86, 0.85, 0.81), DETAIL, roughness=0.9, normal_strength=0.1, grain=0.1)
cardboard = P("paper_cardboard", (0.42, 0.38, 0.32), DETAIL, roughness=0.9, normal_strength=0.3, grain=0.3)
wire = P("paper_wire", (0.08, 0.08, 0.09), DETAIL, roughness=0.3, metallic=0.9, normal_strength=0.05, grain=0.05)
nail = P("paper_nail", (0.55, 0.55, 0.53), DETAIL, roughness=0.35, metallic=0.9, normal_strength=0.05, grain=0.05)
cloth = P("journal_cloth", (0.06, 0.09, 0.20), DETAIL, roughness=0.75, normal_strength=0.6, grain=0.25)
label = P("journal_label", (0.88, 0.84, 0.72), DETAIL, roughness=0.85, normal_strength=0.1, grain=0.1)
endpaper = P("journal_endpaper", (0.62, 0.15, 0.12), DETAIL, roughness=0.8, normal_strength=0.1, grain=0.15)
ribbon = P("journal_ribbon", (0.70, 0.12, 0.12), DETAIL, roughness=0.6, normal_strength=0.2, grain=0.2)


def obj(name, bm):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    COLL.objects.link(ob)
    for p in me.polygons:
        p.use_smooth = True
    return ob


def box(name, x0, y0, z0, x1, y1, z1, mat):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co = Vector((x0 + (v.co.x + 0.5) * (x1 - x0), y0 + (v.co.y + 0.5) * (y1 - y0), z0 + (v.co.z + 0.5) * (z1 - z0)))
    ob = obj(name, bm)
    ob.data.materials.append(mat)
    for p in ob.data.polygons:
        p.use_smooth = False
    return ob


def sheet(name, w, h, nx, ny, mat, at, uv=True):
    """A sheet of paper: a grid w x h, its vertices placed by at(u, v) -> Vector (u, v in 0..1,
    v = 0 the top), UVs 0..1 (v up, as the image)."""
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new("UVMap")
    grid = [[bm.verts.new(at(i / nx, j / ny)) for i in range(nx + 1)] for j in range(ny + 1)]
    for j in range(ny):
        for i in range(nx):
            f = bm.faces.new((grid[j][i], grid[j + 1][i], grid[j + 1][i + 1], grid[j][i + 1]))
            for loop, (a, b) in zip(f.loops, ((i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j))):
                loop[uvl].uv = (a / nx, 1.0 - b / ny)
    ob = obj(name, bm)
    ob.data.materials.append(mat)
    return ob


def torus(name, c, R, r, axis, mat, seg=16, tube=6):
    bm = bmesh.new()
    bmesh.ops.create_circle(bm, segments=seg, radius=R)
    ob_verts = []
    bm.free()
    bm = bmesh.new()
    rings = []
    for i in range(seg):
        a = 2 * math.pi * i / seg
        row = []
        for j in range(tube):
            b = 2 * math.pi * j / tube
            p = Vector(((R + r * math.cos(b)) * math.cos(a), (R + r * math.cos(b)) * math.sin(a), r * math.sin(b)))
            if axis == "x":
                p = Vector((p.z, p.x, p.y))
            rings.append(None)
            row.append(bm.verts.new(c + p))
        ob_verts.append(row)
    for i in range(seg):
        for j in range(tube):
            bm.faces.new((ob_verts[i][j], ob_verts[(i + 1) % seg][j], ob_verts[(i + 1) % seg][(j + 1) % tube], ob_verts[i][(j + 1) % tube]))
    ob = obj(name, bm)
    ob.data.materials.append(mat)
    return ob


def cylinder(name, c, r, depth, axis_y, mat, seg=16):
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=seg, radius1=r, radius2=r, depth=depth)
    for v in bm.verts:
        p = v.co.copy()
        if axis_y:
            p = Vector((p.x, p.z, p.y))
        v.co = c + p
    ob = obj(name, bm)
    ob.data.materials.append(mat)
    return ob


if PID == "wall_calendar":
    W, H = 0.364, 0.515
    top = -0.03  # (the binding hangs 3 cm under the nail)
    page_mat = image_material("kzpage_calendar_sample", os.path.join(PAGES, "calendar_sample.png"))

    # the month: flat at the binding, bowing out a few millimetres towards the bottom, a curl at the corner
    def page(u, v):
        bow = 0.006 * v ** 1.6 + 0.004 * max(0.0, (u - 0.75) / 0.25) ** 2 * max(0.0, (v - 0.8) / 0.2) ** 2
        # (u runs right to left here: the calendar is turned to face -Y at the end)
        return Vector(((0.5 - u) * W, 0.0035 + bow, top - v * H))
    sheet("kzpage_calendar", W, H, 16, 24, page_mat, page)
    # the sheets behind (the rest of the year), the cardboard back
    box("calendar_sheets", -W / 2, 0.0012, top - H + 0.002, W / 2, 0.0034, top, paper_edge)
    box("calendar_back", -W / 2 - 0.002, -0.0005, top - H - 0.004, W / 2 + 0.002, 0.0012, top + 0.006, cardboard)
    # the twin-wire binding along the top, and the hanging loop in the middle
    n = 34
    for k in range(n):
        x = -W / 2 + 0.008 + (W - 0.016) * k / (n - 1)
        if abs(x) < 0.03:
            continue
        torus(f"calendar_wire_{k}", Vector((x, 0.0025, top + 0.002)), 0.0042, 0.0006, "x", wire, seg=10, tube=4)
    bm = bmesh.new()
    pts = [Vector((-0.028, 0.002, top + 0.002)), Vector((0.0, 0.0015, 0.004)), Vector((0.028, 0.002, top + 0.002))]
    obj_loop = None
    for a, b in zip(pts, pts[1:]):
        d = (b - a)
        L = d.length
        c = (a + b) / 2
        cyl = cylinder("calendar_hanger", c, 0.0007, L, False, wire, seg=6)
        cyl.rotation_mode = "QUATERNION"
        cyl.rotation_quaternion = Vector((0, 0, 1)).rotation_difference(d.normalized())
    bm.free()
    # the nail it hangs on
    cylinder("calendar_nail_head", Vector((0, 0.012, 0.0)), 0.003, 0.0015, True, nail)
    cylinder("calendar_nail", Vector((0, 0.005, 0.0)), 0.0012, 0.014, True, nail)

elif PID in ("journal", "journal_open"):
    BW, BH, T = 0.148, 0.210, 0.016  # A5 page, the block's thickness
    C = 0.0025  # cover board
    if PID == "journal":
        # the closed book: boards wrapped in cloth, the page block, a spine, a label, the ribbon
        box("journal_cover_back", -BW / 2 - 0.003, -BH / 2 - 0.003, 0.0, BW / 2 + 0.003, BH / 2 + 0.003, C, cloth)
        box("journal_block", -BW / 2 + 0.002, -BH / 2 + 0.001, C, BW / 2 - 0.001, BH / 2 - 0.001, C + T, paper_edge)
        box("journal_cover_front", -BW / 2 - 0.003, -BH / 2 - 0.003, C + T, BW / 2 + 0.003, BH / 2 + 0.003, 2 * C + T, cloth)
        bm = bmesh.new()
        # the rounded spine along -X
        seg = 10
        verts = []
        for i in range(seg + 1):
            a = math.pi / 2 + math.pi * i / seg
            x = -BW / 2 - 0.003 + math.cos(a) * 0.004
            z = (2 * C + T) / 2 + math.sin(a) * (2 * C + T) / 2
            verts.append((bm.verts.new((x, -BH / 2 - 0.003, z)), bm.verts.new((x, BH / 2 + 0.003, z))))
        for (a0, a1), (b0, b1) in zip(verts, verts[1:]):
            bm.faces.new((a0, b0, b1, a1))
        sp = obj("journal_spine", bm)
        sp.data.materials.append(cloth)
        box("journal_label", -0.045, 0.02, 2 * C + T, 0.045, 0.075, 2 * C + T + 0.0004, label)
        box("journal_ribbon", 0.02, -BH / 2 - 0.035, C + T - 0.0004, 0.026, -BH / 2 + 0.002, C + T, ribbon)
    else:
        page_mat = image_material("kzpage_journal_sample", os.path.join(PAGES, "journal_sample.png"))
        # open flat-ish on the desk: two covers, two page blocks thinning to the gutter, the spread
        box("journal_cover_l", -BW - 0.004, -BH / 2 - 0.003, 0.0, -0.002, BH / 2 + 0.003, C, cloth)
        box("journal_cover_r", 0.002, -BH / 2 - 0.003, 0.0, BW + 0.004, BH / 2 + 0.003, C, cloth)
        box("journal_endpaper_l", -BW - 0.001, -BH / 2, C, -0.004, BH / 2, C + 0.0004, endpaper)

        def block_h(x):
            # pages pile up away from the gutter, dip into it
            ax = abs(x) / BW
            return C + T / 2 * (0.25 + 0.75 * min(1.0, ax * 3.0)) - 0.002 * (1 - ax) ** 6

        def spread(u, v):
            x = (u - 0.5) * 2 * BW
            # (the gutter's curve: pages rise from the middle and settle flat)
            z = block_h(x) + 0.006 * math.sin(min(1.0, abs(x) / (BW * 0.35)) * math.pi / 2)
            return Vector((x, (0.5 - v) * BH, z))
        sheet("kzpage_journal", 2 * BW, BH, 40, 16, page_mat, spread)
        # the page edges along the outside
        for s in (-1, 1):
            box(f"journal_edge_{s}", min(s * (BW - 0.002), s * BW), -BH / 2, C, max(s * (BW - 0.002), s * BW), BH / 2, block_h(BW) + 0.004, paper_edge)
        box("journal_ribbon", -0.004, -BH / 2 - 0.04, C + T * 0.3, 0.002, BH / 2, C + T * 0.3 + 0.0004, ribbon)
else:
    raise SystemExit(f"unknown prop {PID}")

# one mesh for the prop itself (its small parts joined, world-projected UVs), the page its own
# node; the whole sitting on z = 0 (the gate's origin rule: a calendar's origin is the bottom of
# its board, hung on the wall at its height)
parts = [o for o in COLL.objects if not o.name.startswith("kzpage")]
for o in COLL.objects:
    o.data.transform(o.matrix_world)
    o.matrix_world = o.matrix_world.Identity(4)
    if PID == "wall_calendar":
        # (built facing +Y; props face -Y, the way the game and the look-dev camera expect)
        o.data.transform(__import__("mathutils").Matrix.Rotation(math.pi, 4, "Z"))
bpy.ops.object.select_all(action="DESELECT")
for o in parts:
    o.select_set(True)
bpy.context.view_layer.objects.active = parts[0]
bpy.ops.object.join()
body = bpy.context.view_layer.objects.active
body.name = PID
bm = bmesh.new()
bm.from_mesh(body.data)
kz.world_uv(bm, 4.0)
bm.to_mesh(body.data)
bm.free()
zmin = min((o.matrix_world @ v.co).z for o in COLL.objects for v in o.data.vertices)
for o in COLL.objects:
    o.data.transform(__import__("mathutils").Matrix.Translation((0, 0, -zmin)))

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
