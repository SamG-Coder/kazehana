"""The town's sites (the census's places that aren't homes or shops), each as a quality-gated
building laid out on its own lot (assets/town/site_frames.json, tools/site_frames.py):

    python tools/site_frames.py && python tools/make_site_signs.py
    blender --background --factory-startup --python tools/blender/build_site.py -- <kind>

-> assets/buildings/site_<kind>/site_<kind>.gltf

kinds: konbini, petrol, drugstore, coop, car_repair, post_office, town_office, station, plaza,
shrine, inari, temple. (The elementary school's buildings are the junior high's design, placed on
its grounds by the game.)

The local frame is the lot's: origin at ground level in its middle, +X along it, +Y its front (the
road; the station's platform faces the plaza). Every brand is the town's own (make_site_signs.py).
Buildings are solid shells with dim rooms behind their glass; furniture, shelves, goods and
vehicles would be separate approved props.
"""

import json
import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
KIND = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "konbini"
FRAME = json.load(open(os.path.join(ROOT, "assets", "town", "site_frames.json"), encoding="utf8"))[KIND]
LX, LY = FRAME["lx"], FRAME["ly"]
ASSET_ID = "site_" + KIND
OUT = os.path.join(ROOT, "assets", "buildings", ASSET_ID, ASSET_ID + ".gltf")
SIGNS = os.path.join(ROOT, "assets", "cache", "signs", "site_signs.png")
ATLAS = json.load(open(os.path.join(ROOT, "assets", "cache", "signs", "site_signs.json")))

kz.reset_scene()
_c = bpy.data.collections.new("HOUSE")
bpy.context.scene.collection.children.link(_c)

DETAIL = "Precast_Concrete_Wall"
P = kz.painted_material
concrete = P("site_concrete", (0.55, 0.54, 0.51), DETAIL, roughness=0.85, normal_strength=0.7)
asphalt = kz.surface_material("site_asphalt", "Concrete_Floor-f4b85cb0", tint=(0.22, 0.22, 0.22), desaturate=0.9)
paving = kz.surface_material("site_paving", "Concrete_Floor-f4b85cb0", tint=(0.62, 0.60, 0.56), desaturate=0.6)
gravel = kz.surface_material("site_gravel", "Concrete_Floor-f4b85cb0", tint=(0.50, 0.48, 0.45), desaturate=0.7)
line = P("site_paint_white", (0.85, 0.85, 0.82), DETAIL, roughness=0.6, normal_strength=0.1)
yellow = P("site_paint_yellow", (0.85, 0.66, 0.10), DETAIL, roughness=0.6, normal_strength=0.1)
panel = P("site_panel_white", (0.80, 0.80, 0.78), DETAIL, roughness=0.5, metallic=0.2, normal_strength=0.15)
grey = P("site_panel_grey", (0.42, 0.43, 0.43), DETAIL, roughness=0.5, metallic=0.3, normal_strength=0.2)
steel = P("site_steel", (0.30, 0.31, 0.32), DETAIL, roughness=0.45, metallic=0.6, normal_strength=0.15)
alu = P("house_aluminium", (0.09, 0.075, 0.065), DETAIL, roughness=0.35, metallic=0.8, normal_strength=0.1)
silver = P("shop_silver_alu", (0.55, 0.55, 0.54), DETAIL, roughness=0.3, metallic=0.85, normal_strength=0.1)
dark = P("house_dark", (0.02, 0.02, 0.02), DETAIL, roughness=0.6, normal_strength=0.1)
room = P("house_room", (0.28, 0.24, 0.20), DETAIL, roughness=0.9, normal_strength=0.2)
inwall = P("shophouse_shop_wall", (0.62, 0.60, 0.55), DETAIL, roughness=0.9, normal_strength=0.15, grain=0.15)
unit = P("house_unit", (0.70, 0.70, 0.67), DETAIL, roughness=0.45, metallic=0.2, normal_strength=0.15)
wood = P("site_wood", (0.25, 0.16, 0.09), "Wood_Planks", roughness=0.55, normal_strength=0.7)
dark_wood = P("farmhouse_board", (0.07, 0.045, 0.03), "Wood_Planks", roughness=0.6, normal_strength=0.8)
vermilion = P("site_vermilion", (0.62, 0.12, 0.05), DETAIL, roughness=0.5, normal_strength=0.2)
black = P("site_black_lacquer", (0.03, 0.03, 0.03), DETAIL, roughness=0.4, normal_strength=0.1)
stone = kz.surface_material("site_stone", "Japanese_Concrete_Block_Wall", tint=(0.58, 0.56, 0.53), desaturate=0.6)
granite = P("site_granite", (0.48, 0.47, 0.46), DETAIL, roughness=0.6, normal_strength=0.9, grain=0.4)
plaster = P("farmhouse_plaster_a", (0.80, 0.79, 0.75), DETAIL, roughness=0.92, normal_strength=0.35, grain=0.3)
tiles = kz.surface_material("site_wall_tiles", "Japanese_Concrete_Block_Wall", tint=(0.66, 0.56, 0.46), desaturate=0.3)
roof_tile = kz.surface_material("site_roof_tiles", "Japanese_Roof_Tiles", tint=(0.48, 0.50, 0.54), desaturate=0.85)
glass = kz.plain_material("kz_glow_shop_clear_glass", (0.9, 0.95, 0.93), roughness=0.02)
dglass = kz.plain_material("kz_glow_glass", (0.025, 0.03, 0.035), roughness=0.04)
frosted = kz.plain_material("kz_glow_glass_frosted", (0.55, 0.56, 0.56), roughness=0.35)


