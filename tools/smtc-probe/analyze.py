"""Spike A analysis (PLAN.md 5.8): evaluates CSVs written by smtc-probe.ps1.

Usage:
    python tools/smtc-probe/analyze.py docs/spike-smtc-active.csv docs/spike-smtc-passive.csv

Terms used in the output:
    update      a sample where Spotify reported a new (position, last_updated) pair
    residual    position Spotify reports at an update minus the position extrapolated
                from the previous update; identical to the jump a reader would see in
                its extrapolated position at that moment. This is the sync jitter.
    seek        an update whose residual exceeds SEEK_MS (user or tool seek)
"""

from __future__ import annotations

import csv
import statistics
import sys
from dataclasses import dataclass
from pathlib import Path

SEEK_MS = 1500.0


@dataclass
class Sample:
    mono: float      # ms since probe start (monotonic)
    now: float       # precise system UTC time in ms
    status: str
    pos: float       # ms
    lut: float       # LastUpdatedTime, UTC ms
    end: float
    title: str


@dataclass
class Action:
    mono: float
    now: float
    kind: str
    target: float | None


def pct(values: list[float], p: float) -> float:
    if not values:
        return float("nan")
    s = sorted(values)
    k = max(0, min(len(s) - 1, round(p / 100.0 * (len(s) - 1))))
    return s[k]


def fmt(v: float, unit: str = "ms") -> str:
    return "n/a" if v != v else f"{v:.0f} {unit}"


def load(path: Path) -> tuple[list[Sample], list[Action], int]:
    samples: list[Sample] = []
    actions: list[Action] = []
    no_session = 0
    with path.open(newline="", encoding="utf-8") as fh:
        for row in csv.DictReader(fh):
            ev = row["event"]
            mono = float(row["mono_ms"])
            now = int(row["now_utc_ticks"]) / 10_000.0
            if ev == "sample":
                samples.append(Sample(
                    mono=mono, now=now, status=row["status"],
                    pos=float(row["pos_ms"]), lut=int(row["last_updated_utc_ticks"]) / 10_000.0,
                    end=float(row["end_ms"]), title=row["title"]))
            elif ev.startswith("action_"):
                parts = ev.split(":")
                actions.append(Action(mono=mono, now=now, kind=parts[0],
                                      target=float(parts[1]) if len(parts) > 1 else None))
            elif ev == "no_session":
                no_session += 1
    return samples, actions, no_session


