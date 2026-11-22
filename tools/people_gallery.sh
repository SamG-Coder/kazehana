#!/usr/bin/env bash
# Render people in Tsukumodō, six at a time.
#   tools/people_gallery.sh <models dir> <out prefix> <id> <id> ...
#   e.g. tools/people_gallery.sh assets/cache/people_review/head shots/people_review/head/r1 0 1 3 5
# -> <out prefix>_0.png, _1.png ...   (KZ_CLIP=walk to see them walking, KZ_PEOPLE_DIST to move them)
set -u
DIR=$1; OUTP=$2; shift 2
mkdir -p "$(dirname "$OUTP")"
n=0
while [ $# -gt 0 ]; do
  args=()
  for i in 1 2 3 4 5 6; do
    [ $# -gt 0 ] || break
    args+=(--character "$DIR/p$1/p$1.gltf"); shift
  done
  KZ_SHOT_FILE="${OUTP}_${n}_full.png" KZ_PEOPLE_DIST=${KZ_PEOPLE_DIST:-3.6} ./target/release/townview.exe --shot character_test "${args[@]}" >/dev/null 2>&1
  python -c "
from PIL import Image
Image.open('${OUTP}_${n}_full.png').crop((180,100,1740,900)).save('${OUTP}_${n}.png')"
  rm -f "${OUTP}_${n}_full.png"
  n=$((n+1))
done
