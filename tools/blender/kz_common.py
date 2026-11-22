"""Shared helpers for building Kazehana levels in Blender (run headless with --background).

Conventions
- Blender axes: +X east, +Y north, +Z up. The glTF exporter turns this into the game's
  +X east, +Y up, -Z north.
- Level geometry is ordinary mesh objects in the "LEVEL" collection; it is exported into
  level.glb.
- Quality-gated assets (docs/ASSET_QUALITY.md) are imported once into a hidden collection named
  "ASSET:<registry id>" and placed with collection-instance empties. The exporter writes these
  as references, not geometry.
- Markers are empties with a custom property "kz" (spawn, exit, collider, zone, npc_spot,
  light) plus kind-specific properties. Box-shaped markers use cube empties: the box is the
  empty's display cube (size x scale).
"""

import glob
import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

FAB = "D:/EpicVault/VaultCache/FabLibrary"


# ----------------------------------------------------------------------------- scene setup

def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for name in ("LEVEL", "MARKERS", "ASSETS"):
        c = bpy.data.collections.new(name)
        bpy.context.scene.collection.children.link(c)
    # asset sources are kept out of the view layer (instanced, never rendered directly)
    bpy.context.view_layer.layer_collection.children["ASSETS"].exclude = True


def coll(name):
    return bpy.data.collections[name]


# ----------------------------------------------------------------------------- materials

def fab_dir(prefix):
    """The Fab download folder whose name starts with `prefix` (e.g. 'Asphalt_Road')."""
    hits = sorted(glob.glob(os.path.join(FAB, prefix + "*")))
    if not hits:
        raise FileNotFoundError(f"Fab asset '{prefix}' is not downloaded")
    return hits[0]


def fab_textures(prefix):
    """(base colour, normal, ORM) PNG paths of a Fab surface or model."""
    d = fab_dir(prefix)
    def one(pattern):
        found = [f for f in glob.glob(os.path.join(d, "**", pattern), recursive=True) if "Billboard" not in f]
        return found[0] if found else None
    b = one("T_*_B.png") or one("T_*_B-O.png")
    n = one("T_*_N.png")
    orm = one("T_*_ORM.png")
    return b, n, orm


def fab_gltf(prefix):
    d = fab_dir(prefix)
    files = [f for f in glob.glob(os.path.join(d, "**", "*.gltf"), recursive=True) if "nonUE" not in f]
    return files[0]


def _image(path, colorspace):
    img = bpy.data.images.load(path, check_existing=True)
    img.colorspace_settings.name = colorspace
    return img


def tinted_texture(name, src_path, tint, desaturate=0.0):
    """A copy of a colour texture, partly desaturated and multiplied by `tint` (linear), saved as
    PNG (glTF has no colour multiply node: the tint must be in the texture)."""
    import numpy as np
    os.makedirs(PAINT_CACHE, exist_ok=True)
    key = "%s_%s_%.2f_%s" % (name, "_".join("%.3f" % c for c in tint), desaturate, os.path.basename(src_path))
    path = os.path.join(PAINT_CACHE, "%s_%08x.png" % (name, abs(hash(key)) & 0xFFFFFFFF))
    if os.path.exists(path):
        return path
    src = bpy.data.images.load(src_path)
    w, h = src.size
    px = np.empty(w * h * 4, dtype=np.float32)
    src.pixels.foreach_get(px)
    px = px.reshape(h, w, 4)
    step = max(1, w // 2048)
    px = px[::step, ::step]
    lin = px[..., :3]  # Blender holds sRGB images as linear floats
    lum = (lin * np.array([0.2126, 0.7152, 0.0722], np.float32)).sum(-1, keepdims=True)
    lin = (lin * (1 - desaturate) + lum * desaturate) * np.array(tint, np.float32)
    srgb = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(np.clip(lin, 0, None), 1 / 2.4) - 0.055)
    out = np.concatenate([np.clip(srgb, 0, 1), px[..., 3:4]], axis=-1).astype(np.float32)
    hh, ww = out.shape[:2]
    img = bpy.data.images.new(os.path.basename(path)[:-4], ww, hh, alpha=True)
    img.pixels.foreach_set(out.ravel())
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)
    bpy.data.images.remove(src)
    return path


