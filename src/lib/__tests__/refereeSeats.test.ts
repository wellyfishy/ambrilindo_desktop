import { describe, expect, it } from "vitest";
import { matchContingentLogo, matchPhoto, nameScore } from "$lib/refereeSeats";

const PORPROV = [
  "KOTA SAMARINDA", "KOTA BALIKPAPAN", "KABUPATEN KUTAI KARTANEGARA", "KABUPATEN PENAJAM PASER UTARA",
  "KABUPATEN BERAU", "KABUPATEN MAHAKAM ULU", "KOTA BONTANG", "KABUPATEN KUTAI BARAT", "KABUPATEN KUTAI TIMUR",
  "KABUPATEN PASER",
].map((nama_utusan, id) => ({ id, nama_utusan }));
const name = (id: number | null) => (id === null ? null : PORPROV[id].nama_utusan);

describe("contingent logo matching", () => {
  it.each([
    ["logo kabupaten berau.webp", "KABUPATEN BERAU"],
    ["logo kota samarinda.webp", "KOTA SAMARINDA"],
    ["logo kabupaten penajam paser utara.webp", "KABUPATEN PENAJAM PASER UTARA"],
    ["Logo Kab. Kutai Timur.png", "KABUPATEN KUTAI TIMUR"],
    ["kab paser.png", "KABUPATEN PASER"],
    ["berau.png", "KABUPATEN BERAU"],
    ["LOGO_KOTA_BONTANG.jpg", "KOTA BONTANG"],
  ])("%s → %s", (file, expected) => {
    expect(name(matchContingentLogo(file, PORPROV).wasitId)).toBe(expected);
  });

  it("paser is not mistaken for penajam paser utara", () => {
    expect(name(matchContingentLogo("logo kabupaten paser.webp", PORPROV).wasitId)).toBe("KABUPATEN PASER");
  });

  it("kota X and kabupaten X are different contingents", () => {
    const both = [{ id: 1, nama_utusan: "KOTA KEDIRI" }, { id: 2, nama_utusan: "KABUPATEN KEDIRI" }];
    expect(matchContingentLogo("logo kota kediri.png", both).wasitId).toBe(1);
    expect(matchContingentLogo("logo kab kediri.png", both).wasitId).toBe(2);
    // without kota/kab it is ambiguous: the operator picks
    const m = matchContingentLogo("kediri.png", both);
    expect(m.wasitId).toBeNull();
    expect(m.ambiguous).toBe(true);
  });

  it("unknown abbreviations are left for the operator", () => {
    expect(matchContingentLogo("kukar.png", PORPROV).wasitId).toBeNull();
    expect(matchContingentLogo("ppu.jpg", PORPROV).wasitId).toBeNull();
  });
});

describe("referee photo matching", () => {
  const wasits = [
    { id: 1, nama_wasit: "Surya Dharma" },
    { id: 2, nama_wasit: "Samchizal Huzain" },
    { id: 3, nama_wasit: "Abdul Latif" },
    { id: 4, nama_wasit: "Abdul Latif Hakim" },
  ];
  it("exact, prefix and one-typo names match", () => {
    expect(matchPhoto("surya dharma.jpg", wasits).wasitId).toBe(1);
    expect(matchPhoto("samchizal.jpeg", wasits).wasitId).toBe(2);
    expect(matchPhoto("Abdul Latif.png", wasits).wasitId).toBe(3);
  });
  it("unrelated names do not match", () => {
    expect(matchPhoto("wasit-placeholder.jpeg", wasits).wasitId).toBeNull();
    expect(nameScore("budi", "surya dharma")).toBeLessThan(0.8);
  });
});
