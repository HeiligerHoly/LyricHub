# Estimated word times – measurement and new model

> Date: 2026-10-28 · after v1.0.1 · Reference: PLAN.en.md 5.6, `src-tauri/src/lyrics/words.rs`, `parser.rs`
> Result: **The median distance between an estimated and a sung word start fell from 184 to 136 ms, and the slowest tenth from 1.28 to 0.73 s.**

## Starting point

LRCLIB almost only has line times (0 of 160 hits had word times, phase 4). For the word-by-word fill LyricHub therefore estimates the words itself. Up to v1.0.1 each word got a share of the line by its letters, and the line was filled up to the next one. If a pause of more than 2.5 s beyond the estimated singing time followed, 70 ms per letter applied.

## Data and method

- **Dataset:** JamendoLyrics MultiLang (Durand, Stoller, Ewert, ICASSP 2023), 79 songs with annotations, about 20 each in German, English, French and Spanish, 21,580 words on 3,383 lines. The word times were aligned by hand. The annotations are under MIT, the lyrics under each song's Creative Commons licence. The dataset is not in the repository, and the measurement prints no lyrics.
- **Simulation:** Each annotated line becomes an LRC line stamped at its first word. The next line's stamp ends it, as in the parser. For every word the distance between the estimated and the annotated start counts.
- **Tuning and checking:** The parameters were chosen on every other song. The other half was used only to check.
- **Re-running:** `cargo test --lib measure_word_timing -- --ignored --nocapture` in `src-tauri`, with the dataset folder in `LYRICHUB_WORD_TIMES` (folders `annotations/words` and `lyrics` from github.com/f90/jamendolyrics).

## What caused the error

| Case | Share of words | Median error | p90 error |
|---|---|---|---|
| Fill up to the next line | 71 % | 159 ms (mostly late) | 604 ms |
| Long pause after, 70 ms per letter | 29 % | 417 ms (mostly early) | 2,431 ms |

Before long pauses the singing takes 2.3 times as long as 70 ms per letter assume (median). The words then light up far too early. Up to the next line, singers use 87–92 % of the interval (median), not 100 %. Only intervals over 7 s typically end in a pause. Swapping letters for syllables alone changed almost nothing (median 185 → 191 ms).

## New model

- **Singing time of a line** (`parser::fill_end`): 90 % of the interval to the next line, but at most 400 ms per syllable, at least 1 s, and at most 8 s without a next line. After that the line stays filled.
- **Share of a word** (`words::estimate`): syllables plus 0.2 per letter. The last word counts 1.3 times, because it is held. The line is spread from its stamp to the end of the singing time, without lead-in or tail.
- **Syllables:** groups of consecutive vowels (Latin with accents, and Cyrillic), at least one per word. Hangul counts one per syllable block. Kana count 1 and kanji 1.5, as before. A rule for the silent "e" (English, French) gained 1 ms and was left out.

## Result

Measured with the Rust implementation, all 79 songs:

| | up to v1.0.1 | new |
|---|---|---|
| Median | 184 ms | 136 ms |
| Mean | 461 ms | 283 ms |
| p90 | 1,282 ms | 728 ms |
| at most 150 ms off | 44.7 % | 52.5 % |

Only the check half, which was not used for tuning: median 184 → 137 ms, mean 493 → 288 ms, p90 1,466 → 747 ms.

By language (median / p90):

| Language | up to v1.0.1 | new |
|---|---|---|
| German | 172 / 1,176 ms | 111 / 652 ms |
| English | 188 / 1,255 ms | 156 / 783 ms |
| French | 183 / 1,132 ms | 152 / 761 ms |
| Spanish | 193 / 1,549 ms | 131 / 687 ms |

The figures by language and for the halves come from a Python replica that matches the Rust implementation's totals within a few milliseconds.

## What this measurement does not show

- **LRCLIB time stamps:** The simulation puts every stamp exactly on the first word. Real LRC files place their lines earlier or later per song, with a per-song median between −140 and +151 ms ([sync report](sync-report.md)). With an offset of ±150 ms the median is 157–205 ms (tuning half, Python replica), so stamps 150 ms late end up above the old value without an offset. Mean (319–332 ms) and p90 (712–727 ms) stay well below the old values even then. The song offset corrects a per-song shift.
- **Japanese and Korean:** The dataset has no songs in these languages. Kana, kanji and Hangul keep their weights; only the held last word and the new singing time apply to them too.
- **Feel:** The measurement counts word starts. It does not show how smooth the fill looks. Anyone who still finds it too inaccurate can turn it off under Appearance → "Fill word by word".
