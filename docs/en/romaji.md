# Romaji for Japanese lines

*English translation of [romaji.md](../romaji.md).*

> Date: 2026-10-25 · Reference: [PLAN.md](../../PLAN.en.md) 13.6, request from the listening test ([phase2.md](phase2.md)), PLAN.md 2 and 14.2
> Result: **Romanization in the backend** with lindera 6.2 and embedded IPADIC, own Hepburn rules in Rust. Switch `appearance.romaji` (default off), Ctrl+Alt+R.

## Decision: backend instead of frontend

Measured on the user's PC (Windows 11, 16 cores), release build for lindera, Node 24 for kuromoji. All test sentences are invented.

| Criterion | **lindera 6.2 + IPADIC (Rust)** | kuroshiro 1.2.1 + kuromoji 0.1.2 (frontend) | lindera + UniDic 2.1.2 | lindera + SudachiDict |
|---|---|---|---|---|
| License, code | MIT | MIT, Apache-2.0 | MIT | MIT |
| License, dictionary | IPADIC (NAIST/ICOT, BSD-like, see below) | IPADIC (the same file) | BSD-3-Clause | Apache-2.0 |
| Size | Binary +45 MB (embedded uncompressed), about 7 MB compressed (xz) | 17 MB dictionary (gzip) in the assets | Binary +195 MB | 200 MB for the download alone |
| Private memory | **+0.3 MB**; the dictionary stays in the mapped image, the working set grows by 2 MB after 200 lines | **+310 MB RSS** (ArrayBuffers 222 MB, heap 84 MB), and that in the overlay WebView | not measured | not measured |
| Startup time | Dictionary 0.7 ms, 200 lines 2.3 ms | Dictionary 272 ms (local file, without `fetch`) | 1 ms | not measured |
| Readings | Reading and pronunciation separate: は/へ/を, long vowels and "ei" can be told apart cleanly | The same IPADIC, but the output has errors: "tsu te" for って, "kikoe ta", "ki mashi ta" | Only the pronunciation of the surface form: 先生 → "sensē", お母さん as お+母+さん → "ohahasan" | No pronunciation, only reading |

Rationale:

1. **Memory decides.** PLAN.md 2 allows under 200 MB private for the whole app. kuromoji alone needs more, and in the overlay process at that, which runs permanently. lindera 6 embeds the dictionary uncompressed and reads it directly from the image. These pages are file-backed and shareable, not private memory, and only what is needed gets loaded.
2. **Once per song, not per frame.** The backend does the work when the lyrics of a song arrive. The result is kept with the lyrics of the current song (`app_state`), and the overlay receives finished strings with the `lyrics` event. Choruses are converted once. No JavaScript and no IPC per frame.
3. **Quality.** Only IPADIC provides reading and pronunciation separately, and Hepburn needs both: long vowels (トーキョー) and the particles (ワ for は) are in the pronunciation, "sensei" and "iu" (言う) in the reading. kuroshiro uses the same dictionary, but joins the words incorrectly. The own rules are described and tested below.
4. **Testable like the rest of the pipeline.** Rust unit tests with `cargo test`; the frontend has no test runner.
5. **Startup time.** The dictionary loads only with the first Japanese song while the switch is on, in under 1 ms. App startup stays unchanged.

### IPADIC license (deviation from PLAN.md 14.2, for your information)

PLAN.md 14.2 names MIT, Apache-2.0 and BSD. IPADIC is under its own BSD-like license from NAIST with the terms of the ICOT Free Software: use, modification and redistribution are free. The only condition: the warranty disclaimer must be included. kuromoji.js (Apache-2.0) and the Kuromoji analyzer of Apache Lucene ship the same file. A dictionary with kanji readings under MIT, Apache-2.0 or BSD only exists in large sizes: UniDic (BSD-3) needs four times the space and reads worse for Hepburn; SudachiDict (Apache-2.0) is even larger and has no pronunciation. The notice text is in `THIRD_PARTY_NOTICES.md`, which the installer puts next to `lyrichub.exe`.

All Rust dependencies that lindera brings in at runtime are under MIT, Apache-2.0, BSD-3-Clause or Unlicense/MIT. Crates with an ISC or CDLA license (`ring`, `rustls-webpki`, `webpki-roots`) are only needed by the build script for the download; they do not end up in the binary.

### Build

On the first build, the build script of `lindera-ipadic` downloads `mecab-ipadic-2.7.0-20250920.tar.gz` once from `lindera.dev` (MD5 checked) and builds the dictionary from it, in about 45 s. After that, it is in `target/`. For builds without network access, `LINDERA_BUILD_DICTIONARY_CACHE_DIR` can point to a folder with the finished dictionary.

## Implementation

