<script lang="ts">
  // Live view of the 4 judges' sticks (mouse mode) for the kumite control panel.
  // Display only: all state and scoring live in ControlPanelView / judgeSticks.ts.
  import { JUDGE_KEYS, KEY_MAP, LEVEL_SHORT, type SideResult, type StickHistoryRow, type StickState } from "$lib/judgeSticks";

  let {
    votes = [null, null, null, null],
    aka = null,
    ao = null,
    hantei = false,
    hanteiTally = { aka: 0, ao: 0 },
    timerRunning = false,
    checkMode = false,
    tested = [],
    lastKey = null,
    history = [],
    onToggleCheck,
    onResetCheck,
    readOnly = false,
    historyAlwaysOpen = false,
  }: {
    votes?: StickState["votes"];
    aka?: SideResult | null;
    ao?: SideResult | null;
    hantei?: boolean;
    hanteiTally?: { aka: number; ao: number };
    timerRunning?: boolean;
    checkMode?: boolean;
    tested?: string[];
    lastKey?: string | null;
    history?: StickHistoryRow[];
    onToggleCheck?: () => void;
    onResetCheck?: () => void;
    /** watching from another device (Admin Control): no stick-test controls */
    readOnly?: boolean;
    historyAlwaysOpen?: boolean;
  } = $props();

  let showHistoryToggle = $state(false);
  let showHistory = $derived(historyAlwaysOpen || showHistoryToggle);

  function voteLabel(v: StickState["votes"][number]): string {
    if (!v) return "-";
    const side = v.side === "aka" ? "AKA" : "AO";
    return hantei ? side : `${side} ${LEVEL_SHORT[v.level]}`;
  }

  let resultText = $derived.by(() => {
    if (hantei) {
      const { aka: a, ao: o } = hanteiTally;
      if (a === 0 && o === 0) return null;
      if (a > o) return { text: `HANTEI AKA ${a}–${o}`, tone: "aka" };
      if (o > a) return { text: `HANTEI AO ${o}–${a}`, tone: "ao" };
      return { text: `HANTEI SERI ${a}–${o}`, tone: "seri" };
    }
    const parts: { text: string; tone: string }[] = [];
    if (aka && aka.level) parts.push({ text: `AKA ${LEVEL_SHORT[aka.level]} · ${aka.judges} juri`, tone: "aka" });
    if (ao && ao.level) parts.push({ text: `AO ${LEVEL_SHORT[ao.level]} · ${ao.judges} juri`, tone: "ao" });
    if (parts.length === 0) return null;
    return parts.length === 1 ? parts[0] : { text: parts.map((p) => p.text).join("  |  "), tone: "seri" };
  });

  const toneClass: Record<string, string> = {
    aka: "bg-red-600/20 border-red-500 text-red-200",
    ao: "bg-blue-600/20 border-blue-500 text-blue-200",
    seri: "bg-amber-500/20 border-amber-500 text-amber-200",
    none: "bg-[#111216] border-white/10 text-zinc-400",
  };
</script>

