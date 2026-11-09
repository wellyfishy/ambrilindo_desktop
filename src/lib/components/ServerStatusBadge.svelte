<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "$lib/api_client";
  import type { NetworkInfo } from "$lib/types";

  interface Props {
    eventName?: string;
    onServerStopped?: () => void;
  }

  let { eventName = "", onServerStopped = () => {} }: Props = $props();

  let isServerRunning = $state(false);
  let networkInfo = $state<NetworkInfo | null>(null);
  let showModal = $state(false);
  let isStopping = $state(false);
  let isStarting = $state(false);
  let copied = $state(false);
  let activeTab = $state<"info" | "qr" | "ips">("info");
  let pollInterval: any = null;

  function portal(node: HTMLElement) {
    if (typeof document !== "undefined") {
      document.body.appendChild(node);
    }
    return {
      destroy() {
        if (node.parentNode) {
          node.parentNode.removeChild(node);
        }
      },
    };
  }

  async function checkServerStatus() {
    // On a remote device this asks the host over the LAN: no answer means the host (or the
    // network) is down, and the badge must say so instead of claiming "online".
    try {
      isServerRunning = await invoke<boolean>("get_server_status");
    } catch (_) {
      isServerRunning = false;
      return;
    }
    if (isServerRunning) {
      try {
        networkInfo = await invoke<NetworkInfo>("get_network_info");
      } catch (_) {
        // address details only exist on the host machine itself
      }
    }
  }

  onMount(async () => {
    await checkServerStatus();
    // Periodically verify server health every 5 seconds
    pollInterval = setInterval(checkServerStatus, 5000);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
  });

  async function handleOpenModal() {
    await checkServerStatus();
    showModal = true;
  }

  function handleCloseModal() {
    showModal = false;
  }

  async function copyUrlToClipboard() {
    const url = networkInfo?.server_url || "http://127.0.0.1:8000";
    try {
      if (navigator.clipboard) {
        await navigator.clipboard.writeText(url);
      } else {
        const input = document.createElement("input");
        input.value = url;
        document.body.appendChild(input);
        input.select();
        document.execCommand("copy");
        document.body.removeChild(input);
      }
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (_) {}
  }

  function openInBrowser() {
    const url = networkInfo?.server_url || "http://127.0.0.1:8000";
    if (typeof window !== "undefined") {
      window.open(url, "_blank");
    }
  }

  async function handleStopServer() {
    const confirmStop = confirm(
      "Apakah Anda yakin ingin mematikan server arena?\n\nSemua perangkat tablet juri, wasit VR, dan layar scoreboard yang terhubung ke laptop ini akan terputus."
    );
    if (!confirmStop) return;

    isStopping = true;
    try {
      await invoke("stop_arena_server");
      isServerRunning = false;
      showModal = false;
      onServerStopped();
    } catch (err: any) {
      alert("Gagal mematikan server: " + (err?.message || err));
    } finally {
      isStopping = false;
    }
  }

  async function handleStartServer() {
    isStarting = true;
    try {
      const info = await invoke<NetworkInfo>("start_arena_server", { selectedIp: null });
      networkInfo = info;
      isServerRunning = true;
    } catch (err: any) {
      alert("Gagal menyalakan server: " + (err?.message || err));
    } finally {
      isStarting = false;
    }
  }
</script>

<!-- Status Indicator Badge (Top-Right Clickable) -->
<div class="relative inline-flex items-center">
  {#if isServerRunning}
    <button
      type="button"
      onclick={handleOpenModal}
      class="group flex items-center space-x-2 text-xs font-medium text-emerald-300 bg-emerald-500/10 hover:bg-emerald-500/20 border border-emerald-500/30 hover:border-emerald-500/50 px-2.5 py-1.5 rounded-xl transition-all cursor-pointer shadow-sm active:scale-95 outline-none focus-visible:ring-2 focus-visible:ring-emerald-400/50"
      title="Klik untuk membuka Info Server & Kontrol"
    >
      <span class="relative flex h-2 w-2">
        <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
        <span class="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
      </span>
      <span class="font-bold tracking-tight">LAN Server Online</span>
      <span class="text-[10px] font-mono bg-emerald-500/25 px-1.5 py-0.5 rounded text-emerald-200">
        :8000
      </span>
    </button>
  {:else}
    <button
      type="button"
      onclick={handleOpenModal}
      class="group flex items-center space-x-2 text-xs font-medium text-zinc-400 bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] hover:border-white/20 px-2.5 py-1.5 rounded-xl transition-all cursor-pointer shadow-sm active:scale-95 outline-none"
      title="Server tidak aktif. Klik untuk opsi server"
    >
      <span class="w-2 h-2 rounded-full bg-zinc-500"></span>
      <span>Server Offline</span>
    </button>
  {/if}
</div>

<!-- Server Info & Control Modal -->
{#if showModal}
  <div use:portal class="fixed inset-0 z-[99999] bg-black/80 backdrop-blur-xs flex items-center justify-center p-4 font-sans select-none animate-in fade-in duration-150">
    <!-- Click backdrop to close -->
    <button
      type="button"
      class="absolute inset-0 w-full h-full cursor-default border-none bg-transparent"
      onclick={handleCloseModal}
      aria-label="Tutup Modal"
    ></button>

    <div class="relative bg-[#13151b] border border-white/[0.12] rounded-2xl shadow-2xl max-w-lg w-full text-zinc-200 overflow-hidden flex flex-col max-h-[90vh]">
      <!-- Header Bar -->
      <div class="p-4 sm:p-5 border-b border-white/[0.08] flex items-center justify-between shrink-0 bg-[#161820]">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl {isServerRunning ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' : 'bg-zinc-800 text-zinc-400'} flex items-center justify-center shrink-0">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" />
            </svg>
          </div>
          <div>
            <h3 class="text-sm font-bold text-white flex items-center gap-2">
              <span>Status Server Arena LAN</span>
              {#if isServerRunning}
                <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">
                  ONLINE
                </span>
              {:else}
                <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-zinc-700 text-zinc-300">
                  OFFLINE
                </span>
              {/if}
            </h3>
            <p class="text-xs text-zinc-400 mt-0.5">Ambrilindo Embedded Hub &bull; Port 8000</p>
          </div>
        </div>

        <button
          type="button"
          onclick={handleCloseModal}
          aria-label="Tutup"
          class="w-8 h-8 rounded-lg bg-white/[0.06] hover:bg-white/[0.12] text-zinc-400 hover:text-white flex items-center justify-center transition cursor-pointer"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Tab Navigation -->
      {#if isServerRunning}
        <div class="flex items-center border-b border-white/[0.08] px-5 bg-[#14161d] shrink-0 text-xs font-semibold text-zinc-400 gap-4">
          <button
            type="button"
            onclick={() => (activeTab = "info")}
            class="py-2.5 border-b-2 transition {activeTab === 'info' ? 'border-white text-white' : 'border-transparent hover:text-zinc-200'}"
          >
            Koneksi &amp; Info
          </button>
          <button
            type="button"
            onclick={() => (activeTab = "qr")}
            class="py-2.5 border-b-2 transition {activeTab === 'qr' ? 'border-white text-white' : 'border-transparent hover:text-zinc-200'}"
          >
            QR Code Tablet
          </button>
          <button
            type="button"
            onclick={() => (activeTab = "ips")}
            class="py-2.5 border-b-2 transition {activeTab === 'ips' ? 'border-white text-white' : 'border-transparent hover:text-zinc-200'}"
          >
            Antarmuka IP ({networkInfo?.available_ips?.length || 1})
          </button>
        </div>
      {/if}

      <!-- Modal Body Content -->
      <div class="p-5 overflow-y-auto space-y-4 text-xs">
        {#if isServerRunning}
          {#if activeTab === "info"}
            <!-- Host Address Card -->
            <div class="bg-[#181a22] border border-white/[0.08] rounded-xl p-3.5 space-y-2">
              <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-400 block">
                URL Akses Jaringan Lokal (WiFi/LAN)
              </span>
              <div class="flex items-center justify-between gap-2 bg-[#0e1015] border border-white/[0.08] p-2.5 rounded-lg">
                <span class="font-mono text-xs sm:text-sm font-bold text-emerald-400 truncate">
                  {networkInfo?.server_url || "http://127.0.0.1:8000"}
                </span>
                <div class="flex items-center gap-1.5 shrink-0">
                  <button
                    type="button"
                    onclick={copyUrlToClipboard}
                    class="px-2.5 py-1 rounded-md bg-white/[0.08] hover:bg-white/[0.15] text-white font-semibold text-[11px] transition cursor-pointer flex items-center gap-1"
                  >
                    {#if copied}
                      <span class="text-emerald-400 font-bold">✓ Tersalin</span>
                    {:else}
                      <span>Salin</span>
                    {/if}
                  </button>
                  <button
                    type="button"
                    onclick={openInBrowser}
                    class="px-2.5 py-1 rounded-md bg-white text-black hover:bg-zinc-200 font-bold text-[11px] transition cursor-pointer"
                  >
                    Buka
                  </button>
                </div>
              </div>
              <p class="text-[11px] text-zinc-400 leading-relaxed">
                Perangkat wasit VR, tablet juri, coach supervisor, dan TV scoreboard dapat membuka alamat di atas dari browser apa pun yang terhubung ke WiFi/LAN yang sama.
              </p>
            </div>

            <!-- Server Metrics Grid -->
            <div class="grid grid-cols-2 gap-2.5">
              <div class="bg-[#181a22] border border-white/[0.08] p-3 rounded-xl">
                <span class="text-[10px] text-zinc-400 uppercase font-bold block mb-1">Port Server</span>
                <span class="font-mono text-sm font-bold text-white">8000 (TCP/HTTP/WS)</span>
              </div>
              <div class="bg-[#181a22] border border-white/[0.08] p-3 rounded-xl">
                <span class="text-[10px] text-zinc-400 uppercase font-bold block mb-1">Bind Address</span>
                <span class="font-mono text-sm font-bold text-white">0.0.0.0 (Semua)</span>
              </div>
            </div>

            {#if eventName}
              <div class="bg-[#181a22] border border-white/[0.08] p-3 rounded-xl">
                <span class="text-[10px] text-zinc-400 uppercase font-bold block mb-0.5">Event Turnamen Aktif</span>
                <span class="font-semibold text-white text-xs">{eventName}</span>
              </div>
            {/if}

          {:else if activeTab === "qr"}
            <!-- QR Code Card for Mobile / Tablet Scan -->
            <div class="flex flex-col items-center justify-center p-4 bg-[#181a22] border border-white/[0.08] rounded-xl text-center space-y-3">
              <div class="p-3 bg-white rounded-xl shadow-lg inline-flex items-center justify-center max-w-[220px] max-h-[220px]">
                {#if networkInfo?.qr_svg}
                  <div class="w-48 h-48">
                    {@html networkInfo.qr_svg}
                  </div>
                {:else}
                  <div class="w-48 h-48 flex items-center justify-center text-black font-mono text-xs">
                    Memuat QR...
                  </div>
                {/if}
              </div>
              <div>
                <p class="text-xs font-bold text-white">Pindai dengan Kamera HP / Tablet</p>
                <p class="text-[11px] text-zinc-400 mt-0.5">
                  Membuka langsung Portal Venue Lapangan untuk memilih peran Wasit, Juri, atau Scoreboard
                </p>
              </div>
            </div>

          {:else if activeTab === "ips"}
            <!-- Available Network Interfaces -->
            <div class="space-y-2">
              <span class="text-[10px] font-bold uppercase tracking-wider text-zinc-400 block px-1">
                Alamat IP yang Tersedia di Komputer Ini
              </span>
              {#if networkInfo?.available_ips}
                <div class="space-y-1.5 max-h-56 overflow-y-auto pr-1">
                  {#each networkInfo.available_ips as item}
                    <div class="bg-[#181a22] border border-white/[0.08] p-2.5 rounded-xl flex items-center justify-between">
                      <div>
                        <div class="font-mono text-xs font-bold text-white flex items-center gap-1.5">
                          <span>{item.ip}</span>
                          {#if item.is_default}
                            <span class="px-1.5 py-0.2 rounded text-[9px] font-sans font-bold bg-emerald-500/20 text-emerald-300">
                              Default
                            </span>
                          {/if}
                        </div>
                        <span class="text-[10px] text-zinc-400">{item.name}</span>
                      </div>
                      <span class="text-[10px] font-mono text-zinc-500">:8000</span>
                    </div>
                  {/each}
                </div>
              {:else}
                <p class="text-zinc-500 text-xs">Tidak ada data interface tambahan.</p>
              {/if}
            </div>
          {/if}

          <!-- Danger Section: Stop Server -->
          <div class="pt-3 border-t border-white/[0.08] mt-4">
            <div class="bg-rose-500/10 border border-rose-500/25 p-3 rounded-xl flex items-center justify-between gap-3">
              <div>
                <span class="text-xs font-bold text-rose-300 block">Matikan Server Arena</span>
                <span class="text-[10px] text-zinc-400 block mt-0.5">
                  Melepaskan port 8000 &amp; memutuskan koneksi perangkat lapangan
                </span>
              </div>
              <button
                type="button"
                onclick={handleStopServer}
                disabled={isStopping}
                class="px-3.5 py-2 rounded-lg bg-rose-600 hover:bg-rose-500 active:scale-95 text-white font-bold text-xs shadow-md transition cursor-pointer shrink-0 disabled:opacity-50"
              >
                {isStopping ? "Mematikan..." : "Matikan Server"}
              </button>
            </div>
          </div>

        {:else}
          <!-- Offline State View -->
          <div class="text-center py-6 space-y-3">
            <div class="w-12 h-12 rounded-full bg-zinc-800 text-zinc-400 flex items-center justify-center mx-auto">
              <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M18.364 5.636a9 9 0 010 12.728m0 0l-2.829-2.829m2.829 2.829L21 21M15.536 8.464a5 5 0 010 7.072m0 0l-2.829-2.829m-4.243 4.243a9 9 0 01-12.728 0m0 0l2.829-2.829m-2.829 2.829L3 21m2.829-5.657a5 5 0 010-7.072m0 0l2.829 2.829" />
              </svg>
            </div>
            <div>
              <h4 class="text-sm font-bold text-white">Server Arena Sedang Offline</h4>
              <p class="text-xs text-zinc-400 mt-1 max-w-sm mx-auto leading-relaxed">
                Aplikasi saat ini berjalan dalam mode mandiri (Solo). Perangkat lain di jaringan WiFi tidak dapat mengakses portal lapangan sampai server dinyalakan.
              </p>
            </div>
            <button
              type="button"
              onclick={handleStartServer}
              disabled={isStarting}
              class="mt-2 px-4 py-2 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition cursor-pointer"
            >
              {isStarting ? "Menyalakan..." : "Mulai Server Arena Sekarang"}
            </button>
          </div>
        {/if}
      </div>

      <!-- Modal Footer -->
      <div class="p-3 bg-[#111217] border-t border-white/[0.08] flex items-center justify-between text-[11px] text-zinc-400 shrink-0">
        <span class="font-mono text-zinc-500">Ambrilindo v3.0</span>
        <button
          type="button"
          onclick={handleCloseModal}
          class="px-3 py-1 bg-white/[0.08] hover:bg-white/[0.15] text-white font-medium rounded-lg transition cursor-pointer"
        >
          Tutup
        </button>
      </div>
    </div>
  </div>
{/if}
