<!-- LO / Guide entry: pick the tatami of the running event. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { getHostBaseUrl } from "$lib/api_client";

  let eventName = $state("");
  let eventId = $state<number | null>(null);
  let tatamis = $state<number[]>([]);
  let status = $state<"loading" | "ready" | "error">("loading");

  async function load() {
    status = "loading";
    try {
      const base = getHostBaseUrl();
      const evRes = await fetch(`${base}/api/events`);
      const events = (await evRes.json())?.data || [];
      if (!evRes.ok || events.length === 0) throw new Error("no event");
      // Newest event first (the host lists events by id, newest on top).
      const ev = [...events].sort((a: any, b: any) => b.id - a.id)[0];
      eventName = ev.nama_event || "";
      eventId = ev.id;
      const tRes = await fetch(`${base}/api/tatamis/${ev.id}`);
      const list = (await tRes.json())?.data || [];
      tatamis = list
        .map((t: any) => Number(t.tatami_number))
        .filter((n: number) => n > 0)
        .sort((a: number, b: number) => a - b);
      status = "ready";
    } catch (_) {
      status = "error";
    }
  }

  onMount(load);
</script>

<svelte:head>
  <title>Pilih Tatami • LO / Guide</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<div class="min-h-screen bg-[#0c0e12] text-zinc-100 flex flex-col font-sans select-none p-5">
  <div class="max-w-lg mx-auto w-full flex-1 flex flex-col justify-center">
    <h1 class="text-xl font-black text-white tracking-tight uppercase text-center">Panel LO / Guide</h1>
    {#if eventName}
      <p class="text-xs text-zinc-400 text-center mt-1">{eventName}</p>
    {/if}

    {#if status === "loading"}
      <div class="mt-8 text-center text-sm text-zinc-400">Memuat tatami…</div>
    {:else if status === "error"}
      <div class="mt-8 text-center">
        <div class="text-sm text-rose-300 font-semibold">Tidak bisa terhubung ke meja pertandingan.</div>
        <button type="button" onclick={load} class="mt-4 h-11 px-5 rounded-xl bg-white text-black font-black text-sm">Coba lagi</button>
      </div>
    {:else if tatamis.length === 0}
      <div class="mt-8 text-center text-sm text-zinc-400">Event ini belum punya tatami.</div>
    {:else}
      <div class="grid grid-cols-2 gap-3 mt-6">
        {#each tatamis as t (t)}
          <a
            href="/lo-kata/{t}?event={eventId}"
            class="bg-[#16181f] border-2 border-white/10 active:border-white/40 rounded-2xl py-7 flex flex-col items-center gap-1 active:scale-95 transition"
          >
            <span class="text-5xl font-black font-mono text-white leading-none">{t}</span>
            <span class="text-xs font-black font-mono text-zinc-400 tracking-widest">TATAMI</span>
          </a>
        {/each}
      </div>
    {/if}

    <div class="mt-8 text-center">
      <a
        href="/"
        class="inline-flex items-center gap-2 text-xs font-semibold text-zinc-400 hover:text-zinc-200 transition py-2 px-3 rounded-lg hover:bg-white/[0.05]"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18"/>
        </svg>
        <span>Kembali ke Beranda Website</span>
      </a>
    </div>
  </div>
</div>
