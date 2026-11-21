# helper: source me, then `d <cmd...>` drives the r5-t2 session
export BUILD=/home/clintonnguyen/g/code_examples/rrr-wt/r4base
source /tmp/opencode/qa/sessions/r5-t2/env.sh
export SHOTS=/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r5-manage
cd "$BUILD" || return 1
d() { python3 qa/drive.py "$@"; }