def image_material(name, path, roughness=0.45):
    _, n, orm = kz.fab_textures(DETAIL)
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
    nm.inputs["Strength"].default_value = 0.2
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(orm, "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    mul = nt.nodes.new("ShaderNodeMath")
    mul.operation = "MULTIPLY"
    mul.inputs[1].default_value = roughness / 0.75
    nt.links.new(sep.outputs["Green"], mul.inputs[0])
    nt.links.new(mul.outputs["Value"], bsdf.inputs["Roughness"])
    return m


sign_mat = image_material("site_signs", SIGNS)

walls = kz.MeshBuilder("walls", uv_scale=0.5)
fit = kz.MeshBuilder("fittings", uv_scale=1.0)
roofs = kz.MeshBuilder("roof", uv_scale=0.5)
ground = kz.MeshBuilder("ground", uv_scale=0.25)
signs = []          # (corners bl, br, tr, tl; atlas key)


def box(b, x0, y0, z0, x1, y1, z1, mat):
    b.box((min(x0, x1), min(y0, y1), min(z0, z1)), (max(x0, x1), max(y0, y1), max(z0, z1)), mat)


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


def sign(x0, x1, y, z0, z1, key, facing=1):
    """A sign face (atlas piece `key`) on a plane y = const, facing +Y (1) or -Y (-1)."""
    if facing > 0:
        # (seen from +Y, +x is on the viewer's left: the picture's left at x1)
        corners = [(x1, y, z0), (x0, y, z0), (x0, y, z1), (x1, y, z1)]
    else:
        corners = [(x0, y, z0), (x1, y, z0), (x1, y, z1), (x0, y, z1)]
    signs.append((corners, key))


def sign_x(x, y0, y1, z0, z1, key, facing=1):
    """A sign face on a plane x = const, facing +X (1) or -X (-1)."""
    if facing > 0:
        corners = [(x, y0, z0), (x, y1, z0), (x, y1, z1), (x, y0, z1)]
    else:
        corners = [(x, y1, z0), (x, y0, z0), (x, y0, z1), (x, y1, z1)]
    signs.append((corners, key))


def lot(mat, z=0.06, x0=-LX / 2, x1=LX / 2, y0=-LY / 2, y1=LY / 2, skirt=0.7):
    """The lot's surface, slightly raised over the terrain, its edge a concrete skirt 0.7 m down
    into the ground (where the terrain falls away round it, it shows as the lot's retaining edge,
    never a gap under a floating slab)."""
    box(ground, x0, y0, 0.0, x1, y1, z, mat)
    box(fit, x0 - 0.03, y0 - 0.03, -skirt, x1 + 0.03, y1 + 0.03, z - 0.01, concrete)


def kerb(x0, y0, x1, y1, h=0.15, t=0.12):
    for (a, b, c, d) in ((x0, y0, x1, y0 + t), (x0, y1 - t, x1, y1), (x0, y0, x0 + t, y1), (x1 - t, y0, x1, y1)):
        box(fit, a, b, 0.0, c, d, h, concrete)


def parking(x0, x1, y0, depth, n, facing=1):
    """n parking stalls along x from x0 to x1, depth into +Y (facing 1) or -Y, white lines and
    wheel stops."""
    w = (x1 - x0) / n
    for k in range(n + 1):
        x = x0 + k * w
        box(fit, x - 0.06, y0, 0.06, x + 0.06, y0 + facing * depth, 0.065, line)
    box(fit, x0, y0 + facing * depth - 0.06, 0.06, x1, y0 + facing * depth + 0.06, 0.065, line)
    for k in range(n):
        cx = x0 + (k + 0.5) * w
        yy = y0 + facing * 0.7
        box(fit, cx - 0.8, yy - 0.08, 0.06, cx + 0.8, yy + 0.08, 0.2, concrete)


def flat_building(x0, y0, x1, y1, h, wall_mat, front_glass=None, parapet=0.6, back_door=True):
    """A flat-roofed box: walls, a parapet with its coping, the roof's membrane and some plant on
    it, a glazed front (x0g, x1g, z0, z1) on its +Y face with a dim room behind."""
    t = 0.2
    holes = [front_glass] if front_glass else []
    # +Y face
    xs = [x0, x1]
    if front_glass:
        gx0, gx1, gz0, gz1 = front_glass
        box(walls, x0, y1 - t, 0.0, gx0, y1, h, wall_mat)
        box(walls, gx1, y1 - t, 0.0, x1, y1, h, wall_mat)
        box(walls, gx0, y1 - t, gz1, gx1, y1, h, wall_mat)
        box(walls, gx0, y1 - t, 0.0, gx1, y1, gz0, wall_mat)
        box(fit, gx0, y1 - 0.1, gz0, gx1, y1 - 0.09, gz1, glass)
        n = max(2, int((gx1 - gx0) / 1.8))
        for k in range(n + 1):
            x = gx0 + k * (gx1 - gx0) / n
            box(fit, x - 0.04, y1 - 0.14, gz0, x + 0.04, y1 - 0.04, gz1, silver)
        for z in (gz0, gz1 - 0.08, gz0 + 2.2):
            if z < gz1:
                box(fit, gx0, y1 - 0.14, z, gx1, y1 - 0.04, z + 0.08, silver)
        # the room behind: floor, back wall, ceiling (dim, empty)
        box(walls, gx0, y1 - 4.5, 0.0, gx1, y1 - 0.2, 0.08, P("shop_floor_vinyl", (0.50, 0.49, 0.46), "Concrete_Floor-f4b85cb0", roughness=0.55, normal_strength=0.2, grain=0.25))
        box(walls, gx0, y1 - 4.6, 0.0, gx1, y1 - 4.5, gz1 + 0.3, inwall)
        box(walls, gx0, y1 - 4.5, gz1 + 0.25, gx1, y1 - 0.2, gz1 + 0.3, P("shop_ceiling", (0.80, 0.79, 0.76), "Concrete_Floor-f4b85cb0", roughness=0.95, normal_strength=0.06, grain=0.06))
        for xe in (gx0, gx1):
            box(walls, xe - 0.02, y1 - 4.5, 0.0, xe, y1 - 0.2, gz1 + 0.3, inwall)
    else:
        box(walls, x0, y1 - t, 0.0, x1, y1, h, wall_mat)
    box(walls, x0, y0, 0.0, x1, y0 + t, h, wall_mat)
    box(walls, x0, y0, 0.0, x0 + t, y1, h, wall_mat)
    box(walls, x1 - t, y0, 0.0, x1, y1, h, wall_mat)
    box(walls, x0, y0, h - 0.3, x1, y1, h - 0.2, grey)                             # roof slab
    for (a, b, c, d) in ((x0, y0, x1, y0 + t), (x0, y1 - t, x1, y1), (x0, y0, x0 + t, y1), (x1 - t, y0, x1, y1)):
        box(walls, a, b, h, c, d, h + parapet, wall_mat)
        box(fit, a - 0.03, b - 0.03, h + parapet, c + 0.03, d + 0.03, h + parapet + 0.06, silver)
    # roof-top plant: two condensers on a frame
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2 - (y1 - y0) * 0.2
    for k in (-1, 1):
        box(fit, cx + k * 1.6 - 0.8, cy - 0.5, h - 0.2, cx + k * 1.6 + 0.8, cy + 0.5, h + 0.9, unit)
        rcyl(fit, (cx + k * 1.6 - 0.35, cy, h + 0.91), (cx + k * 1.6 - 0.35, cy, h + 0.93), 0.35, dark, seg=20)
        rcyl(fit, (cx + k * 1.6 + 0.35, cy, h + 0.91), (cx + k * 1.6 + 0.35, cy, h + 0.93), 0.35, dark, seg=20)
    if back_door:
        dx = x1 - 2.5
        box(fit, dx, y0 - 0.06, 0.0, dx + 0.9, y0 + 0.01, 2.05, grey)
        box(fit, dx - 0.6, y0 - 0.9, 2.3, dx + 1.5, y0, 2.38, unit)


def gable_roof(b, x0, x1, y0, y1, z, pitch_deg, ov, mat, along_x=True, rt=0.14):
    """A plain gable roof (ridge along x or y) over the rectangle, eaves `ov`, with its gable
    triangles in `mat_end` left to the caller."""
    p = math.radians(pitch_deg)
    if along_x:
        half = (y1 - y0) / 2 + ov
        cy = (y0 + y1) / 2
        rise = ((y1 - y0) / 2) * math.tan(p)
        z_e = z - ov * math.tan(p)
        L = half / math.cos(p)
        for s in (-1, 1):
            rot = Matrix.Rotation(p if s < 0 else -p, 3, "X")
            nrm = rot @ Vector((0, 0, 1))
            c = Vector(((x0 + x1) / 2, cy + s * half / 2, z_e + (z + rise - z_e) / 2 + 0.03)) - nrm * (rt / 2)
            b.oriented_box(c, (x1 - x0 + 2 * ov, L, rt), rot, mat)
        b.box((x0 - ov, cy - 0.12, z + rise - 0.03), (x1 + ov, cy + 0.12, z + rise + 0.1), mat)
        return rise
    half = (x1 - x0) / 2 + ov
    cx = (x0 + x1) / 2
    rise = ((x1 - x0) / 2) * math.tan(p)
    z_e = z - ov * math.tan(p)
    L = half / math.cos(p)
    for s in (-1, 1):
        rot = Matrix.Rotation(-p if s < 0 else p, 3, "Y")
        nrm = rot @ Vector((0, 0, 1))
        c = Vector((cx + s * half / 2, (y0 + y1) / 2, z_e + (z + rise - z_e) / 2 + 0.03)) - nrm * (rt / 2)
        b.oriented_box(c, (L, y1 - y0 + 2 * ov, rt), rot, mat)
    b.box((cx - 0.12, y0 - ov, z + rise - 0.03), (cx + 0.12, y1 + ov, z + rise + 0.1), mat)
    return rise


def gable_ends(b, x0, x1, y0, y1, z, rise, mat, along_x=True):
    if along_x:
        cy = (y0 + y1) / 2
        for sx, x in ((-1, x0), (1, x1)):
            tri = [Vector((x, y0, z)), Vector((x, y1, z)), Vector((x, cy, z + rise))]
            b.quad(tri if sx > 0 else [tri[1], tri[0], tri[2]], mat)
    else:
        cx = (x0 + x1) / 2
        for sy, y in ((-1, y0), (1, y1)):
            tri = [Vector((x1, y, z)), Vector((x0, y, z)), Vector((cx, y, z + rise))]
            b.quad(tri if sy > 0 else [tri[1], tri[0], tri[2]], mat)


def irimoya(cx, cy, w, d, top, pitch_deg, eov, mat, end_mat):
    """An irimoya (hip-and-gable) roof over a w x d rectangle (long side along x), as the
    farmhouses', its tiles running down every slope."""
    rt = 0.18
    tp = math.tan(math.radians(pitch_deg))
    xe, ye = w / 2 + eov, d / 2 + eov
    ze = top - eov * tp + 0.05
    zr = ze + ye * tp
    yg = ye * 0.42
    xg = xe - (ye - yg)
    zg = ze + (ye - yg) * tp
    mb = kz.MeshBuilder("irimoya_%d" % len(bpy.data.meshes), uv_scale=0.5)
    me = kz.MeshBuilder("irimoya_end_%d" % len(bpy.data.meshes), uv_scale=0.5)

    def slab(b, poly):
        poly = [(cx + v[0], cy + v[1], v[2]) for v in poly]
        f = b.bm.faces.new([b.bm.verts.new(Vector(v)) for v in poly])
        f.material_index = b._mat_index(mat)
        f = b.bm.faces.new([b.bm.verts.new(Vector(v) - Vector((0, 0, rt))) for v in reversed(poly)])
        f.material_index = b._mat_index(mat)

    for s in (-1, 1):
        poly = [(-xe, s * ye, ze), (xe, s * ye, ze), (xg, s * yg, zg), (xg, 0, zr), (-xg, 0, zr), (-xg, s * yg, zg)]
        slab(mb, poly if s < 0 else list(reversed(poly)))
    for sx in (-1, 1):
        poly = [(sx * xe, -ye, ze), (sx * xe, ye, ze), (sx * xg, yg, zg), (sx * xg, -yg, zg)]
        slab(me, poly if sx > 0 else list(reversed(poly)))
        tri = [(cx + sx * xg, cy - yg, zg), (cx + sx * xg, cy + yg, zg), (cx + sx * xg, cy, zr)]
        f = mb.bm.faces.new([mb.bm.verts.new(Vector(v)) for v in (tri if sx > 0 else [tri[1], tri[0], tri[2]])])
        f.material_index = mb._mat_index(end_mat)
        for sy in (-1, 1):
            rcyl(roofs, (cx + sx * xe, cy + sy * ye, ze + 0.06), (cx + sx * xg, cy + sy * yg, zg + 0.06), 0.13, mat)
        box(roofs, cx + sx * xg - 0.1, cy - 0.28, zr - 0.05, cx + sx * xg + 0.1, cy + 0.28, zr + 0.55, mat)   # onigawara
    rcyl(roofs, (cx - xg - 0.1, cy, zr + 0.08), (cx + xg + 0.1, cy, zr + 0.08), 0.2, mat, seg=16)
    box(roofs, cx - xg, cy - 0.14, zr - 0.05, cx + xg, cy + 0.14, zr + 0.12, mat)
    for (x0, y0, x1, y1) in ((-xe, -ye, xe, -ye + 0.06), (-xe, ye - 0.06, xe, ye), (-xe, -ye, -xe + 0.06, ye), (xe - 0.06, -ye, xe, ye)):
        box(fit, cx + x0, cy + y0, ze - rt - 0.22, cx + x1, cy + y1, ze - rt, end_mat)      # the eave boards
    mb.finish(collection="HOUSE")
    ends = me.finish(collection="HOUSE")
    for l in ends.data.uv_layers.active.data:
        u, v = l.uv
        l.uv = (v, u)
    return zr


def torii(x, y, w, h, mat, plaque=None, rot90=False):
    """A torii: two pillars, the kasagi with its upturned ends, the nuki, the plaque between."""
    def P2(a, b):
        return (x + b, y + a) if rot90 else (x + a, y + b)
    for s in (-1, 1):
        px, py = P2(s * w / 2, 0)
        rcyl(fit, (px, py, 0.0), (px, py, h), 0.16 * h / 4.5, mat, seg=16)
        box(fit, px - 0.25, py - 0.25, 0.0, px + 0.25, py + 0.25, 0.35, black)               # the base (kamaki)
    ext = w / 2 + 0.9
    a, b = P2(-ext, 0), P2(ext, 0)
    box(fit, min(a[0], b[0]) - (0.12 if rot90 else 0), min(a[1], b[1]) - (0 if rot90 else 0.15), h, max(a[0], b[0]) + (0.12 if rot90 else 0), max(a[1], b[1]) + (0 if rot90 else 0.15), h + 0.22, mat)
    box(fit, min(a[0], b[0]) - (0.14 if rot90 else 0), min(a[1], b[1]) - (0 if rot90 else 0.17), h + 0.22, max(a[0], b[0]) + (0.14 if rot90 else 0), max(a[1], b[1]) + (0 if rot90 else 0.17), h + 0.34, black)  # kasagi
    for s in (-1, 1):
        ex, ey = P2(s * (ext + 0.15), 0)
        box(fit, ex - 0.15, ey - 0.15, h + 0.25, ex + 0.15, ey + 0.15, h + 0.45, black)     # the upturned ends
    a, b = P2(-(w / 2 + 0.45), 0), P2(w / 2 + 0.45, 0)
    box(fit, min(a[0], b[0]) - (0.08 if rot90 else 0), min(a[1], b[1]) - (0 if rot90 else 0.1), h - 0.9, max(a[0], b[0]) + (0.08 if rot90 else 0), max(a[1], b[1]) + (0 if rot90 else 0.1), h - 0.7, mat)  # nuki
    if plaque:
        px, py = P2(0, 0)
        box(fit, px - 0.32, py - 0.06, h - 0.7, px + 0.32, py + 0.06, h, black)
        if rot90:
            sign_x(px + 0.065, py - 0.27, py + 0.27, h - 0.65, h - 0.05, plaque)
        else:
            sign(px - 0.27, px + 0.27, py + 0.065, h - 0.65, h - 0.05, plaque)


def lantern(x, y, h=2.0):
    """A stone lantern (ishidōrō): base, post, fire box, roof, jewel."""
    box(fit, x - 0.35, y - 0.35, 0.0, x + 0.35, y + 0.35, 0.25, granite)
    rcyl(fit, (x, y, 0.25), (x, y, h * 0.55), 0.13, granite, seg=12)
    box(fit, x - 0.3, y - 0.3, h * 0.55, x + 0.3, y + 0.3, h * 0.62, granite)
    box(fit, x - 0.24, y - 0.24, h * 0.62, x + 0.24, y + 0.24, h * 0.8, granite)
    box(fit, x - 0.12, y - 0.25, h * 0.66, x + 0.12, y + 0.26, h * 0.76, dark)
    box(fit, x - 0.42, y - 0.42, h * 0.8, x + 0.42, y + 0.42, h * 0.86, granite)
    box(fit, x - 0.25, y - 0.25, h * 0.86, x + 0.25, y + 0.25, h * 0.92, granite)
    rcyl(fit, (x, y, h * 0.92), (x, y, h), 0.1, granite, seg=10)


def shrine_hall(cx, cy, w, d, top, roof_pitch=30, eov=1.2):
    """A shrine / temple hall: a raised floor on posts, plank walls and lattice doors, a veranda
    round it, the irimoya roof."""
    f0 = 0.9
    box(fit, cx - w / 2 - 0.9, cy - d / 2 - 0.9, f0 - 0.12, cx + w / 2 + 0.9, cy + d / 2 + 0.9, f0, wood)   # veranda
    for sx in (-1, 1):
        for sy in (-1, 1):
            for k in range(int(w / 1.8) + 1):
                px = cx - w / 2 + k * w / int(w / 1.8)
                rcyl(fit, (px, cy + sy * d / 2, 0.0), (px, cy + sy * d / 2, top), 0.14, wood, seg=12)
            for k in range(int(d / 1.8) + 1):
                py = cy - d / 2 + k * d / int(d / 1.8)
                rcyl(fit, (cx + sx * w / 2, py, 0.0), (cx + sx * w / 2, py, top), 0.14, wood, seg=12)
    box(walls, cx - w / 2 + 0.1, cy - d / 2 + 0.1, f0, cx + w / 2 - 0.1, cy + d / 2 - 0.1, top - 0.3, dark_wood)
    # lattice doors across the front
    for k in range(int(w / 1.8)):
        x0 = cx - w / 2 + k * w / int(w / 1.8) + 0.18
        x1 = x0 + w / int(w / 1.8) - 0.36
        y = cy + d / 2 - 0.08
        box(fit, x0, y, f0 + 0.1, x1, y + 0.02, top - 0.5, room)
        xx = x0 + 0.04
        while xx < x1:
            box(fit, xx, y + 0.02, f0 + 0.1, xx + 0.03, y + 0.06, top - 0.5, wood)
            xx += 0.12
        zz = f0 + 0.15
        while zz < top - 0.5:
            box(fit, x0, y + 0.02, zz, x1, y + 0.06, zz + 0.03, wood)
            zz += 0.12
    box(fit, cx - w / 2 - 0.3, cy - d / 2 - 0.3, top - 0.35, cx + w / 2 + 0.3, cy + d / 2 + 0.3, top, wood)  # the beams
    # steps up to the veranda at the front, the base posts' stones
    for k in range(4):
        box(fit, cx - 1.4, cy + d / 2 + 0.9 + (3 - k) * 0.3, 0.0, cx + 1.4, cy + d / 2 + 0.9 + (4 - k) * 0.3, (k + 1) * f0 / 4, wood)
    box(walls, cx - w / 2 - 0.9, cy - d / 2 - 0.9, 0.0, cx + w / 2 + 0.9, cy + d / 2 + 0.9, 0.12, stone)
    return irimoya(cx, cy, w, d, top, roof_pitch, eov, roof_tile, wood)


# ----------------------------------------------------------------------------- the sites
def site_konbini():
    lot(asphalt)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2)
    bx0, bx1, by0, by1, h = -11.0, 11.0, -LY / 2 + 1.0, -LY / 2 + 13.0, 4.6
    flat_building(bx0, by0, bx1, by1, h, panel, front_glass=(bx0 + 1.0, bx1 - 4.0, 0.08, 3.0))
    box(fit, bx0, by1, 3.05, bx1, by1 + 0.25, 4.3, panel)                                     # the fascia
    sign(bx0 + 0.2, bx1 - 0.2, by1 + 0.26, 3.12, 4.22, "konbini_fascia")
    box(fit, bx0 - 0.1, by1, 2.95, bx1 + 0.1, by1 + 1.5, 3.05, panel)                         # the canopy over the glass
    for k in range(3):                                                                         # the entrance's auto doors
        x = bx1 - 7.5 + k * 1.0
        box(fit, x, by1 - 0.06, 0.08, x + 0.04, by1 + 0.02, 2.4, silver)
    box(walls, bx0, by1 - 0.01, 0.0, bx1, by1 + 1.6, 0.12, paving)                             # the apron
    parking(-LX / 2 + 1.5, LX / 2 - 1.5, by1 + 1.8, 5.0, 8)
    # the pole sign by the road
    px, py = LX / 2 - 2.0, LY / 2 - 1.5
    box(fit, px - 0.15, py - 0.15, 0.0, px + 0.15, py + 0.15, 4.0, steel)
    box(fit, px - 0.7, py - 0.22, 4.0, px + 0.7, py + 0.22, 6.6, panel)
    sign(px - 0.62, px + 0.62, py + 0.23, 4.1, 6.5, "konbini_pole")
    sign(px - 0.62, px + 0.62, py - 0.23, 4.1, 6.5, "konbini_pole", facing=-1)
    # the side: the refrigeration units, the bins enclosure
    for k in range(3):
        box(fit, bx1 + 0.3, by0 + 1.0 + k * 1.2, 0.06, bx1 + 1.1, by0 + 2.0 + k * 1.2, 1.2, unit)
    box(fit, bx0 - 2.5, by0 + 1.0, 0.06, bx0 - 0.3, by0 + 3.5, 1.6, grey)


