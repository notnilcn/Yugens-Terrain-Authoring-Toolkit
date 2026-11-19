#!/usr/bin/env bash
# Shared by the editor checks (tests/xogot_i{3,4,5}_check.sh, tests/builder_editor_capture.sh):
# run the editor (optionally with a temporary autoload) and leave the user's state exactly as it was.
#   source "$root/tests/lib/editor_check_env.sh"
#   ecenv_begin "$root" 'XogotI5Check="*res://tests/xogot_i5_check.gd"'   # sets $backup, traps restore
#   ecenv_begin "$root" ''                                               # no autoload
# Backed up and restored on exit: project.godot (the autoload line is appended after the
# backup), every *.import outside .godot/, demo/ (minus demo/bin), the per-user editor settings
# file, .godot/editor/project_metadata.cfg (builder preferences) and
# .godot/editor/editor_layout.cfg (open scene tabs, docks); each is deleted again if there was none.
# The backup lives in .tmp/editor_check_backup/ (not a temp dir), so a run that was killed hard
# leaves it behind: the next ecenv_begin restores it and stops (exit 2) without running, so the
# restore is never silent — check `git status`, then run again. A failed restore keeps the backup.
# Every restore step is checked; the backup is deleted only when all of them succeeded, else its
# path is printed (restore by hand, or let the next run retry).
# Refuses to run while another Godot instance is running (it could save the same editor
# settings / metadata files over ours); ECENV_ALLOW_RUNNING=1 skips that guard.

ecenv_godot_running() {
  if command -v tasklist >/dev/null 2>&1; then
    tasklist //FO CSV //NH 2>/dev/null | grep -iE '^"godot[^"]*\.exe"' | head -1
  else
    pgrep -a -f '(^|/)godot[^ ]*( |$)' 2>/dev/null | head -1
  fi
}

ecenv_settings_path() {
  if [[ -n "${APPDATA:-}" ]]; then
    echo "$APPDATA/Godot/editor_settings-4.7.tres"
  else
    echo "${XDG_CONFIG_HOME:-$HOME/.config}/godot/editor_settings-4.7.tres"
  fi
}

# ecenv_restore_from ROOT BACKUP: 0 when every step succeeded (the backup is then deleted).
ecenv_restore_from() {
  local root="$1" backup="$2" ok=0 f settings
  [[ -n "$backup" && -d "$backup" ]] || return 0
  settings="$(ecenv_settings_path)"
  if [[ -f "$backup/editor_settings.tres" ]]; then
    cp "$backup/editor_settings.tres" "$settings" || { echo "editor_check_env: could not restore $settings" >&2; ok=1; }
  elif [[ -f "$backup/no_editor_settings" ]]; then
    rm -f "$settings" || ok=1
  fi
  for f in project_metadata.cfg editor_layout.cfg; do
    if [[ -f "$backup/$f" ]]; then
      cp "$backup/$f" "$root/.godot/editor/$f" || { echo "editor_check_env: could not restore .godot/editor/$f" >&2; ok=1; }
    elif [[ -f "$backup/no_$f" ]]; then
      rm -f "$root/.godot/editor/$f" || ok=1
    fi
  done
  cp "$backup/project.godot" "$root/project.godot" || { echo "editor_check_env: could not restore project.godot" >&2; ok=1; }
  if [[ -s "$backup/imports.tar" ]]; then
    tar -xf "$backup/imports.tar" -C "$root" || { echo "editor_check_env: could not restore the .import files" >&2; ok=1; }
  fi
  tar -xf "$backup/demo.tar" -C "$root" || { echo "editor_check_env: could not restore demo/" >&2; ok=1; }
  if [[ $ok == 0 ]]; then
    rm -rf "$backup"
  else
    echo "editor_check_env: restore incomplete; the backup is kept in $backup" >&2
  fi
  return $ok
}

ecenv_restore() {
  [[ -n "${ECENV_BACKUP:-}" ]] || return 0
  ecenv_restore_from "$ECENV_ROOT" "$ECENV_BACKUP"
  local r=$?
  ECENV_BACKUP=""
  return $r
}

# EXIT trap: restore; a failed restore turns the script's exit code into 3.
ecenv_on_exit() {
  local code=$?
  ecenv_restore || code=3
  exit $code
}

# ecenv_begin ROOT AUTOLOAD_LINE ('' = no autoload)
ecenv_begin() {
  ECENV_ROOT="$1"
  local line="$2" name="${2%%=*}"
  local running dir="$1/.tmp/editor_check_backup"
  running="$(ecenv_godot_running)"
  if [[ -n "$running" && -z "${ECENV_ALLOW_RUNNING:-}" ]]; then
    echo "editor_check_env: another Godot instance is running ($running); close it first (or set ECENV_ALLOW_RUNNING=1)" >&2
    exit 2
  fi
  # A half-written backup never replaced anything (the autoload is appended after it is complete).
  rm -rf "$dir.partial"
  if [[ -d "$dir" ]]; then
    echo "editor_check_env: restoring the backup a previous run left in $dir" >&2
    if ! ecenv_restore_from "$ECENV_ROOT" "$dir"; then
      echo "editor_check_env: the stale backup could not be restored; fix $dir by hand first" >&2
      exit 2
    fi
    echo "editor_check_env: restored the files a killed run left changed; check git status, then re-run" >&2
    exit 2
  fi
  mkdir -p "$dir.partial" || exit 2
  local b="$dir.partial"
  [[ -n "$line" ]] && sed -i "/^${name}=/d" "$ECENV_ROOT/project.godot"
  cp "$ECENV_ROOT/project.godot" "$b/project.godot" || exit 2
  (cd "$ECENV_ROOT" && find . -name '*.import' -not -path './.godot/*' -not -path './.tmp/*' -print0 | tar --null -cf "$b/imports.tar" --files-from=-) || exit 2
  (cd "$ECENV_ROOT" && tar -cf "$b/demo.tar" --exclude=demo/bin demo) || exit 2
  ECENV_SETTINGS="$(ecenv_settings_path)"
  if [[ -f "$ECENV_SETTINGS" ]]; then cp "$ECENV_SETTINGS" "$b/editor_settings.tres" || exit 2; else touch "$b/no_editor_settings"; fi
  local f
  for f in project_metadata.cfg editor_layout.cfg; do
    if [[ -f "$ECENV_ROOT/.godot/editor/$f" ]]; then cp "$ECENV_ROOT/.godot/editor/$f" "$b/$f" || exit 2; else touch "$b/no_$f"; fi
  done
  mv "$b" "$dir" || exit 2
  ECENV_BACKUP="$dir"
  backup="$ECENV_BACKUP"
  trap ecenv_on_exit EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  if [[ -n "$line" ]]; then
    printf '\n[autoload]\n\n%s\n' "$line" >> "$ECENV_ROOT/project.godot"
  fi
}
