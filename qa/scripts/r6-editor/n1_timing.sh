#!/usr/bin/env bash
# N1 timing probe: after select, dump key rects at several delays.
# usage: n1_timing.sh <tag>
set -uo pipefail
cd /home/clintonnguyen/g/code_examples/rrr-wt/r4base
source /tmp/opencode/qa/sessions/r6-e1/env.sh
tag="$1"
outdir=/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r6-editor/ui
mkdir -p "$outdir"
for d in 0 1 3; do
  if [[ "$d" != "0" ]]; then sleep "$d"; fi
  f="/tmp/opencode/r6-${tag}-t${d}.json"
  t0=$(date +%s.%N)
  python3 qa/drive.py ui --root all --save "$f" >/dev/null
  t1=$(date +%s.%N)
  echo "== $tag delay=${d}s dump_at=+$(python3 -c "print(f'{$t1-$t0:.2f}')")s"
  python3 /home/clintonnguyen/g/code_examples/rrr/qa/scripts/r6-editor/uiq.py "$f" \
    --names PixelizerToolbarRail,PixelizerAttributes,PixelWorldPanel,Node3DEditorViewport,MarchingSquaresToolbarRail,MarchingSquaresToolAttributes
done