def surface_material(name, prefix, tint=None, roughness_scale=1.0, metallic=0.0, desaturate=0.0):
    """A PBR material from a Megascans surface (B / N / ORM), as the glTF exporter expects it.
    `tint` (and `desaturate`) are baked into a copy of the colour texture."""
    if name in bpy.data.materials:
        return bpy.data.materials[name]
    b, n, orm = fab_textures(prefix)
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = _image(b if tint is None and desaturate == 0.0 else tinted_texture(name, b, tint or (1.0, 1.0, 1.0), desaturate), "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    if n:
        tn = nt.nodes.new("ShaderNodeTexImage")
        tn.image = _image(n, "Non-Color")
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    if orm:
        to = nt.nodes.new("ShaderNodeTexImage")
        to.image = _image(orm, "Non-Color")
        sep = nt.nodes.new("ShaderNodeSeparateColor")
        nt.links.new(to.outputs["Color"], sep.inputs["Color"])
        nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
        if metallic > 0.0:
            nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
        else:
            bsdf.inputs["Metallic"].default_value = 0.0
    else:
        bsdf.inputs["Roughness"].default_value = 0.8 * roughness_scale
    return m


PAINT_CACHE = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "assets", "cache", "paint"))


def paint_texture(name, color, detail_b, grain=0.35, size=1024):
    """A base-colour texture for a painted surface: the paint colour (linear) modulated by the
    detail scan's brightness variation (strength `grain`), so a painted wall has the scan's real
    unevenness instead of a flat colour. Cached as PNG."""
    import numpy as np
    os.makedirs(PAINT_CACHE, exist_ok=True)
    key = "%s_%s_%s" % (name, "_".join("%.3f" % c for c in color), os.path.basename(detail_b))
    path = os.path.join(PAINT_CACHE, "%s_%08x.png" % (name, abs(hash(key)) & 0xFFFFFFFF))
    if os.path.exists(path):
        return path
    src = bpy.data.images.load(detail_b)
    w, h = src.size
    px = np.empty(w * h * 4, dtype=np.float32)
    src.pixels.foreach_get(px)
    px = px.reshape(h, w, 4)
    step = max(1, w // size)
    px = px[::step, ::step]
    lum = px[..., 0] * 0.2126 + px[..., 1] * 0.7152 + px[..., 2] * 0.0722
    t = np.clip(1.0 + (lum / max(float(lum.mean()), 1e-4) - 1.0) * grain, 0.5, 1.6)
    lin = np.stack([t * c for c in color], axis=-1)
    srgb = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(np.clip(lin, 0, None), 1 / 2.4) - 0.055)
    out = np.concatenate([np.clip(srgb, 0, 1), np.ones_like(srgb[..., :1])], axis=-1).astype(np.float32)
    hh, ww = out.shape[:2]
    img = bpy.data.images.new(os.path.basename(path)[:-4], ww, hh, alpha=True)
    img.pixels.foreach_set(out.ravel())
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)
    bpy.data.images.remove(src)
    return path


