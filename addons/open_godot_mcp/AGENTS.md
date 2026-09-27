# AGENTS.md — `addons/open_godot_mcp` (GDScript Editor Bridge)

This folder is the **Godot-side half** of Open-Godot-MCP: an `EditorPlugin` that exposes the editor and the running game to the Python MCP server over a WebSocket bridge. The Python side lives in `src/open_godot_mcp/`; the contract docs live in `Docs/` (notably `01-Architecture/`, `02-Tools/`). Read this file before touching any `.gd` here.

Downstream projects consume this folder as a plain addon copy (sometimes symlinked). Keep it self-contained: no imports from outside `res://addons/open_godot_mcp/`.

## Layout

| Path | Role |
|---|---|
| `plugin.gd` / `plugin.cfg` | Entry point. `_enter_tree()` order: EditorSettings defaults → bridge `start_server()` → dock → export plugin → debugger plugin → editor logger → runtime autoload inject. `_exit_tree()` tears down in reverse. |
| `bridge/websocket_server.gd` | TCPServer + WebSocketPeer bridge (Godot 4 `WebSocketPeer` is client-only, so TCP accept + wrap). JSON-RPC 2.0, `tool_invoke` → `tool_result`/`event`, handshake carries `session_id`, versions, `auth_token`. Heartbeat: ping every 5 s. |
| `bridge/dispatcher.gd` | `tool_name` → handler instance registry. Owns `set_bridge()` fan-out (called from server `_ready`). |
| `handlers/*_handler.gd` | One file per tool family (`godot_game`+`godot_game_time` share `game_handler`). Editor-direct ops run locally; game-runtime ops forward over the debugger channel. |
| `debugger/mcp_debugger_plugin.gd` | Editor ↔ game IPC over `EditorDebuggerSession` / `EngineDebugger`. Prefix `ogm`: editor→game `ogm:call [request_id, method, params]`, game→editor `ogm:response` / `ogm:error` / boot beacon `ogm:hello [pid, args]`. |
| `runtime/runtime_autoload.gd` | Runs **in the game process** as `McpRuntimeAutoload`. Input sim, state observation (`_mcp_state()` protocol), eval, screenshots, `Engine.time_scale` clock. Also serves standalone-WebSocket mode for `godot_network launch_instance`. |
| `runtime/mcp_test_suite.gd` (`OgmTestSuite`) | Base class for `godot_test` suites: `res://tests/` scripts with `test_*()` funcs + `set_up()`/`tear_down()` + `assert_*` helpers. |
| `utils/` | Shared: `error_codes.gd` (`MCPErrorCodes`), `variant_codec.gd` (`VariantCodec`), `port_resolver.gd` (`PortResolver`), `scene_path.gd` (`OgmScenePath`), `screenshot_cleanup.gd`, `update_manager.gd`, `update_reload_runner.gd`. |
| `dock/` | Status dock + language + Agnes/NVIDIA opt-in config + update banner. Strings in `dock/i18n/*.json` (`en.json` is the source). |
| `export/mcp_export_plugin.gd` | Strips the runtime autoload entry from exported `project.godot` (restores it in-editor after export). |

## Handler contract

Every handler (except the debugger/export plugins):

```gdscript
extends RefCounted
const _EC = preload("res://addons/open_godot_mcp/utils/error_codes.gd")
var _bridge: Node  # set via set_bridge(); dispatcher fans it out

func handle(tool: String, action: String, params: Dictionary) -> Dictionary:
    match action:
        "status":
            return _EC.ok({...})
        _:
            return _EC.fail("INVALID_ARGUMENT", "Unknown action: %s" % action)
```

- Return **only** `_EC.ok(payload)` / `_EC.fail(code, message, extra?)` — never a raw dict. Codes match `Docs/02-Tools/Index.md §錯誤回傳格式`; the list is not closed.
- Runtime-bound handlers forward via `dbg.call_runtime(method, call_params)` (awaited). Null-guard in order: `_bridge` → `get_debugger()` → `RUNTIME_NOT_CONNECTED`.
- `params.instance` is **1-based launch order** (`godot_game instances`); `0`/omitted = first instance. Never renumber; `_game_sessions` order in the debugger plugin is the source of truth.
- JSON boundary: encode outbound Variants with `VariantCodec.encode_variant`, decode inbound with `decode_variant` (Vector2↔`{"x","y"}`, etc., per `Docs/02-Tools/Index.md §Godot 型別的 JSON 編碼`).

## Conventions

- **Tabs, GDScript, `@tool` only where the engine needs it** (plugin, debugger/export plugins, dock). Plain `extends RefCounted` elsewhere.
- **No `class_name` on handlers.** Only shared utils declare one (`MCPErrorCodes`, `VariantCodec`, `PortResolver`, `OgmScenePath`, `OgmTestSuite`) to avoid polluting consuming projects. Everything else is referenced via `const X = preload("res://addons/open_godot_mcp/...")`.
- Every file opens with a `##` doc header naming its tool(s) and `Docs:` reference, e.g. `## Input handler — godot_input … Docs: 02-Tools/Input.md`.
- **Never hand-edit `.uid` files** — Godot manages them; editing breaks script UIDs.
- Settings keys (don't rename without updating `plugin.gd` + `Docs/06-Installation/`): `open_godot_mcp/bridge/*` (port, auto_port, heartbeat), `open_godot_mcp/runtime/*` (auto_inject, strip_on_export), `open_godot_mcp/security/*` (allow_eval, read_only, auth_token), `open_godot_mcp/ui/language`; ProjectSettings `open_godot_mcp/screenshot_max_count|_max_age_hours` and `autoload/McpRuntimeAutoload`.
- Timeouts: debugger `call_runtime` default **15 s**, game-ready wait **20 s**. An eval timeout usually means the evaluated body errored — surface it, don't raise the timeout.

## Adding a tool or action

1. Implement/extend the `handlers/` file (match on `action`, fail `INVALID_ARGUMENT` on unknown).
2. Register the tool name in `bridge/dispatcher.gd` (`_register("godot_x", _XHandler)`); one handler may serve several tool names.
3. Add the Python wrapper in `src/open_godot_mcp/tools/` + tool description, and document it under `Docs/02-Tools/` (keep the zh-TW doc language; code examples stay English).
4. If the runtime must serve it, add the method to `runtime/runtime_autoload.gd` behind the same `method` string the handler passes to `call_runtime`.

## Verification

- Python side: `pytest tests/` from the repo root (needs dev extras).
- GDScript side: no local unit runner here — verify live via `godot_health check` then the relevant tool (`godot_game status`, `godot_test` for `res://tests/` suites extending `OgmTestSuite`).
- After touching the bridge/dispatcher/runtime channel, re-run a play/stop cycle plus one runtime round-trip (`digest` or `eval`) before calling it done.

## Pitfalls

- `EditorInterface` / `EditorDebuggerPlugin` exist **only in the editor**. `runtime_autoload.gd` must guard with `Engine.is_editor_hint()` and never assume editor APIs.
- Signal/lambda capture from the debugger message handler is unreliable — the codebase polls a `_pending[request_id]` dict instead. Follow that pattern.
- Debugger session 0 is reused for the first game connection; a session counts as a game session only after its `started` signal. Don't skip index 0.
- `input` forwarding renames the action key to `input_type` to avoid colliding with InputMap `params["action"]`. Keep that rename.
- Export must never ship the autoload: test `strip_on_export` on/off after touching `export/` or the autoload registration.
