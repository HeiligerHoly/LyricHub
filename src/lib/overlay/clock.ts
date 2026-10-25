// Local playback clock (PLAN.md 3.4, 5.3). The backend sends an anchor only on changes and
// every 2 s; the overlay extrapolates with performance.now(), so IPC jitter never reaches the
// display. The model mirrors sync/clock.rs: `rate` until the slew ends, then 1.0.
//
// A new anchor that differs from the local prediction by less than 300 ms is blended in over
// 300 ms instead of jumping, so the fill never steps back for small corrections.
import type { ClockEvent } from "$lib/shared/types";

const BLEND_MS = 300;

export class LocalClock {
  private pos = 0;
  private at = 0;
  private playing = false;
  private rate = 1;
  private slewUntil = 0;
  private blendDiff = 0;
  private blendStart = 0;
  /** User offset; positive shows the lyrics later. */
  offsetMs = 0;
  seq = -1;

  apply(e: ClockEvent, now = performance.now()): void {
    const before = this.raw(now);
    const wasPlaying = this.playing;
    this.pos = e.pos_ms;
    this.at = now;
    this.playing = e.playing;
    this.rate = e.rate;
    this.slewUntil = now + e.slew_ms;
    this.offsetMs = e.offset_ms;
    this.seq = e.seq;
    const diff = e.pos_ms - before;
    if (wasPlaying && e.playing && Math.abs(diff) > 0.5 && Math.abs(diff) < BLEND_MS) {
      this.blendDiff = diff;
      this.blendStart = now;
    } else {
      this.blendDiff = 0;
    }
  }

  get isPlaying(): boolean {
    return this.playing;
  }

  /** Raw song position (no offsets). */
  raw(t = performance.now()): number {
    return this.base(t) - this.blend(t);
  }

  /** Position the lyrics are shown at. */
  display(t = performance.now()): number {
    return this.raw(t) - this.offsetMs;
  }

  /** Song milliseconds per real millisecond at `t`; 0 while paused. */
  speed(t = performance.now()): number {
    if (!this.playing) return 0;
    let s = t < this.slewUntil ? this.rate : 1;
    if (this.blendDiff && t < this.blendStart + BLEND_MS) s += this.blendDiff / BLEND_MS;
    return s;
  }

  /** Next moment the speed changes, or Infinity. */
  nextChange(t = performance.now()): number {
    if (!this.playing) return Infinity;
    let next = Infinity;
    if (this.slewUntil > t) next = this.slewUntil;
    const blendEnd = this.blendStart + BLEND_MS;
    if (this.blendDiff && blendEnd > t) next = Math.min(next, blendEnd);
    return next;
  }

  /** Real milliseconds until the display position reaches `ms`, or Infinity. */
  timeUntil(ms: number, t = performance.now()): number {
    const speed = this.speed(t);
    if (speed <= 0) return Infinity;
    return Math.max(0, (ms - this.display(t)) / speed);
  }

  private base(t: number): number {
    if (!this.playing) return this.pos;
    const dt = t - this.at;
    const slew = Math.max(0, this.slewUntil - this.at);
    if (dt <= slew) return this.pos + dt * this.rate;
    return this.pos + slew * this.rate + (dt - slew);
  }

  private blend(t: number): number {
    if (!this.blendDiff) return 0;
    const k = 1 - (t - this.blendStart) / BLEND_MS;
    return k > 0 ? this.blendDiff * k : 0;
  }
}
