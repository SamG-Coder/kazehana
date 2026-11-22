"""Make one Kazehana character as a MetaHuman, headless, and export it for Blender.

    set KZ_CHARACTER=<id> KZ_QUIT=1
    UnrealEditor D:/KazehanaHumans/KazehanaHumans.uproject -RenderOffScreen -unattended -nosplash
        -ExecutePythonScript="D:/TycoonSchoolTradingSim/tools/humans/make_character.py"
(the full editor, drawing off screen: the MetaHuman tools need the editor's UI layer, which the
-run=pythonscript commandlet does not have)

Characters (tools/humans/characters.json): an id, its body measurements (cm: height and the
rest MetaHuman's parametric body takes), its skin tone. Writes
    D:/KazehanaHumans/export/<id>/...      (geometry, DNA, textures: the DCC export)
    D:/KazehanaHumans/export/<id>/report.json (the body measurements available, what was set)
"""

import json
import os
import sys

import unreal

ROOT = "D:/TycoonSchoolTradingSim"
OUT = "D:/KazehanaHumans/export"

# (the character id: the script's argument, or KZ_CHARACTER when run through -ExecutePythonScript)
cid = os.environ.get("KZ_CHARACTER") or (sys.argv[-1] if len(sys.argv) > 1 and not sys.argv[-1].endswith(".py") else "kenji")
spec = json.load(open(os.path.join(ROOT, "tools/humans/characters.json"), encoding="utf8"))[cid]
out = os.path.join(OUT, cid)
os.makedirs(out, exist_ok=True)
report = {"id": cid}

tools = unreal.AssetToolsHelpers.get_asset_tools()
path = "/Game/Characters"
existing = unreal.EditorAssetLibrary.does_asset_exist(f"{path}/{cid}")
if existing:
    unreal.EditorAssetLibrary.delete_asset(f"{path}/{cid}")
# a character starts from one of MetaHuman's preset people (its face, skin and hair: "preset"
# in characters.json), or a blank one
preset = spec.get("preset")
if preset:
    character = unreal.EditorAssetLibrary.duplicate_asset(f"/MetaHumanCharacter/Optional/Presets/{preset}", f"{path}/{cid}")
    report["preset"] = preset
else:
    character = tools.create_asset(cid, path, unreal.MetaHumanCharacter, unreal.MetaHumanCharacterFactoryNew())
report["created"] = character is not None
sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
if not sub.try_add_object_to_edit(character):
    report["error"] = "could not edit the character"
