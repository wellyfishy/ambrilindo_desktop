<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { TatamiItem } from "$lib/types";

  interface Props {
    eventId: number;
    tatamis: TatamiItem[];
    hostAddress: string;
    onRefreshTatamis: () => void;
    onOpenMonitorPicker: (tatamiNum: number) => void;
  }

  let {
    eventId,
    tatamis,
    hostAddress,
    onRefreshTatamis,
    onOpenMonitorPicker,
  }: Props = $props();

  let isAddingModal = $state(false);
  let newTatamiNumber = $state<number>(1);
  let isSubmitting = $state(false);

  // Custom Delete Confirmation Modal State
  let tatamiToDelete = $state<TatamiItem | null>(null);
  let isDeleting = $state(false);

  // Stats calculation
  let totalTatamis = $derived(tatamis.length);
  let activeMatchesCount = $derived(tatamis.filter((t) => t.detail_bagan_id !== null).length);

  async function handleAddTatami() {
    if (!eventId || newTatamiNumber < 1) return;
    isSubmitting = true;
    try {
      await invoke("create_tatami", {
        eventId,
        tatamiNumber: newTatamiNumber,
      });
      isAddingModal = false;
      onRefreshTatamis();
    } catch (err: any) {
      alert("Gagal menambahkan tatami: " + err);
    } finally {
      isSubmitting = false;
    }
  }

  function promptDeleteTatami(tatami: TatamiItem) {
    tatamiToDelete = tatami;
  }

  function cancelDelete() {
    tatamiToDelete = null;
  }

  async function confirmDeleteTatami() {
    if (!tatamiToDelete) return;
    isDeleting = true;
    try {
      await invoke("delete_tatami", { tatamiId: tatamiToDelete.id });
      tatamiToDelete = null;
      onRefreshTatamis();
    } catch (err: any) {
      alert("Gagal menghapus tatami: " + err);
    } finally {
      isDeleting = false;
    }
  }

  onMount(() => {
    // Next proposed tatami number
    if (tatamis.length > 0) {
      const maxNum = Math.max(...tatamis.map((t) => t.tatami_number || 0));
      newTatamiNumber = maxNum + 1;
    }
  });
</script>

