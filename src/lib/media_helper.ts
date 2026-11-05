import { invoke } from "$lib/api_client";

const memoryLogoCache: Record<string, string> = {};

/**
 * Resolves a list of relative media paths to base64 Data URLs via Tauri IPC.
 * Results are cached in memory so subsequent views don't re-read from disk.
 */
export async function resolveMediaLogos(
  paths: (string | null | undefined)[],
  hostAddress?: string
): Promise<Record<string, string>> {
  const needed = new Set<string>();
  for (const p of paths) {
    if (p && typeof p === "string" && !memoryLogoCache[p]) {
      needed.add(p);
    }
  }

  for (const path of needed) {
    if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("data:")) {
      memoryLogoCache[path] = path;
      continue;
    }

    try {
      const dataUrl = await invoke<string | null>("get_media_data_url", {
        relativePath: path,
        relative_path: path,
      });
      if (dataUrl) {
        memoryLogoCache[path] = dataUrl;
        continue;
      }
    } catch (_) {
      // IPC not available (e.g. running in browser dev or client mode)
    }

    if (hostAddress) {
      memoryLogoCache[path] = `${hostAddress.replace(/\/$/, "")}/media/${path.replace(/^\//, "")}`;
    }
  }

  return { ...memoryLogoCache };
}
