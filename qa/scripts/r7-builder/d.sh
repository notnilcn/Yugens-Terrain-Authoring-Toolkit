#!/usr/bin/env bash
# r7-v2 session driver wrapper: sources the session env then runs qa/drive.py in the r7 build.
set -euo pipefail
source /tmp/opencode/qa/sessions/r7-v2/env.sh
cd /home/clintonnguyen/g/code_examples/rrr-wt/r4base
exec python3 qa/drive.py "$@"
