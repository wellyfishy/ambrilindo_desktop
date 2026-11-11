<script lang="ts">
  // One official's seat as in Ambrilindo Local: position badge, photo, name, perguruan • kota.
  // Tatami Manager (tap to choose, "Lepas") and Chief Referee (read only).
  import type { Seat, SeatHolder } from "$lib/refereeSeats";
  import RefereeAvatar from "./RefereeAvatar.svelte";

  interface Props {
    seat: Seat;
    holder: SeatHolder | null;
    /** conflict with AKA / AO, if any */
    conflict?: string | null;
    /** seated on another tatami at the same time (double booking), if so */
    busy?: string | null;
    /** no choosing / releasing (chief referee) */
    readonly?: boolean;
    disabled?: boolean;
    /** photo size in px */
    photo?: number;
    onOpen?: () => void;
    onRelease?: () => void;
  }
  let { seat, holder, conflict = null, busy = null, readonly = false, disabled = false, photo = 88, onOpen, onRelease }: Props = $props();

  /** Position badge colours as in Ambrilindo Local */
  function badgeClass(key: string): string {
    if (key === "referee") return "bg-red-700 border-red-500";
    if (key.startsWith("judge")) return "bg-sky-700 border-sky-400";
    if (key === "kansa") return "bg-amber-700 border-amber-500";
    if (key === "score_supervisor") return "bg-purple-800 border-purple-500";
    return "bg-zinc-900 border-zinc-700";
  }

  function open() {
    if (!readonly && !disabled) onOpen?.();
  }
</script>

<!-- a button only when it can be tapped (readonly on the chief referee page) -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  role={readonly ? undefined : "button"}
  tabindex={readonly ? undefined : 0}
  onclick={open}
  onkeydown={(e) => {
    if (!readonly && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      open();
    }
  }}
  class="group relative min-w-0 flex flex-col rounded-xl border-[1.5px] p-2.5 select-none transition {readonly ? '' : 'cursor-pointer hover:-translate-y-0.5'} {busy
    ? 'border-orange-500 bg-orange-950/30'
    : conflict
    ? 'border-yellow-400 bg-yellow-500/[0.12]'
    : holder
      ? `border-white/35 bg-[#11131a] ${readonly ? '' : 'hover:border-white/70'}`
      : `border-dashed border-white/20 bg-[#0f1116] ${readonly ? '' : 'hover:border-white/60'}`}"
  style="min-height: {photo + 144}px"
>
  <div class="flex items-center justify-between flex-wrap gap-1 pb-1.5 mb-2 border-b border-white/[0.07]">
    <span class="max-w-full truncate px-2 py-0.5 rounded text-[10.5px] font-mono font-black uppercase tracking-wide text-white border {badgeClass(seat.key)}">
      {seat.short === "WASIT" ? "Wasit" : seat.label}
    </span>
    {#if busy}
      <span class="px-1.5 py-0.5 rounded text-[9.5px] font-black bg-orange-600 border border-orange-400 text-white">⛔ BENTROK</span>
    {:else if conflict}
      <span class="px-1.5 py-0.5 rounded text-[9.5px] font-black bg-yellow-400 border border-yellow-300 text-black">⚠ KONFLIK</span>
    {:else if holder}
      <span class="px-1.5 py-0.5 rounded text-[9.5px] font-black bg-emerald-600 border border-emerald-400 text-white">✓ OK</span>
    {/if}
  </div>

  <div class="flex-1 flex flex-col items-center text-center gap-1">
    <div class="relative mb-1.5 {holder ? '' : 'opacity-40 grayscale group-hover:opacity-80'}">
      <RefereeAvatar foto={holder?.foto} nama={holder?.nama_wasit ?? seat.short} size={photo} rounded="rounded-[14px]" />
      {#if holder && !readonly}
        <div class="absolute inset-0 rounded-[14px] bg-black/75 text-white text-xs font-bold flex items-center justify-center opacity-0 group-hover:opacity-100 transition pointer-events-none">
          Ganti
        </div>
      {:else if !holder && !readonly}
        <div class="absolute -bottom-1 -right-1 w-6 h-6 rounded-full bg-white text-black border-2 border-[#0f1116] flex items-center justify-center text-sm font-black">+</div>
      {/if}
    </div>
    {#if holder}
      <div class="w-full text-[13.5px] font-bold text-white leading-tight line-clamp-2" title={holder.nama_wasit}>{holder.nama_wasit}</div>
      <div class="w-full text-xs font-semibold text-zinc-400 leading-tight">
        <span class="text-sky-300 font-bold">{holder.nama_perguruan || "PB / Umum"}</span>{#if holder.kab_kota}<span class="text-zinc-600 mx-1">•</span><span class="text-zinc-200">{holder.kab_kota}</span>{/if}
      </div>
      {#if busy}
        <div class="w-full text-[10px] font-bold text-orange-300 leading-tight">⛔ {busy}</div>
      {/if}
      {#if conflict}
        <div class="w-full text-[10px] font-bold text-yellow-300 leading-tight">⚠ {conflict}</div>
      {/if}
    {:else}
      <div class="text-[13px] font-bold {readonly ? 'text-zinc-500' : 'text-white'}">{readonly ? "Belum diisi" : "+ Pilih Wasit"}</div>
    {/if}
  </div>

  {#if !readonly}
    <div class="mt-2 pt-2 border-t border-white/[0.08] flex items-center justify-between gap-1.5">
      {#if holder}
        <span class="text-[11px] font-semibold text-zinc-400"><span class="text-emerald-400">●</span> Aktif</span>
        <button
          type="button"
          {disabled}
          onclick={(e) => {
            e.stopPropagation();
            onRelease?.();
          }}
          class="px-2 py-1 rounded-md text-[11px] font-bold bg-red-500/10 border border-red-500/30 text-red-300 hover:bg-red-600 hover:text-white transition"
        >
          Lepas
        </button>
      {:else}
        <span class="w-full text-center text-[10.5px] font-semibold text-zinc-500">Kosong</span>
      {/if}
    </div>
  {/if}
</div>
