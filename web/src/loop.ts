// The frame loop, outside the component tree. It submits a frame every animation frame while the person interacts,
// whether or not anything moved: after idleness Firefox reported finished GPU work about 100 ms late in P12, so
// input must never wait on it. It also runs while uploads are pending and for a moment after the last change, then
// rests until woken.

/** How long the loop keeps going after the last thing that woke it, in milliseconds. */
export const LINGER = 500;

export class FrameLoop {
  /** Frames submitted. */
  frames = 0;
  private holds = 0;
  private until = 0;
  private running = false;
  /** Draws one frame, and says whether work is still pending. */
  private readonly step: (frame: number) => boolean;

  constructor(step: (frame: number) => boolean) {
    this.step = step;
  }

  /** Keeps the loop going for at least `LINGER` from now. */
  wake(): void {
    this.until = Math.max(this.until, performance.now() + LINGER);
    this.start();
  }

  /** Keeps the loop going until as many `release`s, as while a pointer is held down. */
  hold(): void {
    this.holds += 1;
    this.wake();
  }

  release(): void {
    this.holds = Math.max(0, this.holds - 1);
    this.wake();
  }

  private start(): void {
    if (this.running) return;
    this.running = true;
    requestAnimationFrame((now) => this.tick(now));
  }

  private tick(now: number): void {
    this.frames += 1;
    const pending = this.step(this.frames);
    if (pending || this.holds > 0 || now < this.until) {
      requestAnimationFrame((next) => this.tick(next));
    } else {
      this.running = false;
    }
  }
}
