<script lang="ts">
  import { onMount } from "svelte";

  let tatamis = $state<number[]>([1, 2, 3, 4]);

  onMount(async () => {
    try {
      const res = await fetch("/api/events");
      if (res.ok) {
        const json = await res.json();
        const ev = json.data?.[0];
        if (ev) {
          const tRes = await fetch(`/api/tatamis/${ev.id}`);
          if (tRes.ok) {
            const tJson = await tRes.json();
            if (tJson.data && tJson.data.length > 0) {
              tatamis = tJson.data.map((t: any) => t.tatami_number);
            }
          }
        }
      }
    } catch {}
  });
</script>

<div class="min-h-screen bg-[#0f1115] text-zinc-200 flex flex-col items-center justify-center p-4 font-sans select-none">
  <div class="w-full max-w-md bg-[#16181e] border border-white/10 rounded-2xl p-6 shadow-2xl text-center space-y-6">
    <div class="space-y-2">
      <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-red-500/10 border border-red-500/20 text-red-400 text-xs font-mono font-bold tracking-wider uppercase">
        <span class="w-2 h-2 rounded-full bg-red-500 animate-pulse"></span>
        <span>Ambrilindo VR Console</span>
      </div>
      <h1 class="text-xl font-black text-white uppercase tracking-tight">
        Pengawas Pelatih (Coach Supervisor)
      </h1>
      <p class="text-xs text-zinc-400">
        Pilih Tatami penugasan Anda untuk mengawasi dan mengajukan kartu Video Review (VR) pelatih:
      </p>
    </div>

    <div class="grid grid-cols-2 gap-3">
      {#each tatamis as tNum}
        <a
          href="/coach-supervisor/{tNum}"
          class="p-4 rounded-xl bg-white/[0.03] hover:bg-white/[0.08] border border-white/10 hover:border-amber-400/50 transition flex flex-col items-center justify-center gap-1.5 active:scale-95 group"
        >
          <span class="text-2xl font-black text-white group-hover:text-amber-400 font-mono">
            {tNum}
          </span>
          <span class="text-[11px] font-bold text-zinc-400 uppercase tracking-wider">
            TATAMI {tNum}
          </span>
        </a>
      {/each}
    </div>

    <div class="pt-4 border-t border-white/5 flex items-center justify-between text-[11px] text-zinc-500">
      <a href="/" class="hover:text-zinc-300 transition">← Kembali ke Beranda</a>
      <a href="/wasit-vr" class="text-amber-400 hover:underline">Panel Wasit VR →</a>
    </div>
  </div>
</div>
