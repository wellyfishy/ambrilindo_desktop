/**
 * Venue name from the Kop Surat "Alamat venue": its first line, or the text before the
 * first comma when everything is on one line ("GOR Segiri, Samarinda" -> "GOR Segiri").
 * Same rule as the results booklet (db.rs get_results_booklet_data).
 */
export function venueFromAlamat(alamat: string | null | undefined): string {
  const lines = (alamat || "")
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  if (lines.length === 0) return "";
  if (lines.length === 1 && lines[0].includes(",")) return lines[0].split(",")[0].trim();
  return lines[0];
}
