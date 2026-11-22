"""Tsukumodō's noren, the split cotton curtain in the doorway from the shop to the back: its
fabric as a cloth-ready mesh, the `kzcloth_noren` node of the shop (tools/blender/
build_shop_tsukumodo.py hangs it on the doorway's rod). The game simulates it
(crates/play/src/cloth.rs, crates/town/src/cloth.rs); the shop's collision skips it.

    python tools/make_noren_cloth.py      (its cotton textures, before building the shop)

The fabric (sizes and texture layout: tools/blender/noren_layout.py): two panels of heavy cotton,
each a regular grid of about 2.5 cm (fine enough for real folds, few enough particles for the
cloth solver), one sheet (double-sided: the print is dyed through, so the back shows it
mirrored). Each panel runs from its bottom hem up the front, round the rod in its sleeve, and
down the back to the sleeve's seam; the top `PIN_BAND` of it (the sleeve) is what the simulation
pins to the rod. Its rest shape hangs straight and flat: the simulation gives it its weight and
its folds.
"""

import math
import os
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kz_common as kz  # noqa: E402
import noren_layout as L  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
TEX = os.path.join(ROOT, "assets", "cache", "cloth")


def cotton_material():
    """Heavy cotton: the dyed print (base), the weave and hems (normal), occlusion and roughness
    (ORM), no metal. Double-sided."""
    m = bpy.data.materials.new("shop_noren_cotton")
    m.use_nodes = True
    m.use_backface_culling = False
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = kz._image(os.path.join(TEX, "noren_base.png"), "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = kz._image(os.path.join(TEX, "noren_normal.png"), "Non-Color")
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nm.inputs["Strength"].default_value = 1.0
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    to = nt.nodes.new("ShaderNodeTexImage")
    to.image = kz._image(os.path.join(TEX, "noren_orm.png"), "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(to.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
    # occlusion: the glTF exporter reads it from a "glTF Material Output" group's input
    grp = bpy.data.node_groups.get("glTF Material Output") or bpy.data.node_groups.new("glTF Material Output", "ShaderNodeTree")
    if "Occlusion" not in [i.name for i in grp.interface.items_tree]:
        grp.interface.new_socket("Occlusion", in_out="INPUT", socket_type="NodeSocketFloat")
    gn = nt.nodes.new("ShaderNodeGroup")
    gn.node_tree = grp
    nt.links.new(sep.outputs["Red"], gn.inputs["Occlusion"])
    bsdf.inputs["Sheen Weight"].default_value = 0.0
    return m


def fabric_point(s):
    """(y, z) of the fabric at path length s from the bottom hem: up the front, round the rod,
    down the back."""
    R = L.SLEEVE_R
    if s <= L.HANG:
        return R, s
    if s <= L.HANG + math.pi * R:
        a = (s - L.HANG) / R
        return R * math.cos(a), L.HANG + R * math.sin(a)
    return -R, L.HANG - (s - L.HANG - math.pi * R)


def fabric_rows():
    """Path lengths of the grid's rows: ~2.5 cm up the front, finer round the rod."""
    rows = [L.HANG * i / 33 for i in range(34)]
    rows += [L.HANG + math.pi * L.SLEEVE_R * i / 6 for i in range(1, 7)]
    rows += [L.HANG + math.pi * L.SLEEVE_R + L.BACK * i / 2 for i in range(1, 3)]
    return rows


def fold(k, x, s):
    """The fabric's soft vertical folds (its rest shape: woven cotton keeps them, and gravity
    does not pull vertical folds flat): none at the sleeve, growing to FOLD_DEPTH over the top
    30 cm, two waves of different lengths so they are not regular, a different phase per panel.
    x: metres across the panel, s: path length from the hem."""
    if s > L.HANG:
        return 0.0
    a = (L.HANG - L.SEAM - s) / 0.30
    a = max(0.0, min(1.0, a))
    a = a * a * (3 - 2 * a)
    ph = 1.7 * k + 0.6
    w = 0.75 * math.sin(2 * math.pi * x / L.FOLD_WAVE + ph) + 0.35 * math.sin(2 * math.pi * x / (L.FOLD_WAVE * 2.3) + 2.1 * ph)
    return L.FOLD_DEPTH * a * w


def build_cloth(coll, at, mat=None):
    """The noren's fabric, its rod's centre at `at` (Blender, the shop's frame), facing +Y: the
    `kzcloth_noren` object in `coll`."""
    mat = mat or cotton_material()
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new()
    rows = fabric_rows()
    cols = 16
    for k in range(2):
        # panel 0 on +X (the viewer's left, seen from the front, +Y): the print's left half
        x_left = L.SLIT / 2 + L.PANEL_W if k == 0 else -L.SLIT / 2
        grid = []
        for s in rows:
            y, z = fabric_point(s)
            line = []
            for i in range(cols + 1):
                t = i / cols
                yy = y + fold(k, t * L.PANEL_W, s)
                line.append((bm.verts.new((at[0] + x_left - t * L.PANEL_W, at[1] + yy, at[2] - L.HANG + z)), (L.panel_u(k, t), L.fabric_v(s))))
            grid.append(line)
        for j in range(len(rows) - 1):
            for i in range(cols):
                a, b, c, d = grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]
                # wound so the front face looks towards +Y
                f = bm.faces.new([a[0], b[0], c[0], d[0]])
                for loop, uv in zip(f.loops, [a[1], b[1], c[1], d[1]]):
                    loop[uvl].uv = uv
                f.smooth = True
    bm.normal_update()
    me = bpy.data.meshes.new("kzcloth_noren")
    bm.to_mesh(me)
    bm.free()
    me.materials.append(mat)
    ob = bpy.data.objects.new("kzcloth_noren", me)
    coll.objects.link(ob)
    # the front faces must look at the shop (+Y)
    n = sum((p.normal.y for p in me.polygons if p.center.z < at[2] - 0.05), 0.0)
    assert n > 0, "the noren's front faces the wrong way"
    return ob
