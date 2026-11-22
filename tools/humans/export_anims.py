"""Export MetaHuman's own body animations (Epic's optional MetaHuman content) as FBX, for
Blender to put on our characters.

    set KZ_QUIT=1
    UnrealEditor D:/KazehanaHumans/KazehanaHumans.uproject -RenderOffScreen -unattended -nosplash
        -ExecutePythonScript="D:/TycoonSchoolTradingSim/tools/humans/export_anims.py"
-> D:/KazehanaHumans/export/anims/<clip>.fbx, report.json
"""
import json
import os

import unreal

OUT = "D:/KazehanaHumans/export/anims"
BASE = "/MetaHumanCharacter/Optional/Animation/UEFNAnimPreset/Locomotion"
CLIPS = {
    "idle": "AS_MH_Neutral_Stand_Idle_Loop",
    "walk": "AS_MH_Neutral_Walk_Loop_F",
    "run": "AS_MH_Neutral_Run_Loop_F",
}
os.makedirs(OUT, exist_ok=True)
report = {}
for name, asset in CLIPS.items():
    seq = unreal.load_asset(f"{BASE}/{asset}")
    if seq is None:
        report[name] = "not found"
        continue
    task = unreal.AssetExportTask()
    task.object = seq
    task.filename = os.path.join(OUT, f"{name}.fbx")
    task.automated = True
    task.replace_identical = True
    task.prompt = False
    task.options = unreal.FbxExportOption()
    ok = unreal.Exporter.run_asset_export_task(task)
    report[name] = {"asset": asset, "ok": bool(ok), "length": seq.get_play_length(), "skeleton": str(seq.get_editor_property("skeleton").get_path_name())}
json.dump(report, open(os.path.join(OUT, "report.json"), "w"), indent=1)
if os.environ.get("KZ_QUIT"):
    unreal.SystemLibrary.quit_editor()
