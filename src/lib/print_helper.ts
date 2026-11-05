/**
 * Utility to reliably enforce print orientation and margins in Chromium/WebView2.
 * Directly injects a top-level @page rule in document.head before calling window.print().
 */
export function printWithOrientation(
  orientation: "portrait" | "landscape" = "portrait",
  margin: string = "8mm"
) {
  if (typeof document !== "undefined") {
    let styleEl = document.getElementById("print-page-orientation-style") as HTMLStyleElement | null;
    if (!styleEl) {
      styleEl = document.createElement("style");
      styleEl.id = "print-page-orientation-style";
      document.head.appendChild(styleEl);
    }
    styleEl.textContent = `@page { size: A4 ${orientation}; margin: ${margin}; }`;
  }
  window.print();
}