def analyse(path: Path) -> None:
    samples, actions, no_session = load(path)
    print(f"\n## {path.name}\n")
    if not samples:
        print("No samples.")
        return

    duration_s = (samples[-1].mono - samples[0].mono) / 1000.0
    playing = [s for s in samples if s.status == "Playing"]
    intervals = [b.mono - a.mono for a, b in zip(samples, samples[1:])]
    titles = []
    for s in samples:
        if not titles or titles[-1] != s.title:
            titles.append(s.title)

    print(f"- Duration: {duration_s:.0f} s, samples: {len(samples)}, "
          f"sampling interval median {statistics.median(intervals):.1f} ms, p95 {pct(intervals, 95):.1f} ms")
    print(f"- Playing share: {100.0 * len(playing) / len(samples):.0f} %, "
          f"tracks seen: {len(titles)}, no-session samples: {no_session}")

    # Updates = change of (pos, lut)
    updates: list[tuple[Sample, Sample]] = []  # (previous sample, sample with new values)
    for a, b in zip(samples, samples[1:]):
        if (b.pos, b.lut) != (a.pos, a.lut):
            updates.append((a, b))

    # Cadence and residuals only between consecutive updates during continuous playback
    cadence: list[float] = []
    residuals: list[float] = []
    seeks: list[tuple[Sample, float]] = []
    ages: list[float] = []
    prev_upd: Sample | None = None
    for a, b in updates:
        ages.append(b.now - b.lut)
        if prev_upd is not None and prev_upd.status == "Playing" and b.status == "Playing" \
                and prev_upd.title == b.title:
            dt_lut = b.lut - prev_upd.lut
            pred = prev_upd.pos + dt_lut
            r = b.pos - pred
            if abs(r) > SEEK_MS:
                seeks.append((b, r))
            else:
                cadence.append(dt_lut)
                residuals.append(r)
        prev_upd = b

    play_minutes = len(playing) * statistics.median(intervals) / 60000.0
    print(f"- Updates total: {len(updates)}; during continuous playback: {len(cadence)} "
          f"(~{len(cadence) / play_minutes:.1f} per playing minute)" if play_minutes > 0 else
          f"- Updates total: {len(updates)}")
    if cadence:
        print(f"- Time between updates (Spotify timestamps): median {fmt(statistics.median(cadence))}, "
              f"p10 {fmt(pct(cadence, 10))}, p90 {fmt(pct(cadence, 90))}, max {fmt(max(cadence))}")
    if ages:
        print(f"- Age of an update when first seen (now - LastUpdatedTime): median {fmt(statistics.median(ages))}, "
              f"p95 {fmt(pct(ages, 95))}, min {fmt(min(ages))}")
    if residuals:
        absr = [abs(r) for r in residuals]
        back = sum(1 for r in residuals if r < -50)
        print(f"- **Residual / correction jump** (n={len(residuals)}): median {residuals and fmt(statistics.median(residuals))}, "
              f"mean {fmt(statistics.fmean(residuals))}, |p50| {fmt(pct(absr, 50))}, "
              f"**|p95| {fmt(pct(absr, 95))}**, |max| {fmt(max(absr))}, backward jumps > 50 ms: {back}")
    print(f"- Seeks/jumps detected by residual > {SEEK_MS:.0f} ms: {len(seeks)}")

    # Rate: slope of position over Spotify time across long playback stretches
    seg: list[Sample] = []
    slopes: list[tuple[float, float]] = []

    def flush(seg: list[Sample]) -> None:
        if len(seg) >= 3 and seg[-1].lut - seg[0].lut > 20_000:
            xs = [s.lut for s in seg]
            ys = [s.pos for s in seg]
            mx, my = statistics.fmean(xs), statistics.fmean(ys)
            num = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
            den = sum((x - mx) ** 2 for x in xs)
            if den > 0:
                slopes.append((num / den, seg[-1].lut - seg[0].lut))

    last: Sample | None = None
    for _, b in updates:
        cont = last is not None and last.status == "Playing" and b.status == "Playing" \
            and last.title == b.title and abs(b.pos - (last.pos + b.lut - last.lut)) <= SEEK_MS
        if not cont:
            flush(seg)
            seg = []
        if b.status == "Playing":
            seg.append(b)
        last = b
    flush(seg)
    if slopes:
        total = sum(w for _, w in slopes)
        rate = sum(s * w for s, w in slopes) / total
        print(f"- Playback rate (position per Spotify time, {len(slopes)} stretches): "
              f"{rate:.5f} ({(rate - 1) * 1e6:+.0f} ppm)")

    # Paused stretches: does position stay frozen? Only directly consecutive samples count,
    # otherwise positions from two different pauses would be compared.
    paused = sum(1 for s in samples if s.status == "Paused")
    if paused:
        moving = sum(1 for a, b in zip(samples, samples[1:])
                     if a.status == b.status == "Paused" and a.title == b.title and abs(b.pos - a.pos) > 1)
        print(f"- Paused samples: {paused}, position changes while paused: {moving}")

    # Track changes: Spotify publishes the new timeline (position ~0) before the new title,
    # so they show up as large negative residuals with the old title.
    resets = sum(1 for _, r in seeks if r < -SEEK_MS and _.pos < 1000)
    if resets:
        print(f"- Of the jumps, {resets} are position resets to ~0 (track change arriving before the title)")

    # Active-mode latencies
    if actions:
        print("\n### Reaction to SMTC commands (active mode)\n")
        print("| Command | n | Latency median | Latency p95 | Note |")
        print("|---|---|---|---|---|")
        groups: dict[str, list[float]] = {}
        notes: dict[str, list[float]] = {}
        for act in actions:
            after = [s for s in samples if s.mono > act.mono and s.mono < act.mono + 5000]
            hit: Sample | None = None
            if act.kind == "action_seek" and act.target is not None:
                hit = next((s for s in after if abs(s.pos + max(0.0, s.now - s.lut) * (s.status == "Playing")
                                                    - act.target) < 2000), None)
                if hit is not None:
                    # How precisely Spotify landed on the requested position
                    landed = hit.pos + (hit.now - hit.lut if hit.status == "Playing" else 0) - (hit.mono - act.mono)
                    notes.setdefault(act.kind, []).append(landed - act.target)
            elif act.kind == "action_pause":
                hit = next((s for s in after if s.status == "Paused"), None)
            elif act.kind == "action_play":
                hit = next((s for s in after if s.status == "Playing"), None)
            if hit is not None:
                groups.setdefault(act.kind, []).append(hit.mono - act.mono)
            else:
                groups.setdefault(act.kind, [])
        for kind, lat in groups.items():
            note = ""
            if kind in notes and notes[kind]:
                note = f"landing error vs target median {statistics.median(notes[kind]):.0f} ms"
            missed = sum(1 for a in actions if a.kind == kind) - len(lat)
            if missed:
                note += f"{'; ' if note else ''}{missed} without visible reaction within 5 s"
            print(f"| {kind.replace('action_', '')} | {len(lat)} | {fmt(statistics.median(lat)) if lat else 'n/a'} | "
                  f"{fmt(pct(lat, 95)) if lat else 'n/a'} | {note} |")


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__)
        return 2
    print("# SMTC probe analysis")
    for arg in argv[1:]:
        analyse(Path(arg))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
