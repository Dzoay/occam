// The mouse moves the camera: drag to orbit, drag with the right button or shift to pan, and the wheel to zoom.
// While a button is down the frame loop keeps submitting, whether or not the pointer moves.

import type { Viewer } from "./viewer.ts";

/** Radians a pixel of drag turns the view. */
const ORBIT_RATE = 0.008;
/** The wheel's zoom: each unit of `deltaY` scales the distance by `exp` of this. */
const ZOOM_RATE = 0.001;

export function attach(canvas: HTMLCanvasElement, viewer: Viewer): void {
  let last: { x: number; y: number; pan: boolean } | null = null;
  canvas.addEventListener("pointerdown", (event) => {
    if (last) return;
    canvas.setPointerCapture(event.pointerId);
    last = { x: event.clientX, y: event.clientY, pan: event.button !== 0 || event.shiftKey };
    viewer.loop.hold();
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!last) return;
    const [dx, dy] = [event.clientX - last.x, event.clientY - last.y];
    last = { ...last, x: event.clientX, y: event.clientY };
    const height = Math.max(1, canvas.clientHeight);
    if (last.pan) viewer.pan(-dx / height, dy / height);
    else viewer.orbit(-dx * ORBIT_RATE, dy * ORBIT_RATE);
  });
  const end = () => {
    if (!last) return;
    last = null;
    viewer.loop.release();
  };
  canvas.addEventListener("pointerup", end);
  canvas.addEventListener("pointercancel", end);
  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    viewer.zoom(Math.exp(event.deltaY * ZOOM_RATE));
  }, { passive: false });
  canvas.addEventListener("contextmenu", (event) => event.preventDefault());
}