def site_petrol():
    lot(paving)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2)
    # the canopy over two islands
    cx0, cx1, cy0, cy1, ch = -10.0, 8.0, -4.0, 6.0, 5.2
    for x in (cx0 + 2.0, cx1 - 2.0):
        for y in (cy0 + 2.5, cy1 - 2.5):
            box(fit, x - 0.25, y - 0.25, 0.0, x + 0.25, y + 0.25, ch, panel)
    box(fit, cx0, cy0, ch, cx1, cy1, ch + 0.9, panel)
    sign(cx0 + 0.1, cx1 - 0.1, cy1 + 0.01, ch + 0.1, ch + 0.8, "petrol_band")
    sign(cx0 + 0.1, cx1 - 0.1, cy0 - 0.01, ch + 0.1, ch + 0.8, "petrol_band", facing=-1)
    box(fit, cx0 + 0.2, cy0 + 0.2, ch - 0.05, cx1 - 0.2, cy1 - 0.2, ch, unit)                  # the soffit
    for y in (cy0 + 2.5, cy1 - 2.5):
        box(fit, -6.5, y - 0.6, 0.06, 4.5, y + 0.6, 0.25, concrete)                           # the island
        for x in (-4.0, 2.0):
            box(fit, x - 0.5, y - 0.35, 0.25, x + 0.5, y + 0.35, 1.9, panel)                  # dispenser
            box(fit, x - 0.4, y - 0.37, 1.3, x + 0.4, y + 0.37, 1.7, dark)                    # its screen panel
            box(fit, x - 0.55, y - 0.4, 1.9, x + 0.55, y + 0.4, 2.1, P("site_red", (0.55, 0.06, 0.06), DETAIL, roughness=0.5, normal_strength=0.1))
            rcyl(fit, (x + 0.5, y, 1.2), (x + 0.75, y, 0.9), 0.03, dark)                      # hose
    # the kiosk at the back
    flat_building(-LX / 2 + 2.0, -LY / 2 + 1.0, -LX / 2 + 10.0, -LY / 2 + 7.5, 3.6, panel, front_glass=(-LX / 2 + 2.8, -LX / 2 + 9.2, 0.08, 2.6), parapet=0.4)
    # the price pole
    px, py = LX / 2 - 2.0, LY / 2 - 1.5
    box(fit, px - 0.15, py - 0.15, 0.0, px + 0.15, py + 0.15, 3.0, steel)
    box(fit, px - 0.55, py - 0.2, 3.0, px + 0.55, py + 0.2, 6.3, panel)
    sign(px - 0.5, px + 0.5, py + 0.21, 3.05, 6.25, "petrol_pole")
    sign(px - 0.5, px + 0.5, py - 0.21, 3.05, 6.25, "petrol_pole", facing=-1)


