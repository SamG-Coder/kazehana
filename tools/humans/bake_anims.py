"""Bake the body clips onto one character, with MetaHuman's corrective joints.

A MetaHuman body is skinned to its main joints plus corrective ones (around the shoulders,
chest, elbows, hips, knees) that Unreal drives while it plays, from the body's rig (its DNA,
through the body's post-process animation blueprint). The raw clips (tools/humans/export_anims.py)
carry only the main joints, so on their own the shoulders and chest crumple when the arms come
down. This plays each clip on the character's own body in a level sequence, with the
post-process blueprint on, and records every joint (Sequencer's bake), then writes FBX.
The clips play on the character's own MetaHuman actor (the editor's, as the MetaHuman tools
set it up: its body with the rig logic that drives the correctives); a plain actor with the
exported body mesh and the post-process blueprint is the fallback.

    set KZ_CHARACTER=<id> KZ_QUIT=1
    UnrealEditor D:/KazehanaHumans/KazehanaHumans.uproject -RenderOffScreen -unattended -nosplash
        -ExecutePythonScript="D:/TycoonSchoolTradingSim/tools/humans/bake_anims.py"
-> D:/KazehanaHumans/export/<id>/anims/<clip>.fbx, bake.json
"""
import json
import os

import unreal

EXPORT = "D:/KazehanaHumans/export"
BASE = "/MetaHumanCharacter/Optional/Animation/UEFNAnimPreset/Locomotion"
CLIPS = {"idle": "AS_MH_Neutral_Stand_Idle_Loop", "walk": "AS_MH_Neutral_Walk_Loop_F", "run": "AS_MH_Neutral_Run_Loop_F"}
POST = "/MetaHumanCharacter/Body/ABP_Body_PostProcess.ABP_Body_PostProcess_C"

cid = os.environ.get("KZ_CHARACTER", "test_child")
out = os.path.join(EXPORT, cid, "anims")
os.makedirs(out, exist_ok=True)
report = {"id": cid}


def save():
    json.dump(report, open(os.path.join(out, "bake.json"), "w"), indent=1)


reg = unreal.AssetRegistryHelpers.get_asset_registry()
reg.scan_paths_synchronous(["/Game/Export", "/Game/Characters"], True)
body = None
for a in reg.get_assets_by_path(f"/Game/Export/{cid}", recursive=True):
    if str(a.asset_class_path.asset_name) == "SkeletalMesh" and "body" in str(a.asset_name).lower() and "full" not in str(a.asset_name).lower():
        body = a.get_asset()
report["body"] = body.get_path_name() if body else None
if body is None:
    report["error"] = "no body skeletal mesh under /Game/Export/" + cid
    save()
else:
    skel = body.get_editor_property("skeleton")
    report["skeleton"] = skel.get_path_name()
    post = unreal.load_object(None, POST)
    report["post_process"] = post.get_path_name() if post else None
    if post:
        body.set_editor_property("post_process_anim_blueprint", post)
    world = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()
    actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    # one actor for every clip: the character's own MetaHuman actor if it can be had
    actor, comp = None, None
    character = unreal.load_asset(f"/Game/Characters/{cid}")
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    if character and sub.try_add_object_to_edit(character):
        try:
            actor = sub.spawn_meta_human_actor(character, True)
            comps = list(actor.get_components_by_class(unreal.SkeletalMeshComponent))
            report["components"] = [c.get_name() for c in comps]
            comp = next((c for c in comps if "body" in c.get_name().lower()), None)
        except Exception as e:  # noqa: BLE001
            report["actor_error"] = repr(e)
    if comp is None:
        if actor:
            actors.destroy_actor(actor)
        actor = actors.spawn_actor_from_class(unreal.SkeletalMeshActor, unreal.Vector(0, 0, 0))
        comp = actor.skeletal_mesh_component
        comp.set_skeletal_mesh_asset(body) if hasattr(comp, "set_skeletal_mesh_asset") else comp.set_editor_property("skeletal_mesh", body)
        report["on"] = "plain actor"
    else:
        report["on"] = "metahuman actor"
    for name, asset in CLIPS.items():
        r = report.setdefault("clips", {}).setdefault(name, {})
        src = unreal.load_asset(f"{BASE}/{asset}")
        if src is None:
            r["error"] = "clip not found"
            continue
        # the sequence: the clip on the actor, its length at 30 fps
        seq_path = f"/Game/Bake/{cid}"
        for old in (f"{seq_path}/SEQ_{name}", f"{seq_path}/{name}"):
            if unreal.EditorAssetLibrary.does_asset_exist(old):
                unreal.EditorAssetLibrary.delete_asset(old)
        seq = tools.create_asset(f"SEQ_{name}", seq_path, unreal.LevelSequence, unreal.LevelSequenceFactoryNew())
        seq.set_display_rate(unreal.FrameRate(30, 1))
        seq.set_tick_resolution(unreal.FrameRate(30, 1)) if hasattr(seq, "set_tick_resolution") else None
        frames = int(round(src.get_play_length() * 30))
        seq.set_playback_start(0)
        seq.set_playback_end(frames)
        binding = seq.add_possessable(actor)
        if report["on"] == "metahuman actor":
            binding = seq.add_possessable(comp)
        r["binding"] = str(binding.get_display_name())
        track = binding.add_track(unreal.MovieSceneSkeletalAnimationTrack)
        section = track.add_section()
        section.set_range(0, frames)
        params = section.get_editor_property("params")
        params.set_editor_property("animation", src)
        section.set_editor_property("params", params)
        # the recording, on the body's own skeleton
        fac = unreal.AnimSequenceFactory()
        fac.set_editor_property("target_skeleton", skel)
        baked = tools.create_asset(name, seq_path, unreal.AnimSequence, fac)
        opt = unreal.AnimSeqExportOption()
        opt.set_editor_property("export_transforms", True)
        opt.set_editor_property("export_morph_targets", False)
        opt.set_editor_property("record_in_world_space", False)
        opt.set_editor_property("evaluate_all_skeletal_mesh_components", True)
        opt.set_editor_property("warm_up_frames", unreal.FrameNumber(5))
        try:
            ok = unreal.SequencerTools.export_anim_sequence(world, seq, baked, opt, binding, False)
        except Exception as e:  # noqa: BLE001
            ok = False
            r["error"] = repr(e)
        r["baked"] = bool(ok)
        r["frames"] = frames
        # out as FBX
        task = unreal.AssetExportTask()
        task.object = baked
        task.filename = os.path.join(out, f"{name}.fbx")
        task.automated = True
        task.replace_identical = True
        task.prompt = False
        task.options = unreal.FbxExportOption()
        r["fbx"] = bool(unreal.Exporter.run_asset_export_task(task))
        save()
    actors.destroy_actor(actor)
    save()
if os.environ.get("KZ_QUIT"):
    unreal.SystemLibrary.quit_editor()
