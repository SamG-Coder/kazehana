#!/usr/bin/env bash
# Build characters end to end: MetaHuman (rig, textures, export) -> clips baked on its body ->
# game glTF.  tools/humans/build.sh <id> [<id> ...]   (ids from tools/humans/characters.json)
# -> assets/cache/humans/<id>/<id>.gltf, logs in D:/KazehanaHumans/Saved/Logs/<id>_*.log
set -u
UE=/d/UnrealEngine/UE_5.9/Engine/Binaries/Win64/UnrealEditor.exe
PROJ=D:/KazehanaHumans/KazehanaHumans.uproject
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
for id in "$@"; do
  for step in make_character bake_anims; do
    KZ_CHARACTER="$id" KZ_QUIT=1 "$UE" "$PROJ" -RenderOffScreen -unattended -nosplash \
      -ExecutePythonScript="D:/TycoonSchoolTradingSim/tools/humans/$step.py" -log="${id}_${step}.log" >/dev/null 2>&1
  done
  /d/Blender/blender.exe --background --factory-startup --python "$ROOT/tools/humans/to_gltf.py" -- "$id" 2>&1 | grep -E "^wrote|Traceback|Error:" | cut -c1-200
  python -c "
import json; r=json.load(open('D:/KazehanaHumans/export/$id/report.json')); b=json.load(open('D:/KazehanaHumans/export/$id/anims/bake.json'))
print('$id', {k: r.get(k) for k in ('face','skin','rig','textures','dcc','error')}, b.get('on'), {k: v.get('baked') for k, v in b.get('clips', {}).items()})"
done
