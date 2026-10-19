<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AtletItem, UtusanItem } from "$lib/types";

  interface Props {
    viewType: 'roster' | 'contingent' | 'sync';
    eventId: number;
    hostAddress: string;
  }

  let { viewType, eventId, hostAddress }: Props = $props();

  let athletes = $state<AtletItem[]>([]);
  let contingents = $state<UtusanItem[]>([]);
  let isLoading = $state(false);
  let searchKeyword = $state("");

  async function loadData() {
    if (!eventId) return;
    isLoading = true;
    try {
      if (viewType === 'roster') {
        athletes = await invoke<AtletItem[]>("get_athletes", { eventId });
      } else if (viewType === 'contingent') {
        contingents = await invoke<UtusanItem[]>("get_contingents", { eventId });
      }
    } catch (err) {
      console.error("Gagal memuat data roster:", err);
    } finally {
      isLoading = false;
    }
  }

  let filteredAthletes = $derived(
    athletes.filter((a) =>
      (a.nama_atlet || "").toLowerCase().includes(searchKeyword.toLowerCase()) ||
      (a.kode_atlet || "").toLowerCase().includes(searchKeyword.toLowerCase())
    )
  );

  let filteredContingents = $derived(
    contingents.filter((c) =>
      (c.nama_utusan || "").toLowerCase().includes(searchKeyword.toLowerCase())
    )
  );

  $effect(() => {
    if (viewType && eventId) {
      loadData();
    }
  });

  onMount(() => {
    loadData();
  });
</script>

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans">
  <!-- Top Bar -->
  <div class="flex items-center justify-between pb-4 border-b border-white/[0.08] shrink-0">
    <div>
      <h2 class="text-xl font-black text-white tracking-tight uppercase">
        {viewType === 'roster' ? 'DATA ATLET & PESERTA' : viewType === 'contingent' ? 'KONTINGEN & UTUSAN' : 'SINKRONISASI WEB'}
      </h2>
      <p class="text-xs text-zinc-400 mt-0.5 font-normal">
        {viewType === 'roster' ? `${athletes.length} Atlet terdaftar dalam database` : viewType === 'contingent' ? `${contingents.length} Kontingen terdaftar` : 'Live Sync status portal ambrilindo.com'}
      </p>
    </div>

    {#if viewType !== 'sync'}
      <div class="w-64">
        <input
          type="text"
          bind:value={searchKeyword}
          placeholder="Cari nama, kode..."
          class="w-full bg-[#111215] text-xs text-white px-3 py-1.5 rounded-xl border border-white/[0.08] focus:border-white outline-none"
        />
      </div>
    {/if}
  </div>

  <!-- Content Body -->
  <div class="flex-1 overflow-y-auto py-4 scrollbar-thin">
    {#if isLoading}
      <div class="h-64 flex items-center justify-center text-sm text-zinc-500">
        Memuat data dari database...
      </div>
    {:else if viewType === 'roster'}
      <!-- Athletes Table -->
      <div class="bg-[#181a20] rounded-2xl border border-white/[0.08] overflow-hidden shadow-lg">
        <table class="w-full text-left text-xs">
          <thead class="bg-[#111215] text-zinc-400 uppercase font-bold border-b border-white/[0.08]">
            <tr>
              <th class="px-5 py-3">No</th>
              <th class="px-5 py-3">Nama Atlet</th>
              <th class="px-5 py-3">Kode Atlet</th>
              <th class="px-5 py-3">ID Kontingen</th>
              <th class="px-5 py-3">ID Kategori</th>
              <th class="px-5 py-3">Prioritas</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-white/[0.06] font-medium">
            {#each filteredAthletes as atlet, idx}
              <tr class="hover:bg-white/[0.02] transition">
                <td class="px-5 py-3 text-zinc-500 font-mono">{idx + 1}</td>
                <td class="px-5 py-3 font-bold text-white uppercase">{atlet.nama_atlet}</td>
                <td class="px-5 py-3 font-mono text-zinc-300">{atlet.kode_atlet || '-'}</td>
                <td class="px-5 py-3 text-zinc-400 font-mono">#{atlet.utusan_id || '-'}</td>
                <td class="px-5 py-3 text-zinc-400 font-mono">#{atlet.nomor_tanding_id || '-'}</td>
                <td class="px-5 py-3">
                  {#if atlet.is_priority}
                    <span class="bg-white/[0.08] text-white text-[10px] font-mono px-2 py-0.5 rounded border border-white/[0.1]">
                      SEED
                    </span>
                  {:else}
                    <span class="text-zinc-600">-</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else if viewType === 'contingent'}
      <!-- Contingents Grid -->
      <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
        {#each filteredContingents as contingent}
          <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-4 flex items-center space-x-3.5 shadow hover:border-white/[0.2] transition">
            <div class="w-10 h-10 rounded-lg bg-white/[0.04] border border-white/[0.08] flex items-center justify-center font-mono font-bold text-sm text-zinc-200 shrink-0">
              {contingent.nama_utusan ? contingent.nama_utusan.charAt(0).toUpperCase() : 'U'}
            </div>
            <div class="truncate">
              <h4 class="text-xs font-bold text-white truncate uppercase">{contingent.nama_utusan}</h4>
              <p class="text-[10px] font-mono text-zinc-500">ID #{contingent.id}</p>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <!-- Cloud Sync Panel -->
      <div class="max-w-2xl bg-[#181a20] border border-white/[0.08] rounded-2xl p-6 shadow-xl space-y-4">
        <h3 class="text-sm font-bold text-white uppercase tracking-wider">STATUS LIVE SYNC AMBRILINDO.COM</h3>
        <p class="text-xs text-zinc-400 leading-relaxed font-normal">
          Pengiriman data hasil pertandingan dan bagan ke portal live publik melalui antrean FIFO background worker.
        </p>

        <div class="p-4 bg-[#111215] rounded-xl border border-white/[0.08] space-y-2">
          <div class="flex items-center justify-between text-xs">
            <span class="text-zinc-400">Target Server:</span>
            <span class="font-mono text-white font-bold">https://www.ambrilindo.com</span>
          </div>
          <div class="flex items-center justify-between text-xs">
            <span class="text-zinc-400">Status Antrean Sync:</span>
            <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              READY
            </span>
          </div>
        </div>

        <div class="pt-2 flex items-center space-x-3">
          <button
            class="px-4 py-2 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow transition"
          >
            Push Bagan ke Web
          </button>
          <button
            class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium text-xs rounded-xl border border-white/[0.08] transition"
          >
            Sinkronkan Hasil (Force)
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>
