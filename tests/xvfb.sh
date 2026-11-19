# Sourced by the windowed editor test scripts: `xvfb_wrap <display>` fills the array `wrap`
# with the command prefix that runs a program on a private X display.
#   1. PT_XVFB=<wrapper> or ~/.local/bin/pt-xvfb (`wrapper <display> cmd...`), else
#   2. xvfb-run -a (any free display; screen size from XVFB_SCREEN, default 1600x900x24), else
#   3. no wrapper: DISPLAY=:<display> (an existing X server).
xvfb_wrap() {
  local disp="$1" helper="${PT_XVFB:-$HOME/.local/bin/pt-xvfb}"
  if [[ -x "$helper" ]]; then
    wrap=("$helper" "$disp")
  elif command -v xvfb-run >/dev/null 2>&1; then
    wrap=(xvfb-run -a -s "-screen 0 ${XVFB_SCREEN:-1600x900x24}")
  else
    export DISPLAY=":$disp"
    wrap=()
  fi
}
