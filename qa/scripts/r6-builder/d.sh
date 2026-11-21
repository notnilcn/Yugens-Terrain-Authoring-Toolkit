#!/usr/bin/env bash
# r6-e2 session driver wrapper: sources the session env then runs qa/drive.py.
set -euo pipefail
source /tmp/opencode/qa/sessions/r6-e2/env.sh
cd /home/clintonnguyen/g/code_examples/rrr-wt/r4base
exec python3 qa/drive.py "$@"