def site_drugstore():
    lot(asphalt)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2)
    bx0, bx1, by0, by1, h = -17.0, 17.0, -LY / 2 + 1.0, -LY / 2 + 19.0, 5.5
    flat_building(bx0, by0, bx1, by1, h, grey, front_glass=(-6.0, 6.0, 0.08, 3.0))
    box(fit, -10.5, by1, 3.3, 10.5, by1 + 0.3, 5.3, P("site_blue", (0.10, 0.36, 0.55), DETAIL, roughness=0.5, normal_strength=0.1))
    sign(-10.3, 10.3, by1 + 0.31, 3.4, 5.2, "drugstore_fascia")
    box(fit, -7.0, by1, 3.0, 7.0, by1 + 2.0, 3.15, panel)
    box(walls, bx0, by1, 0.0, bx1, by1 + 2.2, 0.12, paving)
    parking(-LX / 2 + 1.5, LX / 2 - 1.5, by1 + 2.6, 5.0, 14)
    parking(-LX / 2 + 1.5, LX / 2 - 1.5, LY / 2 - 1.0, 5.0, 14, facing=-1)


def site_coop():
    lot(asphalt)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2)
    # the shop (west) and the warehouse (east)
    flat_building(-LX / 2 + 2.0, -LY / 2 + 2.0, -LX / 2 + 26.0, -LY / 2 + 16.0, 4.5, tiles, front_glass=(-LX / 2 + 5.0, -LX / 2 + 23.0, 0.08, 2.8))
    sign(-LX / 2 + 4.0, -LX / 2 + 24.0, -LY / 2 + 16.27, 3.0, 4.25, "coop_board")
    box(fit, -LX / 2 + 3.8, -LY / 2 + 16.0, 2.9, -LX / 2 + 24.2, -LY / 2 + 16.26, 4.35, panel)
    wx0, wx1, wy0, wy1 = -LX / 2 + 30.0, LX / 2 - 3.0, -LY / 2 + 2.0, -LY / 2 + 18.0
    corr = P("shed_corrugated", (0.42, 0.42, 0.40), DETAIL, roughness=0.55, metallic=0.55, normal_strength=0.5)
    for (a, b, c, d) in ((wx0, wy0, wx1, wy0 + 0.15), (wx0, wy0, wx0 + 0.15, wy1), (wx1 - 0.15, wy0, wx1, wy1)):
        box(walls, a, b, 0.0, c, d, 6.5, corr)
    for k in range(3):                                                                         # the loading bays
        x0 = wx0 + 2.0 + k * 12.0
        box(walls, x0 - (2.0 if k == 0 else 0.0), wy1 - 0.15, 0.0, x0, wy1, 6.5, corr)
        box(fit, x0, wy1 - 0.12, 0.0, x0 + 6.0, wy1 - 0.05, 4.5, P("shed_door", (0.33, 0.33, 0.31), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.4))
        box(walls, x0, wy1 - 0.15, 4.5, x0 + 6.0, wy1, 6.5, corr)
        box(walls, x0 + 6.0, wy1 - 0.15, 0.0, min(x0 + 12.0, wx1), wy1, 6.5, corr)
    box(fit, wx0, wy1, 0.0, wx1, wy1 + 2.5, 1.1, concrete)                                      # the loading dock
    rise = gable_roof(roofs, wx0, wx1, wy0, wy1, 6.5, 12, 0.4, P("shed_roof_sheet", (0.30, 0.26, 0.22), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.4))
    gable_ends(walls, wx0, wx1, wy0, wy1, 6.5, rise, corr)
    parking(-LX / 2 + 2.0, -LX / 2 + 26.0, LY / 2 - 1.0, 5.0, 9, facing=-1)


