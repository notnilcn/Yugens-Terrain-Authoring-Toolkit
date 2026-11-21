#!/bin/bash
# thin wrapper: run qa/drive.py from the merged build with the r5-t3 env
set -e
cd /home/clintonnguyen/g/code_examples/rrr-wt/r4base
export DISPLAY=:53 MST_QA_PORT=8853 MST_SESSION=r5-t3 MST_PROJECT=/tmp/opencode/qa/sessions/r5-t3/project
exec python3 qa/drive.py "$@"