| Part | File |
|---|---|
| Segmentation, word building, kana → Hepburn, tests | `src-tauri/src/lyrics/romaji.rs` |
| `romaji` per line (`null` while off) | `src-tauri/src/lyrics/types.rs`, `src/lib/shared/types.ts` |
| Once per song when the lyrics arrive; when switched on, for the current song, before the `settings` event | `src-tauri/src/app_state.rs` (`add_romaji`, `romaji_changed`), `ipc/mod.rs` |
| Setting `appearance.romaji`, default off, without a schema change | `src-tauri/src/settings/schema.rs` |
| Ctrl+Alt+R | `hotkeys.rs`, `ipc::toggle_romaji` |
| Display: `shownText()` for the current and the next line (also in the "Bar" („Leiste“) layout); the width per song measures the displayed text; "Romaji on/off" („Romaji an/aus“) notice in the overlay | `src/lib/overlay/` |
| Switch "Japanese lines: show in romaji (Hepburn)" („Japanische Zeilen: in Romaji (Hepburn) zeigen“) | Settings → Overlay |

Karaoke fill and line timing stay unchanged: the line only gets a different text. The timestamps still come from `start_ms` and `fill_end_ms`, and the fill stays a `transform` on the same animation. If the switch is toggled during a line, the fill continues at the same position, and the line refits its font size.

### Rules (modified Hepburn)

- **Detection** like `langOf`: lines with hiragana, katakana, kanji or half-width katakana. Other lines stay as they are. So do Latin words in Japanese lines.
- **Particles** は, へ, を → wa, e, o, also in set phrases (こんにちは → konnichiwa). The source is the pronunciation.
- **Long vowels** with a macron where IPADIC lists them as ー in the pronunciation: Tōkyō, arigatō, okāsan, kōhī, sūpā. "ei" stays ei (sensei), "ii" stays ii (chiisai, oniisan). Nothing is lengthened across morpheme boundaries: 思う stays omou. The volitional う after o becomes ō: ikō, deshō, darō.
- **っ** doubles the following consonant (hashitte, gakkō); before ch it becomes t (itchau). At the end of a word it is dropped.
- **ん** before a vowel or y becomes n' (kin'en, kon'ya); before b, m, p it stays n (shinbun).
- **Words**: conjugation endings and auxiliary verbs attach to the verb (kimashita, hashitte, matteru, iwanaide, tsunaida). Particles, the copula (da, desu, datta) and auxiliary verbs such as いる and しまう stand alone (kimi no, suki da, hashitte iru). The prefix お binds to the next word, suffixes such as ら and さん to the previous one (bokura, okāsan).
- **Capitalization** only for proper names from the dictionary (Tōkyō); otherwise lowercase.
- **Characters**: NFKC makes half-width katakana full-width and full-width ASCII narrow. 、 → ",", 。 → ".", 「」 → quotation marks, ・ separates words.
- **Unknown kanji** without a reading stay as kanji. That is better than wrong romaji.

### Limitations

- IPADIC picks readings by cost within the sentence. Ambiguous kanji can be read incorrectly (次の行 → "tsugi no kudari" instead of "gyō"), and so can names.
- Song lyrics sometimes write a reading above the kanji that is not in the dictionary, such as 本気 as "maji" or 運命 as "sadame". LRC files do not contain this reading.
- Spaces between words follow fixed rules. Spellings like "suki nan da" versus "suki nanda" are a matter of taste.

## Verification

| Criterion | Result | Evidence |
|---|---|---|
| Unit tests of the conversion, invented sentences only | 8 tests (kana table, reading/pronunciation, particles, long vowels, verb endings, mixed script, widths, once per song), plus 1 settings test | `cargo test`: 73 green |
| `npm run check` | 0 errors, 0 warnings | |
| Switch live | **met**: user at the PC, "Only Wonder" (frederic). Ctrl+Alt+R: 38 of 41 lines in romaji, 12.5 ms after the key press, including dictionary loading, in the debug build. Off and on again without recomputation. With the setting saved, at the next start immediately with the song (19 ms). The fill kept running normally. | Log `romaji toggled`, `romaji ready` |
| CPU with the fill running ≤ 2 % of all cores | **met**: 8.5 % of one core = **0.53 % of 16 cores**, romaji on, Japanese song, 60 s (Phase 2: 0.59 %) | `measure-cpu.ps1 -Seconds 60` |
| Private memory | LyricHub process 9.8 MB private, dictionary included. With all WebView2 processes 195 MB private (GPU process 89 MB), working set 480 MB. Romaji's share is about 0.3 MB (measured on the prototype); even so, the total is only just under the 200 MB limit: a finding for Phase 5. | `Get-Process`, process tree as in `measure-cpu.ps1` |

User request from the live test: the preview line shows only the romaji of the next line. The first version appended the original in small print after it.
