<script lang="ts">
  import { onMount } from "svelte";
  import { apiCall } from "$lib/api_client";
  import type {
    TatamiItem,
    WasitItem,
    WasitTatamiAssignment,
    WasitConflict,
    TatamiPanelOverview,
    SaveWasitPayload,
    AssignWasitPayload,
    AutoAssignPanelPayload,
    PerguruanItem,
  } from "$lib/types";

  interface Props {
    eventId: number;
    eventName: string;
    tatamis: TatamiItem[];
    hostAddress?: string;
    onShowToast?: (msg: string) => void;
  }

  let {
    eventId,
    eventName,
    tatamis,
    hostAddress,
    onShowToast,
  }: Props = $props();

  // Tabs
  let activeTab = $state<"panel" | "roster">("panel");

  // Tatami selection
  let selectedTatamiId = $state<number>(1);
  let panelOverview = $state<TatamiPanelOverview | null>(null);
  let wasits = $state<WasitItem[]>([]);
  let perguruans = $state<PerguruanItem[]>([]);
  let isLoading = $state(false);

  // Filter & Search in Roster
  let searchQuery = $state("");
  let selectedLicenseFilter = $state<string>("all");

  // Modals
  let isWasitModalOpen = $state(false);
  let editingWasit = $state<WasitItem | null>(null);
  let formNama = $state("");
  let formPerguruanId = $state<number | null>(null);
  let formKabKota = $state("");
  let formLisensi = $state("Wasit Nasional A");
  let formNoLisensi = $state("");
  let formIsActive = $state(true);

  // Position Assignment Modal
  let isAssignModalOpen = $state(false);
  let targetAssignPosisi = $state<string>("referee");
  let assignSearch = $state("");

  const STANDARD_ROLES: { key: string; label: string; icon: string; desc: string }[] = [
    { key: "tatami_manager", label: "Tatami Manager", icon: "👑", desc: "Ketua & Pengawas Meja Tatami" },
    { key: "kansa", label: "Kansa / VR", icon: "📺", desc: "Supervisor Pertandingan & Video Review" },
    { key: "referee", label: "Referee (Wasit)", icon: "🥋", desc: "Wasit Tengah Lapangan" },
    { key: "judge_1", label: "Juri 1", icon: "🚩", desc: "Hakim Garis Sudut 1" },
    { key: "judge_2", label: "Juri 2", icon: "🚩", desc: "Hakim Garis Sudut 2" },
    { key: "judge_3", label: "Juri 3", icon: "🚩", desc: "Hakim Garis Sudut 3" },
    { key: "judge_4", label: "Juri 4", icon: "🚩", desc: "Hakim Garis Sudut 4" },
    { key: "pool", label: "Cadangan / Meja", icon: "📋", desc: "Wasit Cadangan / Petugas Penunjang" },
  ];

  const LICENSE_OPTIONS = [
    "Wasit WKF / AKF",
    "Wasit Nasional A",
    "Wasit Nasional B",
    "Juri Nasional A",
    "Juri Nasional B",
    "Wasit Daerah",
    "Juri Daerah",
  ];

  onMount(async () => {
    await loadInitialData();
  });

  $effect(() => {
    if (selectedTatamiId) {
      loadPanelOverview(selectedTatamiId);
    }
  });

  async function loadInitialData() {
    isLoading = true;
    try {
      await Promise.all([
        loadWasits(),
        loadPerguruans(),
      ]);
      if (tatamis.length > 0 && !tatamis.some(t => t.id === selectedTatamiId)) {
        selectedTatamiId = tatamis[0].id;
      }
      await loadPanelOverview(selectedTatamiId);
    } catch (e: any) {
      onShowToast?.("Gagal memuat data: " + (e?.message || e));
    } finally {
      isLoading = false;
    }
  }

  async function loadWasits() {
    try {
      wasits = await apiCall<WasitItem[]>("get_wasits", { eventId, event_id: eventId });
    } catch (e) {
      console.warn("loadWasits err:", e);
    }
  }

  async function loadPerguruans() {
    try {
      perguruans = await apiCall<PerguruanItem[]>("get_perguruans", {});
    } catch (e) {
      console.warn("loadPerguruans err:", e);
    }
  }

  async function loadPanelOverview(tatamiId: number) {
    if (!tatamiId) return;
    try {
      panelOverview = await apiCall<TatamiPanelOverview>("get_tatami_panel_overview", {
        eventId,
        event_id: eventId,
        tatamiId,
        tatami_id: tatamiId,
      });
    } catch (e) {
      console.warn("loadPanelOverview err:", e);
    }
  }

  // Assign referee to slot
  function openAssignModal(posisi: string) {
    targetAssignPosisi = posisi;
    assignSearch = "";
    isAssignModalOpen = true;
  }

  async function handleAssignWasit(wasitId: number) {
    try {
      await apiCall("assign_wasit_tatami", {
        input: {
          event_id: eventId,
          tatami_id: selectedTatamiId,
          wasit_id: wasitId,
          posisi: targetAssignPosisi,
        },
      });
      isAssignModalOpen = false;
      onShowToast?.("Penugasan wasit berhasil diperbarui.");
      await Promise.all([
        loadPanelOverview(selectedTatamiId),
        loadWasits(),
      ]);
    } catch (e: any) {
      onShowToast?.("Gagal menugaskan wasit: " + (e?.message || e));
    }
  }

  async function handleRemoveAssignment(assignmentId: number) {
    try {
      await apiCall("remove_wasit_tatami", {
        assignmentId,
        assignment_id: assignmentId,
        id: assignmentId,
      });
      onShowToast?.("Wasit berhasil dibebas-tugaskan dari posisi ini.");
      await Promise.all([
        loadPanelOverview(selectedTatamiId),
        loadWasits(),
      ]);
    } catch (e: any) {
      onShowToast?.("Gagal menghapus penugasan: " + (e?.message || e));
    }
  }

  async function handleClearPanel() {
    if (!confirm(`Kosongkan seluruh panel pejabat di Tatami ${currentTatamiNumber}?`)) return;
    try {
      await apiCall("clear_tatami_panel", {
        tatamiId: selectedTatamiId,
        tatami_id: selectedTatamiId,
      });
      onShowToast?.("Panel Tatami berhasil dikosongkan.");
      await Promise.all([
        loadPanelOverview(selectedTatamiId),
        loadWasits(),
      ]);
    } catch (e: any) {
      onShowToast?.("Gagal mengosongkan panel: " + (e?.message || e));
    }
  }

  async function handleAutoAssign(count: number = 5) {
    // Pick available active referees not currently assigned to this or any tatami
    const available = wasits.filter((w) => w.is_active && (!w.assigned_tatami_id || w.assigned_tatami_id === selectedTatamiId));
    if (available.length < count) {
      alert(`Hanya ada ${available.length} wasit aktif yang tersedia, dibutuhkan minimal ${count} wasit!`);
      return;
    }

    // Sort by license priority
    const priorityLicense = (lic: string | null) => {
      if (!lic) return 0;
      if (lic.includes("WKF") || lic.includes("AKF")) return 100;
      if (lic.includes("Wasit A") || lic.includes("Wasit Nasional A")) return 80;
      if (lic.includes("Wasit B") || lic.includes("Wasit Nasional B")) return 60;
      if (lic.includes("Juri A") || lic.includes("Juri Nasional A")) return 50;
      if (lic.includes("Juri B") || lic.includes("Juri Nasional B")) return 40;
      return 10;
    };

    const sorted = [...available].sort((a, b) => priorityLicense(b.lisensi) - priorityLicense(a.lisensi));
    const selectedIds = sorted.slice(0, count).map((w) => w.id);

    try {
      await apiCall("auto_assign_tatami_panel", {
        input: {
          event_id: eventId,
          tatami_id: selectedTatamiId,
          wasit_ids: selectedIds,
        },
      });
      onShowToast?.(`Berhasil menugaskan ${selectedIds.length} wasit otomatis secara terstruktur.`);
      await Promise.all([
        loadPanelOverview(selectedTatamiId),
        loadWasits(),
      ]);
    } catch (e: any) {
      onShowToast?.("Gagal auto-assign panel: " + (e?.message || e));
    }
  }

  // WKF Standard Rotation: Referee moves to Judge 4, Judge 1 moves to Referee, etc.
  async function handleRotatePanel() {
    if (!panelOverview || panelOverview.assignments.length === 0) return;
    const assignments = panelOverview.assignments;
    const ref = assignments.find((a) => a.posisi === "referee");
    const j1 = assignments.find((a) => a.posisi === "judge_1");
    const j2 = assignments.find((a) => a.posisi === "judge_2");
    const j3 = assignments.find((a) => a.posisi === "judge_3");
    const j4 = assignments.find((a) => a.posisi === "judge_4");

    if (!ref || !j1) {
      onShowToast?.("Rotasi memerlukan minimal Referee dan Juri 1.");
      return;
    }

    try {
      // Rotation order:
      // J1 -> Referee
      // J2 -> J1
      // J3 -> J2
      // J4 -> J3
      // Referee -> J4 (or J2 if 2 judges)
      const newOrders: { wasit_id: number; posisi: string }[] = [];
      newOrders.push({ wasit_id: j1.wasit_id, posisi: "referee" });
      if (j2) newOrders.push({ wasit_id: j2.wasit_id, posisi: "judge_1" });
      if (j3) newOrders.push({ wasit_id: j3.wasit_id, posisi: "judge_2" });
      if (j4) newOrders.push({ wasit_id: j4.wasit_id, posisi: "judge_3" });

      const lastPos = j4 ? "judge_4" : j3 ? "judge_3" : j2 ? "judge_2" : "judge_1";
      newOrders.push({ wasit_id: ref.wasit_id, posisi: lastPos });

      for (const item of newOrders) {
        await apiCall("assign_wasit_tatami", {
          input: {
            event_id: eventId,
            tatami_id: selectedTatamiId,
            wasit_id: item.wasit_id,
            posisi: item.posisi,
          },
        });
      }

      onShowToast?.("🔄 Rotasi wasit berhasil (Wasit Tengah & Juri bergeser).");
      await loadPanelOverview(selectedTatamiId);
    } catch (e: any) {
      onShowToast?.("Gagal melakukan rotasi wasit: " + (e?.message || e));
    }
  }

  // Remove conflicted referees in one click
  async function handleDismissConflictedWasits() {
    if (!panelOverview || panelOverview.conflicts.length === 0) return;
    const conflictedWasitIds = new Set(panelOverview.conflicts.map((c) => c.wasit_id));
    const toRemove = panelOverview.assignments.filter((a) => conflictedWasitIds.has(a.wasit_id));

    try {
      for (const item of toRemove) {
        await apiCall("remove_wasit_tatami", {
          assignmentId: item.id,
          assignment_id: item.id,
          id: item.id,
        });
      }
      onShowToast?.(`Berhasil membebaskan ${toRemove.length} wasit bentrok dari panel.`);
      await Promise.all([
        loadPanelOverview(selectedTatamiId),
        loadWasits(),
      ]);
    } catch (e: any) {
      onShowToast?.("Gagal membebaskan wasit bentrok: " + (e?.message || e));
    }
  }

  // Wasit Roster CRUD
  function openAddWasitModal() {
    editingWasit = null;
    formNama = "";
    formPerguruanId = null;
    formKabKota = "";
    formLisensi = "Wasit Nasional A";
    formNoLisensi = "";
    formIsActive = true;
    isWasitModalOpen = true;
  }

  function openEditWasitModal(w: WasitItem) {
    editingWasit = w;
    formNama = w.nama_wasit;
    formPerguruanId = w.perguruan_id;
    formKabKota = w.kab_kota || "";
    formLisensi = w.lisensi || "Wasit Nasional A";
    formNoLisensi = w.no_lisensi || "";
    formIsActive = w.is_active;
    isWasitModalOpen = true;
  }

  async function handleSaveWasit() {
    if (!formNama.trim()) {
      alert("Nama wasit wajib diisi!");
      return;
    }

    const payload: SaveWasitPayload = {
      id: editingWasit ? editingWasit.id : null,
      event_id: eventId,
      nama_wasit: formNama.trim(),
      perguruan_id: formPerguruanId,
      kab_kota: formKabKota.trim() || null,
      lisensi: formLisensi.trim() || null,
      no_lisensi: formNoLisensi.trim() || null,
      is_active: formIsActive,
    };

    try {
      await apiCall("save_wasit", { input: payload });
      isWasitModalOpen = false;
      onShowToast?.(editingWasit ? "Data wasit berhasil diperbarui." : "Wasit baru berhasil ditambahkan.");
      await loadWasits();
      if (selectedTatamiId) await loadPanelOverview(selectedTatamiId);
    } catch (e: any) {
      onShowToast?.("Gagal menyimpan wasit: " + (e?.message || e));
    }
  }

  async function handleDeleteWasit(w: WasitItem) {
    if (!confirm(`Hapus data wasit "${w.nama_wasit}"? Semua penugasan tataminya juga akan dicabut.`)) return;
    try {
      await apiCall("delete_wasit", { wasitId: w.id, wasit_id: w.id, id: w.id });
      onShowToast?.(`Wasit "${w.nama_wasit}" berhasil dihapus.`);
      await loadWasits();
      if (selectedTatamiId) await loadPanelOverview(selectedTatamiId);
    } catch (e: any) {
      onShowToast?.("Gagal menghapus wasit: " + (e?.message || e));
    }
  }

  // Filtered Roster
  let filteredWasits = $derived(
    wasits.filter((w) => {
      // Query filter
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase();
        const matchName = w.nama_wasit.toLowerCase().includes(q);
        const matchPerg = w.nama_perguruan?.toLowerCase().includes(q);
        const matchKab = w.kab_kota?.toLowerCase().includes(q);
        const matchLic = w.lisensi?.toLowerCase().includes(q);
        if (!matchName && !matchPerg && !matchKab && !matchLic) return false;
      }
      // License filter
      if (selectedLicenseFilter !== "all") {
        if (!w.lisensi) return false;
        const lic = w.lisensi.toLowerCase();
        if (selectedLicenseFilter === "wkf" && !lic.includes("wkf") && !lic.includes("akf")) return false;
        if (selectedLicenseFilter === "wasit_a" && !lic.includes("wasit a") && !lic.includes("wasit nasional a")) return false;
        if (selectedLicenseFilter === "wasit_b" && !lic.includes("wasit b") && !lic.includes("wasit nasional b")) return false;
        if (selectedLicenseFilter === "juri_a" && !lic.includes("juri a") && !lic.includes("juri nasional a")) return false;
        if (selectedLicenseFilter === "juri_b" && !lic.includes("juri b") && !lic.includes("juri nasional b")) return false;
        if (selectedLicenseFilter === "daerah" && !lic.includes("daerah")) return false;
      }
      return true;
    })
  );

  let currentTatamiNumber = $derived(
    tatamis.find((t) => t.id === selectedTatamiId)?.tatami_number || 1
  );

  let assignedCountOnSelectedTatami = $derived(
    panelOverview?.assignments.length || 0
  );

  let conflictCount = $derived(
    panelOverview?.conflicts.length || 0
  );

  let candidateWasits = $derived(
    wasits.filter((w) => {
      if (!assignSearch.trim()) return true;
      const q = assignSearch.toLowerCase();
      return (
        w.nama_wasit.toLowerCase().includes(q) ||
        w.nama_perguruan?.toLowerCase().includes(q) ||
        w.kab_kota?.toLowerCase().includes(q) ||
        w.lisensi?.toLowerCase().includes(q)
      );
    })
  );

  function getWasitCountForTatami(tatamiId: number): number {
    return wasits.filter((w) => w.assigned_tatami_id === tatamiId).length;
  }
