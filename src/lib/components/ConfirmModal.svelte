<script lang="ts">
  interface Props {
    open: boolean;
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    isDestructive?: boolean;
    isLoading?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let {
    open,
    title,
    message,
    confirmText = "Lanjutkan",
    cancelText = "Batal",
    isDestructive = true,
    isLoading = false,
    onConfirm,
    onCancel,
  }: Props = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape" && !isLoading) {
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs animate-in fade-in duration-150"
    onclick={(e) => {
      if (e.target === e.currentTarget && !isLoading) onCancel();
    }}
    onkeydown={(e) => {
      if (e.key === "Escape" && !isLoading) onCancel();
    }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div
      class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-md w-full relative animate-in zoom-in-95 duration-150 text-zinc-200"
    >
      <div class="flex items-start space-x-3.5">
        <div
          class="w-10 h-10 rounded-xl flex items-center justify-center shrink-0 {isDestructive
            ? 'bg-rose-500/15 text-rose-400 border border-rose-500/25'
            : 'bg-amber-500/15 text-amber-400 border border-amber-500/25'}"
        >
          {#if isDestructive}
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
              />
            </svg>
          {:else}
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
          {/if}
        </div>

        <div class="flex-1 min-w-0">
          <h3 class="text-base font-bold text-white tracking-tight uppercase">
            {title}
          </h3>
          <p class="text-xs text-zinc-300 mt-1.5 leading-relaxed break-words whitespace-pre-line">
            {message}
          </p>
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2.5 mt-6 pt-4 border-t border-white/[0.08]">
        <button
          type="button"
          onclick={onCancel}
          disabled={isLoading}
          class="h-9 px-4 bg-white/[0.06] hover:bg-white/[0.1] text-zinc-300 font-semibold text-xs rounded-xl border border-white/[0.1] transition-all disabled:opacity-50 cursor-pointer"
        >
          {cancelText}
        </button>

        <button
          type="button"
          onclick={onConfirm}
          disabled={isLoading}
          class="h-9 px-4 {isDestructive
            ? 'bg-rose-600 hover:bg-rose-500 text-white'
            : 'bg-white text-black hover:bg-zinc-200'} font-bold text-xs rounded-xl shadow-md transition-all active:scale-95 flex items-center space-x-1.5 disabled:opacity-50 cursor-pointer"
        >
          {#if isLoading}
            <span class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
            <span>Memproses...</span>
          {:else}
            <span>{confirmText}</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