def painted_material(name, color, detail_prefix, roughness=0.6, metallic=0.0, normal_strength=0.5, grain=0.35):
    """A painted or metal surface: the paint colour as a texture carrying the grain of a
    Megascans surface (paint_texture), with its normal map and roughness variation, so paint and
    metal never look like plastic."""
    if name in bpy.data.materials:
        return bpy.data.materials[name]
    b, n, orm = fab_textures(detail_prefix)
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    if b:
        tb = nt.nodes.new("ShaderNodeTexImage")
        tb.image = _image(paint_texture(name, color, b, grain), "sRGB")
        nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    else:
        bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    bsdf.inputs["Metallic"].default_value = metallic
    if n:
        tn = nt.nodes.new("ShaderNodeTexImage")
        tn.image = _image(n, "Non-Color")
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nm.inputs["Strength"].default_value = normal_strength
        nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    if orm:
        to = nt.nodes.new("ShaderNodeTexImage")
        to.image = _image(orm, "Non-Color")
        sep = nt.nodes.new("ShaderNodeSeparateColor")
        nt.links.new(to.outputs["Color"], sep.inputs["Color"])
        # roughness = texture variation scaled towards the paint's own roughness
        mul = nt.nodes.new("ShaderNodeMath")
        mul.operation = "MULTIPLY"
        mul.inputs[1].default_value = roughness / 0.75
        nt.links.new(sep.outputs["Green"], mul.inputs[0])
        nt.links.new(mul.outputs["Value"], bsdf.inputs["Roughness"])
    else:
        bsdf.inputs["Roughness"].default_value = roughness
    return m


def plain_material(name, color, roughness=0.6, metallic=0.0, emission=None, emission_strength=0.0):
    """A factor-only material (paint, glass, emissive signs)."""
    if name in bpy.data.materials:
        return bpy.data.materials[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    if emission is not None:
        bsdf.inputs["Emission Color"].default_value = (*emission, 1.0)
        bsdf.inputs["Emission Strength"].default_value = emission_strength
    return m


# ----------------------------------------------------------------------------- geometry

def world_uv(bm, scale):
    """Box-projected UVs from world positions (texture repeats per metre = scale), so tiling
    Megascans surfaces line up across pieces and texel density is consistent."""
    uv = bm.loops.layers.uv.verify()
    for f in bm.faces:
        n = f.normal
        ax = max(range(3), key=lambda i: abs(n[i]))
        for l in f.loops:
            p = l.vert.co
            if ax == 2:
                u, v = p.x, p.y
            elif ax == 0:
                u, v = p.y * (1 if n.x > 0 else -1), p.z
            else:
                u, v = p.x * (-1 if n.y > 0 else 1), p.z
            l[uv].uv = (u * scale, v * scale)


class MeshBuilder:
    """Accumulates boxes / prisms in world space into one mesh object (one material slot per
    material), with world-projected UVs."""

    def __init__(self, name, uv_scale=0.5):
        self.name = name
        self.bm = bmesh.new()
        self.mats = []
        self.uv_scale = uv_scale

    def _mat_index(self, mat):
        if mat not in self.mats:
            self.mats.append(mat)
        return self.mats.index(mat)

    def box(self, mn, mx, mat, rot_z=0.0, pivot=None):
        """Axis-aligned box from min to max corners (optionally turned about Z at pivot)."""
        mn, mx = Vector(mn), Vector(mx)
        size = mx - mn
        if min(size) <= 0:
            return
        res = bmesh.ops.create_cube(self.bm, size=1.0)
        verts = res["verts"]
        m = Matrix.Translation((mn + mx) / 2) @ Matrix.Diagonal((*size, 1.0)).to_4x4()
        if rot_z:
            pv = Vector(pivot) if pivot is not None else (mn + mx) / 2
            m = Matrix.Translation(pv) @ Matrix.Rotation(rot_z, 4, "Z") @ Matrix.Translation(-pv) @ m
        bmesh.ops.transform(self.bm, matrix=m, verts=verts)
        idx = self._mat_index(mat)
        for f in {f for v in verts for f in v.link_faces}:
            f.material_index = idx

    def oriented_box(self, center, size, matrix_rot, mat):
        res = bmesh.ops.create_cube(self.bm, size=1.0)
        verts = res["verts"]
        m = Matrix.Translation(center) @ matrix_rot.to_4x4() @ Matrix.Diagonal((*size, 1.0)).to_4x4()
        bmesh.ops.transform(self.bm, matrix=m, verts=verts)
        idx = self._mat_index(mat)
        for f in {f for v in verts for f in v.link_faces}:
            f.material_index = idx

    def cylinder(self, base, radius, height, mat, segments=16):
        res = bmesh.ops.create_cone(self.bm, cap_ends=True, segments=segments, radius1=radius, radius2=radius, depth=height)
        bmesh.ops.translate(self.bm, vec=Vector(base) + Vector((0, 0, height / 2)), verts=res["verts"])
        idx = self._mat_index(mat)
        for f in {f for v in res["verts"] for f in v.link_faces}:
            f.material_index = idx

    def quad(self, corners, mat):
        vs = [self.bm.verts.new(c) for c in corners]
        f = self.bm.faces.new(vs)
        f.material_index = self._mat_index(mat)

    def finish(self, collection="LEVEL"):
        self.bm.normal_update()
        world_uv(self.bm, self.uv_scale)
        me = bpy.data.meshes.new(self.name)
        self.bm.to_mesh(me)
        self.bm.free()
        for m in self.mats:
            me.materials.append(m)
        ob = bpy.data.objects.new(self.name, me)
        coll(collection).objects.link(ob)
        return ob


# ----------------------------------------------------------------------------- assets

def asset(asset_id, fab_prefix):
    """Import a quality-gated Fab model once into the hidden collection ASSET:<id> (LOD0, or the
    most detailed LOD the download has; other LODs are removed)."""
    cname = "ASSET:" + asset_id
    if cname in bpy.data.collections:
        return bpy.data.collections[cname]
    c = bpy.data.collections.new(cname)
    coll("ASSETS").children.link(c)
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=fab_gltf(fab_prefix))
    new = [o for o in bpy.data.objects if o not in before]
    # keep the best LOD per base name (same rule as crates/assets lod_of)
    def lod(o):
        n = o.name
        i = n.find("_LOD")
        if i < 0:
            return (n, 0)
        digits = "".join(ch for ch in n[i + 4:] if ch.isdigit())[:1]
        return (n[:i], int(digits or 0))
    best = {}
    for o in new:
        if o.type == "MESH":
            b, l = lod(o)
            best[b] = min(best.get(b, 99), l)
    for o in new:
        for uc in list(o.users_collection):
            uc.objects.unlink(o)
        b, l = lod(o)
        if o.type == "MESH" and l > best.get(b, 0):
            bpy.data.objects.remove(o)
            continue
        c.objects.link(o)
    c["kz_asset"] = asset_id
    return c


