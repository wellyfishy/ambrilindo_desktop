<script lang="ts">
  interface Props {
    isSubmitting: boolean;
    onConfirm: (namaEvent: string, kodeEvent?: string) => void;
    onClose: () => void;
  }

  let { isSubmitting, onConfirm, onClose }: Props = $props();

  let namaEvent = $state("");
  let kodeEvent = $state("");
  let errorMessage = $state("");

  function handleSubmit() {
    errorMessage = "";
    const trimmedNama = namaEvent.trim();
    if (!trimmedNama) {
      errorMessage = "Nama event / kejuaraan wajib diisi.";
      return;
    }
    const trimmedKode = kodeEvent.trim();
    onConfirm(trimmedNama, trimmedKode ? trimmedKode : undefined);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4 select-none font-sans">
  <div class="bg-[#181a20] border border-white/[0.12] rounded-2xl max-w-md w-full p-6 shadow-2xl space-y-5 animate-in zoom-in-95 duration-150">
    <!-- Header -->
    <div class="flex items-center justify-between border-b border-white/[0.08] pb-3">
      <div class="flex items-center space-x-2">
        <div class="w-7 h-7 rounded-lg bg-white/[0.06] border border-white/[0.1] flex items-center justify-center text-zinc-300">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
          </svg>
        </div>
        <h3 class="text-sm font-bold text-white uppercase tracking-wider">TAMBAH EVENT BARU</h3>
      </div>
      <button
        onclick={onClose}
        class="text-zinc-500 hover:text-white transition p-1"
        aria-label="Tutup"
      >
        ✕
      </button>
    </div>

    <!-- Error Banner -->
    {#if errorMessage}
      <div class="p-3 bg-red-500/10 border border-red-500/20 text-red-400 text-xs rounded-xl">
        {errorMessage}
      </div>
    {/if}

    <!-- Form Fields -->
    <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="space-y-4">
      <div class="space-y-1.5">
        <label for="event-name-input" class="text-xs font-bold text-zinc-300 block">
          Nama Event / Kejuaraan <span class="text-red-400">*</span>
        </label>
        <input
          id="event-name-input"
          type="text"
          bind:value={namaEvent}
          placeholder="Contoh: KEJUARAAN KARATE AMBRILINDO OPEN 2026"
          class="w-full bg-[#111215] border border-white/[0.1] text-white text-xs px-3.5 py-2.5 rounded-xl outline-none focus:border-white transition"
          required
        />
        <p class="text-[11px] text-zinc-500">
          Nama lengkap turnamen kejuaraan yang akan dikelola di server lokal.
        </p>
      </div>

      <div class="space-y-1.5">
        <label for="event-code-input" class="text-xs font-bold text-zinc-300 block">
          Kode Event (Opsional)
        </label>
        <input
          id="event-code-input"
          type="text"
          bind:value={kodeEvent}
          placeholder="Contoh: EV-AMB2026 (kosongkan untuk auto-generate)"
          class="w-full bg-[#111215] border border-white/[0.1] text-white font-mono text-xs px-3.5 py-2.5 rounded-xl outline-none focus:border-white transition uppercase"
        />
        <p class="text-[11px] text-zinc-500">
          Kode unik identitas kejuaraan (maksimal 50 karakter alfanumerik).
        </p>
      </div>

      <!-- Action Buttons -->
      <div class="flex items-center justify-end space-x-2 pt-3 border-t border-white/[0.08]">
        <button
          type="button"
          onclick={onClose}
          disabled={isSubmitting}
          class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium text-xs rounded-xl transition"
        >
          Batal
        </button>
        <button
          type="submit"
          disabled={isSubmitting}
          class="px-5 py-2 bg-white hover:bg-zinc-200 text-black font-bold text-xs rounded-xl shadow transition active:scale-[0.98] disabled:opacity-50 flex items-center space-x-1.5"
        >
          {#if isSubmitting}
            <span>Menyimpan...</span>
          {:else}
            <span>Buat Event</span>
          {/if}
        </button>
      </div>
    </form>
  </div>
</div>