def site_car_repair():
    lot(paving)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2)
    wx0, wx1, wy0, wy1, h = -LX / 2 + 2.0, -LX / 2 + 22.0, -LY / 2 + 1.0, -LY / 2 + 13.0, 6.0
    corr = P("shed_corrugated", (0.46, 0.47, 0.47), DETAIL, roughness=0.55, metallic=0.55, normal_strength=0.5)
    box(walls, wx0, wy0, 0.0, wx1, wy0 + 0.2, h, corr)
    box(walls, wx0, wy0, 0.0, wx0 + 0.2, wy1, h, corr)
    box(walls, wx1 - 0.2, wy0, 0.0, wx1, wy1, h, corr)
    for k in range(2):                                                                         # two bays, shutters down
        x0 = wx0 + 1.0 + k * 9.5
        box(fit, x0, wy1 - 0.15, 0.0, x0 + 7.5, wy1 - 0.08, 4.2, silver)
        z = 0.1
        while z < 4.2:
            box(fit, x0, wy1 - 0.08, z, x0 + 7.5, wy1 - 0.06, z + 0.02, steel)
            z += 0.1
        box(fit, x0 - 0.2, wy1 - 0.2, 4.2, x0 + 7.7, wy1 + 0.1, 4.6, steel)                    # the shutter box
    for (a, b) in ((wx0, wx0 + 1.0), (wx0 + 8.5, wx0 + 10.5), (wx0 + 18.0, wx1)):
        box(walls, a, wy1 - 0.2, 0.0, b, wy1, h, corr)
    box(walls, wx0, wy1 - 0.2, 4.6, wx1, wy1, h, corr)
    sign(wx0 + 1.0, wx1 - 1.0, wy1 + 0.02, 4.75, 5.85, "garage_board")
    rise = gable_roof(roofs, wx0, wx1, wy0, wy1, h, 10, 0.4, P("shed_roof_sheet", (0.30, 0.26, 0.22), DETAIL, roughness=0.5, metallic=0.5, normal_strength=0.4), along_x=False)
    gable_ends(walls, wx0, wx1, wy0, wy1, h, rise, corr, along_x=False)
    # the office beside it
    flat_building(wx1 + 1.0, wy0, LX / 2 - 2.0, wy0 + 8.0, 3.4, panel, front_glass=(wx1 + 2.0, LX / 2 - 3.0, 0.08, 2.4), parapet=0.4)


