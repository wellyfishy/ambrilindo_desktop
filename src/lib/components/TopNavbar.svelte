<script lang="ts">
  import type { ActiveChannel } from "$lib/types";
  import ServerStatusBadge from "$lib/components/ServerStatusBadge.svelte";
  import { invoke } from "$lib/api_client";
  import { license, openLicensePanel } from "$lib/license.svelte";

  interface Props {
    activeChannel: ActiveChannel;
    eventName?: string;
    onServerStopped?: () => void;
    onShowToast?: (msg: string) => void;
  }

  let { activeChannel, eventName = "", onServerStopped = () => {}, onShowToast = () => {} }: Props = $props();

  let isBackingUp = $state(false);

  async function handleQuickBackup() {
    isBackingUp = true;
    try {
      const backupPath = await invoke<string>("backup_database");
      const filename = backupPath.split(/[\\/]/).pop() || backupPath;
      onShowToast(`Cadangan database berhasil dibuat: ${filename}`);
    } catch (err: any) {
      const msg = typeof err === "string" ? err : err?.message || "Gagal mencadangkan database.";
      alert(msg);
    } finally {
      isBackingUp = false;
    }
  }
</script>

<header class="no-print h-14 bg-[#111216] border-b border-white/[0.08] px-5 flex items-center justify-between shadow-sm select-none shrink-0 relative z-40 font-sans">
  <!-- Left: Channel Title & Description -->
  <div class="flex items-center space-x-3 truncate">
    <div class="flex items-center space-x-2 font-bold text-white text-sm truncate">
      <span class="text-zinc-500 font-mono text-base">#</span>
      <span class="truncate tracking-wide">{activeChannel.name}</span>
    </div>

    {#if activeChannel.subtitle}
      <div class="hidden sm:flex items-center space-x-3 text-xs text-zinc-400 truncate border-l border-white/[0.08] pl-3 font-normal">
        <span class="truncate">{activeChannel.subtitle}</span>
      </div>
    {/if}
  </div>

  <!-- Right: Top-Right Actions (Backup & Server Status) -->
  <div class="flex items-center space-x-2 shrink-0">
    <!-- License status -->
    <button
      type="button"
      onclick={openLicensePanel}
      class="h-8 px-2.5 rounded-xl border flex items-center gap-1.5 text-xs font-mono transition cursor-pointer active:scale-95 shadow-sm {license.status?.state === 'active'
        ? 'bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white border-white/[0.08]'
        : 'bg-amber-500/[0.08] hover:bg-amber-500/[0.14] text-amber-300 border-amber-500/30'}"
      title="Lisensi Ambrilindo Desktop"
    >
      <svg class="w-3.5 h-3.5 {license.status?.state === 'active' ? 'text-emerald-400' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
      </svg>
      <span class="hidden md:inline text-[11px] font-semibold">{license.status?.state === "active" ? "Lisensi" : "Belum Berlisensi"}</span>
    </button>

    <!-- Quick 1-Click Database Backup -->
    <button
      type="button"
      onclick={handleQuickBackup}
      disabled={isBackingUp}
      class="h-8 px-2.5 rounded-xl bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white border border-white/[0.08] flex items-center gap-1.5 text-xs font-mono transition cursor-pointer disabled:opacity-50 active:scale-95 shadow-sm"
      title="Cadangkan database turnamen (.sqlite3) sekarang"
    >
      {#if isBackingUp}
        <svg class="w-3.5 h-3.5 animate-spin text-blue-400" viewBox="0 0 24 24" fill="none">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        <span class="hidden md:inline text-[11px] font-semibold">Mencadangkan...</span>
      {:else}
        <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
        </svg>
        <span class="hidden md:inline text-[11px] font-semibold text-zinc-300">Backup DB</span>
      {/if}
    </button>

    <ServerStatusBadge {eventName} {onServerStopped} />
  </div>
</header>