else:
    # the face: a mix of preset faces ("face": {preset: weight}), each weight taking the face that
    # far from MetaHuman's average face toward that preset's, value by value (the face model's
    # leading values are structure, not shape, and are the same in every face). Presets bring
    # their features without their own neck and build (the body here is the blank one).
    def preset_copy(name):
        p = f"/Game/Characters/_preset_{name}"
        if not unreal.EditorAssetLibrary.does_asset_exist(p):
            unreal.EditorAssetLibrary.duplicate_asset(f"/MetaHumanCharacter/Optional/Presets/{name}", p)
        return unreal.load_asset(p)

    mix = spec.get("face", {})
    if mix:
        base = list(sub.get_face_model_coefficients(character))
        face = list(base)
        used = {}
        for name, w in mix.items():
            donor = preset_copy(name)
            if donor and sub.try_add_object_to_edit(donor):
                d = list(sub.get_face_model_coefficients(donor))
                sub.remove_object_to_edit(donor)
                if len(d) == len(base):
                    face = [f + float(w) * (x - b) for f, x, b in zip(face, d, base)]
                    used[name] = w
        sub.set_face_model_coefficients(character, face)
        sub.commit_face_state(character)
        report["face"] = used
    # the skin: a preset's (tone and texture sets), with any of its values changed ("skin":
    # {"from": preset, "u": tone, "face_texture_index": n, ...}); the high-resolution textures
    # are made from it
    sk = spec.get("skin")
    if sk:
        try:
            src = preset_copy(sk["from"]) if sk.get("from") else character
            settings = src.get_editor_property("skin_settings")
            props = settings.get_editor_property("skin")
            for k, v in sk.items():
                if k != "from":
                    props.set_editor_property(k, v)
            settings.set_editor_property("skin", props)
            sub.commit_skin_settings(character, settings)
            report["skin"] = sk
        except Exception as e:  # noqa: BLE001
            report["skin"] = repr(e)
    # the body: what can be set, and in what range
    cons = sub.get_body_constraints(character, False)
    report["body_constraints"] = [{"name": str(c.name), "min": c.min_measurement, "max": c.max_measurement, "target": c.target_measurement} for c in cons]
    wanted = spec.get("body", {})
    cons = list(cons)
    for i, c in enumerate(cons):
        n = str(c.name)
        if n in wanted:
            c = c.copy()
            c.set_editor_property("is_active", True)
            c.set_editor_property("target_measurement", max(c.min_measurement, min(c.max_measurement, float(wanted[n]))))
            cons[i] = c
    sub.set_body_constraints(character, cons)
    sub.commit_body_state(character)
    after = sub.get_body_constraints(character, False)
    report["set"] = {n: v for n, v in wanted.items()}
    report["measured"] = {str(c.name): round(c.target_measurement, 1) for c in after}
    # rig (joints and face blend shapes) and the high-resolution textures: Epic's cloud services,
    # waited for (KZ_FAST skips both and the DCC export: shape checks only)
    fast = bool(os.environ.get("KZ_FAST"))
    try:
        if fast:
            raise RuntimeError("skipped (KZ_FAST)")
        rp = unreal.MetaHumanCharacterAutoRiggingRequestParams()
        rp.rig_type = unreal.MetaHumanRigType.JOINTS_AND_BLEND_SHAPES
        rp.blocking = True
        rp.report_progress = False
        sub.request_auto_rigging(character, rp)
        report["rig"] = "requested"
    except Exception as e:  # noqa: BLE001
        report["rig"] = repr(e)
    try:
        if fast:
            raise RuntimeError("skipped (KZ_FAST)")
        tp = unreal.MetaHumanCharacterTextureRequestParams()
        tp.blocking = True
        tp.report_progress = False
        sub.request_texture_sources(character, tp)
        report["textures"] = "requested"
    except Exception as e:  # noqa: BLE001
        report["textures"] = repr(e)
    # build and export: geometry (skeletal meshes into the project) and the DCC export
    try:
        # (a previous run's meshes are deleted first: the export doesn't replace them)
        if unreal.EditorAssetLibrary.does_directory_exist(f"/Game/Export/{cid}"):
            unreal.EditorAssetLibrary.delete_directory(f"/Game/Export/{cid}")
        geo = unreal.MetaHumanGeometryExportParams()
        geo.project_path = f"/Game/Export/{cid}"
        geo.full_body_skeletal_mesh = True
        unreal.MetaHumanCharacterExportBlueprintLibrary.export_geometry(character, geo)
        report["geometry"] = "ok"
    except Exception as e:  # noqa: BLE001
        report["geometry"] = repr(e)
    try:
        if fast:
            raise RuntimeError("skipped (KZ_FAST)")
        dcc = unreal.MetaHumanDCCExportParams()
        dcc.external_path = out
        dcc.archive_name = cid
        dcc.compress_in_zip_file = False
        unreal.MetaHumanCharacterExportBlueprintLibrary.export_dcc(character, dcc)
        report["dcc"] = "ok"
    except Exception as e:  # noqa: BLE001
        report["dcc"] = repr(e)
    # the skeletal meshes made by the geometry export, out as FBX (Blender takes them on)
    reg = unreal.AssetRegistryHelpers.get_asset_registry()
    fbx = []
    for a in reg.get_assets_by_path(f"/Game/Export/{cid}", recursive=True):
        if str(a.asset_class_path.asset_name) == "SkeletalMesh":
            mesh = a.get_asset()
            task = unreal.AssetExportTask()
            task.object = mesh
            task.filename = os.path.join(out, f"{a.asset_name}.fbx")
            task.automated = True
            task.replace_identical = True
            task.prompt = False
            opts = unreal.FbxExportOption()
            opts.export_morph_targets = True
            task.options = opts
            ok = unreal.Exporter.run_asset_export_task(task)
            fbx.append({"mesh": str(a.package_name), "file": task.filename, "ok": bool(ok)})
    report["fbx"] = fbx
    sub.remove_object_to_edit(character)
unreal.EditorAssetLibrary.save_directory(path)
# (the meshes too: tools/humans/bake_anims.py plays the clips on them)
unreal.EditorAssetLibrary.save_directory(f"/Game/Export/{cid}")
json.dump(report, open(os.path.join(out, "report.json"), "w"), indent=1)
print("character", cid, "->", out)
if os.environ.get("KZ_QUIT"):
    unreal.SystemLibrary.quit_editor()
