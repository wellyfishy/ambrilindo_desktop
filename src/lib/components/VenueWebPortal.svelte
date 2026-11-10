<script lang="ts">
  import { onMount } from "svelte";
  import type { EventItem, TatamiItem } from "$lib/types";
  import { getHostBaseUrl } from "$lib/api_client";

  let events = $state<EventItem[]>([]);
  let selectedEventId = $state<number | null>(null);
  let tatamis = $state<number[]>([1, 2, 3, 4]);
  let selectedTatami = $state<number>(1);
  let selectedRole = $state<string>("wasit_vr");
  let isLoading = $state(true);
  let hostUrl = $state("");
  let loadError = $state("");
  let hostOnline = $derived(events.length > 0 && !loadError);

  interface RoleOption {
    id: string;
    title: string;
    subtitle: string;
    badge?: string;
    color: string;
    icon: string;
    getUrl: (tatami: number, eventId: number | null) => string;
  }

  // In pairs that work together (shown side by side): VR, monitoring, kata, operator + board
  const roleOptions: RoleOption[] = [
    {
      id: "wasit_vr",
      title: "Wasit VR",
      subtitle: "Meja Wasit Video Review",
      badge: "VR Table",
      color: "amber",
      icon: "video",
      getUrl: (t, e) => `/wasit-vr/${t}${e ? `?event=${e}` : ""}`,
    },
    {
      id: "coach_sup",
      title: "Coach Supervisor",
      subtitle: "Panel Challenge Pelatih VR",
      badge: "Challenge",
      color: "rose",
      icon: "whistle",
      getUrl: (t, e) => `/coach-supervisor/${t}${e ? `?event=${e}` : ""}`,
    },
    {
      id: "admin_control",
      title: "Admin Control",
      subtitle: "Monitor Stick / Mouse Juri",
      badge: "Monitor",
      color: "purple",
      icon: "sliders",
      getUrl: (t, e) => `/admin-control/${t}${e ? `?event=${e}` : ""}`,
    },
    {
      id: "tatami_manager",
      title: "Tatami Manager",
      subtitle: "Atur Wasit & Juri per Partai",
      badge: "Manager",
      color: "zinc",
      icon: "shield",
      getUrl: (t, e) => `/tatami-manager/${t}${e ? `?event=${e}` : ""}`,
    },
    {
      id: "lo_kata",
      title: "LO Kata",
      subtitle: "Pencatatan & Cek Kata Ulang",
      badge: "LO Table",
      color: "cyan",
      icon: "clipboard",
      getUrl: (t, e) => `/lo-kata/${t}${e ? `?event=${e}` : ""}`,
    },
    {
      id: "juri",
      title: "Juri Tatami",
      subtitle: "Tablet Nilai Poin & Bendera",
      badge: "Jury Tablet",
      color: "emerald",
      icon: "flag",
      getUrl: (t, e) => `/jury/${t}${e ? `?event=${e}` : ""}`,
    },
    {
      id: "admtatami",
      title: "Admin Tatami",
      subtitle: "Operator Scoring Lapangan",
      badge: "Operator",
      color: "blue",
      icon: "monitor",
      getUrl: (t, e) => `/control-panel?tatami=${t}${e ? `&event=${e}` : ""}`,
    },
    {
      id: "scoreboard",
      title: "Scoreboard Arena",
      subtitle: "Layar TV / Proyektor Arena",
      badge: "Display",
      color: "yellow",
      icon: "tv",
      getUrl: (t, e) => `/scoreboard?tatami=${t}${e ? `&event=${e}` : ""}`,
    },
  ];

  let activeRoleInfo = $derived(
    roleOptions.find((r) => r.id === selectedRole) || roleOptions[0]
  );

  // Newest event first (the host lists them that way). Retries until the host answers,
  // so a tablet switched on before the host (or on a flaky LAN) still gets the list.
  let retryTimer: ReturnType<typeof setTimeout> | null = null;
  async function loadEvents() {
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/events`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = await res.json();
      const evList: EventItem[] = json.data || [];
      events = evList;
      loadError = evList.length ? "" : "Belum ada event di meja utama";
      if (evList.length > 0 && !evList.some((e) => e.id === selectedEventId)) {
        selectedEventId = evList[0].id;
        await loadTatamisForEvent(evList[0].id);
      }
    } catch (err) {
      loadError = "Meja utama belum terhubung, mencoba lagi…";
      retryTimer = setTimeout(loadEvents, 4000);
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    if (typeof window !== "undefined") {
      hostUrl = getHostBaseUrl();
    }
    loadEvents();
    return () => {
      if (retryTimer) clearTimeout(retryTimer);
    };
  });

  async function handleEventChange(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    const id = parseInt(val, 10);
    if (!isNaN(id)) {
      selectedEventId = id;
      await loadTatamisForEvent(id);
    }
  }

  async function loadTatamisForEvent(eventId: number) {
    try {
      const res = await fetch(`${getHostBaseUrl()}/api/tatamis/${eventId}`);
      if (res.ok) {
        const json = await res.json();
        const list: TatamiItem[] = json.data || [];
        if (Array.isArray(list) && list.length > 0) {
          tatamis = list.map((t) => t.tatami_number || t.id).sort((a, b) => a - b);
          if (!tatamis.includes(selectedTatami)) {
            selectedTatami = tatamis[0];
          }
        }
      }
    } catch (_) {}
  }

  function handleNavigate() {
    const targetUrl = activeRoleInfo.getUrl(selectedTatami, selectedEventId);
    window.location.href = targetUrl;
  }
</script>

<svelte:head>
  <title>Portal Lapangan &bull; Ambrilindo Arena</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
</svelte:head>

<!-- Scrolls on its own: the page shell around it is fixed to the screen height -->
<div class="flex-1 h-full w-full overflow-y-auto overscroll-contain bg-[#0c0e12]">
<div class="min-h-full bg-[#0c0e12] text-zinc-100 flex flex-col justify-between font-sans select-none p-4 sm:p-6 md:p-8">
  <!-- Brand & Live Host Header -->
  <header class="max-w-2xl mx-auto w-full flex flex-wrap items-center justify-between gap-3 border-b border-white/[0.08] pb-4 shrink-0">
    <div class="flex items-center space-x-3 min-w-0">
      <div class="w-9 h-9 rounded-xl bg-white text-black font-black flex items-center justify-center text-sm shadow-md shrink-0">
        <svg class="w-5 h-5 text-black" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2L3 7v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V7l-9-5z"/>
          <path d="M12 8v8"/>
          <path d="M8 12h8"/>
        </svg>
      </div>
      <div>
        <div class="flex items-center space-x-2">
          <h1 class="text-sm sm:text-base font-black text-white tracking-wider uppercase">AMBRILINDO ARENA</h1>
          <span class="px-1.5 py-0.2 rounded text-[10px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
            v3.0
          </span>
        </div>
        <p class="text-[11px] text-zinc-400 font-normal">Portal Perangkat Arena &amp; Tablet Lapangan</p>
      </div>
    </div>

    <div class="flex items-center gap-2 px-2.5 py-1 rounded-full text-xs font-semibold border {hostOnline ? 'bg-emerald-500/10 border-emerald-500/25 text-emerald-400' : 'bg-amber-500/10 border-amber-500/25 text-amber-300'}">
      <span class="w-2 h-2 rounded-full animate-pulse {hostOnline ? 'bg-emerald-400' : 'bg-amber-300'}"></span>
      <span class="text-[11px] font-mono">{hostOnline ? "LAN HOST ONLINE" : "MENYAMBUNG…"}</span>
    </div>
  </header>

  <!-- Main Selection Container -->
  <main class="max-w-2xl mx-auto w-full my-auto py-5 space-y-5 shrink-0">
    <!-- Top Filter Bar: Event & Tatami -->
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 bg-[#13151b] border border-white/[0.08] p-3.5 rounded-2xl shadow-sm">
      <!-- 1. Event Selector -->
      <div>
        <label for="event-select" class="block text-[10px] font-bold text-zinc-400 uppercase tracking-wider mb-1.5">
          Event Pertandingan
        </label>
        <div class="relative">
          <select
            id="event-select"
            value={selectedEventId ?? ""}
            onchange={handleEventChange}
            class="w-full bg-[#181a22] border border-white/[0.12] focus:border-white text-white text-xs font-medium px-3 py-2 pr-8 rounded-xl outline-none appearance-none cursor-pointer"
          >
            {#if events.length === 0}
              <option disabled selected value="">{loadError || "Memuat event turnamen…"}</option>
            {:else}
              {#each events as ev}
                <option value={ev.id}>
                  {ev.nama_event || `Event #${ev.id}`} (ID: {ev.id})
                </option>
              {/each}
            {/if}
          </select>
          <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center px-3 text-zinc-400">
            <svg class="w-3.5 h-3.5" aria-hidden="true" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
            </svg>
          </div>
        </div>
      </div>

      <!-- 2. Tatami Selector -->
      <div>
        <label for="tatami-select" class="block text-[10px] font-bold text-zinc-400 uppercase tracking-wider mb-1.5">
          Pilih Tatami / Matras
        </label>
        <div class="flex items-center gap-1.5 overflow-x-auto pb-0.5">
          {#each tatamis as tNum}
            <button
              type="button"
              onclick={() => (selectedTatami = tNum)}
              class="flex-1 min-w-[50px] py-2 rounded-xl text-xs font-bold font-mono transition-all {selectedTatami === tNum
                ? 'bg-white text-black shadow-md scale-[1.02]'
                : 'bg-[#181a22] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.08]'}"
            >
              T.{tNum}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <!-- Role Selection Header -->
    <div class="flex items-end justify-between gap-2 px-1">
      <div>
        <h2 class="text-xs font-bold text-zinc-200 uppercase tracking-wider">PILIH PERAN / LAYAR ARENA</h2>
        <p class="text-[11px] text-zinc-400 mt-0.5">Ketuk peran yang akan dijalankan pada perangkat ini</p>
      </div>
      <span class="text-[10px] font-mono text-zinc-500">Tatami #{selectedTatami}</span>
    </div>

    <!-- Role Cards Grid (Inspired by auth.html) -->
    <div class="grid grid-cols-2 gap-2 sm:gap-2.5">
      {#each roleOptions as role}
        <button
          type="button"
          onclick={() => (selectedRole = role.id)}
          class="text-left p-2.5 sm:p-3 rounded-2xl border transition-all relative flex flex-col justify-between min-w-0 {selectedRole === role.id
            ? 'bg-[#1a1c24] border-white/40 ring-1 ring-white/30 shadow-md'
            : 'bg-[#13151b] hover:bg-[#181a22] border-white/[0.06] text-zinc-300'}"
        >
          <!-- Top Badge & Icon -->
          <div class="flex items-center justify-between mb-2">
            <div class="w-7 h-7 rounded-lg flex items-center justify-center {selectedRole === role.id ? 'bg-white text-black' : 'bg-white/[0.06] text-zinc-300'}">
              {#if role.icon === "video"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <polygon points="23 7 16 12 23 17 23 7"/>
                  <rect x="1" y="5" width="15" height="14" rx="2" ry="2"/>
                </svg>
              {:else if role.icon === "monitor"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                  <line x1="8" y1="21" x2="16" y2="21"/>
                  <line x1="12" y1="17" x2="12" y2="21"/>
                </svg>
              {:else if role.icon === "clipboard"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
                  <rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>
                </svg>
              {:else if role.icon === "whistle"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="12" cy="12" r="10"/>
                  <polyline points="12 6 12 12 16 14"/>
                </svg>
              {:else if role.icon === "flag"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z"/>
                  <line x1="4" y1="22" x2="4" y2="15"/>
                </svg>
              {:else if role.icon === "sliders"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <line x1="4" y1="21" x2="4" y2="14"/>
                  <line x1="4" y1="10" x2="4" y2="3"/>
                  <line x1="12" y1="21" x2="12" y2="12"/>
                  <line x1="12" y1="8" x2="12" y2="3"/>
                  <line x1="20" y1="21" x2="20" y2="16"/>
                  <line x1="20" y1="12" x2="20" y2="3"/>
                </svg>
              {:else if role.icon === "tv"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="2" y="7" width="20" height="15" rx="2" ry="2"/>
                  <polyline points="17 2 12 7 7 2"/>
                </svg>
              {:else if role.icon === "megaphone"}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M3 11l18-5v12L3 13v-2z"/>
                  <path d="M11.6 16.8a3 3 0 1 1-5.8-1.6"/>
                </svg>
              {:else}
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                </svg>
              {/if}
            </div>

            {#if role.badge}
              <span class="hidden min-[400px]:inline text-[9px] font-mono uppercase px-1.5 py-0.5 rounded bg-white/[0.04] text-zinc-400 border border-white/[0.06] truncate max-w-[60%]">
                {role.badge}
              </span>
            {/if}
          </div>

          <!-- Title & Subtitle -->
          <div>
            <div class="text-xs font-bold text-white tracking-tight flex items-center justify-between gap-1">
              <span class="truncate">{role.title}</span>
              {#if selectedRole === role.id}
                <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
              {/if}
            </div>
            <p class="text-[10px] text-zinc-400 mt-0.5 leading-snug line-clamp-1">{role.subtitle}</p>
          </div>
        </button>
      {/each}
    </div>

    <!-- Big Action Launch Button -->
    <button
      type="button"
      onclick={handleNavigate}
      class="w-full h-12 bg-white hover:bg-zinc-200 active:scale-[0.99] text-black font-black text-xs sm:text-sm rounded-2xl shadow-xl flex items-center justify-center gap-2 transition-all mt-4 cursor-pointer outline-none focus-visible:ring-2 focus-visible:ring-white/50"
    >
      <span class="truncate">Buka {activeRoleInfo.title} &bull; Tatami {selectedTatami}</span>
      <svg class="w-4 h-4 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3"/>
      </svg>
    </button>
  </main>

  <!-- Clean Minimal Footer -->
  <footer class="max-w-2xl mx-auto w-full pt-4 border-t border-white/[0.08] flex flex-wrap items-center justify-between gap-2 text-[11px] text-zinc-400 shrink-0">
    <div class="flex items-center gap-2">
      <span class="text-zinc-500">Host:</span>
      <span class="font-mono text-zinc-300 break-all">{hostUrl || "http://127.0.0.1:8000"}</span>
    </div>
    <span class="text-zinc-500">Ambrilindo Venue System</span>
  </footer>
</div>
</div>
