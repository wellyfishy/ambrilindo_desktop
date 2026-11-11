<script lang="ts">
  // Bulk referee photos: drop files or a whole folder, each photo is matched to a referee
  // by its file name, converted to WebP (≤ 600 px) and uploaded to the host. Original files
  // on disk are never touched; the app keeps only the WebP.
  import { onDestroy } from "svelte";
  import { apiCall } from "$lib/api_client";
  import { matchPhoto, photoToWebp } from "$lib/refereeSeats";
  import type { WasitItem } from "$lib/types";

  interface Props {
    wasits: WasitItem[];
    onClose: () => void;
    onDone: (saved: number) => void;
  }
  let { wasits, onClose, onDone }: Props = $props();

  interface Item {
    key: string;
    file: File;
    url: string;
    wasitId: number | null;
    auto: boolean;
    ambiguous: boolean;
    status: "siap" | "perlu" | "duplikat" | "lewati" | "menyimpan" | "tersimpan" | "gagal";
    error?: string;
  }

  // one row per referee (get_wasits returns one row per tatami assignment)
  let roster = $derived([...new Map(wasits.map((w) => [w.id, w])).values()].sort((a, b) => a.nama_wasit.localeCompare(b.nama_wasit)));
  let items = $state<Item[]>([]);
  let dragOver = $state(false);
  let saving = $state(false);
  let savedCount = $state(0);

  const IMG = /\.(jpe?g|png|webp|gif|bmp|avif)$/i;

  function recompute() {
    // a referee gets one photo: the best (first exact) match wins, others are duplicates
    const owner = new Map<number, string>();
    for (const it of items) {
      if (it.status === "tersimpan" || it.status === "lewati") continue;
      if (!it.wasitId) {
        it.status = "perlu";
        continue;
      }
      if (owner.has(it.wasitId)) {
        it.status = "duplikat";
      } else {
        owner.set(it.wasitId, it.key);
        it.status = "siap";
      }
    }
    items = [...items];
  }

  function addFiles(files: File[]) {
    const fresh: Item[] = [];
    for (const f of files) {
      if (!IMG.test(f.name) && !f.type.startsWith("image/")) continue;
      const key = `${f.name}|${f.size}|${f.lastModified}`;
      if (items.some((i) => i.key === key)) continue;
      const m = matchPhoto(f.name, roster);
      fresh.push({ key, file: f, url: URL.createObjectURL(f), wasitId: m.wasitId, auto: !!m.wasitId, ambiguous: m.ambiguous, status: "perlu" });
    }
    // exact name matches first, so they win over looser duplicates
    items = [...items, ...fresh].sort((a, b) => {
      const sa = a.wasitId ? matchPhoto(a.file.name, roster).score : 0;
      const sb = b.wasitId ? matchPhoto(b.file.name, roster).score : 0;
      return sb - sa;
    });
    recompute();
  }

  async function filesFromDrop(dt: DataTransfer): Promise<File[]> {
    const out: File[] = [];
    const entries = [...dt.items].map((i) => i.webkitGetAsEntry?.()).filter(Boolean) as FileSystemEntry[];
    if (entries.length === 0) return [...dt.files];
    async function walk(e: FileSystemEntry) {
      if (e.isFile) {
        out.push(await new Promise<File>((res, rej) => (e as FileSystemFileEntry).file(res, rej)));
      } else if (e.isDirectory) {
        const reader = (e as FileSystemDirectoryEntry).createReader();
        let batch: FileSystemEntry[];
        do {
          batch = await new Promise((res, rej) => reader.readEntries(res, rej));
          for (const c of batch) await walk(c);
        } while (batch.length > 0);
      }
    }
    for (const e of entries) await walk(e);
    return out;
  }

  function setMatch(it: Item, value: string) {
    it.wasitId = value ? Number(value) : null;
    it.auto = false;
    if (it.status === "lewati") it.status = "perlu";
    recompute();
  }

  function skip(it: Item) {
    it.status = it.status === "lewati" ? "perlu" : "lewati";
    recompute();
  }

  let ready = $derived(items.filter((i) => i.status === "siap"));
  let needs = $derived(items.filter((i) => i.status === "perlu").length);
  let dupes = $derived(items.filter((i) => i.status === "duplikat").length);
  let withoutPhoto = $derived(roster.filter((w) => !w.foto && !items.some((i) => i.wasitId === w.id && (i.status === "siap" || i.status === "tersimpan"))));

  async function saveAll() {
    saving = true;
    for (const it of ready) {
      it.status = "menyimpan";
      items = [...items];
      try {
        const { dataUrl } = await photoToWebp(it.file);
        await apiCall("save_wasit_photo", { wasitId: it.wasitId, base64Data: dataUrl });
        it.status = "tersimpan";
        savedCount += 1;
      } catch (e: any) {
        it.status = "gagal";
        it.error = typeof e === "string" ? e : e?.message || String(e);
      }
      items = [...items];
    }
    saving = false;
    if (items.every((i) => i.status !== "gagal")) onDone(savedCount);
  }

  onDestroy(() => {
    for (const it of items) URL.revokeObjectURL(it.url);
  });

  const STATUS: Record<Item["status"], string> = {
    siap: "Cocok",
    perlu: "Pilih wasit",
    duplikat: "Ganda",
    lewati: "Dilewati",
    menyimpan: "Menyimpan…",
    tersimpan: "Tersimpan",
    gagal: "Gagal",
  };
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs" role="dialog" aria-modal="true">
  <div class="bg-[#181a20] border border-white/10 rounded-2xl shadow-2xl overflow-hidden w-full max-w-5xl max-h-[92vh] flex flex-col">
    <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between gap-3">
      <h3 class="text-sm font-bold text-white tracking-tight">Foto Wasit Massal</h3>
      <button type="button" onclick={onClose} disabled={saving} class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08]" aria-label="Tutup">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
      </button>
    </div>

    <div class="p-6 space-y-4 overflow-y-auto">
      <div
        role="region"
        aria-label="Letakkan foto wasit"
        class="rounded-xl border-2 border-dashed p-6 text-center transition {dragOver ? 'border-white/60 bg-white/[0.04]' : 'border-white/[0.12]'}"
        ondragover={(e) => {
          e.preventDefault();
          dragOver = true;
        }}
        ondragleave={() => (dragOver = false)}
        ondrop={async (e) => {
          e.preventDefault();
          dragOver = false;
          if (e.dataTransfer) addFiles(await filesFromDrop(e.dataTransfer));
        }}
      >
        <div class="text-sm font-bold text-white">Letakkan foto atau folder di sini</div>
        <div class="text-xs text-zinc-400 mt-1">Nama file = nama wasit (contoh: "surya dharma.jpg"). JPG/PNG diubah ke WebP otomatis.</div>
        <div class="mt-3 flex justify-center gap-2">
          <label class="h-9 px-3.5 inline-flex items-center bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl cursor-pointer">
            Pilih foto
            <input type="file" accept="image/*" multiple class="hidden" onchange={(e) => addFiles([...(e.currentTarget.files ?? [])])} />
          </label>
          <label class="h-9 px-3.5 inline-flex items-center bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl cursor-pointer">
            Pilih folder
            <input type="file" multiple webkitdirectory class="hidden" onchange={(e) => addFiles([...(e.currentTarget.files ?? [])])} />
          </label>
        </div>
      </div>

      {#if items.length > 0}
        <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-3">
            <div class="text-[10px] font-bold tracking-wider uppercase text-emerald-400">Siap disimpan</div>
            <div class="text-2xl font-black font-mono text-white">{ready.length}</div>
          </div>
          <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-3">
            <div class="text-[10px] font-bold tracking-wider uppercase text-amber-400">Perlu dipilih</div>
            <div class="text-2xl font-black font-mono text-white">{needs}</div>
          </div>
          <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-3">
            <div class="text-[10px] font-bold tracking-wider uppercase text-zinc-400">Ganda (dilewati)</div>
            <div class="text-2xl font-black font-mono text-white">{dupes}</div>
          </div>
          <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-3">
            <div class="text-[10px] font-bold tracking-wider uppercase text-zinc-400">Wasit tanpa foto</div>
            <div class="text-2xl font-black font-mono text-white">{withoutPhoto.length}</div>
          </div>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
          {#each items as it (it.key)}
            {@const current = roster.find((w) => w.id === it.wasitId)}
            <div
              class="flex gap-3 p-2.5 rounded-xl border {it.status === 'siap' || it.status === 'tersimpan'
                ? 'bg-[#111215] border-white/[0.08]'
                : it.status === 'perlu' || it.status === 'gagal'
                  ? 'bg-amber-500/[0.05] border-amber-500/30'
                  : 'bg-[#111215] border-white/[0.04] opacity-60'}"
            >
              <img src={it.url} alt={it.file.name} class="w-16 h-20 rounded-lg object-cover object-top shrink-0 bg-black/40" />
              <div class="min-w-0 flex-1 flex flex-col gap-1">
                <div class="text-[11px] font-mono text-zinc-400 truncate" title={it.file.name}>{it.file.name}</div>
                <select
                  value={it.wasitId ?? ""}
                  onchange={(e) => setMatch(it, e.currentTarget.value)}
                  disabled={saving || it.status === "tersimpan"}
                  class="w-full bg-[#181a20] text-xs text-white px-2 py-1.5 rounded-lg border border-white/[0.1]"
                >
                  <option value="">— Pilih wasit —</option>
                  {#each roster as w (w.id)}
                    <option value={w.id}>{w.nama_wasit}{w.foto ? " (sudah ada foto)" : ""}</option>
                  {/each}
                </select>
                <div class="flex items-center gap-1.5 text-[10px] font-mono font-bold">
                  <span
                    class="px-1.5 py-0.5 rounded {it.status === 'siap' || it.status === 'tersimpan'
                      ? 'bg-emerald-500/15 text-emerald-300'
                      : it.status === 'perlu' || it.status === 'gagal'
                        ? 'bg-amber-500/15 text-amber-300'
                        : 'bg-white/[0.06] text-zinc-400'}"
                  >
                    {STATUS[it.status]}{it.ambiguous && it.status === "perlu" ? " (mirip >1)" : ""}
                  </span>
                  {#if current?.foto && it.status === "siap"}
                    <span class="px-1.5 py-0.5 rounded bg-white/[0.06] text-zinc-300">ganti foto lama</span>
                  {/if}
                  {#if it.status !== "tersimpan" && it.status !== "menyimpan"}
                    <button type="button" onclick={() => skip(it)} class="ml-auto text-zinc-500 hover:text-white">
                      {it.status === "lewati" ? "Pakai" : "Lewati"}
                    </button>
                  {/if}
                </div>
                {#if it.error}<div class="text-[10px] text-red-300">{it.error}</div>{/if}
              </div>
            </div>
          {/each}
        </div>

        {#if withoutPhoto.length > 0}
          <div class="text-xs text-zinc-400">
            Belum ada foto: <span class="text-zinc-200">{withoutPhoto.map((w) => w.nama_wasit).join(", ")}</span>
          </div>
        {/if}
      {/if}
    </div>

    <div class="px-6 py-3.5 bg-[#111215] border-t border-white/[0.08] flex items-center justify-end gap-2">
      {#if savedCount > 0}<span class="mr-auto text-xs font-mono text-emerald-300">{savedCount} foto tersimpan</span>{/if}
      <button type="button" onclick={onClose} disabled={saving} class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">
        {savedCount > 0 ? "Tutup" : "Batal"}
      </button>
      <button
        type="button"
        onclick={saveAll}
        disabled={saving || ready.length === 0}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md disabled:opacity-40"
      >
        {saving ? "Menyimpan…" : `Simpan ${ready.length} foto`}
      </button>
    </div>
  </div>
</div>
