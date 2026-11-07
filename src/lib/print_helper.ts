/**
 * Utility to reliably enforce print orientation and margins in Chromium/WebView2.
 * Supports:
 * - "combined": Named pages mode supporting both A4 portrait and A4 landscape sheets in the same print job.
 * - "portrait": Enforces all pages to A4 portrait.
 * - "landscape": Enforces all pages to A4 landscape.
 */
export function printWithOrientation(
  orientation: "combined" | "portrait" | "landscape" = "combined",
  margin: string = "8mm"
) {
  if (typeof document !== "undefined") {
    let styleEl = document.getElementById("print-page-orientation-style") as HTMLStyleElement | null;
    if (!styleEl) {
      styleEl = document.createElement("style");
      styleEl.id = "print-page-orientation-style";
      document.head.appendChild(styleEl);
    }

    if (orientation === "combined") {
      styleEl.textContent = `
        @page {
          margin: 0;
        }
        @page portrait-sheet {
          size: A4 portrait;
          margin: ${margin};
        }
        @page landscape-sheet {
          size: A4 landscape;
          margin: 6mm;
        }
        /* Results booklet cover: 16:9 like its source design (297 x 167 mm). */
        @page cover-sheet {
          size: 297mm 167.0625mm;
          margin: 0;
        }
        .booklet-page-cover {
          page: cover-sheet !important;
          break-after: page !important;
          page-break-after: always !important;
          height: 167mm !important;
          max-height: 167mm !important;
          overflow: hidden !important;
        }
        .booklet-page-portrait {
          page: portrait-sheet !important;
          break-after: page !important;
          page-break-after: always !important;
        }
        .booklet-page-landscape {
          page: landscape-sheet !important;
          break-after: page !important;
          page-break-after: always !important;
        }
      `;
    } else {
      styleEl.textContent = `@page { size: A4 ${orientation}; margin: ${margin}; }`;
    }
  }
  window.print();
}