</script>

<div class="h-full flex flex-col bg-[#111214] text-zinc-100 overflow-hidden font-sans">
  <!-- Top Navigation & Action Header -->
  <header class="px-6 py-4 bg-[#181a20] border-b border-[#2b2d31] flex flex-wrap items-center justify-between gap-4 shrink-0 shadow-md">
    <div class="flex items-center space-x-3">
      <div class="w-10 h-10 rounded-xl bg-gradient-to-tr from-amber-600 to-amber-400 flex items-center justify-center text-white shadow-lg text-lg">
        ⚖️
      </div>
      <div>
        <div class="flex items-center space-x-2">
          <h1 class="text-lg font-black tracking-tight text-white uppercase">Manajemen Wasit & Dewan Juri</h1>
          <span class="px-2 py-0.5 rounded-full bg-amber-500/10 border border-amber-500/30 text-[10px] font-mono font-bold text-amber-400">
            WKF OFFICIAL PANEL
          </span>
        </div>
        <p class="text-xs text-zinc-400 mt-0.5">
          Penugasan Pejabat Tatami, Manajemen Roster, dan Deteksi Bentrok Afiliasi (Anti-Collision Engine)
        </p>
      </div>
    </div>

    <!-- Tab Switcher -->
    <div class="flex items-center space-x-1 p-1 bg-[#111214] rounded-xl border border-[#2b2d31]">
      <button
        type="button"
        onclick={() => (activeTab = "panel")}
        class="px-4 py-2 rounded-lg text-xs font-bold transition flex items-center space-x-2 {activeTab === 'panel' ? 'bg-[#5865f2] text-white shadow-md' : 'text-zinc-400 hover:text-white'}"
      >
        <span>🏛️</span>
        <span>Panel Penugasan Tatami</span>
        {#if conflictCount > 0}
          <span class="w-2 h-2 rounded-full bg-red-500 animate-pulse"></span>
        {/if}
      </button>

      <button
        type="button"
        onclick={() => (activeTab = "roster")}
        class="px-4 py-2 rounded-lg text-xs font-bold transition flex items-center space-x-2 {activeTab === 'roster' ? 'bg-[#5865f2] text-white shadow-md' : 'text-zinc-400 hover:text-white'}"
      >
        <span>📋</span>
        <span>Direktori Wasit ({wasits.length})</span>
      </button>
    </div>
  </header>

  <!-- Main Content Area -->
  <div class="flex-1 overflow-y-auto p-6 space-y-6">
    {#if activeTab === "panel"}
      <!-- TAB 1: TATAMI PANEL ASSIGNER & ANTI-COLLISION -->

      <!-- Tatami Court Selector Bar -->
      <div class="flex items-center justify-between flex-wrap gap-3 pb-3 border-b border-[#2b2d31]">
        <div class="flex items-center space-x-2 overflow-x-auto py-1">
          <span class="text-xs font-bold uppercase tracking-wider text-zinc-400 mr-2 flex items-center space-x-1">
            <span>Pilih Tatami:</span>
          </span>
          {#each (tatamis.length > 0 ? tatamis : [{ id: 1, tatami_number: 1 }]) as tItem}
            {@const tNum = tItem.tatami_number || 1}
            {@const isSelected = selectedTatamiId === tItem.id}
            <button
              type="button"
              onclick={() => (selectedTatamiId = tItem.id)}
              class="px-3.5 py-2 rounded-xl text-xs font-bold transition flex items-center space-x-2 border {isSelected ? 'bg-amber-500/15 border-amber-500 text-amber-300 shadow-md ring-1 ring-amber-500/30' : 'bg-[#181a20] border-[#2b2d31] text-zinc-300 hover:border-zinc-500'}"
            >
              <span>🥋 Tatami {tNum}</span>
              <span class="px-1.5 py-0.5 rounded text-[10px] font-mono {isSelected ? 'bg-amber-500/30 text-amber-200' : 'bg-white/[0.05] text-zinc-400'}">
                {getWasitCountForTatami(tItem.id)}
              </span>
            </button>
          {/each}
        </div>

        <!-- Quick Automation Controls -->
        <div class="flex items-center space-x-2">
          <button
            type="button"
            onclick={() => handleAutoAssign(5)}
            class="px-3 py-1.5 rounded-lg bg-emerald-600/20 hover:bg-emerald-600/30 border border-emerald-500/40 text-emerald-300 text-xs font-bold transition flex items-center space-x-1.5 shadow-sm active:scale-95"
            title="Auto-Assign 5 Wasit: Referee + 4 Juri Sudut"
          >
            <span>⚡ Auto-Assign 5</span>
          </button>

          <button
            type="button"
            onclick={() => handleAutoAssign(7)}
            class="px-3 py-1.5 rounded-lg bg-blue-600/20 hover:bg-blue-600/30 border border-blue-500/40 text-blue-300 text-xs font-bold transition flex items-center space-x-1.5 shadow-sm active:scale-95"
            title="Auto-Assign 7 Pejabat: Manager, Kansa, Referee + 4 Juri"
          >
            <span>✨ Auto-Assign 7</span>
          </button>

          <button
            type="button"
            onclick={handleRotatePanel}
            class="px-3 py-1.5 rounded-lg bg-purple-600/20 hover:bg-purple-600/30 border border-purple-500/40 text-purple-300 text-xs font-bold transition flex items-center space-x-1.5 shadow-sm active:scale-95"
            title="Rotasi Wasit WKF Kumite: Wasit Tengah bergeser ke Juri 4, Juri 1 menjadi Wasit Tengah"
          >
            <span>🔄 Rotasi</span>
          </button>

          <button
            type="button"
            onclick={handleClearPanel}
            class="px-3 py-1.5 rounded-lg bg-red-600/20 hover:bg-red-600/30 border border-red-500/40 text-red-300 text-xs font-bold transition flex items-center space-x-1.5 shadow-sm active:scale-95"
            title="Kosongkan seluruh pejabat di tatami ini"
          >
            <span>🗑️ Kosongkan</span>
          </button>
        </div>
      </div>

      <!-- Live Match & Anti-Collision Cockpit Banner -->
      <div class="rounded-2xl bg-[#181a20] border border-[#2b2d31] p-5 shadow-xl relative overflow-hidden">
        <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-[#2b2d31]">
          <div class="space-y-1">
            <div class="flex items-center space-x-2">
              <span class="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse"></span>
              <span class="text-xs font-mono font-bold uppercase tracking-wider text-emerald-400">STATUS PARTAI AKTIF • TATAMI {currentTatamiNumber}</span>
            </div>
            {#if panelOverview?.active_match}
              <h2 class="text-base font-black text-white flex items-center space-x-2">
                <span>{panelOverview.category_name || "Kategori Pertandingan"}</span>
                <span class="text-xs font-mono text-amber-400 bg-amber-500/10 px-2 py-0.5 rounded border border-amber-500/20">
                  {panelOverview.active_match.kode || `Partai #${panelOverview.active_match.id}`}
                </span>
              </h2>
            {:else}
              <h2 class="text-base font-bold text-zinc-400">
                Tidak ada partai aktif yang terpasang di Tatami {currentTatamiNumber}
              </h2>
            {/if}
          </div>

          <!-- Combatants Quick Capsule (Aka vs Ao) -->
          {#if panelOverview?.active_match}
            <div class="flex items-center space-x-3 bg-[#111214] px-4 py-2.5 rounded-xl border border-[#2b2d31]">
              <!-- AKA -->
              <div class="text-right">
                <div class="text-xs font-black text-red-400 flex items-center justify-end space-x-1">
                  <span>{panelOverview.active_match.atlet1_nama || "Atlet Aka"}</span>
                  <span class="w-2 h-2 rounded-full bg-red-500"></span>
                </div>
                <div class="text-[10px] text-zinc-400 font-mono truncate max-w-[140px]">
                  {panelOverview.active_match.atlet1_kontingen || "-"}
                </div>
              </div>

              <span class="text-xs font-black text-zinc-500 font-mono">VS</span>

              <!-- AO -->
              <div class="text-left">
                <div class="text-xs font-black text-blue-400 flex items-center space-x-1">
                  <span class="w-2 h-2 rounded-full bg-blue-500"></span>
                  <span>{panelOverview.active_match.atlet2_nama || "Atlet Ao"}</span>
                </div>
                <div class="text-[10px] text-zinc-400 font-mono truncate max-w-[140px]">
                  {panelOverview.active_match.atlet2_kontingen || "-"}
                </div>
              </div>
            </div>
          {/if}
        </div>

        <!-- Conflict Engine Indicator -->
        <div class="mt-4">
          {#if conflictCount > 0}
            <div class="rounded-xl bg-red-500/10 border border-red-500/30 p-4 space-y-3 animate-in fade-in duration-200">
              <div class="flex items-center justify-between flex-wrap gap-2">
                <div class="flex items-center space-x-2 text-red-400 font-bold text-xs uppercase tracking-wide">
                  <span class="text-base">⚠️</span>
                  <span>Peringatan Anti-Collision: Ditemukan {conflictCount} Bentrok Afiliasi Wasit!</span>
                </div>
                <button
                  type="button"
                  onclick={handleDismissConflictedWasits}
                  class="px-2.5 py-1 rounded-md bg-red-600 hover:bg-red-500 text-white text-[11px] font-bold tracking-wide transition shadow active:scale-95 flex items-center space-x-1"
                >
                  <span>Bebas-Tugaskan Wasit Bentrok</span>
                </button>
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-2 text-xs">
                {#each panelOverview?.conflicts || [] as c}
                  <div class="p-2.5 rounded-lg bg-red-950/40 border border-red-800/40 flex items-start space-x-2">
                    <span class="text-red-400 shrink-0 mt-0.5">●</span>
                    <div class="min-w-0 flex-1">
                      <div class="font-bold text-red-200 flex items-center space-x-1.5">
                        <span>{c.nama_wasit}</span>
                        <span class="text-[10px] uppercase font-mono px-1.5 py-0.2 bg-red-500/20 text-red-300 rounded">
                          {c.posisi}
                        </span>
                      </div>
                      <div class="text-[11px] text-red-300/90 mt-0.5 leading-snug">
                        {c.reason}
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {:else}
            <div class="rounded-xl bg-emerald-500/10 border border-emerald-500/20 p-3 flex items-center space-x-3 text-xs text-emerald-400 font-medium">
              <span class="text-lg">🛡️</span>
              <div>
                <span class="font-bold">Panel Wasit Netral & Aman:</span>
                <span class="text-emerald-300/80 ml-1">Tidak ditemukan bentrok perguruan atau kontingen/daerah antara dewan juri dengan atlet yang bertanding.</span>
              </div>
            </div>
          {/if}
        </div>
      </div>

      <!-- Court Position Grid (The Mat Layout) -->
      <div>
        <div class="flex items-center justify-between mb-3">
          <h3 class="text-xs font-mono font-bold uppercase tracking-wider text-zinc-400 flex items-center space-x-2">
            <span>SUSUNAN DEWAN WASIT & JURI (TATAMI {currentTatamiNumber})</span>
            <span class="text-[10px] text-zinc-500 font-normal">• {assignedCountOnSelectedTatami} Pejabat Ditugaskan</span>
          </h3>

          <div class="text-[11px] text-zinc-400">
            WKF Official Structure: Manager • Kansa • Referee • 4 Judges
          </div>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {#each STANDARD_ROLES as role}
            {@const assignment = panelOverview?.assignments.find((a) => a.posisi === role.key)}
            {@const hasConflict = panelOverview?.conflicts.some((c) => c.wasit_id === assignment?.wasit_id)}
            {@const conflictInfo = panelOverview?.conflicts.find((c) => c.wasit_id === assignment?.wasit_id)}

            <div class="rounded-2xl border transition relative overflow-hidden flex flex-col justify-between {hasConflict ? 'bg-red-950/20 border-red-500/50 shadow-red-950/20 shadow-lg' : assignment ? 'bg-[#181a20] border-[#2b2d31] hover:border-zinc-500' : 'bg-[#14161b] border-dashed border-[#2b2d31] hover:border-amber-500/50'} p-4 min-h-[170px]">
              <!-- Header / Role Pill -->
              <div>
                <div class="flex items-center justify-between pb-2 border-b border-[#2b2d31]">
                  <div class="flex items-center space-x-2">
                    <span class="text-base">{role.icon}</span>
                    <span class="text-xs font-black uppercase tracking-wider text-zinc-200">{role.label}</span>
                  </div>

                  {#if hasConflict}
                    <span class="px-1.5 py-0.5 rounded bg-red-600 text-white font-mono text-[9px] font-bold animate-pulse">
                      BENTROK
                    </span>
                  {:else if assignment}
                    <span class="px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 font-mono text-[9px] font-bold">
                      AKTIF
                    </span>
                  {:else}
                    <span class="px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-500 font-mono text-[9px]">
                      KOSONG
                    </span>
                  {/if}
                </div>

                <!-- Referee Info Card -->
                {#if assignment}
                  <div class="mt-3 space-y-1">
                    <div class="text-sm font-black text-white tracking-tight leading-snug">
                      {assignment.nama_wasit}
                    </div>

                    <div class="flex items-center flex-wrap gap-1 text-[11px] text-zinc-400">
                      {#if assignment.lisensi}
                        <span class="px-1.5 py-0.5 rounded bg-white/[0.05] text-amber-300 font-mono text-[10px]">
                          {assignment.lisensi}
                        </span>
                      {/if}
                      {#if assignment.nama_perguruan}
                        <span class="text-zinc-300 font-semibold">{assignment.nama_perguruan}</span>
                      {/if}
                      {#if assignment.kab_kota}
                        <span class="text-zinc-500">• {assignment.kab_kota}</span>
                      {/if}
                    </div>

                    {#if hasConflict && conflictInfo}
                      <div class="mt-2 text-[10px] text-red-300 bg-red-950/60 p-1.5 rounded border border-red-800/50 leading-tight">
                        ⚠️ {conflictInfo.reason}
                      </div>
                    {/if}
                  </div>
                {:else}
                  <div class="mt-4 text-center py-3">
                    <div class="text-xs text-zinc-500 font-medium">Belum ada wasit ditugaskan</div>
                    <div class="text-[10px] text-zinc-600 mt-0.5">{role.desc}</div>
                  </div>
                {/if}
              </div>

              <!-- Footer Buttons -->
              <div class="mt-4 pt-3 border-t border-[#2b2d31]/60 flex items-center justify-between gap-2">
                {#if assignment}
                  <button
                    type="button"
                    onclick={() => openAssignModal(role.key)}
                    class="flex-1 py-1 px-2 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-[11px] font-bold transition text-center"
                  >
                    Ganti
                  </button>
                  <button
                    type="button"
                    onclick={() => handleRemoveAssignment(assignment.id)}
                    class="py-1 px-2.5 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-400 hover:text-red-300 text-[11px] font-bold transition"
                    title="Hapus penugasan posisi ini"
                  >
                    ✕
                  </button>
                {:else}
                  <button
                    type="button"
                    onclick={() => openAssignModal(role.key)}
                    class="w-full py-1.5 px-3 rounded-lg bg-[#5865f2]/20 hover:bg-[#5865f2]/30 text-[#7289da] hover:text-white border border-[#5865f2]/40 text-xs font-bold transition flex items-center justify-center space-x-1"
                  >
                    <span>+ Tugaskan Wasit</span>
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </div>

    {:else}
      <!-- TAB 2: REFEREE ROSTER DIRECTORY -->

      <!-- Action & Filter Bar -->
      <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-2 border-b border-[#2b2d31]">
        <!-- Search & Quick Filters -->
        <div class="flex flex-1 items-center space-x-3 max-w-2xl">
          <div class="relative flex-1">
            <span class="absolute left-3 top-2.5 text-zinc-500 text-xs">🔍</span>
            <input
              type="text"
              bind:value={searchQuery}
              placeholder="Cari nama wasit, perguruan, daerah, atau lisensi..."
              class="w-full bg-[#181a20] border border-[#2b2d31] rounded-xl pl-8 pr-4 py-2 text-xs text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-[#5865f2]"
            />
          </div>

          <div class="flex items-center space-x-1 overflow-x-auto py-1">
            {#each [
              { id: "all", label: "Semua" },
              { id: "wkf", label: "WKF / AKF" },
              { id: "wasit_a", label: "Wasit A" },
              { id: "wasit_b", label: "Wasit B" },
              { id: "juri_a", label: "Juri A" },
              { id: "juri_b", label: "Juri B" },
              { id: "daerah", label: "Daerah" },
            ] as filter}
              <button
                type="button"
                onclick={() => (selectedLicenseFilter = filter.id)}
                class="px-2.5 py-1.5 rounded-lg text-xs font-semibold whitespace-nowrap transition {selectedLicenseFilter === filter.id ? 'bg-[#5865f2] text-white shadow' : 'bg-[#181a20] text-zinc-400 hover:text-white border border-[#2b2d31]'}"
              >
                {filter.label}
              </button>
            {/each}
          </div>
        </div>

        <!-- Add Wasit Button -->
        <button
          type="button"
          onclick={openAddWasitModal}
          class="px-4 py-2 rounded-xl bg-[#5865f2] hover:bg-[#4752c4] text-white text-xs font-bold transition flex items-center space-x-1.5 shadow-md active:scale-95 shrink-0"
        >
          <span>+ Tambah Wasit Baru</span>
        </button>
      </div>

      <!-- Statistics summary capsules -->
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <div class="p-4 rounded-xl bg-[#181a20] border border-[#2b2d31]">
          <div class="text-[11px] font-mono uppercase text-zinc-400">Total Wasit Terdaftar</div>
          <div class="text-2xl font-black text-white mt-1">{wasits.length}</div>
        </div>
        <div class="p-4 rounded-xl bg-[#181a20] border border-[#2b2d31]">
          <div class="text-[11px] font-mono uppercase text-emerald-400">Aktif Bertugas di Tatami</div>
          <div class="text-2xl font-black text-emerald-300 mt-1">
            {wasits.filter((w) => w.assigned_tatami_id).length}
          </div>
        </div>
        <div class="p-4 rounded-xl bg-[#181a20] border border-[#2b2d31]">
          <div class="text-[11px] font-mono uppercase text-amber-400">Tersedia / Bebas Tugas</div>
          <div class="text-2xl font-black text-amber-300 mt-1">
            {wasits.filter((w) => w.is_active && !w.assigned_tatami_id).length}
          </div>
        </div>
      </div>

      <!-- Wasit Directory Table -->
      <div class="rounded-2xl bg-[#181a20] border border-[#2b2d31] overflow-hidden shadow-xl">
        <div class="overflow-x-auto">
          <table class="w-full text-left text-xs">
            <thead>
              <tr class="bg-[#14161b] border-b border-[#2b2d31] text-[10px] font-mono uppercase text-zinc-400 tracking-wider">
                <th class="py-3 px-4">Nama Wasit / Juri</th>
                <th class="py-3 px-4">Tingkat Lisensi</th>
                <th class="py-3 px-4">Perguruan</th>
                <th class="py-3 px-4">Kab / Kota Asal</th>
                <th class="py-3 px-4">Status Penugasan</th>
                <th class="py-3 px-4 text-right">Aksi</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-[#2b2d31]">
              {#if filteredWasits.length === 0}
                <tr>
                  <td colspan="6" class="py-8 text-center text-zinc-500">
                    Tidak ada wasit yang sesuai dengan pencarian atau filter.
                  </td>
                </tr>
              {:else}
                {#each filteredWasits as w}
                  <tr class="hover:bg-white/[0.02] transition">
                    <td class="py-3 px-4">
                      <div class="font-bold text-white flex items-center space-x-2">
                        <span>{w.nama_wasit}</span>
                        {#if !w.is_active}
                          <span class="px-1.5 py-0.2 rounded bg-zinc-800 text-zinc-500 text-[9px] font-mono">NON-AKTIF</span>
                        {/if}
                      </div>
                      {#if w.no_lisensi}
                        <div class="text-[10px] font-mono text-zinc-500">{w.no_lisensi}</div>
                      {/if}
                    </td>

                    <td class="py-3 px-4">
                      {#if w.lisensi}
                        <span class="px-2 py-0.5 rounded font-mono text-[10px] font-bold {w.lisensi.includes('WKF') || w.lisensi.includes('AKF') ? 'bg-amber-500/20 text-amber-300 border border-amber-500/30' : 'bg-blue-500/20 text-blue-300 border border-blue-500/30'}">
                          {w.lisensi}
                        </span>
                      {:else}
                        <span class="text-zinc-600">-</span>
                      {/if}
                    </td>

                    <td class="py-3 px-4">
                      <span class="font-medium text-zinc-200">{w.nama_perguruan || "-"}</span>
                    </td>

                    <td class="py-3 px-4 text-zinc-300">
                      {w.kab_kota || "-"}
                    </td>

                    <td class="py-3 px-4">
                      {#if w.assigned_tatami_id}
                        <span class="px-2 py-0.5 rounded-full bg-emerald-500/20 border border-emerald-500/40 text-emerald-300 font-mono text-[10px] font-bold">
                          Tatami {w.assigned_tatami_number} ({w.assigned_posisi})
                        </span>
                      {:else if w.is_active}
                        <span class="px-2 py-0.5 rounded-full bg-zinc-800 text-zinc-400 font-mono text-[10px]">
                          Tersedia (Cadangan)
                        </span>
                      {:else}
                        <span class="px-2 py-0.5 rounded-full bg-zinc-900 text-zinc-600 font-mono text-[10px]">
                          Non-Aktif
                        </span>
                      {/if}
                    </td>

                    <td class="py-3 px-4 text-right space-x-1">
                      <button
                        type="button"
                        onclick={() => openEditWasitModal(w)}
                        class="px-2.5 py-1 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-bold transition"
                      >
                        Edit
                      </button>
                      <button
                        type="button"
                        onclick={() => handleDeleteWasit(w)}
                        class="px-2.5 py-1 rounded bg-red-500/10 hover:bg-red-500/20 text-red-400 text-xs font-bold transition"
                      >
                        Hapus
                      </button>
                    </td>
                  </tr>
                {/each}
              {/if}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  </div>

  <!-- POSITION ASSIGNMENT MODAL -->
  {#if isAssignModalOpen}
    <div
      role="dialog"
      aria-modal="true"
      class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4 animate-in fade-in duration-100"
    >
      <div class="bg-[#181a20] border border-[#2b2d31] rounded-2xl w-full max-w-lg shadow-2xl overflow-hidden flex flex-col max-h-[85vh]">
        <!-- Header -->
        <div class="px-5 py-4 border-b border-[#2b2d31] flex items-center justify-between">
          <div>
            <h3 class="text-sm font-black text-white uppercase tracking-tight">
              Tugaskan Pejabat: {STANDARD_ROLES.find(r => r.key === targetAssignPosisi)?.label}
            </h3>
            <p class="text-xs text-zinc-400">Pilih wasit aktif untuk Tatami {currentTatamiNumber}</p>
          </div>
          <button
            type="button"
            onclick={() => (isAssignModalOpen = false)}
            class="w-7 h-7 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-400 hover:text-white flex items-center justify-center text-sm font-bold"
          >
            ✕
          </button>
        </div>

        <!-- Search Bar -->
        <div class="p-4 border-b border-[#2b2d31]">
          <input
            type="text"
            bind:value={assignSearch}
            placeholder="Ketik untuk mencari wasit..."
            class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-xs text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-[#5865f2]"
          />
        </div>

        <!-- List of available referees -->
        <div class="flex-1 overflow-y-auto p-4 space-y-1.5 divide-y divide-[#2b2d31]/50">
          {#if candidateWasits.length === 0}
            <div class="text-center py-8 text-xs text-zinc-500">
              Tidak ditemukan wasit yang sesuai.
            </div>
          {:else}
            {#each candidateWasits as w}
              <button
                type="button"
                onclick={() => handleAssignWasit(w.id)}
                class="w-full text-left p-3 rounded-xl hover:bg-white/[0.04] transition flex items-center justify-between group pt-2.5"
              >
                <div>
                  <div class="font-bold text-white text-xs group-hover:text-amber-300 transition">
                    {w.nama_wasit}
                  </div>
                  <div class="flex items-center space-x-2 text-[10px] text-zinc-400 mt-0.5">
                    {#if w.lisensi}
                      <span class="text-amber-400 font-mono font-bold">{w.lisensi}</span>
                    {/if}
                    {#if w.nama_perguruan}
                      <span>• {w.nama_perguruan}</span>
                    {/if}
                    {#if w.kab_kota}
                      <span>• {w.kab_kota}</span>
                    {/if}
                  </div>
                </div>

                <div class="text-right">
                  {#if w.assigned_tatami_id}
                    <span class="px-2 py-0.5 rounded text-[9px] font-mono bg-zinc-800 text-zinc-400">
                      Tatami {w.assigned_tatami_number} ({w.assigned_posisi})
                    </span>
                  {:else}
                    <span class="px-2 py-0.5 rounded text-[9px] font-mono bg-emerald-500/20 text-emerald-300 font-bold">
                      Tersedia
                    </span>
                  {/if}
                </div>
              </button>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <!-- ADD / EDIT WASIT MODAL -->
  {#if isWasitModalOpen}
    <div
      role="dialog"
      aria-modal="true"
      class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4 animate-in fade-in duration-100"
    >
      <div class="bg-[#181a20] border border-[#2b2d31] rounded-2xl w-full max-w-md shadow-2xl overflow-hidden flex flex-col">
        <!-- Header -->
        <div class="px-5 py-4 border-b border-[#2b2d31] flex items-center justify-between">
          <h3 class="text-sm font-black text-white uppercase tracking-tight">
            {editingWasit ? "Edit Data Wasit" : "Tambah Wasit / Juri Baru"}
          </h3>
          <button
            type="button"
            onclick={() => (isWasitModalOpen = false)}
            class="w-7 h-7 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-400 hover:text-white flex items-center justify-center text-sm font-bold"
          >
            ✕
          </button>
        </div>

        <!-- Body Form -->
        <div class="p-5 space-y-4 text-xs">
          <div>
            <label for="form-wasit-nama" class="block font-bold text-zinc-300 mb-1">Nama Lengkap & Gelar *</label>
            <input
              id="form-wasit-nama"
              type="text"
              bind:value={formNama}
              placeholder="Contoh: Sensei Bambang Suryono, S.Pd"
              class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-[#5865f2]"
            />
          </div>

          <div>
            <label for="form-wasit-perguruan" class="block font-bold text-zinc-300 mb-1">Perguruan / Aliran</label>
            <select
              id="form-wasit-perguruan"
              bind:value={formPerguruanId}
              class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 focus:outline-none focus:border-[#5865f2]"
            >
              <option value={null}>-- Tidak Ada / Netral --</option>
              {#each perguruans as p}
                <option value={p.id}>{p.nama_perguruan}</option>
              {/each}
            </select>
          </div>

          <div>
            <label for="form-wasit-kabkota" class="block font-bold text-zinc-300 mb-1">Asal Daerah (Kabupaten / Kota)</label>
            <input
              id="form-wasit-kabkota"
              type="text"
              bind:value={formKabKota}
              placeholder="Contoh: Kota Surabaya, Kab. Bandung Barat, DKI Jakarta"
              class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-[#5865f2]"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label for="form-wasit-lisensi" class="block font-bold text-zinc-300 mb-1">Tingkat Lisensi</label>
              <select
                id="form-wasit-lisensi"
                bind:value={formLisensi}
                class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 focus:outline-none focus:border-[#5865f2]"
              >
                {#each LICENSE_OPTIONS as lic}
                  <option value={lic}>{lic}</option>
                {/each}
              </select>
            </div>

            <div>
              <label for="form-wasit-nolisensi" class="block font-bold text-zinc-300 mb-1">Nomor Lisensi (Opsional)</label>
              <input
                id="form-wasit-nolisensi"
                type="text"
                bind:value={formNoLisensi}
                placeholder="Contoh: LIS-INK-001"
                class="w-full bg-[#111214] border border-[#2b2d31] rounded-xl px-3 py-2 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-[#5865f2]"
              />
            </div>
          </div>

          <div class="flex items-center space-x-2 pt-2">
            <input
              id="form-wasit-active"
              type="checkbox"
              bind:checked={formIsActive}
              class="rounded bg-[#111214] border-[#2b2d31] text-[#5865f2] focus:ring-0"
            />
            <label for="form-wasit-active" class="font-bold text-zinc-300 cursor-pointer">
              Status Wasit Aktif (Dapat ditugaskan pada Tatami)
            </label>
          </div>
        </div>

        <!-- Footer -->
        <div class="px-5 py-4 border-t border-[#2b2d31] flex items-center justify-end space-x-2 bg-[#14161b]">
          <button
            type="button"
            onclick={() => (isWasitModalOpen = false)}
            class="px-4 py-2 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-xs font-bold transition"
          >
            Batal
          </button>
          <button
            type="button"
            onclick={handleSaveWasit}
            class="px-5 py-2 rounded-xl bg-[#5865f2] hover:bg-[#4752c4] text-white text-xs font-bold transition shadow-md active:scale-95"
          >
            Simpan Wasit
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
