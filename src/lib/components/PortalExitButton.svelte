<script lang="ts">
  // "Keluar" back to the venue portal landing page (/), with a confirmation, for the pages
  // opened from the portal on tablets and LAN laptops. Hidden inside the desktop app's own
  // windows, which have their own close buttons.
  import { onMount } from "svelte";

  interface Props {
    /** confirmation title, e.g. "Keluar dari LO Kata?" */
    title?: string;
    /** extra line in the confirmation (e.g. unsaved work) */
    warning?: string | null;
    /** floating button that only shows while the mouse moves (scoreboard on a TV) */
    autohide?: boolean;
  }

  let { title = "Keluar ke halaman utama?", warning = null, autohide = false }: Props = $props();

  let isBrowser = $state(false);
  let confirmOpen = $state(false);
  let awake = $state(false);
  let sleepTimer: ReturnType<typeof setTimeout> | null = null;

  function wake() {
    awake = true;
    if (sleepTimer) clearTimeout(sleepTimer);
    sleepTimer = setTimeout(() => (awake = false), 3000);
  }

  function exitToLanding() {
    window.location.href = "/";
  }

  onMount(() => {
    isBrowser = !(window as any).__TAURI_INTERNALS__;
    if (!isBrowser || !autohide) return;
    window.addEventListener("mousemove", wake);
    window.addEventListener("touchstart", wake);
    return () => {
      window.removeEventListener("mousemove", wake);
      window.removeEventListener("touchstart", wake);
      if (sleepTimer) clearTimeout(sleepTimer);
    };
  });
</script>

{#if isBrowser}
  <button
    type="button"
    onclick={() => (confirmOpen = true)}
    class="h-9 px-3 rounded-xl bg-white/[0.06] hover:bg-rose-500/20 active:bg-rose-500/30 border border-white/10 hover:border-rose-500/40 text-zinc-300 hover:text-rose-200 flex items-center gap-1.5 text-xs font-bold transition active:scale-95 shrink-0 {autohide
      ? `fixed top-3 left-3 z-[90] bg-black/70 backdrop-blur-sm transition-opacity duration-300 ${awake || confirmOpen ? 'opacity-100' : 'opacity-0 pointer-events-none'}`
      : ''}"
    title="Keluar ke halaman utama portal"
  >
    <svg class="w-4 h-4 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
    </svg>
    <span>Keluar</span>
  </button>

  {#if confirmOpen}
    <div class="fixed inset-0 z-[100] bg-black/80 backdrop-blur-sm flex items-center justify-center p-5" role="presentation">
      <div class="w-full max-w-sm bg-[#181a20] border border-white/10 rounded-2xl p-5 shadow-2xl" role="dialog" aria-modal="true" aria-label={title}>
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-rose-500/15 border border-rose-500/30 text-rose-400 flex items-center justify-center shrink-0">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
            </svg>
          </div>
          <h3 class="text-base font-black text-white">{title}</h3>
        </div>
        {#if warning}
          <div class="mt-4 px-3 py-2 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-300 text-xs font-semibold">⚠ {warning}</div>
        {/if}
        <div class="grid grid-cols-2 gap-2 mt-4">
          <button
            type="button"
            onclick={() => (confirmOpen = false)}
            class="h-11 rounded-xl bg-white/[0.08] hover:bg-white/15 border border-white/10 text-zinc-200 font-bold text-sm transition active:scale-95"
          >
            Batal
          </button>
          <button
            type="button"
            onclick={exitToLanding}
            class="h-11 rounded-xl bg-rose-600 hover:bg-rose-500 text-white font-black text-sm shadow-lg shadow-rose-950/40 transition active:scale-95"
          >
            Ya, Keluar
          </button>
        </div>
      </div>
    </div>
  {/if}
{/if}
