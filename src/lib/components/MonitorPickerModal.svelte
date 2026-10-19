<script lang="ts">
  import type { MonitorOption } from "$lib/types";

  interface Props {
    tatamiNumber: number;
    availableMonitors: MonitorOption[];
    selectedMonitorId: number;
    isProjecting: boolean;
    onSelectMonitor: (id: number) => void;
    onConfirmProject: () => void;
    onClose: () => void;
  }

  let {
    tatamiNumber,
    availableMonitors,
    selectedMonitorId,
    isProjecting,
    onSelectMonitor,
    onConfirmProject,
    onClose,
  }: Props = $props();
</script>

<div class="fixed inset-0 bg-black/85 backdrop-blur-sm z-50 flex items-center justify-center p-4 select-none font-sans">
  <div class="bg-[#181a20] border border-white/[0.12] rounded-2xl max-w-lg w-full p-6 shadow-2xl space-y-5 animate-in fade-in zoom-in-95 duration-150">
    <!-- Header -->
    <div class="flex items-start justify-between border-b border-white/[0.08] pb-3">
      <div>
        <h3 class="text-sm font-bold text-white uppercase tracking-wider">
          PROYEKSI SCOREBOARD TATAMI {tatamiNumber}
        </h3>
        <p class="text-xs text-zinc-400 mt-0.5">Pilih layar monitor eksternal atau proyektor tujuan</p>
      </div>
      <button
        onclick={onClose}
        class="text-zinc-500 hover:text-white p-1 rounded-lg transition"
      >
        ✕
      </button>
    </div>

    <!-- Monitor Picker List -->
    <div class="space-y-2.5">
      <span class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400">MONITOR TERDETEKSI</span>

      {#if availableMonitors.length > 0}
        <div class="space-y-2">
          {#each availableMonitors as monitor}
            <button
              onclick={() => onSelectMonitor(monitor.id)}
              class="w-full p-3.5 rounded-xl border text-left transition flex items-center justify-between {selectedMonitorId === monitor.id ? 'bg-white/[0.08] border-white text-white shadow ring-1 ring-white/20' : 'bg-[#111215] border-white/[0.08] text-zinc-300 hover:bg-white/[0.03]'}"
            >
              <div class="flex items-center space-x-3 truncate mr-2">
                <div class="w-8 h-8 rounded-lg {selectedMonitorId === monitor.id ? 'bg-white text-black font-black' : 'bg-white/[0.04] text-zinc-400'} flex items-center justify-center font-mono text-xs shrink-0">
                  {monitor.id + 1}
                </div>
                <div class="truncate">
                  <p class="text-xs font-bold tracking-wide truncate">
                    {monitor.label}
                  </p>
                  <p class="text-[10px] text-zinc-500 font-mono">
                    {monitor.width} x {monitor.height} px
                  </p>
                </div>
              </div>

              {#if selectedMonitorId === monitor.id}
                <span class="text-[10px] font-mono font-bold text-black bg-white px-2 py-0.5 rounded uppercase shrink-0">
                  PILIH
                </span>
              {/if}
            </button>
          {/each}
        </div>
      {:else}
        <div class="p-4 bg-[#111215] rounded-xl text-center text-xs text-zinc-500">
          Satu layar terdeteksi. Scoreboard akan dibuka pada layar default ini.
        </div>
      {/if}
    </div>

    <!-- Info note -->
    <div class="bg-[#111215] p-3 rounded-xl border border-white/[0.06] text-xs text-zinc-400 leading-relaxed">
      Jendela scoreboard akan otomatis dibuka secara <strong>borderless fullscreen</strong> pada monitor yang dipilih. Tekan <code>Esc</code> atau <code>Alt+F4</code> untuk menutup window jika diperlukan.
    </div>

    <!-- Actions -->
    <div class="flex items-center justify-end space-x-2 pt-2">
      <button
        onclick={onClose}
        class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium text-xs rounded-xl transition"
      >
        Batal
      </button>
      <button
        onclick={onConfirmProject}
        disabled={isProjecting}
        class="px-5 py-2 bg-white hover:bg-zinc-200 text-black font-bold text-xs rounded-xl shadow transition active:scale-95 disabled:opacity-50"
      >
        {isProjecting ? 'Membuka...' : 'Luncurkan Fullscreen'}
      </button>
    </div>
  </div>
</div>
