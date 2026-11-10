/**
 * A steady tick for match timers that keeps running when the window is hidden,
 * minimized or covered by another window.
 *
 * requestAnimationFrame stops in a hidden or occluded window (WebView2 too), and plain
 * timers in a hidden page are slowed to once a second or less. A Web Worker's timer is
 * not throttled, so the worker drives the tick; plain setInterval is the fallback where
 * workers are unavailable. Callers still measure elapsed time with performance.now(),
 * so a late tick never makes the clock drift.
 */
export interface Ticker {
  stop(): void;
}

const WORKER_SOURCE = `
let id = null;
onmessage = (e) => {
  if (id) clearInterval(id);
  id = null;
  if (e.data > 0) id = setInterval(() => postMessage(0), e.data);
};`;

export function startTicker(onTick: () => void, intervalMs = 40): Ticker {
  if (typeof Worker !== "undefined" && typeof Blob !== "undefined" && typeof URL !== "undefined") {
    try {
      const url = URL.createObjectURL(new Blob([WORKER_SOURCE], { type: "text/javascript" }));
      const worker = new Worker(url);
      URL.revokeObjectURL(url);
      worker.onmessage = () => onTick();
      worker.postMessage(intervalMs);
      return {
        stop() {
          worker.postMessage(0);
          worker.terminate();
        },
      };
    } catch {
      // blocked by a content policy: fall back below
    }
  }
  const id = setInterval(onTick, intervalMs);
  return { stop: () => clearInterval(id) };
}