<div class="w-full bg-[#111216] border border-white/10 rounded-xl p-2.5 flex flex-col gap-2 text-xs font-mono">
  <div class="flex items-center gap-2">
    <span class="font-bold uppercase tracking-wider text-zinc-300">Monitor Juri</span>
    {#if checkMode}
      <span class="px-1.5 py-0.5 rounded bg-amber-500 text-black font-black text-[10px]">UJI STICK</span>
    {:else if hantei}
      <span class="px-1.5 py-0.5 rounded bg-rose-600 text-white font-black text-[10px]">HANTEI</span>
    {:else if !timerRunning}
      <span class="px-1.5 py-0.5 rounded bg-white/[0.06] text-zinc-400 font-bold text-[10px]" title="Banner poin hanya muncul saat timer berjalan">YAME</span>
    {/if}
    <div class="ml-auto flex items-center gap-1">
      {#if !readOnly}
      <button
        type="button"
        onclick={onToggleCheck}
        class="h-7 px-2 rounded-md border font-bold transition {checkMode ? 'bg-amber-500 border-amber-400 text-black' : 'bg-[#1e2029] border-white/10 text-zinc-300 hover:text-white'}"
        title="Uji 24 tombol stick juri (tampil juga di TV)"
      >
        Uji Stick
      </button>
      {/if}
      {#if !checkMode && !historyAlwaysOpen}
        <button
          type="button"
          onclick={() => (showHistoryToggle = !showHistoryToggle)}
          class="h-7 px-2 rounded-md border font-bold transition {showHistory ? 'bg-white/[0.08] border-white/20 text-white' : 'bg-[#1e2029] border-white/10 text-zinc-300 hover:text-white'}"
        >
          Riwayat
        </button>
      {/if}
    </div>
  </div>

  {#if checkMode}
    <!-- 4 judges × (AKA 1 2 3 / AO 1 2 3): lit once each button has been pressed -->
    <div class="flex items-center justify-between text-[11px] text-zinc-400">
      <span>Tekan setiap tombol stick sekali</span>
      <span class="font-black text-white">{tested.length} / 24</span>
    </div>
    <!-- 4 judges side by side; top row AKA 1-2-3, bottom row AO 1-2-3 -->
    <div class="grid grid-cols-4 gap-1.5">
      {#each JUDGE_KEYS as keys, j}
        {@const done = keys.filter((k) => tested.includes(k)).length}
        <div class="rounded-lg border p-1 {done === 6 ? 'border-emerald-500/60 bg-emerald-500/5' : 'border-white/10'}">
          <div class="flex items-center justify-between mb-1 px-0.5 text-[9px] font-bold">
            <span class="text-zinc-300">J{j + 1}</span>
            <span class={done === 6 ? "text-emerald-400" : "text-zinc-500"}>{done}/6</span>
          </div>
          <div class="grid grid-cols-3 gap-0.5">
            {#each keys as k}
              {@const info = KEY_MAP[k]}
              {@const hit = tested.includes(k)}
              <div
                class="h-5 rounded flex items-center justify-center text-[10px] font-black border transition
                  {hit ? (info.side === 'aka' ? 'bg-red-600 border-red-400 text-white' : 'bg-blue-600 border-blue-400 text-white') : info.side === 'aka' ? 'bg-red-950/40 border-red-500/20 text-red-300/50' : 'bg-blue-950/40 border-blue-500/20 text-blue-300/50'}
                  {lastKey === k ? 'ring-2 ring-amber-400' : ''}"
                title="{info.side === 'aka' ? 'AKA' : 'AO'} {info.level} (tombol '{k}')"
              >
                {info.level}
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
    {#if !readOnly}
      <button
        type="button"
        onclick={onResetCheck}
        class="self-end h-7 px-2.5 rounded-md bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-300 font-bold"
      >
        Ulangi Uji
      </button>
    {/if}
  {:else}
    <div class="grid grid-cols-4 gap-1.5">
      {#each votes as v, j}
        <div
          class="rounded-lg border px-1 py-1.5 text-center transition
            {v ? (v.side === 'aka' ? 'bg-red-600/20 border-red-500' : 'bg-blue-600/20 border-blue-500') : 'bg-[#1e2029] border-white/5'}"
        >
          <div class="text-[9px] font-bold text-zinc-500">JURI {j + 1}</div>
          <div class="text-[11px] font-black truncate {v ? 'text-white' : 'text-zinc-600'}">{voteLabel(v)}</div>
        </div>
      {/each}
    </div>

    <div class="h-8 rounded-lg border flex items-center justify-center font-black text-[12px] {toneClass[resultText?.tone ?? 'none']}">
      {resultText ? resultText.text : "Menunggu sinyal juri…"}
    </div>

    {#if showHistory}
      <div class="{historyAlwaysOpen ? 'flex-1 min-h-0' : 'max-h-40'} overflow-y-auto rounded-lg border border-white/10">
        {#if history.length === 0}
          <div class="py-2 text-center text-zinc-500 text-[11px]">Belum ada sinyal</div>
        {:else}
          <table class="w-full text-[10.5px]">
            <thead class="sticky top-0 bg-[#181a20] text-zinc-500">
              <tr><th class="py-1">Waktu</th><th>J1</th><th>J2</th><th>J3</th><th>J4</th><th>Hasil</th></tr>
            </thead>
            <tbody>
              {#each history as row (row.incidentId)}
                <tr class="border-t border-white/5 text-center">
                  <td class="py-1 text-zinc-300">{row.time}</td>
                  {#each row.votes as v}
                    <td class={v ? (v.side === "aka" ? "text-red-300" : "text-blue-300") : "text-zinc-600"}>{voteLabel(v)}</td>
                  {/each}
                  <td class="font-black {row.tone === 'aka' ? 'text-red-300' : row.tone === 'ao' ? 'text-blue-300' : row.tone === 'seri' ? 'text-amber-300' : 'text-zinc-500'}">{row.result}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    {/if}
  {/if}
</div>
