#!/bin/bash
# helper: qa helper for r5-t1
# usage: h.sh <drive args...>
cd /home/clintonnguyen/g/code_examples/rrr-wt/r4base || exit 1
source /tmp/opencode/qa/sessions/r5-t1/env.sh
exec python3 qa/drive.py "$@"
