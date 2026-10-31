<script lang="ts">
  import { onMount } from "svelte";

  let tatamis = $state<number[]>([1, 2, 3, 4]);
  let isLoading = $state(true);

  onMount(async () => {
    try {
      const res = await fetch("/api/events");
      if (res.ok) {
        const json = await res.json();
        const evList = json.data || [];
        if (evList.length > 0) {
          const tRes = await fetch(`/api/tatamis/${evList[0].id}`);
          if (tRes.ok) {
            const tJson = await tRes.json();
            if (Array.isArray(tJson.data) && tJson.data.length > 0) {
              tatamis = tJson.data
                .map((t: any) => t.tatami_number || t.id)
                .sort((a: number, b: number) => a - b);
            }
          }
        }
      }
    } catch (_) {
    } finally {
      isLoading = false;
    }
  });
</script>

<svelte:head>
  <title>Pilih Tatami • Panel Juri Ambrilindo</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<div class="min-h-screen bg-[#0c0e12] text-zinc-100 flex flex-col justify-between font-sans select-none p-5 sm:p-8">
  <!-- Top Minimal Header -->
  <header class="max-w-md mx-auto w-full flex items-center justify-between border-b border-white/10 pb-3">
    <div class="flex items-center gap-2">
      <div class="w-7 h-7 rounded-lg bg-white text-black font-black flex items-center justify-center text-xs shadow">
        A
      </div>
      <h1 class="text-xs font-black tracking-widest text-white uppercase">PANEL WASIT / JURI</h1>
    </div>
    <span class="px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 text-[10px] font-mono font-bold">
      LIVE
    </span>
  </header>

  <!-- Main Center Selection -->
  <main class="max-w-md mx-auto w-full my-auto py-6">
    <div class="text-center mb-5">
      <h2 class="text-lg font-black text-white tracking-tight uppercase">PILIH TATAMI</h2>
    </div>

    <!-- Tatami Cards Grid -->
    <div class="grid grid-cols-2 gap-3">
      {#each tatamis as tNum}
        <a
          href="/jury/{tNum}"
          class="bg-[#16181f] hover:bg-[#1f232c] border-2 border-white/10 hover:border-white/30 rounded-2xl p-5 flex flex-col items-center justify-center gap-2 transition-all active:scale-95 shadow-lg group text-center"
        >
          <span class="text-3xl font-black font-mono text-white">{tNum}</span>
          <span class="text-xs font-black font-mono text-zinc-300 tracking-wider">TATAMI {tNum}</span>
        </a>
      {/each}
    </div>

    <!-- Quick Tatami 1 Default Button -->
    <div class="mt-5 pt-4 border-t border-white/[0.08] text-center">
      <a
        href="/jury/1"
        class="inline-flex items-center gap-2 px-5 py-2.5 rounded-xl bg-white text-black hover:bg-zinc-200 text-xs font-black shadow transition active:scale-95"
      >
        <span>Tatami 1</span>
        <span>→</span>
      </a>
    </div>
  </main>

  <!-- Footer -->
  <footer class="max-w-md mx-auto w-full pt-4 border-t border-white/10 text-center text-[10px] text-zinc-500 font-mono">
    Ambrilindo Karate Arena • Mobile Jury Hub
  </footer>
</div>
