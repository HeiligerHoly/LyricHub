# Auto-Sync (Q1 and Q2 of PLAN-plugins-2.md)

> Date: 2026-10-31 · after v1.2.0 · Related: [PLAN-plugins-2.md](../../PLAN-plugins-2.md) sections 3, 5 and 6, [docs/en/spike-recognize.md](spike-recognize.md) (M1), [docs/en/plugin-align.md](plugin-align.md)
> Result: **Once Align has measured a song, each line starts 150 ms before its first sung word. On four JamendoLyrics songs with deliberately shifted timestamps, line starts are then 38 ms off the singing in the median, 165 ms at p90. The larger search windows the plan intended measure clearly worse and were dropped.**

German: [docs/auto-sync.md](../auto-sync.md)

## What was built

Auto-Sync needs neither a second capture nor extra computing time. The data is in the host already: Align's measured word times.

| Part | Implementation | Code |
|---|---|---|
| Switch | Align declares the setting `auto_sync` in its manifest (on/off, on by default). The plugin page draws it as a switch. | `plugins/align/plugin.json` |
| Plugin settings (Q1) | Manifests may declare up to 8 switches: an `id` of lowercase letters, digits and `_`, type `bool`, a default, and a label and help text in German and English. LyricHub stores the values in `settings.json` under `plugins.values` and sends them to the plugin with `settings`, at start and on every change. | `plugins/manifest.rs`, `plugins/mod.rs`, `settings/schema.rs`, `PluginsPage.svelte` |
| Line start | A line with measured words starts at its first word minus 150 ms, at most 1.5 s from the LRC stamp and never before the line above. The line above ends at the new start. | `plugins/words.rs` (`auto_sync`) |
| Lines without a measurement | They move, estimated words included, by the median of the song's measured lines. | same |
| When | While taking over the plugin's words, that is each time a measured song is shown. The cache keeps the LRC stamps; switched off, they apply again at once. A song offset applies on top. | `app_state.rs` (`add_plugin_words`) |
| Diagnostics | Row "Lines: moved onto the singing (Auto-Sync), X ms in the median" (`line_sync_ms` in the lyrics event). | `DiagnosticsPage.svelte` |

**Align itself** does not change for Auto-Sync. Its window starts 0.5 s before a line's timestamp, as in P0 (section "Measurement").

## Checks

### Unit tests

- **Host** (`cargo test` in `src-tauri`): Auto-Sync moves lines to their first word and lines without a measurement by the median; limits, order and a song without measurements; manifests with switches, including rejected entries.
- **Align** (`cargo test` in `plugins/align`): windows with a free lead.

### Measurement on JamendoLyrics

Four songs from P0 (German, English, French, Spanish, 13.7 min in all), Align's pipeline with 4 threads (`cargo run --release --example jamendo`). Measured is the distance from each line's first aligned word, minus the 150 ms lead, to the annotated singing. "Shifted" means the timestamps were moved like in a sloppy LRC file, by up to ±300 ms per song plus up to ±200 ms per line (`--jitter 300 200`).

| Run | Window starts before the stamp | Stamps | Line start median / p90 | within 100 ms | Word start median / p90 |
|---|---|---|---|---|---|
| M1 from Q0 (24 songs) | 0.5 s | exact | 37 / 138 ms | – | – |
| 0 | 0.5 s | exact | 40 / 161 ms | 69.8 % | 48 / 271 ms |
| a | 0.5 s | shifted | **38 / 165 ms** | 72.5 % | 49 / 274 ms |
| b | 1.5 s | shifted | 94 / 1225 ms | 50.5 % | 57 / 894 ms |
| c | 1.5 s | exact | 104 / 1279 ms | 46.7 % | 58 / 937 ms |

- **Align is unchanged after the rework** (run 0): word starts 48 ms off in the median, 271 ms at p90, 82.8 % within 150 ms, exactly the values from P3. That was the acceptance check of Q1 (shared code, settings, new capability).
- **The gate (median ≤ 50 ms, p90 ≤ 200 ms) holds with shifted stamps too** (run a).
- **Larger windows hurt.** The plan had Align's windows start 1.5 s before the stamp with Auto-Sync, so even a very late stamp would still find its first word. The measurement shows the opposite: with 1.5 s, every tenth line start lands more than a second off, even with exact stamps (run c). The longer window probably holds the end of the line above, and the alignment puts the first words there.
- **Decision:** the window stays at 0.5 s. Auto-Sync can therefore move a line at most about 0.65 s earlier than its stamp (window start plus lead), and later up to the 1.5 s limit. Typical LRC errors are smaller: per song −140 … +151 ms in the median ([docs/en/sync-report.md](sync-report.md)).

### In the real app

Spotify, 2026-10-31, a song Align had measured on the first listen (29 of 30 lines with word times), on the second listen:

| State | first word after the line start, p10 / median / p90 | Diagnostics |
|---|---|---|
| Auto-Sync on | 150 / 150 / 150 ms | median shift −108 ms |
| switch off | −79 / 42 / 181 ms (the LRC stamps) | no row |
| on again | 150 / 150 / 150 ms | −108 ms |

- **Every measured line starts exactly 150 ms before its first word.** Without Auto-Sync, in every tenth line the first word came 79 ms before the line, in another tenth only 181 ms after it.
- **The switch takes effect at once** on the playing song, both ways.
- **Lyrics from Recognize** barely move: their lines start at the first word already (shift 4 ms).

## Limits

- **Only from the second listen on,** and only for songs Align has measured.
- **More than 0.65 s before its stamp** Auto-Sync finds no line (see above). If a whole song is that far off, the song offset helps.
- **Where Align aligns badly,** Auto-Sync is off too. In the English test song every tenth word start is more than 0.8 s off (run a), in the other three under 0.7 s. The 1.5 s limit catches gross errors, but single lines can still jump. Then the switch helps.
- **A song offset after the measurement** moves lines and words together against the singing, as with Align.