def site_post_office():
    lot(paving)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2)
    bx0, bx1, by0, by1, h = -7.5, 7.5, -LY / 2 + 0.8, -LY / 2 + 9.8, 4.2
    flat_building(bx0, by0, bx1, by1, h, tiles, front_glass=(-3.5, 3.5, 0.08, 2.6))
    box(fit, -5.2, by1, 2.85, 5.2, by1 + 0.2, 3.95, panel)
    sign(-5.1, 5.1, by1 + 0.21, 2.9, 3.9, "post_board")
    box(fit, -4.0, by1, 2.7, 4.0, by1 + 1.8, 2.85, panel)                                     # the canopy
    for x in (-3.8, 3.8):
        box(fit, x - 0.08, by1 + 1.6, 0.0, x + 0.08, by1 + 1.76, 2.7, silver)
    for k in range(4):                                                                         # the ramp and steps
        box(fit, -3.0, by1 + k * 0.3, 0.0, 3.0, by1 + 0.3 + k * 0.3, 0.45 - k * 0.1, concrete)
    parking(-LX / 2 + 1.0, -LX / 2 + 8.0, LY / 2 - 0.8, 4.4, 3, facing=-1)


def site_town_office():
    lot(paving)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2)
    bx0, bx1, by0, by1, h = -14.0, 14.0, -LY / 2 + 1.0, -LY / 2 + 13.0, 7.6
    beige = P("site_office_beige", (0.66, 0.62, 0.54), DETAIL, roughness=0.8, normal_strength=0.5)
    flat_building(bx0, by0, bx1, by1, h, beige, front_glass=(-3.0, 3.0, 0.08, 2.8), parapet=0.8)
    # bands of windows on both floors
    for fz in (0.9, 4.6):
        for k in range(8):
            x0 = bx0 + 1.2 + k * 3.35
            if fz < 2 and -3.4 < x0 + 1.2 and x0 < 3.4:
                continue
            box(fit, x0, by1 - 0.05, fz, x0 + 2.4, by1 + 0.02, fz + 1.6, dglass)
            box(fit, x0 - 0.05, by1 + 0.02, fz - 0.08, x0 + 2.45, by1 + 0.12, fz, silver)
            box(fit, x0 + 1.17, by1, fz, x0 + 1.23, by1 + 0.06, fz + 1.6, silver)
    box(fit, -4.5, by1, 3.0, 4.5, by1 + 2.6, 3.2, beige)                                       # the porch roof
    for x in (-4.2, 4.2):
        box(fit, x - 0.2, by1 + 2.2, 0.0, x + 0.2, by1 + 2.6, 3.0, beige)
    sign(-4.4, 4.4, by1 + 2.61, 3.25, 3.95, "office_board")
    box(fit, -4.5, by1 + 2.4, 3.2, 4.5, by1 + 2.6, 4.0, beige)
    for k, x in enumerate((-10.0, -8.5, -7.0)):                                               # flagpoles
        rcyl(fit, (x, LY / 2 - 2.0, 0.0), (x, LY / 2 - 2.0, 9.0), 0.05, silver, seg=10)
        box(fit, x - 0.3, LY / 2 - 2.3, 0.0, x + 0.3, LY / 2 - 1.7, 0.3, concrete)
    parking(1.0, LX / 2 - 1.5, LY / 2 - 0.8, 5.0, 7, facing=-1)


def site_station():
    # the platform: 0.92 m high, its edge with the yellow tactile strip to the tracks (-Y), the
    # shelter with its benches' wall, the name boards, a fence along the back (+Y, the plaza)
    pz = 0.92
    px0, px1 = -LX / 2 + 2.0, LX / 2 - 2.0
    py0, py1 = -LY / 2 + 0.2, LY / 2 - 0.2
    box(walls, px0, py0, 0.0, px1, py1, pz, concrete)
    box(fit, px0, py0 - 0.05, pz - 0.15, px1, py0 + 0.4, pz, concrete)                         # the coping
    box(fit, px0, py0 + 0.8, pz, px1, py0 + 1.1, pz + 0.01, yellow)                              # tactile strip
    box(fit, px0, py0 + 0.45, pz, px1, py0 + 0.5, pz + 0.005, line)
    for e, s in ((px0, -1), (px1, 1)):                                                         # ramps at the ends
        a, b = Vector((e, 0, pz - 0.01)), Vector((e + s * 6.0, 0, 0.0))
        mid, d = (a + b) / 2, b - a
        fit.oriented_box(Vector((mid.x, (py0 + py1) / 2, mid.z - 0.1)), (d.length, py1 - py0, 0.2), Matrix.Rotation(math.atan2(d.z, d.x), 3, "Y").inverted(), concrete)
    # the shelter: a steel frame, a back wall, a flat roof
    sx0, sx1 = -10.0, 10.0
    for x in (sx0 + 0.5, 0.0, sx1 - 0.5):
        box(fit, x - 0.1, py1 - 1.6, pz, x + 0.1, py1 - 1.4, pz + 3.0, steel)
    box(fit, sx0, py1 - 0.3, pz, sx1, py1 - 0.2, pz + 2.6, panel)
    box(fit, sx0 - 0.3, py0 + 1.4, pz + 3.0, sx1 + 0.3, py1, pz + 3.15, grey)
    for k in range(4):                                                                         # bench seats built to the wall
        x = sx0 + 2.0 + k * 4.5
        box(fit, x, py1 - 0.8, pz + 0.42, x + 2.4, py1 - 0.3, pz + 0.47, wood)
        box(fit, x + 0.1, py1 - 0.7, pz, x + 0.2, py1 - 0.4, pz + 0.42, steel)
        box(fit, x + 2.2, py1 - 0.7, pz, x + 2.3, py1 - 0.4, pz + 0.42, steel)
    # the name boards (both ends of the shelter, facing the tracks)
    for x in (-22.0, 22.0):
        for dx in (-1.3, 1.3):
            box(fit, x + dx - 0.05, py0 + 1.6, pz, x + dx + 0.05, py0 + 1.7, pz + 2.6, steel)
        box(fit, x - 1.4, py0 + 1.6, pz + 1.6, x + 1.4, py0 + 1.72, pz + 2.5, panel)
        sign(x - 1.35, x + 1.35, py0 + 1.59, pz + 1.62, pz + 2.48, "station_name", facing=-1)
    # the fence along the back
    for k in range(int((px1 - px0) / 2.0) + 1):
        x = px0 + k * 2.0
        box(fit, x - 0.04, py1 - 0.1, pz, x + 0.04, py1 - 0.02, pz + 1.2, steel)
    box(fit, px0, py1 - 0.1, pz + 1.1, px1, py1 - 0.02, pz + 1.16, steel)
    box(fit, px0, py1 - 0.1, pz + 0.5, px1, py1 - 0.02, pz + 0.54, steel)


