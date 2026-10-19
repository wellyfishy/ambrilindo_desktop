<script lang="ts">
  interface Props {
    errorMessage: string;
    isRetrying: boolean;
    onRetry: () => void;
    onClose: () => void;
  }

  let { errorMessage, isRetrying, onRetry, onClose }: Props = $props();
</script>

<div class="fixed inset-0 bg-black/90 backdrop-blur-md z-50 flex items-center justify-center p-4 select-none font-sans">
  <div class="bg-[#181a20] border border-red-500/30 rounded-2xl max-w-lg w-full p-6 shadow-2xl space-y-4 animate-in zoom-in-95 duration-150">
    <div class="flex items-start space-x-3.5">
      <div class="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-400 shrink-0">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
        </svg>
      </div>
      <div>
        <h3 class="text-sm font-bold text-white uppercase tracking-wider">
          PORT 8000 SEDANG DIGUNAKAN
        </h3>
        <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
          Server turnamen tidak dapat dimulai karena port <strong>8000</strong> sedang dipakai oleh proses lain pada komputer ini.
        </p>
      </div>
    </div>

    <div class="bg-black/40 p-3.5 rounded-xl border border-red-500/20 text-xs font-mono text-red-300 break-words">
      {errorMessage}
    </div>

    <div class="text-xs text-zinc-400 space-y-1">
      <p class="font-bold text-zinc-200">Langkah Penyelesaian:</p>
      <ol class="list-decimal list-inside space-y-1 text-zinc-400">
        <li>Tutup server Django legacy yang masih aktif di terminal.</li>
        <li>Pastikan tidak ada aplikasi lokal lain yang memakai port 8000.</li>
        <li>Klik tombol <strong>"Coba Lagi"</strong> setelah menutup aplikasi tersebut.</li>
      </ol>
    </div>

    <div class="flex items-center justify-end space-x-2 pt-3 border-t border-white/[0.08]">
      <button
        onclick={onClose}
        class="px-4 py-2 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-medium text-xs rounded-xl transition"
      >
        Abaikan Sementara
      </button>
      <button
        onclick={onRetry}
        disabled={isRetrying}
        class="px-5 py-2 bg-red-600 hover:bg-red-500 text-white font-bold text-xs rounded-xl shadow transition active:scale-95 disabled:opacity-50"
      >
        {isRetrying ? 'Memeriksa...' : 'Coba Lagi'}
      </button>
    </div>
  </div>
</div>
