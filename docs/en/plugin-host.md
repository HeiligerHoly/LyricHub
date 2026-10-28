# Plugin host (P1 of PLAN-plugins.md)

> Date: 2026-10-28 · after v1.1.0 · Reference: [PLAN-plugins.md](../../PLAN-plugins.md) sections 4 and 6 (German)
> Result: **LyricHub installs, starts, watches and ends plugins. All acceptance criteria of P1 are checked in the real app.** Plugins cannot deliver anything yet; word times follow in P2.

## What is built

| Part | Implementation | Code |
|---|---|---|
| Manifest | `plugin.json` with ID, name, version, protocol version, program, capabilities and permissions. Unsafe or unknown fields are rejected; another protocol version counts as "incompatible". | `plugins/manifest.rs` |
| Installation | The ZIP is checked completely before anything is written: manifest, program, no paths outside the folder, no absolute paths, no links, size limits. It is unpacked into a staging folder that only then replaces the old version. | `plugins/install.rs` |
| Process | A separate `.exe` without a console window, at "below normal" priority, in a job object with `KILL_ON_JOB_CLOSE`. JSON-RPC 2.0, one message per line; stderr goes to the log, cut short. | `plugins/process.rs` |
| Supervisor | Handshake `initialize` (5 s); after a crash, restarts after 2, 10 and 60 s, then "error" with the last stderr line; when switched off, `shutdown` and 2 s of grace. | `plugins/supervisor.rs` |
| App | `plugins.enabled` in the settings, start at program start, end at exit, page "Plugins" (install with confirmation including SHA-256, switch, state, remove), plugins in Diagnostics and in the diagnostics export (only ID, version, state). | `plugins/mod.rs`, `PluginsPage.svelte` |
| Example plugin | Answers `initialize`, reports "ready", ends on `shutdown`; a `behavior.json` makes it crash, report another version or stay silent. Tests only, not shipped. | `plugins/example/` |

Folders: programs under `%APPDATA%\LyricHub\plugins\<id>`, large data such as models under `%LOCALAPPDATA%\LyricHub\plugins\<id>`.

## Checks

**Unit tests (run with `cargo test`):**

- Manifest: valid, unsafe values, another protocol version.
- ZIP:
  - Installing from the root folder and from a subfolder.
  - Replacing the old version.
  - Rejected are parent and absolute paths, missing manifests, missing programs and several root folders.
- Messages: format and cutting, restart pauses.
- Job object: a process in it ends as soon as the job object closes.

**With the example plugin** (`cargo test --lib supervisor -- --ignored`):

- Start, status and stop.
- Crash with restarts after 2 s and 10 s.
- Another protocol version gives "incompatible", silence gives "error".

**In the real app:**

1. ZIP checked (ID, version, SHA-256) and installed: the plugin is off afterwards.
2. Switched on: "running" with the status "ready" after about 1 s.
3. LyricHub ended hard (`taskkill /F`): the plugin process was gone 2 s later.
4. Restarted: the switched-on plugin ran again 40 ms after the start.
5. Switched off: the process ends.
6. Removed: program and data folders are empty, and it is gone from the settings.

Without plugins nothing starts. The installer stays small: the only new libraries are `zip` and `typed-path`.

## Open for P2

- The capability `word_timing`: notifications `track`, `playback`, `lyrics` to the plugin, `word_times` back, a cache table and taking the times over into the lyrics.
- The player's process ID only for plugins with `audio:player`.