def site_plaza():
    lot(paving)
    kerb(-LX / 2, -LY / 2, LX / 2, LY / 2, h=0.12)
    # the station house at the back of the plaza (by the platform), its name over the door
    bx0, bx1, by0, by1, h = -9.0, 9.0, -LY / 2 + 0.5, -LY / 2 + 8.5, 4.0
    flat_building(bx0, by0, bx1, by1, h, plaster, front_glass=(-3.0, 3.0, 0.08, 2.6), parapet=0.0, back_door=False)
    rise = gable_roof(roofs, bx0, bx1, by0, by1, h, 22, 0.8, roof_tile)
    gable_ends(walls, bx0, bx1, by0, by1, h, rise, plaster)
    box(fit, -5.5, by1, 2.9, 5.5, by1 + 0.15, 3.8, wood)
    sign(-5.4, 5.4, by1 + 0.16, 2.95, 3.75, "station_board")
    box(fit, -4.0, by1, 2.7, 4.0, by1 + 2.2, 2.85, wood)
    for x in (-3.8, 3.8):
        box(fit, x - 0.1, by1 + 2.0, 0.0, x + 0.1, by1 + 2.2, 2.7, wood)
    # the bus and taxi stops: a kerbed island, painted bays, a shelter
    box(fit, 6.0, -1.0, 0.0, 22.0, 2.0, 0.18, concrete)
    for x in (8.0, 14.0, 20.0):
        box(fit, x - 0.06, 2.5, 0.06, x + 0.06, 7.5, 0.065, line)
    for x in (12.0, 16.0):
        box(fit, x - 0.06, -0.8, 0.18, x + 0.06, -0.6, 2.5, steel)
    box(fit, 11.5, -1.0, 2.5, 16.5, 0.6, 2.6, panel)
    # planters with clipped shrubs' beds, a clock pole
    for x in (-20.0, -14.0):
        box(fit, x - 1.5, 2.0, 0.0, x + 1.5, 4.0, 0.5, stone)
    rcyl(fit, (-17.0, -2.0, 0.0), (-17.0, -2.0, 3.6), 0.07, steel)
    rcyl(fit, (-17.0, -2.2, 3.9), (-17.0, -1.8, 3.9), 0.35, panel, seg=24)


def site_shrine():
    # (no slab over the grounds: the grove's moss; a gravel forecourt round the halls)
    box(ground, -9.0, -LY / 2 + 4.0, -0.05, 9.0, -LY / 2 + 38.0, 0.05, gravel)
    # the approach: a torii at the road, a paved path, stone lanterns, the hall and the sanctuary
    torii(0.0, LY / 2 - 4.0, 4.0, 4.8, P("site_torii_stone", (0.50, 0.48, 0.45), DETAIL, roughness=0.6, normal_strength=0.9, grain=0.4), plaque="hachiman_plaque")
    box(ground, -1.5, -LY / 2 + 22.0, 0.05, 1.5, LY / 2, 0.09, stone)
    for y in (LY / 2 - 14.0, -LY / 2 + 30.0):
        for x in (-3.0, 3.0):
            lantern(x, y)
    torii(0.0, -LY / 2 + 34.0, 3.6, 4.2, vermilion, plaque="hachiman_plaque")
    # the purification pavilion (temizuya)
    tx, ty = 7.0, -LY / 2 + 34.0
    box(fit, tx - 1.0, ty - 0.6, 0.0, tx + 1.0, ty + 0.6, 0.7, granite)
    box(fit, tx - 0.9, ty - 0.5, 0.6, tx + 0.9, ty + 0.5, 0.68, P("site_water_dark", (0.05, 0.07, 0.08), DETAIL, roughness=0.05, normal_strength=0.0))
    for x in (tx - 1.4, tx + 1.4):
        for y in (ty - 1.0, ty + 1.0):
            rcyl(fit, (x, y, 0.0), (x, y, 2.6), 0.08, wood)
    gable_roof(roofs, tx - 1.4, tx + 1.4, ty - 1.0, ty + 1.0, 2.6, 30, 0.5, roof_tile)
    # the worship hall (haiden) and behind it the sanctuary (honden) on a stone platform
    shrine_hall(0.0, -LY / 2 + 22.0, 9.0, 6.0, 4.0)
    box(walls, -4.0, -LY / 2 + 6.0, 0.0, 4.0, -LY / 2 + 13.0, 1.0, stone)
    shrine_hall(0.0, -LY / 2 + 9.5, 4.0, 3.5, 3.4, roof_pitch=34, eov=0.9)
    for k in range(13):
        x = -6.5 + k * 1.083
        box(fit, x - 0.05, -LY / 2 + 4.5, 0.0, x + 0.05, -LY / 2 + 4.6, 1.3, vermilion)
    box(fit, -6.5, -LY / 2 + 4.5, 1.2, 6.5, -LY / 2 + 4.6, 1.3, vermilion)


def gate_row_torii(y, w, h, post, mat):
    """One torii of a senbon-torii tunnel: slim posts on black bases, the kasagi with a short
    overhang, the nuki, close-set (the next one a post's width behind)."""
    for sx in (-1, 1):
        rcyl(fit, (sx * w / 2, y, 0.0), (sx * w / 2, y, h), post, mat, seg=12)
        box(fit, sx * w / 2 - post * 1.4, y - post * 1.4, 0.0, sx * w / 2 + post * 1.4, y + post * 1.4, 0.28, black)
    ext = w / 2 + 0.28
    box(fit, -ext, y - post * 1.1, h, ext, y + post * 1.1, h + 0.14, mat)                      # shimaki
    box(fit, -ext - 0.04, y - post * 1.3, h + 0.14, ext + 0.04, y + post * 1.3, h + 0.24, black)   # kasagi
    box(fit, -w / 2 - 0.12, y - post * 0.8, h - 0.55, w / 2 + 0.12, y + post * 0.8, h - 0.42, mat)  # nuki


