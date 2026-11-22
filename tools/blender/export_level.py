"""Export a Kazehana level from the open .blend (headless):

    blender.exe assets/levels/path/path.blend --background --python tools/blender/export_level.py -- assets/levels/path

Writes
- level.glb: every mesh object in the LEVEL collection (the level's own geometry), Y-up.
- level.json: quality-gated asset placements (collection instances of ASSET:<id>) and markers,
  in game coordinates (+X east, +Y up, -Z north).
"""

import json
import os
import sys

import bpy
from mathutils import Matrix, Vector

# Blender (X east, Y north, Z up) -> game (X east, Y up, -Z north)
C = Matrix(((1, 0, 0, 0), (0, 0, 1, 0), (0, -1, 0, 0), (0, 0, 0, 1)))


def to_game(v):
    return [v.x, v.z, -v.y]


def game_matrix(mw):
    m = C @ mw @ C.inverted()
    # column-major 4x4 (glam Mat4::from_cols_array)
    return [m[r][c] for c in range(4) for r in range(4)]


def box(o):
    mn = o.location - o.scale
    mx = o.location + o.scale
    a, b = to_game(mn), to_game(mx)
    return {"min": [min(a[i], b[i]) for i in range(3)], "max": [max(a[i], b[i]) for i in range(3)]}


def main(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    level = bpy.data.collections["LEVEL"]
    markers = bpy.data.collections["MARKERS"]

    # 1. geometry
    bpy.ops.object.select_all(action="DESELECT")
    meshes = [o for o in level.all_objects if o.type == "MESH"]
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0] if meshes else None
    bpy.ops.export_scene.gltf(
        filepath=os.path.join(out_dir, "level.glb"),
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_tangents=True,
        export_image_format="AUTO",
        export_materials="EXPORT",
    )

    # 2. asset placements
    assets = []
    for o in level.all_objects:
        if o.instance_type == "COLLECTION" and o.instance_collection is not None:
            aid = o.instance_collection.get("kz_asset")
            if aid:
                assets.append({"id": aid, "transform": game_matrix(o.matrix_world)})

    # 3. markers
    out = {"spawns": [], "exits": [], "colliders": [], "zones": [], "npc_spots": [], "lights": []}
    for o in markers.all_objects:
        k = o.get("kz")
        if k == "spawn":
            out["spawns"].append({"name": o.get("spawn", o.name), "pos": to_game(o.location), "yaw_deg": float(o.get("yaw_deg", 0.0))})
        elif k == "exit":
            e = box(o)
            e.update({"to": o["to"], "spawn": o.get("spawn", "default")})
            out["exits"].append(e)
        elif k == "collider":
            out["colliders"].append(box(o))
        elif k == "zone":
            z = box(o)
            z["kind"] = o["kind"]
            out["zones"].append(z)
        elif k == "npc_spot":
            out["npc_spots"].append({"spot": o["spot"], "pos": to_game(o.location), "yaw_deg": float(o.get("yaw_deg", 0.0))})
        elif k == "light":
            l = {
                "pos": to_game(o.location),
                "color": list(o["color"]),
                "intensity": float(o["intensity"]),
                "radius": float(o.get("radius", 0.1)),
                "range": float(o.get("range", 20.0)),
                "night_only": bool(o.get("night_only", True)),
            }
            if "spot_dir" in o:
                l["spot_dir"] = to_game(Vector(o["spot_dir"]))
                l["cone_deg"] = float(o.get("cone_deg", 60.0))
            out["lights"].append(l)

    scene = bpy.context.scene
    doc = {
        "version": 1,
        "name": scene.get("kz_name", "level"),
        "id": scene.get("kz_id", "level"),
        "fog": float(scene.get("kz_fog", 1.0)),
        "geometry": "level.glb",
        "assets": assets,
        **out,
    }
    with open(os.path.join(out_dir, "level.json"), "w", encoding="utf8") as f:
        json.dump(doc, f, indent=1)
    print(f"EXPORTED {len(meshes)} meshes, {len(assets)} asset placements, "
          f"{sum(len(v) for v in out.values())} markers -> {out_dir}")


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    main(argv[0] if argv else os.path.dirname(bpy.data.filepath))