<div class="h-full flex flex-col p-6 space-y-6 overflow-y-auto select-none bg-[#14161b] text-zinc-200 font-sans">
  <!-- Header Bar -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between pb-4 border-b border-white/[0.08] gap-4">
    <div>
      <div class="flex items-center space-x-2">
        <h2 class="text-xl font-black text-white tracking-tight uppercase">TATAMI MANAGER</h2>
      </div>
      <p class="text-xs text-zinc-400 mt-0.5 font-normal">
        Kelola alokasi tatami, monitor scoreboard, dan panel juri turnamen
      </p>
    </div>

    <button
      onclick={() => {
        const maxNum = tatamis.length > 0 ? Math.max(...tatamis.map((t) => t.tatami_number || 0)) : 0;
        newTatamiNumber = maxNum + 1;
        isAddingModal = true;
      }}
      class="px-4 py-2 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow transition-all active:scale-[0.98] flex items-center space-x-2 shrink-0 self-start sm:self-auto"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
      </svg>
      <span>Tambah Tatami</span>
    </button>
  </div>

  <!-- Summary Stat Cards -->
  <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-4 flex items-center justify-between shadow-sm">
      <div>
        <span class="text-[10px] font-bold text-zinc-500 uppercase tracking-wider block">TOTAL TATAMI</span>
        <span class="text-2xl font-black font-mono text-white mt-1 block">{totalTatamis}</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-white/[0.04] text-zinc-400 flex items-center justify-center border border-white/[0.06]">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
          <line x1="3" y1="9" x2="21" y2="9"/>
          <line x1="9" y1="21" x2="9" y2="9"/>
        </svg>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-4 flex items-center justify-between shadow-sm">
      <div>
        <span class="text-[10px] font-bold text-zinc-500 uppercase tracking-wider block">PERTANDINGAN BERJALAN</span>
        <span class="text-2xl font-black font-mono text-amber-400 mt-1 block">{activeMatchesCount}</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-amber-500/10 text-amber-400 flex items-center justify-center border border-amber-500/20">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
          <circle cx="12" cy="12" r="10"/>
          <polyline points="12 6 12 12 16 14"/>
        </svg>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-4 flex items-center justify-between shadow-sm">
      <div>
        <span class="text-[10px] font-bold text-zinc-500 uppercase tracking-wider block">STANDBY / SIAP</span>
        <span class="text-2xl font-black font-mono text-emerald-400 mt-1 block">{totalTatamis - activeMatchesCount}</span>
      </div>
      <div class="w-10 h-10 rounded-xl bg-emerald-500/10 text-emerald-400 flex items-center justify-center border border-emerald-500/20">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      </div>
    </div>
  </div>

  <!-- Tatami Management Table -->
  <div class="bg-[#181a20] rounded-2xl border border-white/[0.08] overflow-hidden shadow-lg">
    <table class="w-full text-left text-xs">
      <thead class="bg-[#111215] text-zinc-400 uppercase font-bold border-b border-white/[0.08]">
        <tr>
          <th class="px-5 py-3">Nomor Tatami</th>
          <th class="px-5 py-3">Partai Aktif</th>
          <th class="px-5 py-3">Status</th>
          <th class="px-5 py-3">Launcher Cepat</th>
          <th class="px-5 py-3 text-right">Aksi</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-white/[0.06] font-medium">
        {#if tatamis.length === 0}
          <tr>
            <td colspan="5" class="px-5 py-8 text-center text-zinc-500 italic">
              Belum ada tatami yang dibuat untuk event ini. Klik "Tambah Tatami" di atas.
            </td>
          </tr>
        {:else}
          {#each tatamis as tatami}
            {@const tNum = tatami.tatami_number ?? 1}
            <tr class="hover:bg-white/[0.02] transition">
              <!-- Tatami Number Badge -->
              <td class="px-5 py-3.5">
                <div class="flex items-center space-x-3">
                  <div class="w-8 h-8 rounded-lg bg-white/[0.06] border border-white/[0.1] font-mono font-black text-sm flex items-center justify-center text-white">
                    {tNum}
                  </div>
                  <div>
                    <span class="font-bold text-white block">Tatami {tNum}</span>
                    <span class="text-[10px] font-mono text-zinc-500">ID #{tatami.id}</span>
                  </div>
                </div>
              </td>

              <!-- Active Match Detail -->
              <td class="px-5 py-3.5 font-mono text-xs">
                {#if tatami.detail_bagan_id}
                  <div class="flex items-center space-x-1.5 text-amber-300 font-bold">
                    <span class="w-2 h-2 rounded-full bg-amber-400 animate-ping"></span>
                    <span>Partai #{tatami.detail_bagan_id}</span>
                  </div>
                {:else}
                  <span class="text-zinc-500 italic">Tidak ada partai aktif</span>
                {/if}
              </td>

              <!-- Status Badge -->
              <td class="px-5 py-3.5">
                {#if tatami.detail_bagan_id}
                  <span class="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold bg-amber-500/10 text-amber-400 border border-amber-500/20">
                    <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
                    <span>LIVE</span>
                  </span>
                {:else}
                  <span class="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold bg-white/[0.04] text-zinc-400 border border-white/[0.06]">
                    <span class="w-1.5 h-1.5 rounded-full bg-zinc-500"></span>
                    <span>READY</span>
                  </span>
                {/if}
              </td>

              <!-- Quick Launchers (Scoreboard Projector & Referee Panel) -->
              <td class="px-5 py-3.5">
                <div class="flex items-center space-x-2">
                  <button
                    onclick={() => onOpenMonitorPicker(tNum)}
                    class="px-2.5 py-1 rounded-lg bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/20 text-[11px] font-semibold transition flex items-center space-x-1.5"
                    title="Buka proyeksi monitor fullscreen untuk Tatami {tNum}"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                      <line x1="8" y1="21" x2="16" y2="21"/>
                      <line x1="12" y1="17" x2="12" y2="21"/>
                    </svg>
                    <span>Scoreboard</span>
                  </button>

                  <a
                    href="{hostAddress}/tatami-{tNum}/"
                    target="_blank"
                    class="px-2.5 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.08] text-[11px] font-semibold transition"
                    title="Buka panel operator tatami di browser"
                  >
                    Operator
                  </a>

                  <a
                    href="{hostAddress}/jury/{tNum}/"
                    target="_blank"
                    class="px-2.5 py-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.08] text-[11px] font-semibold transition"
                    title="Buka panel juri wasit"
                  >
                    Panel Juri
                  </a>
                </div>
              </td>

              <!-- Actions (Delete) -->
              <td class="px-5 py-3.5 text-right">
                <button
                  onclick={() => promptDeleteTatami(tatami)}
                  class="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition"
                  title="Hapus Tatami {tNum}"
                >
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                  </svg>
                </button>
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>

  <!-- Modal: Tambah Tatami -->
  {#if isAddingModal}
    <div class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4">
      <div class="bg-[#181a20] border border-white/[0.12] rounded-2xl max-w-sm w-full p-6 shadow-2xl space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center justify-between border-b border-white/[0.08] pb-3">
          <h3 class="text-sm font-bold text-white uppercase tracking-wider">TAMBAH TATAMI BARU</h3>
          <button
            onclick={() => (isAddingModal = false)}
            class="text-zinc-500 hover:text-white transition"
          >
            ✕
          </button>
        </div>

        <div class="space-y-2">
          <label for="tatami-num-input" class="text-xs font-bold text-zinc-400 block">Nomor Tatami</label>
          <input
            id="tatami-num-input"
            type="number"
            min="1"
            bind:value={newTatamiNumber}
            class="w-full bg-[#111215] border border-white/[0.1] text-white font-mono text-base px-3 py-2 rounded-xl outline-none focus:border-white transition"
          />
          <p class="text-[11px] text-zinc-500">
            Tatami baru akan dibuat di database lokal untuk event ini.
          </p>
        </div>

        <div class="flex items-center justify-end space-x-2 pt-2">
          <button
            onclick={() => (isAddingModal = false)}
            class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium text-xs rounded-xl transition"
          >
            Batal
          </button>
          <button
            onclick={handleAddTatami}
            disabled={isSubmitting}
            class="px-5 py-2 bg-white hover:bg-zinc-200 text-black font-bold text-xs rounded-xl shadow transition active:scale-95 disabled:opacity-50"
          >
            {isSubmitting ? "Menyimpan..." : "Simpan Tatami"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal: Konfirmasi Hapus Tatami -->
  {#if tatamiToDelete}
    <div class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4">
      <div class="bg-[#181a20] border border-red-500/30 rounded-2xl max-w-sm w-full p-6 shadow-2xl space-y-4 animate-in zoom-in-95 duration-150">
        <div class="flex items-center space-x-3">
          <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-400 shrink-0">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
            </svg>
          </div>
          <div>
            <h3 class="text-sm font-bold text-white uppercase tracking-wider">HAPUS TATAMI {tatamiToDelete.tatami_number}</h3>
            <p class="text-[11px] text-zinc-400 mt-0.5">Konfirmasi penghapusan</p>
          </div>
        </div>

        <p class="text-xs text-zinc-300 leading-relaxed">
          Apakah Anda yakin ingin menghapus <strong>Tatami {tatamiToDelete.tatami_number}</strong>? Tindakan ini akan menghapus tatami ini dari event turnamen.
        </p>

        <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
          <button
            onclick={cancelDelete}
            disabled={isDeleting}
            class="px-4 py-2 rounded-xl text-xs font-bold text-zinc-400 hover:text-white hover:bg-white/[0.06] transition"
          >
            Batal
          </button>
          <button
            onclick={confirmDeleteTatami}
            disabled={isDeleting}
            class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white font-bold text-xs rounded-xl shadow transition active:scale-[0.98] disabled:opacity-50 flex items-center space-x-1.5"
          >
            {#if isDeleting}
              <span>Menghapus...</span>
            {:else}
              <span>Hapus Tatami</span>
            {/if}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
