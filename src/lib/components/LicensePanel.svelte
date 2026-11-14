<script lang="ts">
  import { invoke, isClientMode } from "$lib/api_client";
  import {
    license,
    loadLicense,
    hasModule,
    errorText,
    MODULE_LABELS,
    type LicenseModule,
    type LicenseStatus,
  } from "$lib/license.svelte";

  interface Props {
    onShowToast?: (msg: string) => void;
  }
  let { onShowToast = () => {} }: Props = $props();

  let keyInput = $state("");
  let busy = $state<"" | "activate" | "refresh" | "deactivate">("");
  let error = $state<string | null>(null);
  let confirmRelease = $state(false);

  const clientMode = $derived(isClientMode());
  const st = $derived(license.status);
  const modules: LicenseModule[] = ["kata", "kumite", "drawing"];

  const stateLabel: Record<string, { text: string; cls: string }> = {
    active: { text: "AKTIF", cls: "bg-emerald-500/10 text-emerald-300 border-emerald-500/30" },
    expired: { text: "EXPIRED", cls: "bg-amber-500/10 text-amber-300 border-amber-500/30" },
    invalid: { text: "TIDAK BERLAKU", cls: "bg-rose-500/10 text-rose-300 border-rose-500/30" },
    clock_tampered: { text: "JAM BERUBAH", cls: "bg-rose-500/10 text-rose-300 border-rose-500/30" },
    none: { text: "BELUM AKTIVASI", cls: "bg-white/[0.06] text-zinc-300 border-white/[0.1]" },
  };
  const badge = $derived(stateLabel[st?.state ?? "none"] ?? stateLabel.none);
  const hasSavedKey = $derived(!!st?.key);

  function fmtDate(epoch: number | null | undefined): string {
    if (!epoch) return "Lifetime";
    return new Date(epoch * 1000).toLocaleDateString("id-ID", { day: "2-digit", month: "short", year: "numeric" });
  }
  function daysLeft(epoch: number | null | undefined): string {
    if (!epoch) return "";
    const d = Math.ceil((epoch * 1000 - Date.now()) / 86_400_000);
    if (d < 0) return `lewat ${-d} hari`;
    if (d === 0) return "hari terakhir";
    return `${d} hari lagi`;
  }
  function fmtDateTime(epoch: number | null | undefined): string {
    if (!epoch) return "-";
    return new Date(epoch * 1000).toLocaleString("id-ID", { day: "2-digit", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
  }

  async function run(kind: "activate" | "refresh" | "deactivate") {
    error = null;
    busy = kind;
    try {
      let res: LicenseStatus;
      if (kind === "activate") {
        res = await invoke<LicenseStatus>("activate_license", { key: keyInput });
        keyInput = "";
        onShowToast("Lisensi berhasil diaktivasi di komputer ini.");
      } else if (kind === "refresh") {
        res = await invoke<LicenseStatus>("refresh_license");
        onShowToast("Lisensi diperbarui.");
      } else {
        res = await invoke<LicenseStatus>("deactivate_license");
        confirmRelease = false;
        onShowToast("Lisensi dilepas dari komputer ini.");
      }
      license.status = res;
    } catch (err) {
      error = errorText(err);
      await loadLicense();
    } finally {
      busy = "";
    }
  }

  function close() {
    license.panelOpen = false;
    error = null;
    confirmRelease = false;
  }
</script>

{#if license.panelOpen}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs"
    role="presentation"
    onclick={(e) => e.target === e.currentTarget && close()}
  >
    <div class="w-full max-w-lg bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl text-zinc-200 font-sans" role="dialog" aria-modal="true" aria-labelledby="license-title">
      <div class="flex items-center justify-between mb-5">
        <div class="flex items-center gap-2">
          <h2 id="license-title" class="text-xl font-black text-white tracking-tight uppercase">Lisensi</h2>
          <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold border {badge.cls}">{badge.text}</span>
        </div>
        <button type="button" onclick={close} class="p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition" aria-label="Tutup">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
        </button>
      </div>

      <!-- Modules -->
      <div class="grid grid-cols-3 gap-2 mb-4">
        {#each modules as m}
          {@const on = hasModule(m, st)}
          <div class="rounded-xl border p-3 {on ? 'border-emerald-500/30 bg-emerald-500/[0.06]' : 'border-white/[0.08] bg-[#14161b]'}">
            <div class="flex items-center gap-1.5 text-[10px] font-bold tracking-wider uppercase {on ? 'text-emerald-300' : 'text-zinc-500'}">
              {#if on}
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
              {:else}
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V7a4 4 0 018 0v4" /></svg>
              {/if}
              <span>{on ? "Terbuka" : "Terkunci"}</span>
            </div>
            <div class="mt-1 text-xs font-bold text-white">{MODULE_LABELS[m].replace("Ambrilindo ", "")}</div>
          </div>
        {/each}
      </div>

      {#if st && st.buyer}
        <dl class="grid grid-cols-[110px_1fr] gap-x-3 gap-y-1.5 text-xs mb-4 bg-[#14161b] border border-white/[0.08] rounded-xl p-3">
          <dt class="text-zinc-500">Pembeli</dt><dd class="text-white font-semibold">{st.buyer}</dd>
          {#if st.org}<dt class="text-zinc-500">Organisasi</dt><dd class="text-zinc-200">{st.org}</dd>{/if}
          <dt class="text-zinc-500">Berlaku sampai</dt>
          <dd class="text-zinc-200">{fmtDate(st.expiresAt)} {#if st.expiresAt}<span class="text-zinc-500">· {daysLeft(st.expiresAt)}</span>{/if}</dd>
          <dt class="text-zinc-500">Key</dt><dd class="font-mono text-zinc-200">{st.key ?? `…${st.keyHint}`}</dd>
          <dt class="text-zinc-500">Maks. komputer</dt><dd class="text-zinc-200">{st.maxDevices}</dd>
          <dt class="text-zinc-500">Komputer ini</dt><dd class="font-mono text-zinc-400">{st.deviceName}</dd>
          <dt class="text-zinc-500">Cek online</dt><dd class="text-zinc-400">{fmtDateTime(st.lastCheckedAt)}</dd>
        </dl>
      {/if}

      {#if st?.message && st.state !== "active"}
        <p role="status" class="text-[11px] text-amber-300 bg-amber-500/[0.06] border border-amber-500/20 rounded-xl px-3 py-2 mb-4">{st.message}</p>
      {/if}

      {#if clientMode}
        <p class="text-xs text-zinc-400">Laptop operator mengikuti lisensi laptop host. Aktivasi dilakukan di laptop host.</p>
      {:else}
        {#if st?.state !== "active"}
          <form
            class="flex gap-2"
            onsubmit={(e) => {
              e.preventDefault();
              if (keyInput.trim()) run("activate");
            }}
          >
            <input
              type="text"
              bind:value={keyInput}
              placeholder="AMBR-XXXXX-XXXXX-XXXXX-XXXXX"
              autocomplete="off"
              spellcheck="false"
              aria-label="Key lisensi"
              class="flex-1 min-w-0 bg-[#0e0f14] text-xs font-mono text-white px-3 py-2 rounded-xl border border-white/[0.12] focus:border-white outline-none uppercase select-text"
            />
            <button
              type="submit"
              disabled={!keyInput.trim() || busy !== ""}
              class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md disabled:opacity-40 transition"
            >
              {busy === "activate" ? "Mengaktivasi…" : "Aktivasi"}
            </button>
          </form>
          <p class="text-[11px] text-zinc-500 mt-2">Aktivasi butuh internet. Setelah aktif, aplikasi bisa dipakai offline.</p>
        {/if}

        {#if error}
          <p role="alert" class="text-[11px] text-rose-400 mt-3 break-words">{error}</p>
        {/if}

        {#if hasSavedKey}
          <div class="flex items-center justify-end gap-2 mt-5 pt-4 border-t border-white/[0.08]">
            {#if confirmRelease}
              <span class="text-[11px] text-zinc-400 mr-auto">Lepas lisensi dari komputer ini?</span>
              <button type="button" onclick={() => (confirmRelease = false)} class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">Batal</button>
              <button type="button" onclick={() => run("deactivate")} disabled={busy !== ""} class="h-9 px-3.5 bg-rose-500/10 text-rose-300 border border-rose-500/30 font-semibold text-xs rounded-xl disabled:opacity-40">
                {busy === "deactivate" ? "Melepas…" : "Ya, lepas"}
              </button>
            {:else}
              <button type="button" onclick={() => (confirmRelease = true)} disabled={busy !== ""} class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 border border-white/[0.1] font-semibold text-xs rounded-xl mr-auto disabled:opacity-40">
                Lepas dari komputer ini
              </button>
              <button type="button" onclick={() => run("refresh")} disabled={busy !== ""} class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md disabled:opacity-40">
                {busy === "refresh" ? "Memperbarui…" : "Perbarui"}
              </button>
            {/if}
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}