def place(asset_coll, location, rot_z_deg=0.0, scale=1.0, name=None):
    """A collection-instance empty placing an asset."""
    e = bpy.data.objects.new(name or asset_coll.name.replace("ASSET:", "") + ".inst", None)
    e.instance_type = "COLLECTION"
    e.instance_collection = asset_coll
    e.location = location
    e.rotation_euler = (0, 0, math.radians(rot_z_deg))
    e.scale = (scale, scale, scale)
    coll("LEVEL").objects.link(e)
    return e


# ----------------------------------------------------------------------------- markers

def marker(kind, location, size=(0.5, 0.5, 0.5), name=None, **props):
    """An empty marker. Box markers: `size` are half-extents (the empty's cube, scaled)."""
    e = bpy.data.objects.new(name or f"{kind}", None)
    e.empty_display_type = "CUBE"
    e.empty_display_size = 1.0
    e.location = location
    e.scale = size
    e["kz"] = kind
    for k, v in props.items():
        e[k] = v
    coll("MARKERS").objects.link(e)
    return e


def box_marker(kind, mn, mx, name=None, **props):
    mn, mx = Vector(mn), Vector(mx)
    return marker(kind, (mn + mx) / 2, (mx - mn) / 2, name=name, **props)


def light(location, color, intensity, radius=0.1, range_m=20.0, night_only=True, spot_dir=None, cone_deg=None, name=None):
    props = dict(color=list(color), intensity=intensity, radius=radius, range=range_m, night_only=night_only)
    if spot_dir is not None:
        props["spot_dir"] = list(spot_dir)
        props["cone_deg"] = cone_deg or 60.0
    return marker("light", location, (0.2, 0.2, 0.2), name=name or "light", **props)
