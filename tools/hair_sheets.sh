#!/usr/bin/env bash
# Hair review sheets: tools/hair_sheets.sh <models dir> <out prefix>  (ids from <models dir>/ids.txt, six per sheet)
D=$1; O=$2; set -- $(cat $D/ids.txt); n=0
while [ $# -gt 0 ]; do
  ids=(); for i in 1 2 3 4 5 6; do [ $# -gt 0 ] || break; ids+=($1); shift; done
  /d/Blender/blender.exe --background --factory-startup --python tools/blender/hair_sheet.py -- $D ${O}_$n.png "${ids[@]}" 2>&1 | grep -E "^sheet|Error"
  rm -rf ${O}_${n}_tmp; n=$((n+1))
done
