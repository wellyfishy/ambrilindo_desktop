<script lang="ts">
  import type { NetworkInfo } from "$lib/types";

  interface Props {
    networkInfo: NetworkInfo | null;
    onClose: () => void;
    onShowToast: (msg: string) => void;
  }

  let { networkInfo, onClose, onShowToast }: Props = $props();

  function copyUrl() {
    if (networkInfo?.server_url) {
      navigator.clipboard.writeText(networkInfo.server_url);
      onShowToast("URL server berhasil disalin!");
    }
  }
</script>

<div class="fixed inset-0 bg-black/85 backdrop-blur-sm z-50 flex items-center justify-center p-4 select-none font-sans">
  <div class="bg-[#181a20] border border-white/[0.12] rounded-2xl max-w-sm w-full p-6 shadow-2xl space-y-4 animate-in fade-in zoom-in-95 duration-150 text-center">
    <div class="flex items-center justify-between border-b border-white/[0.08] pb-2">
      <h3 class="text-xs font-bold text-white uppercase tracking-wider">PAIRING TABLET JURI</h3>
      <button
        onclick={onClose}
        class="text-zinc-500 hover:text-white p-1 rounded-lg transition"
      >
        ✕
      </button>
    </div>

    {#if networkInfo}
      <div class="bg-white p-3 rounded-xl w-52 h-52 mx-auto flex items-center justify-center shadow-md">
        {@html networkInfo.qr_svg}
      </div>

      <div>
        <span class="text-[10px] font-mono font-bold text-zinc-500 uppercase tracking-wider block">ALAMAT LAN HOST</span>
        <p class="text-base font-mono font-bold text-white mt-0.5">
          {networkInfo.server_url}
        </p>
        <p class="text-xs text-zinc-400 mt-1">
          Pindai QR code ini pada browser smartphone atau tablet wasit yang terhubung ke Wi-Fi yang sama.
        </p>
      </div>

      <div class="pt-2 flex items-center justify-center space-x-2">
        <button
          onclick={copyUrl}
          class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium text-xs rounded-xl transition"
        >
          Salin URL
        </button>
        <button
          onclick={onClose}
          class="px-4 py-2 bg-white hover:bg-zinc-200 text-black font-bold text-xs rounded-xl shadow transition"
        >
          Tutup
        </button>
      </div>
    {:else}
      <div class="py-8 text-xs text-zinc-500">
        Server LAN belum aktif.
      </div>
    {/if}
  </div>
</div>
