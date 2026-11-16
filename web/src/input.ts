// The mouse moves the camera: drag to orbit, drag with the right button or shift to pan, and the wheel to zoom.
// While a button is down the frame loop keeps submitting, whether or not the pointer moves. A click, the left button
// let go where it went down, picks: it selects what is under it, and with shift adds or removes it.

import type { Viewer } from "./viewer.ts";

/** Radians a pixel of drag turns the view. */
const ORBIT_RATE = 0.008;
/** The wheel's zoom: each unit of `deltaY` scales the distance by `exp` of this. */
const ZOOM_RATE = 0.001;
/** How far the pointer may move, in pixels, between going down and coming up, for a click. */
const CLICK = 3;

/** What a click does: at the point `(x, y)` of the window, adding to the selection if `add`. */
export type Click = (x: number, y: number, add: boolean) => void;

export function attach(canvas: HTMLCanvasElement, viewer: Viewer, click: Click | null = null): void {
  let last: { x: number; y: number; pan: boolean; button: number; moved: number } | null = null;
  canvas.addEventListener("pointerdown", (event) => {
    if (last) return;
    canvas.setPointerCapture(event.pointerId);
    const pan = event.button !== 0 || event.shiftKey;
    last = { x: event.clientX, y: event.clientY, pan, button: event.button, moved: 0 };
    viewer.loop.hold();
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!last) return;
    const [dx, dy] = [event.clientX - last.x, event.clientY - last.y];
    last = { ...last, x: event.clientX, y: event.clientY, moved: last.moved + Math.abs(dx) + Math.abs(dy) };
    if (last.moved <= CLICK) return;
    const height = Math.max(1, canvas.clientHeight);
    if (last.pan) viewer.pan(-dx / height, dy / height);
    else viewer.orbit(-dx * ORBIT_RATE, dy * ORBIT_RATE);
  });
  const end = (event: PointerEvent) => {
    if (!last) return;
    const clicked = event.type === "pointerup" && last.button === 0 && last.moved <= CLICK;
    last = null;
    viewer.loop.release();
    if (clicked) click?.(event.clientX, event.clientY, event.shiftKey);
  };
  canvas.addEventListener("pointerup", end);
  canvas.addEventListener("pointercancel", end);
  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    viewer.zoom(Math.exp(event.deltaY * ZOOM_RATE));
  }, { passive: false });
  canvas.addEventListener("contextmenu", (event) => event.preventDefault());
}
