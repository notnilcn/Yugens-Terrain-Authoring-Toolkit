#!/usr/bin/env bash
# Settings page sweep: click each page tab, dump the strip height + page content.
# usage: settings_pages.sh <tag>
set -uo pipefail
cd /home/clintonnguyen/g/code_examples/rrr-wt/r4base
source /tmp/opencode/qa/sessions/r6-e1/env.sh
tag="$1"
outdir=/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r6-editor/ui
mkdir -p "$outdir"
declare -A tabs=( [Layout]="601 694" [NewChunks]="682 694" [Grass]="759 694" [Display]="819 694" [LOD]="905 694" [Storage]="992 694" )
for t in Layout NewChunks Grass Display LOD Storage; do
  read -r x y <<<"${tabs[$t]}"
  python3 qa/drive.py click "$x" "$y" >/dev/null
  sleep 2.5
  f="$outdir/${tag}-settings-${t}.json"
  python3 qa/drive.py ui --root all --save "$f" >/dev/null
  echo "== $t"
  python3 - "$f" <<'EOF'
import json,sys
d=json.load(open(sys.argv[1]))
def show(name):
    for c in d['controls']:
        if c.get('name')==name and c.get('visible'):
            return c
    return None
strip=show('MarchingSquaresToolAttributes')
page=show('SettingsPage')
tabs=show('SettingsPages')
view=None
for c in d['controls']:
    if c.get('visible') and c.get('name','').startswith('@Node3DEditorViewport@') and c['class']=='Node3DEditorViewport':
        view=c
for lbl,c in (('strip',strip),('page',page),('tabs',tabs),('view',view)):
    if c: print('  %-6s %-22s %r' % (lbl, '[%g,%g %gx%g]'%tuple(c['rect']), c.get('text') or ''))
# page row labels to identify the page
labels=[ (c['rect'],c.get('text')) for c in d['controls'] if c.get('visible') and 'MarchingSquaresToolAttributes' in c.get('path','') and c.get('class')=='Label' and c.get('text')][:4]
print('  labels', labels)
EOF
done