def site_inari():
    """A small Inari shrine as they stand at a village's edge: a grove of old trees (left
    standing on its ground: plants.rs), a gravel path from the road through a tunnel of
    close-set vermilion torii (senbon torii) offered by the parishioners, a pair of stone foxes
    and lanterns, the little shrine on its stone base at the back. No paved lot: the ground is the
    grove's moss (the land use)."""
    y_road = LY / 2
    y_shrine = -LY / 2 + 5.0
    # the path: gravel between stone edging, from the road to the shrine's steps
    box(ground, -0.95, y_shrine + 2.2, -0.05, 0.95, y_road, 0.05, gravel)
    for sx in (-1, 1):
        box(fit, sx * 0.95 - 0.06, y_shrine + 2.2, -0.05, sx * 0.95 + 0.06, y_road, 0.1, granite)
    # the big torii at the road, with the plaque
    torii(0.0, y_road - 1.2, 2.6, 3.4, vermilion, plaque="inari_plaque")
    # the tunnel: torii every 0.42 m (a post's width apart), a little lower towards the shrine
    y = y_road - 3.2
    k = 0
    while y > y_shrine + 4.2:
        gate_row_torii(y, 1.55, 2.45 - 0.004 * k, 0.07, vermilion)
        y -= 0.42
        k += 1
    # the foxes (kitsune) on their plinths either side, the lanterns
    for sx in (-1, 1):
        x = sx * 1.6
        yy = y_shrine + 3.4
        box(fit, x - 0.3, yy - 0.3, 0.0, x + 0.3, yy + 0.3, 0.9, granite)
        box(fit, x - 0.12, yy - 0.18, 0.9, x + 0.12, yy + 0.18, 1.2, granite)                  # body
        box(fit, x - 0.08, yy + 0.04, 1.2, x + 0.08, yy + 0.18, 1.42, granite)                 # chest, head
        box(fit, x - 0.06, yy + 0.12, 1.42, x - 0.02, yy + 0.16, 1.5, granite)                 # ears
        box(fit, x + 0.02, yy + 0.12, 1.42, x + 0.06, yy + 0.16, 1.5, granite)
        box(fit, x - 0.1, yy + 0.16, 1.18, x + 0.1, yy + 0.19, 1.3, vermilion)                 # the bib
        lantern(sx * 2.4, y_shrine + 5.2, 1.7)
    # the shrine on its stone base, a few steps up, the offering box in front
    box(walls, -2.2, y_shrine - 2.2, 0.0, 2.2, y_shrine + 2.2, 0.7, stone)
    for k in range(3):
        box(fit, -0.9, y_shrine + 2.2 + (2 - k) * 0.28, 0.0, 0.9, y_shrine + 2.2 + (3 - k) * 0.28, (k + 1) * 0.23, granite)
    shrine_hall(0.0, y_shrine, 2.2, 1.8, 2.5, roof_pitch=34, eov=0.6)
    box(fit, -0.45, y_shrine + 1.45, 0.78, 0.45, y_shrine + 1.85, 1.3, wood)                    # saisen-bako


def site_temple():
    # (no slab over the grounds: gravel forecourt between the gate and the halls)
    box(ground, -18.0, -LY / 2 + 2.0, -0.05, 18.0, LY / 2 - 4.5, 0.05, gravel)
    # the gate (sanmon) at the road, its plaque; a paved path to the main hall
    gy = LY / 2 - 3.0
    for x in (-2.4, -0.9, 0.9, 2.4):
        for y in (gy - 1.2, gy + 1.2):
            rcyl(fit, (x, y, 0.0), (x, y, 3.6), 0.18, wood, seg=14)
    for y in (gy - 1.2, gy + 1.2):
        box(fit, -3.0, y - 0.15, 3.4, 3.0, y + 0.15, 3.7, wood)
    box(fit, -1.0, gy + 1.36, 2.7, 1.0, gy + 1.42, 3.3, black)
    sign(-0.95, 0.95, gy + 1.43, 2.75, 3.25, "temple_plaque")
    irimoya(0.0, gy, 6.0, 3.0, 3.8, 32, 0.9, roof_tile, wood)
    for sx in (-1, 1):                                                                         # the earthen walls either side
        box(walls, sx * 3.2, gy - 0.4, 0.0, sx * (LX / 2 - 1.0), gy + 0.4, 2.2, plaster)
        box(roofs, sx * 3.0, gy - 0.6, 2.2, sx * (LX / 2 - 0.8), gy + 0.6, 2.45, roof_tile)
    box(ground, -1.5, -LY / 2 + 25.0, 0.05, 1.5, gy - 1.5, 0.09, stone)
    # the main hall (hondō)
    shrine_hall(-8.0, -LY / 2 + 16.0, 16.0, 13.0, 5.4, roof_pitch=32, eov=1.8)
    # the bell tower (shōrō)
    bx, by = 14.0, -LY / 2 + 28.0
    box(walls, bx - 2.5, by - 2.5, 0.0, bx + 2.5, by + 2.5, 0.8, stone)
    for x in (bx - 1.8, bx + 1.8):
        for y in (by - 1.8, by + 1.8):
            rcyl(fit, (x, y, 0.8), (x, y, 4.6), 0.15, wood)
    rcyl(fit, (bx, by, 2.4), (bx, by, 4.2), 0.55, P("site_bronze", (0.18, 0.22, 0.16), DETAIL, roughness=0.5, metallic=0.7, normal_strength=0.3), seg=24)
    irimoya(bx, by, 3.6, 3.6, 4.6, 34, 0.8, roof_tile, wood)
    # the cemetery: rows of family graves (a base, the tall stone, the flower holders)
    cx0, cx1, cy0, cy1 = 14.0, LX / 2 - 3.0, -LY / 2 + 3.0, -LY / 2 + 22.0
    box(ground, cx0 - 1.0, cy0 - 1.0, 0.05, cx1 + 1.0, cy1 + 1.0, 0.08, stone)
    import random
    rng = random.Random(7)
    y = cy0
    while y < cy1:
        x = cx0
        while x < cx1:
            h = rng.uniform(0.9, 1.35)
            tint = granite if rng.random() < 0.7 else P("site_granite_dark", (0.20, 0.20, 0.21), DETAIL, roughness=0.35, normal_strength=0.6)
            box(fit, x, y, 0.0, x + 1.6, y + 1.4, 0.25, granite)
            box(fit, x + 0.4, y + 0.35, 0.25, x + 1.2, y + 1.0, 0.45, tint)
            box(fit, x + 0.55, y + 0.5, 0.45, x + 1.05, y + 0.85, 0.45 + h, tint)
            for dx in (0.28, 1.32):
                box(fit, x + dx - 0.05, y + 0.95, 0.25, x + dx + 0.05, y + 1.05, 0.6, granite)
            x += 2.0
        y += 2.4


{"konbini": site_konbini, "petrol": site_petrol, "drugstore": site_drugstore, "coop": site_coop,
 "car_repair": site_car_repair, "post_office": site_post_office, "town_office": site_town_office,
 "station": site_station, "plaza": site_plaza, "shrine": site_shrine, "inari": site_inari,
 "temple": site_temple}[KIND]()

# the signs: quads mapped onto the atlas
if signs:
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new()
    for corners, key in signs:
        x0, y0, x1, y1 = ATLAS[key]
        u0, u1, v0, v1 = x0 / 8192, x1 / 8192, 1 - y1 / 8192, 1 - y0 / 8192
        f = bm.faces.new([bm.verts.new(c) for c in corners])
        for l, uv in zip(f.loops, [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]):
            l[uvl].uv = uv
    bm.normal_update()
    me = bpy.data.meshes.new("signs")
    bm.to_mesh(me)
    bm.free()
    me.materials.append(sign_mat)
    ob = bpy.data.objects.new("signs", me)
    kz.coll("HOUSE").objects.link(ob)

for b in (walls, fit, roofs, ground):
    if b.bm.faces:
        b.finish(collection="HOUSE")
os.makedirs(os.path.dirname(OUT), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLTF_SEPARATE", export_texture_dir="textures", export_yup=True)
print("wrote", OUT)
