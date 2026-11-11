<script lang="ts">
  // Import referees from Excel / CSV: pick a file → preview what changes → import.
  import { apiCall } from "$lib/api_client";
  import { fileToBase64 } from "$lib/refereeSeats";
  import type { WasitImportPreview, WasitImportSummary } from "$lib/types";

  interface Props {
    eventId: number;
    onClose: () => void;
    onDone: (summary: WasitImportSummary) => void;
  }
  let { eventId, onClose, onDone }: Props = $props();

  let fileName = $state("");
  let preview = $state<WasitImportPreview | null>(null);
  let error = $state("");
  let busy = $state(false);
  let dragOver = $state(false);

  const ACTION_LABEL: Record<string, string> = { baru: "Baru", ubah: "Diperbarui", sama: "Sudah ada", duplikat: "Ganda (dilewati)" };
  let counts = $derived.by(() => {
    const c: Record<string, number> = { baru: 0, ubah: 0, sama: 0, duplikat: 0 };
    for (const r of preview?.rows ?? []) c[r.action] = (c[r.action] ?? 0) + 1;
    return c;
  });
  let importable = $derived((preview?.rows ?? []).filter((r) => r.action === "baru" || r.action === "ubah"));

  async function readFile(file: File | undefined) {
    if (!file) return;
    error = "";
    preview = null;
    fileName = file.name;
    if (!/\.(xlsx|xls|ods|csv|txt)$/i.test(file.name)) {
      error = "Pilih file Excel (.xlsx / .xls) atau CSV.";
      return;
    }
    busy = true;
    try {
      const base64Data = await fileToBase64(file);
      preview = await apiCall<WasitImportPreview>("preview_wasit_import", { eventId, fileName: file.name, base64Data });
      if (preview.rows.length === 0) error = "Tidak ada baris wasit di file ini.";
    } catch (e: any) {
      error = typeof e === "string" ? e : e?.message || String(e);
    } finally {
      busy = false;
    }
  }

  async function runImport() {
    if (!preview || importable.length === 0) return;
    busy = true;
    error = "";
    try {
      const summary = await apiCall<WasitImportSummary>("import_wasits", { eventId, rows: importable });
      onDone(summary);
    } catch (e: any) {
      error = typeof e === "string" ? e : e?.message || String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs" role="dialog" aria-modal="true">
  <div class="bg-[#181a20] border border-white/10 rounded-2xl shadow-2xl overflow-hidden w-full max-w-4xl max-h-[90vh] flex flex-col">
    <div class="px-6 py-4 bg-[#111215] border-b border-white/[0.08] flex items-center justify-between gap-3">
      <h3 class="text-sm font-bold text-white tracking-tight">Import Wasit &amp; Juri</h3>
      <button type="button" onclick={onClose} class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08]" aria-label="Tutup">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
      </button>
    </div>

    <div class="p-6 space-y-4 overflow-y-auto">
      <label
        class="block rounded-xl border-2 border-dashed p-6 text-center cursor-pointer transition {dragOver ? 'border-white/60 bg-white/[0.04]' : 'border-white/[0.12] hover:border-white/30'}"
        ondragover={(e) => {
          e.preventDefault();
          dragOver = true;
        }}
        ondragleave={() => (dragOver = false)}
        ondrop={(e) => {
          e.preventDefault();
          dragOver = false;
          readFile(e.dataTransfer?.files?.[0]);
        }}
      >
        <input type="file" accept=".xlsx,.xls,.ods,.csv" class="hidden" onchange={(e) => readFile(e.currentTarget.files?.[0])} />
        <div class="text-sm font-bold text-white">{fileName || "Pilih atau letakkan file Excel / CSV"}</div>
        <div class="text-xs text-zinc-400 mt-1">Kolom: Nama, Perguruan, Kab/Kota, Lisensi, No Lisensi, Keterangan (urutan bebas)</div>
      </label>

      {#if busy}
        <div class="text-xs font-mono text-zinc-400">Memproses…</div>
      {/if}
      {#if error}
        <div class="p-3 rounded-xl bg-red-500/10 border border-red-500/25 text-xs text-red-300">{error}</div>
      {/if}

      {#if preview && preview.rows.length > 0}
        <div class="flex flex-wrap gap-2 text-[11px] font-mono">
          {#each preview.columns as c (c)}
            <span class="px-2 py-0.5 rounded bg-white/[0.06] text-zinc-300 border border-white/[0.1]">{c}</span>
          {/each}
        </div>
        <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
          {#each Object.entries(ACTION_LABEL) as [k, label] (k)}
            <div class="bg-[#111215] border border-white/[0.08] rounded-xl p-3">
              <div class="text-[10px] font-bold tracking-wider uppercase text-zinc-400">{label}</div>
              <div class="text-2xl font-black font-mono text-white">{counts[k] ?? 0}</div>
            </div>
          {/each}
        </div>
        <div class="rounded-2xl border border-white/[0.08] overflow-hidden">
          <table class="w-full text-xs">
            <thead class="bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider">
              <tr>
                <th class="text-left px-3 py-2">Nama</th>
                <th class="text-left px-3 py-2">Perguruan</th>
                <th class="text-left px-3 py-2">Kab/Kota</th>
                <th class="text-left px-3 py-2">Lisensi</th>
                <th class="text-left px-3 py-2">Keterangan</th>
                <th class="text-left px-3 py-2">Status</th>
              </tr>
            </thead>
            <tbody>
              {#each preview.rows as r, i (i)}
                <tr class="border-t border-white/[0.06] {r.action === 'sama' || r.action === 'duplikat' ? 'text-zinc-500' : 'text-zinc-200'}">
                  <td class="px-3 py-1.5 font-semibold">{r.nama}</td>
                  <td class="px-3 py-1.5">{r.perguruan || "-"}</td>
                  <td class="px-3 py-1.5">{r.kab_kota || "-"}</td>
                  <td class="px-3 py-1.5">{r.lisensi || "-"}</td>
                  <td class="px-3 py-1.5">{r.keterangan || "-"}</td>
                  <td class="px-3 py-1.5 font-mono text-[11px] {r.action === 'baru' ? 'text-emerald-300' : r.action === 'ubah' ? 'text-amber-300' : ''}">
                    {ACTION_LABEL[r.action] ?? r.action}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>

    <div class="px-6 py-3.5 bg-[#111215] border-t border-white/[0.08] flex justify-end gap-2">
      <button type="button" onclick={onClose} class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">Batal</button>
      <button
        type="button"
        onclick={runImport}
        disabled={busy || importable.length === 0}
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md disabled:opacity-40"
      >
        Import {importable.length} wasit
      </button>
    </div>
  </div>
</div>
