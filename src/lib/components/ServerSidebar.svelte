<script lang="ts">
  import type { EventItem } from "$lib/types";

  interface Props {
    events: EventItem[];
    selectedEventId: number | null;
    onSelectEvent: (eventId: number) => void;
    onAddEvent: () => void;
    onReturnToLanding: () => void;
  }

  let {
    events,
    selectedEventId,
    onSelectEvent,
    onAddEvent,
    onReturnToLanding,
  }: Props = $props();

  function getEventSingkatan(nama: string | null): string {
    if (!nama) return "EV";
    const words = nama.trim().split(/\s+/).filter((w) => w.length > 0);
    if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
    if (words.length === 2) return (words[0][0] + words[1][0]).toUpperCase();
    // 3 or more words: take first 2-3 initials (e.g. "Kejuaraan Karate Ambrilindo" -> "KKA" or "KK")
    return (words[0][0] + words[1][0] + (words[2] ? words[2][0] : "")).slice(0, 3).toUpperCase();
  }
</script>

<nav class="no-print w-[72px] bg-[#0d0e12] flex flex-col items-center py-3 select-none shrink-0 z-20 border-r border-white/[0.08]">
  <!-- Top Home / Gateway Switcher Icon -->
  <div class="relative group flex items-center justify-center w-full my-1">
    <div
      class="absolute left-0 w-1 bg-white rounded-r-full transition-all duration-200 h-0 group-hover:h-5"
    ></div>

    <button
      onclick={onReturnToLanding}
      class="w-12 h-12 rounded-2xl bg-white/[0.04] hover:bg-white text-zinc-400 hover:text-black flex items-center justify-center transition-all duration-200 border border-white/[0.08] hover:border-white shadow-sm"
      title="Kembali ke Mode Gateway"
    >
      <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
      </svg>
    </button>
  </div>

  <!-- Divider -->
  <div class="w-8 h-[1px] bg-white/[0.08] my-2"></div>

  <!-- Events List as Individual Servers -->
  <div class="flex-1 w-full overflow-y-auto space-y-2 py-1 scrollbar-none flex flex-col items-center">
    {#each events as ev}
      {@const isSelected = selectedEventId === ev.id}
      {@const singkatan = getEventSingkatan(ev.nama_event)}
      <div class="relative group flex items-center justify-center w-full">
        <!-- Discord-style active/hover indicator pill on left edge -->
        <div
          class="absolute left-0 w-1 bg-white rounded-r-full transition-all duration-200 {isSelected ? 'h-10' : 'h-0 group-hover:h-5'}"
        ></div>

        <button
          onclick={() => onSelectEvent(ev.id)}
          class="w-12 h-12 flex items-center justify-center transition-all duration-200 font-mono font-bold text-xs tracking-wider border {isSelected ? 'rounded-2xl bg-white text-black border-white shadow-lg' : 'rounded-full bg-[#181a20] hover:bg-white/[0.08] text-zinc-300 hover:text-white border-white/[0.08] hover:rounded-2xl'}"
          title={ev.nama_event || 'Event'}
        >
          {singkatan}
        </button>
      </div>
    {/each}

    <!-- Add Event Icon (+) - Discord Style Server Creator -->
    <div class="relative group flex items-center justify-center w-full pt-1">
      <button
        onclick={onAddEvent}
        class="w-12 h-12 rounded-full bg-[#181a20] hover:bg-emerald-500 hover:text-slate-950 text-zinc-400 border border-white/[0.08] hover:border-emerald-400 flex items-center justify-center transition-all duration-200 group-hover:rounded-2xl shadow-sm"
        title="Tambah Event / Kejuaraan Baru"
      >
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
        </svg>
      </button>
    </div>
  </div>
</nav>
