#!/bin/bash
# setnum.sh 0.25 -- types a number into the focused LineEdit (EditorSpinSlider value box)
# Usage: click the value text first (or pass --click X Y).
set -e
D=$(dirname "$0")/d.sh
if [ "$1" = "--click" ]; then
  "$D" click "$2" "$3" >/dev/null
  sleep 0.6
  shift 3
fi
VAL="$1"
"$D" key a --ctrl >/dev/null
for (( i=0; i<${#VAL}; i++ )); do
  ch="${VAL:$i:1}"
  case "$ch" in
    .) "$D" key period >/dev/null ;;
    -) "$D" key minus >/dev/null ;;
    *) "$D" key "$ch" >/dev/null ;;
  esac
done
"$D" key enter >/dev/null
