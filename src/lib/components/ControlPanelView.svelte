<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";
  import { emit, listen } from "@tauri-apps/api/event";
  import { invoke, isHostUi, getHostBaseUrl, withHostParam } from "$lib/api_client";
  import type { DetailBaganItem, BaganItem, SaveMatchInput, TeamMatchInfo, TeamCandidateAthlete, AssignBoutAthletesInput, BoutApprovalState } from "$lib/types";
  import { KATA_ENTRIES, type KataItem } from "$lib/kata_data";
  import ServerStatusBadge from "$lib/components/ServerStatusBadge.svelte";
  import ScoreboardDisplay from "$lib/components/ScoreboardDisplay.svelte";
  import JudgeStickMonitor from "$lib/components/JudgeStickMonitor.svelte";
  import { StickTracker, LEVEL_LABEL, LEVEL_SHORT, type StickState, type StickHistoryRow } from "$lib/judgeSticks";
  import { playWarningSound, playFinishSound, unlockAudio } from "$lib/sounds";
  import { startTicker, type Ticker } from "$lib/ticker";
  import { broadcastLiveMatch, initLiveSync, connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import { license, loadLicense, disciplineOf, canOpenDiscipline, lockedMessage, type LicenseModule } from "$lib/license.svelte";

  interface Props {
    match: DetailBaganItem;
    baganInfo?: BaganItem | null;
    baganName?: string;
    eventName?: string;
    tatamiNumber?: number;
    hostUrl?: string;
    logoMap?: Record<string, string>;
    teamMatchInfo?: TeamMatchInfo | null;
    onBack?: () => void;
    onSave: (input: SaveMatchInput) => Promise<void> | void;
    onRefreshTeam?: () => Promise<void> | void;
    onSwitchBout?: (boutId: number) => Promise<void> | void;
    onAddBout?: () => Promise<void> | void;
    onDeleteBout?: (boutId: number) => Promise<void> | void;
    onAssignBoutAthletes?: (input: AssignBoutAthletesInput) => Promise<void> | void;
    onFinishTeamMatch?: (winnerOverride?: string) => Promise<void> | void;
  }

  let {
    match,
    baganInfo = null,
    baganName = "",
    eventName = "",
    tatamiNumber = 1,
    hostUrl = "",
    logoMap = {},
    teamMatchInfo = null,
    onBack = () => {},
    onSave,
    onRefreshTeam = () => {},
    onSwitchBout = () => {},
    onAddBout = () => {},
    onDeleteBout = () => {},
    onAssignBoutAthletes = () => {},
    onFinishTeamMatch = () => {},
  }: Props = $props();

  // Automatic Discipline Detection (No manual switching required, with manual override for festival)
  let isFestival = $derived(
    match.id === 0 ||
    match.kode === "000" ||
    baganInfo?.kode === "000" ||
    String(baganInfo?.kode || "").toUpperCase().startsWith("F") ||
    (baganName || baganInfo?.nama_bagan || "").toUpperCase().includes("FESTIVAL")
  );
  let manualDiscipline = $state<"kumite" | "kata" | null>(null);

  // License: this panel needs the Kata or Kumite module (the host's license for operator laptops).
  let requiredModule = $derived<LicenseModule | null>(
    manualDiscipline ??
      (isFestival ? null : disciplineOf(baganInfo?.tipe_tanding, baganName || baganInfo?.nama_bagan))
  );
  let licenseBlocked = $derived(!!license.status && !canOpenDiscipline(requiredModule));
  onMount(() => {
    void loadLicense();
  });

  // Match Alarm Threshold Options (30s, 15s, 0s)
  let alarm30sEnabled = $state<boolean>(false);
  let alarm15sEnabled = $state<boolean>(true); // Default ON for karate Atoshibaraku
  let alarm0sEnabled = $state<boolean>(true);  // Default ON for match finish buzzer
  let alarmTriggered30s = $state<boolean>(false);
  let alarmTriggered15s = $state<boolean>(false);
  let alarmTriggered0s = $state<boolean>(false);

  function resetAlarmTriggerFlags() {
    if (timerMs > 30000) {
      alarmTriggered30s = false;
      alarmTriggered15s = false;
      alarmTriggered0s = false;
    } else if (timerMs > 15000) {
      alarmTriggered15s = false;
      alarmTriggered0s = false;
    } else if (timerMs > 0) {
      alarmTriggered0s = false;
    }
  }

  function playWarningChime(seconds: number = 15) {
    try {
      playWarningSound();
    } catch {}
  }

  function playEndBuzzerSound() {
    try {
      playFinishSound();
    } catch {}
  }

  function triggerManualBuzzer() {
    playEndBuzzerSound();
    const payload = {
      type: "manual_buzzer",
      tatami: activeTatami,
      timestamp: Date.now(),
    };
    if (liveBroadcastChannel) {
      liveBroadcastChannel.postMessage(payload);
    }
    emit("tatami_manual_buzzer", payload).catch(() => {});
    if (claimStatus === "owned") {
      tatamiSocket?.send({ ...payload, origin: isHostUi() ? "host" : undefined });
    }
  }
  let activeTatami = $state<number>(1);

  $effect(() => {
    activeTatami = tatamiNumber;
  });

  // ==================== TATAMI OWNERSHIP (KLAIM TATAMI) ====================
  // Only one control panel may drive a tatami. This window claims its tatami on the
  // host, renews the claim every few seconds, and goes inert (no broadcasts, no
  // saving) while another operator holds it.
  interface ClaimResult {
    granted: boolean;
    tatami: number;
    owner_id: string | null;
    owner_label: string | null;
    claimed_at_ms: number | null;
  }

  const CLAIM_HEARTBEAT_MS = 3000;
  const claimOwnerId =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `cp-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const claimOwnerLabel = isHostUi()
    ? "Laptop host"
    : typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
      ? "Laptop operator tambahan"
      : "Perangkat browser";

  let claimStatus = $state<"pending" | "owned" | "blocked">("pending");
  let claimHolderLabel = $state<string>("");
  let claimHolderSince = $state<number | null>(null);
  let isTakingOver = $state<boolean>(false);
  let tatamiSocket: TatamiSocket | null = null;

  async function requestClaim(tatami: number, force = false) {
    try {
      const res = await invoke<ClaimResult>("claim_tatami", {
        tatami,
        ownerId: claimOwnerId,
        ownerLabel: claimOwnerLabel,
        force,
      });
      if (tatami !== activeTatami) return; // switched tatami while waiting
      if (res.granted) {
        const wasOwned = claimStatus === "owned";
        claimStatus = "owned";
        if (!wasOwned) {
          broadcastLiveState();
          recordRunningMatch();
        }
      } else {
        // Mark as blocked first so pausing the timer does not broadcast.
        claimStatus = "blocked";
        claimHolderLabel = res.owner_label || "operator lain";
        claimHolderSince = res.claimed_at_ms ?? null;
        if (isTimerRunning) pauseTimer();
      }
    } catch (err) {
      // Host briefly unreachable, or an older host without claim support: never
      // lock an operator out of a match because of it. Keep the current state, and
      // let a first claim that cannot be checked proceed.
      if (claimStatus === "pending" && tatami === activeTatami) {
        console.warn("Klaim tatami tidak dapat diperiksa, lanjut tanpa klaim:", err);
        claimStatus = "owned";
        broadcastLiveState();
        recordRunningMatch();
      }
    }
  }

  /**
   * The host's database records which match runs on which tatami (marshalling, jury and LO
   * tablets read it). The host's own window does this when it opens; a control panel on
   * another device (browser, operator laptop) has to tell the host.
   */
  let recordedRunning = "";
  $effect(() => {
    // another match or bout opened in this window while it holds the tatami
    void match?.id;
    if (claimStatus === "owned") recordRunningMatch();
  });
  function recordRunningMatch() {
    if (isHostUi() || !match?.id || match.id <= 0) return;
    const key = `${activeTatami}:${match.id}`;
    if (recordedRunning === key) return;
    recordedRunning = key;
    invoke("notify_match_running", { matchId: match.id, tatami: activeTatami }).catch((e) => {
      recordedRunning = "";
      console.warn("Gagal mencatat partai berjalan di host:", e);
    });
  }

  // ==================== CHIEF REFEREE APPROVAL ====================
  // Events that need it: opening a bout asks the chief referee (once the Tatami Manager
  // has submitted its officials). Until approved the panel shows a waiting screen; the
  // scoreboard still shows the bout. Checked every 2 s, so it continues by itself.
  let approval = $state<BoutApprovalState | null>(null);
  let approvalMatchId = $derived(teamMatchInfo?.is_team_match ? teamMatchInfo.parent_match_id : match?.id);
  let approvalLocked = $derived(!!approval && approval.required && approval.status !== "approved" && !match?.selesai && !isFestival);

  async function checkApproval() {
    const id = approvalMatchId;
    if (!id || id <= 0 || isFestival || claimStatus !== "owned") return;
    try {
      const res = await invoke<BoutApprovalState>("request_bout_approval", { matchId: id, tatamiNumber: activeTatami });
      if (id === approvalMatchId) approval = res;
    } catch (e) {
      // an older host without approvals: never lock the operator out
      console.warn("Status persetujuan tidak dapat diperiksa:", e);
    }
  }
  $effect(() => {
    void approvalMatchId;
    untrack(() => {
      approval = null;
      checkApproval();
    });
  });
  $effect(() => {
    if (claimStatus === "owned") untrack(() => checkApproval());
  });
  $effect(() => {
    if (!approvalLocked) return;
    if (untrack(() => isTimerRunning)) pauseTimer();
    const t = setInterval(checkApproval, 2000);
    return () => clearInterval(t);
  });

  async function takeOverTatami() {
    if (isTakingOver) return;
    isTakingOver = true;
    try {
      await requestClaim(activeTatami, true);
    } finally {
      isTakingOver = false;
    }
  }

  function formatClaimTime(ms: number | null): string {
    if (!ms) return "";
    return new Date(ms).toLocaleTimeString("id-ID", { hour: "2-digit", minute: "2-digit" });
  }

  // Claim the current tatami, keep the claim alive, and listen to that tatami's
  // devices. Re-runs when the festival tatami selector changes.
  //
  // Opening a match (or picking a tatami) is an explicit choice, so this window
  // takes the tatami over: whoever opened a match last controls it. This also covers
  // the previous control panel window, which the app closes without giving it a
  // chance to release its claim. The heartbeat never forces, so a window that was
  // taken over stops instead of fighting back.
  $effect(() => {
    const tatami = activeTatami;
    untrack(() => {
      claimStatus = "pending";
      requestClaim(tatami, true);
    });
    const heartbeat = setInterval(() => requestClaim(tatami), CLAIM_HEARTBEAT_MS);
    const socket = connectTatamiSocket("admin-control", tatami, handleSocketMessage, (sock) => {
      if (claimStatus === "owned") {
        broadcastLiveState();
        // The host may have restarted (or been offline): record the running match again.
        recordedRunning = "";
        recordRunningMatch();
      }
    });
    tatamiSocket = socket;
    const releaseOnUnload = () => {
      invoke("release_tatami", { tatami, ownerId: claimOwnerId }).catch(() => {});
    };
    window.addEventListener("beforeunload", releaseOnUnload);
    return () => {
      clearInterval(heartbeat);
      window.removeEventListener("beforeunload", releaseOnUnload);
      socket.close();
      if (tatamiSocket === socket) tatamiSocket = null;
      releaseOnUnload();
    };
  });

  let isKata = $derived.by(() => {
    if (manualDiscipline === "kata") return true;
    if (manualDiscipline === "kumite") return false;

    // 1. Check tipe_tanding (Ambrilindo standard: '1' is Kata, '2' is Kumite)
    const tt = String(baganInfo?.tipe_tanding || "").trim().toLowerCase();
    if (tt === "1" || tt === "kata") return true;
    if (tt === "2" || tt === "kumite") return false;

    // 2. Check bagan name or category name text
    const name = (baganName || baganInfo?.nama_bagan || "").toLowerCase();
    if (name.includes("kata")) return true;
    if (name.includes("kumite")) return false;

    // 3. Check match round/kata entries if present
    if (match.kata1 || match.kata2) return true;

    return false;
  });

  // Common Match State
  let matchWinner = $state<string>("");
  // Banner pemenang di scoring board. Hanya muncul saat operator memilih pemenang
  // (atau kiken) di sesi ini, tidak saat membuka ulang partai yang sudah selesai.
  let showWinnerBanner = $state<boolean>(false);
  let isMatchFinished = $state<boolean>(false);
  let isSaving = $state<boolean>(false);
  let hasVr1 = $state<boolean>(false);
  let hasVr2 = $state<boolean>(false);
  let isHantei = $state<boolean>(false);

  // Video Review (VR), as in Ambrilindo Local: the coach asks for a value
  // (1/2/3), the VR referee tablet decides. Each corner has its own request, so
  // AKA and AO can be under review at the same time.
  // hasVr1 / hasVr2 = true means that corner's VR card is used up (hangus).
  // The match record (vr1 / vr2) uses the opposite meaning, as in Ambrilindo Local
  // and the bracket: true = the athlete still holds a VR card.
  type VrSide = "aka" | "ao";
  interface VrChallengeData {
    side: VrSide;
    reason: string;
    timestamp: number;
    status: "in_progress" | "accepted" | "rejected" | "minai";
    /** value asked for while in progress; points awarded once accepted */
    points?: number;
  }
  let vrChallenges = $state<{ aka: VrChallengeData | null; ao: VrChallengeData | null }>({ aka: null, ao: null });
  // Decided requests stay up until every open request is decided, then all clear together
  let vrClearTimer: ReturnType<typeof setTimeout> | null = null;
  // First open (or just decided) request, for single-request consumers
  let activeVrChallenge = $derived(
    vrChallenges.aka?.status === "in_progress" ? vrChallenges.aka
    : vrChallenges.ao?.status === "in_progress" ? vrChallenges.ao
    : vrChallenges.aka ?? vrChallenges.ao
  );
  // Category uses Video Review (as in Ambrilindo Local: controls + board logo only then)
  let vrEnabled = $derived(!!baganInfo?.has_vr);

  const VR_POINT_NAME: Record<number, string> = { 1: "YUKO", 2: "WAZA-ARI", 3: "IPPON" };
  function vrReason(points?: number): string {
    return points && VR_POINT_NAME[points] ? `${VR_POINT_NAME[points]} (${points} POINT)` : "";
  }

  function setVrChallenge(side: VrSide, value: VrChallengeData | null) {
    vrChallenges = { ...vrChallenges, [side]: value };
    scheduleVrClear();
  }

  /**
   * While any corner is still under review, decided results stay on screen.
   * Once none is open, all results clear together a few seconds later.
   */
  function scheduleVrClear() {
    if (vrClearTimer) clearTimeout(vrClearTimer);
    vrClearTimer = null;
    const list = [vrChallenges.aka, vrChallenges.ao];
    if (list.some((c) => c?.status === "in_progress") || !list.some(Boolean)) return;
    vrClearTimer = setTimeout(() => {
      vrClearTimer = null;
      vrChallenges = { aka: null, ao: null };
      broadcastLiveState();
    }, 3000);
  }

  function clearVrChallenges() {
    if (vrClearTimer) clearTimeout(vrClearTimer);
    vrClearTimer = null;
    vrChallenges = { aka: null, ao: null };
  }

  /** Card status as stored on the match (true = card still held) */
  // (only shown / saved when the category uses VR, so this does not depend on baganInfo being loaded yet)
  function vrUsedFromRecord(held: boolean | null | undefined) {
    return !held;
  }

  function isVrCardUsed(side: VrSide) {
    return side === "aka" ? hasVr1 : hasVr2;
  }

  /** Open a review for a corner (from the operator, or the coach's tablet) */
  // The timer keeps running: stopping it stays the operator's call.
  function startVrChallenge(side: VrSide, points?: number, reason?: string) {
    setVrChallenge(side, {
      side,
      reason: reason || vrReason(points),
      timestamp: Date.now(),
      status: "in_progress",
      points: points && points >= 1 && points <= 3 ? points : undefined,
    });
    broadcastLiveState();
  }

  // Operator asks for a review (same as "Request Review" in Ambrilindo Local)
  function requestVr(side: VrSide, points?: number) {
    startVrChallenge(side, points);
  }

  /**
   * Apply a decision to a corner's open review:
   * - "accept" (value, default the value asked for): card kept. The score is NOT
   *   added here; the operator adds it on the scoring buttons.
   * - "decline": card lost (hangus)
   * - "minai": inconclusive, card kept
   * - "cancel": withdrawn, nothing changes
   */
  function applyVrDecision(side: VrSide, decision: "accept" | "decline" | "minai" | "cancel", points?: number) {
    const current = vrChallenges[side];
    if (!current || current.status !== "in_progress") return;
    if (decision === "cancel") {
      setVrChallenge(side, null);
      broadcastLiveState();
      return;
    }
    const awarded = decision === "accept" ? (points && points > 0 ? points : current.points || 0) : 0;
    if (decision === "decline") {
      if (side === "aka") hasVr1 = true;
      else hasVr2 = true;
    }
    setVrChallenge(side, {
      ...current,
      timestamp: Date.now(),
      status: decision === "accept" ? "accepted" : decision === "decline" ? "rejected" : "minai",
      points: decision === "accept" ? awarded || undefined : current.points,
    });
    broadcastLiveState();
  }

  /** Operator decides here; the coach and VR tablets are told too */
  function resolveVr(side: VrSide, decision: "accept" | "decline" | "minai" | "cancel", points?: number) {
    const current = vrChallenges[side];
    if (!current || current.status !== "in_progress") return;
    const awarded = decision === "accept" ? (points && points > 0 ? points : current.points || 0) : undefined;
    applyVrDecision(side, decision, points);
    // Contract with the coach / VR devices: accepted | rejected | minai | cancelled
    const wire = decision === "accept" ? "accepted" : decision === "decline" ? "rejected" : decision === "minai" ? "minai" : "cancelled";
    const msg = { type: "vr_challenge_resolved", tatami: activeTatami, side, decision: wire, points: awarded, timestamp: Date.now() };
    if (liveBroadcastChannel) liveBroadcastChannel.postMessage(msg);
    fetch(`${getHostBaseUrl()}/api/vr/resolve`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tatami: activeTatami, side, decision: wire, points: awarded }),
    }).catch(() => {});
  }

  // A request from the coach's tablet (via the server or, on the same machine, BroadcastChannel)
  function applyRemoteVrStarted(data: any) {
    const side: VrSide = data.side === "ao" ? "ao" : "aka";
    if (!vrEnabled || isKata || isVrCardUsed(side)) return;
    const current = vrChallenges[side];
    const points = Number(data.points) || undefined;
    // Same request arriving twice (server + BroadcastChannel): only update the value
    if (current?.status === "in_progress") {
      if (points && current.points !== points) {
        setVrChallenge(side, { ...current, points, reason: vrReason(points) });
        broadcastLiveState();
      }
      return;
    }
    // New request from the coach: alert the operator (same sound as the 15 s warning)
    playWarningChime();
    startVrChallenge(side, points, data.reason);
  }

  // A decision made on another device (VR referee tablet, or the coach cancelling).
  // Our own decisions come back from the server too; once the request is no longer
  // open they are ignored, so the echo can never apply a decision twice.
  function applyRemoteVrResolved(data: any) {
    const side: VrSide = data.side === "ao" ? "ao" : "aka";
    const decision = String(data.decision || "");
    const points = Number(data.points) || undefined;
    if (decision === "cancelled") applyVrDecision(side, "cancel");
    else if (decision === "accepted") applyVrDecision(side, "accept", points);
    else if (decision === "minai") applyVrDecision(side, "minai");
    else if (decision === "rejected") applyVrDecision(side, "decline");
  }

  // Manual correction of a corner's card (e.g. restored after an operator mistake)
  function toggleVrCard(side: "aka" | "ao") {
    if (side === "aka") hasVr1 = !hasVr1;
    else hasVr2 = !hasVr2;
    broadcastLiveState();
  }

  // Reactive Sync when match prop changes (switching bouts in team kumite or festival pairs)
  let lastLoadedMatchKey = $state<string>("");

  // Kata: muat skor juri yang sudah tersimpan untuk partai ini
  function parseJudgeArray(raw: unknown): string[] | null {
    if (typeof raw !== "string" || !raw) return null;
    try {
      const arr = JSON.parse(raw);
      if (!Array.isArray(arr)) return null;
      return judgeIdx.map((i) => (arr[i] == null ? "" : String(arr[i])));
    } catch {
      return null;
    }
  }

  async function loadSavedJudgeScores(matchId: number) {
    try {
      const res: any = await invoke("get_match_juri_scores", { matchId });
      // Ignore if the operator already moved on, or started typing scores
      if (match.id !== matchId) return;
      if ([...akaJudgeScores, ...aoJudgeScores].some((s) => s.trim() !== "")) return;
      const aka = parseJudgeArray(res?.juri_scores1);
      const ao = parseJudgeArray(res?.juri_scores2);
      if (!aka && !ao) return;
      if (aka) akaJudgeScores = aka;
      if (ao) aoJudgeScores = ao;
      manualFlags = null;
      broadcastLiveState();
    } catch (err) {
      console.warn("Gagal memuat skor juri tersimpan:", err);
    }
  }

  function syncFromMatch(m: DetailBaganItem) {
    matchWinner = m.pemenang || "";
    autoWinner = null;
    showWinnerBanner = false;
    stopPrepCountdown(false);
    clearStickState();
    clearVrChallenges();
    isMatchFinished = m.selesai || false;
    hasVr1 = vrUsedFromRecord(m.vr1);
    hasVr2 = vrUsedFromRecord(m.vr2);
    isHantei = m.hantei || false;

    akaScore = parseInt(m.score1 || "0", 10) || 0;
    aoScore = parseInt(m.score2 || "0", 10) || 0;

    kataAkaName = m.kata1 || "";
    kataAoName = m.kata2 || "";
    kataFromLo = { aka: false, ao: false };

    // Reset judges scores and overlays for new match, then restore any saved ones
    akaJudgeScores = emptyJudges();
    aoJudgeScores = emptyJudges();
    showKataScoreOnBoard = false;
    if (isKata && m.id) loadSavedJudgeScores(m.id);

    akaPenalties = { c1: false, c2: false, c3: false, hc: false, h: false };
    aoPenalties = { c1: false, c2: false, c3: false, hc: false, h: false };
    akaSenshu = false;
    aoSenshu = false;

    if (m.score1 || m.score2 || m.pemenang || m.selesai) {
      boardAkaScore = parseInt(m.score1 || "0", 10) || 0;
      boardAoScore = parseInt(m.score2 || "0", 10) || 0;
      isScorePublished = true;
      manualFlags = isKata ? { aka: boardAkaScore, ao: boardAoScore } : null;
    } else {
      boardAkaScore = 0;
      boardAoScore = 0;
      isScorePublished = false;
      manualFlags = null;
    }

    timerMs = selectedPresetSeconds * 1000;
    isTimerRunning = false;
  }

  $effect(() => {
    const currentKey = `${match.id}_${match.atlet1_nama || ""}_${match.atlet2_nama || ""}_${match.kata1 || ""}_${match.kata2 || ""}`;
    if (currentKey !== lastLoadedMatchKey) {
      lastLoadedMatchKey = currentKey;
      syncFromMatch(match);
      broadcastLiveState();
    }
  });

  // Team Kumite Modal States
  // Atur atlet partai (kumite beregu): pick from the athletes registered for this
  // team category and kontingen (team members first), as in Ambrilindo Local
  let isRosterModalOpen = $state(false);
  let rosterAkaId = $state<number | null>(null);
  let rosterAoId = $state<number | null>(null);
  let isSavingRoster = $state(false);

  function openRosterModal() {
    rosterAkaId = match.atlet1_id;
    rosterAoId = match.atlet2_id;
    isRosterModalOpen = true;
  }

  /** Bouts (other than this one) where an athlete already fought for that corner */
  function rosterPlayedIn(side: "aka" | "ao", atletId: number): number[] {
    if (!teamMatchInfo) return [];
    return teamMatchInfo.bouts
      .filter((b) => b.bout_id !== match.id && (side === "aka" ? b.atlet1_id : b.atlet2_id) === atletId)
      .map((b) => b.round);
  }

  async function handleSaveRoster() {
    if (isSavingRoster) return;
    isSavingRoster = true;
    try {
      await onAssignBoutAthletes({
        bout_id: match.id,
        aka_atlet_id: rosterAkaId,
        ao_atlet_id: rosterAoId,
        aka_custom_name: null,
        ao_custom_name: null,
      });
      isRosterModalOpen = false;
    } finally {
      isSavingRoster = false;
    }
  }

  let isFinishTeamModalOpen = $state(false);
  let selectedTeamWinner = $state<"1" | "2" | "">("");

  function openFinishTeamModal() {
    if (!teamMatchInfo) return;
    if (teamMatchInfo.team_aka_score > teamMatchInfo.team_ao_score) {
      selectedTeamWinner = "1";
    } else if (teamMatchInfo.team_ao_score > teamMatchInfo.team_aka_score) {
      selectedTeamWinner = "2";
    } else if (teamMatchInfo.team_aka_points > teamMatchInfo.team_ao_points) {
      selectedTeamWinner = "1";
    } else if (teamMatchInfo.team_ao_points > teamMatchInfo.team_aka_points) {
      selectedTeamWinner = "2";
    } else {
      selectedTeamWinner = "";
    }
    isFinishTeamModalOpen = true;
  }

  async function handleConfirmFinishTeam() {
    if (!selectedTeamWinner) {
      alert("Pilih tim pemenang (AKA atau AO) terlebih dahulu!");
      return;
    }
    await onFinishTeamMatch(selectedTeamWinner);
    isFinishTeamModalOpen = false;
  }

  // Kumite Score State
  let akaScore = $state<number>(0);
  let aoScore = $state<number>(0);
  let akaSenshu = $state<boolean>(false);
  let aoSenshu = $state<boolean>(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });
  let aoPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // Kata judges: 3, 5 or 7 per tatami, chosen in ⚙ and remembered on this device
  // (the next match keeps it). The scoreboard pop up and jury tablets follow it.
  const KATA_JUDGE_OPTIONS = [3, 5, 7];
  const JUDGE_COUNT_KEY = "ambrilindo.kataJudgeCount";
  function loadJudgeCount(): number {
    try {
      const v = Number(localStorage.getItem(JUDGE_COUNT_KEY));
      return KATA_JUDGE_OPTIONS.includes(v) ? v : 5;
    } catch {
      return 5;
    }
  }
  let kataJudgeCount = $state<number>(loadJudgeCount());
  let judgeIdx = $derived(Array.from({ length: kataJudgeCount }, (_, i) => i));
  function emptyJudges(fill = ""): string[] {
    return Array.from({ length: kataJudgeCount }, () => fill);
  }
  function fitJudges(arr: string[]): string[] {
    return Array.from({ length: kataJudgeCount }, (_, i) => arr[i] ?? "");
  }
  function setKataJudgeCount(n: number) {
    if (!KATA_JUDGE_OPTIONS.includes(n) || n === kataJudgeCount) return;
    kataJudgeCount = n;
    try {
      localStorage.setItem(JUDGE_COUNT_KEY, String(n));
    } catch {}
    akaJudgeScores = fitJudges(akaJudgeScores);
    aoJudgeScores = fitJudges(aoJudgeScores);
    if (activeJudgePicker !== null && activeJudgePicker >= n) activeJudgePicker = null;
    // Flags can never exceed the number of judges
    if (manualFlags) {
      const aka = Math.min(manualFlags.aka, n);
      manualFlags = { aka, ao: n - aka };
    }
    broadcastLiveState();
  }

  // Kata Judges Comparative Scoring State
  let kataAkaName = $state<string>("");
  let kataAoName = $state<string>("");
  // Kata filled in by the LO Kata tablet (badge "LO"); flash bumps to highlight a new one
  let kataFromLo = $state<{ aka: boolean; ao: boolean }>({ aka: false, ao: false });
  let kataLoFlash = $state<{ aka: number; ao: number }>({ aka: 0, ao: 0 });
  let akaJudgeScores = $state<string[]>(emptyJudges());
  let aoJudgeScores = $state<string[]>(emptyJudges());
  let showKataScoreOnBoard = $state<boolean>(false);

  // Kata Bendera (flags) — Ambrilindo Local standard.
  // Each corner's flags are the judges who gave it the higher score, counted per corner:
  // a tied or not-yet-entered judge gives no flag to either side (so AO is NOT simply
  // "judges − AKA"). The operator can override with the flag selector (chief referee
  // flags without scores); any judge score entered afterwards returns to counting.
  // A saved result without judge scores is restored as an override with its own totals.
  let manualFlags = $state<{ aka: number; ao: number } | null>(null);
  let hasJudgeInput = $derived(
    akaJudgeScores.some((s) => s?.trim() !== "") || aoJudgeScores.some((s) => s?.trim() !== "")
  );

  let effectiveAkaPoints = $derived.by(() => (manualFlags ? manualFlags.aka : judgeComparisons.akaPoints));
  let effectiveAoPoints = $derived.by(() => (manualFlags ? manualFlags.ao : judgeComparisons.aoPoints));

  // Shown in the flag column ("-" = nothing entered yet)
  let akaBendera = $derived.by(() => (manualFlags || hasJudgeInput ? String(effectiveAkaPoints) : "-"));
  let aoBendera = $derived.by(() => (manualFlags || hasJudgeInput ? String(effectiveAoPoints) : "-"));

  function handleAkaBenderaChange(val: string) {
    if (val === "-") {
      manualFlags = null;
      return;
    }
    const aka = Math.max(0, Math.min(kataJudgeCount, parseInt(val, 10) || 0));
    manualFlags = { aka, ao: kataJudgeCount - aka };
    // Automatically output score to arena scoreboard when using manual bendera!
    publishScoresToBoard();
  }

  // Last live payload, drawn by the same ScoreboardDisplay template the TV uses
  let previewState = $state.raw<any>(null);

  // Board Published Score State (Do NOT output scores to scoreboard while inputting!)
  let boardAkaScore = $state<number>(0);
  let boardAoScore = $state<number>(0);
  let isScorePublished = $state<boolean>(false);
  let publishFeedbackActive = $state<boolean>(false);

  // Button 1: Tampilkan Skor di Board (Versi tanpa pop up)
  function publishScoresToBoard() {
    boardAkaScore = effectiveAkaPoints;
    boardAoScore = effectiveAoPoints;
    isScorePublished = true;
    publishFeedbackActive = true;
    broadcastLiveState();
    setTimeout(() => {
      publishFeedbackActive = false;
    }, 1500);
  }

  // Button 1 sebagai toggle: tampilkan / sembunyikan skor besar kata di board
  // (saat disembunyikan, board menampilkan 0). Menampilkan selalu mengirim nilai terbaru.
  // (Input bendera manual & kiken tetap langsung menampilkan skor lewat publishScoresToBoard.)
  function toggleBoardScore() {
    if (isScorePublished) {
      isScorePublished = false;
      broadcastLiveState();
    } else {
      publishScoresToBoard();
    }
  }

  // Button 2: Pop Up Skor Juri. Membuka pop up juga memaksa skor besar tampil
  // (scoreboard baru memunculkannya setelah 5 skor juri selesai muncul).
  // Menutup pop up tidak menyembunyikan skor besar; itu lewat tombol 1.
  function toggleKataScorePopup() {
    showKataScoreOnBoard = !showKataScoreOnBoard;
    if (showKataScoreOnBoard) {
      boardAkaScore = effectiveAkaPoints;
      boardAoScore = effectiveAoPoints;
      isScorePublished = true;
    }
    broadcastLiveState();
  }

  // Quick Score Picker Popover State (Clean, AKA atas, AO bawah, Horizontal scroll)
  let activeJudgePicker = $state<number | null>(null);
  // Alarm / bel popover under the timer (kata and kumite)
  let showTimerSettings = $state<boolean>(false);

  // ==================== KATA: TIMER PERSIAPAN 35 DETIK ====================
  // Pop up hitung mundur di scoring board agar atlet segera memulai kata.
  // Setelah habis, board menampilkan "WAKTU HABIS" sampai operator menekan Stop.
  // Board countdown pop up: kata preparation (35 s) or kumite medical / injury time (3 min)
  type PrepKind = "kata" | "medis";
  const PREP_DURATION_MS: Record<PrepKind, number> = { kata: 35000, medis: 180000 };
  let prepCountdown = $state<{ id: number; side: "aka" | "ao"; kind: PrepKind; durationMs: number; startedAt: number } | null>(null);
  let prepRemainingMs = $state<number>(0);
  let prepTicker: ReturnType<typeof setInterval> | null = null;

  function prepRemaining(): number {
    if (!prepCountdown) return 0;
    return Math.max(0, prepCountdown.durationMs - (performance.now() - prepCountdown.startedAt));
  }

  function startPrepCountdown(side: "aka" | "ao", kind: PrepKind = "kata") {
    const durationMs = PREP_DURATION_MS[kind];
    prepCountdown = { id: Date.now(), side, kind, durationMs, startedAt: performance.now() };
    prepRemainingMs = durationMs;
    if (prepTicker) clearInterval(prepTicker);
    prepTicker = setInterval(() => {
      prepRemainingMs = prepRemaining();
      if (prepRemainingMs <= 0 && prepTicker) {
        clearInterval(prepTicker);
        prepTicker = null;
      }
    }, 100);
    broadcastLiveState();
  }

  function stopPrepCountdown(broadcast = true) {
    if (prepTicker) clearInterval(prepTicker);
    prepTicker = null;
    prepCountdown = null;
    prepRemainingMs = 0;
    if (broadcast) broadcastLiveState();
  }
  const KATA_SCORE_LIST = [
    "0.0",
    ...Array.from({ length: 51 }, (_, i) => (5.0 + i * 0.1).toFixed(1))
  ];

  // Kata Discord-style Selection Modal State
  let isKataModalOpen = $state<boolean>(false);
  let kataModalTarget = $state<"aka" | "ao">("aka");
  let kataSearchQuery = $state<string>("");
  let kataModalSelectedIndex = $state<number>(0);
  let searchInputEl = $state<HTMLInputElement | null>(null);

  let filteredKataList = $derived.by(() => {
    const q = kataSearchQuery.trim().toLowerCase();
    if (!q) return KATA_ENTRIES;
    return KATA_ENTRIES.filter(
      (k) =>
        k.code.includes(q) ||
        k.name.toLowerCase().includes(q) ||
        k.label.toLowerCase().includes(q)
    );
  });

  function openKataModal(target: "aka" | "ao") {
    kataModalTarget = target;
    kataSearchQuery = "";
    kataModalSelectedIndex = 0;
    isKataModalOpen = true;
    setTimeout(() => {
      searchInputEl?.focus();
    }, 50);
  }

  function selectKataItem(item: KataItem) {
    if (kataModalTarget === "aka") {
      kataAkaName = item.label;
    } else {
      kataAoName = item.label;
    }
    kataFromLo = { ...kataFromLo, [kataModalTarget]: false };
    isKataModalOpen = false;
    broadcastLiveState();
  }

  function handleKataModalKeydown(e: KeyboardEvent) {
    if (!isKataModalOpen) return;
    if (e.key === "Escape") {
      isKataModalOpen = false;
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (filteredKataList.length > 0) {
        kataModalSelectedIndex = (kataModalSelectedIndex + 1) % filteredKataList.length;
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (filteredKataList.length > 0) {
        kataModalSelectedIndex =
          (kataModalSelectedIndex - 1 + filteredKataList.length) % filteredKataList.length;
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filteredKataList[kataModalSelectedIndex]) {
        selectKataItem(filteredKataList[kataModalSelectedIndex]);
      }
    }
  }

  // ==================== MATCH TIMER STATE & PRESETS (MILLISECONDS) ====================
  const TIMER_PRESETS = [30, 60, 90, 120, 150, 180, 300];

  // Preset waktu terakhir diingat per disiplin (kata / kumite), jadi partai
  // berikutnya langsung memakai waktu yang sama tanpa diatur ulang.
  function presetStorageKey(kata: boolean) {
    return `ambrilindo.timerPreset.${kata ? "kata" : "kumite"}`;
  }
  function readStoredPreset(kata: boolean): number {
    try {
      const v = Number(localStorage.getItem(presetStorageKey(kata)));
      if (Number.isFinite(v) && v > 0) return v;
    } catch {}
    return 120;
  }
  function storePreset(kata: boolean, seconds: number) {
    try {
      localStorage.setItem(presetStorageKey(kata), String(seconds));
    } catch {}
  }

  const initialPresetSeconds = readStoredPreset(untrack(() => isKata));
  let selectedPresetSeconds = $state<number>(initialPresetSeconds);
  let timerMs = $state<number>(initialPresetSeconds * 1000);
  let isTimerRunning = $state<boolean>(false);
  // Match clock tick: a worker-driven ticker, so the clock, the board updates and the end
  // buzzer keep going while this window is minimized or behind another window.
  let timerTicker: Ticker | null = null;
  function stopTimerTicker() {
    timerTicker?.stop();
    timerTicker = null;
  }
  let lastTickTimestamp: number = 0;
  let lastBroadcastTimestamp: number = 0;
  let liveBroadcastChannel: BroadcastChannel | null = null;

  let timerFormatted = $derived.by(() => {
    const safeMs = Math.max(0, Math.floor(timerMs));
    const totalSec = Math.floor(safeMs / 1000);
    const m = Math.floor(totalSec / 60);
    const s = totalSec % 60;
    const hundredths = Math.floor((safeMs % 1000) / 10);
    const mStr = m < 10 ? `0${m}` : `${m}`;
    const sStr = s < 10 ? `0${s}` : `${s}`;
    const msStr = hundredths < 10 ? `0${hundredths}` : `${hundredths}`;
    return {
      mStr,
      sStr,
      msStr,
      mmss: `${mStr}:${sStr}`,
      full: `${mStr}:${sStr}.${msStr}`,
    };
  });

  // Consistent digital timer format (m:ss e.g. 0:30, 1:00, 1:30, 2:00, 3:00, 5:00)
  function formatPresetLabel(preset: number): string {
    const m = Math.floor(preset / 60);
    const s = preset % 60;
    return `${m}:${s < 10 ? "0" + s : s}`;
  }

  function runTimerLoop() {
    if (!isTimerRunning) return;
    const now = performance.now();
    const delta = now - lastTickTimestamp;
    lastTickTimestamp = now;

    const prevMs = timerMs;
    timerMs = Math.max(0, timerMs - delta);

    // Alarm thresholds checking with auto-rearm if time increases
    if (alarm30sEnabled && prevMs > 30000 && timerMs <= 30000 && !alarmTriggered30s) {
      playWarningChime(30);
      alarmTriggered30s = true;
    } else if (timerMs > 30000) {
      alarmTriggered30s = false;
    }

    if (alarm15sEnabled && prevMs > 15000 && timerMs <= 15000 && !alarmTriggered15s) {
      playWarningChime(15);
      alarmTriggered15s = true;
    } else if (timerMs > 15000) {
      alarmTriggered15s = false;
    }

    if (alarm0sEnabled && prevMs > 0 && timerMs <= 0 && !alarmTriggered0s) {
      playEndBuzzerSound();
      alarmTriggered0s = true;
    } else if (timerMs > 0) {
      alarmTriggered0s = false;
    }

    // Periodic live sync pulse (every 150ms) to sync scoreboard without flooding IPC
    if (now - lastBroadcastTimestamp > 150) {
      lastBroadcastTimestamp = now;
      broadcastLiveState();
    }

    if (timerMs <= 0) {
      timerMs = 0;
      isTimerRunning = false;
      stopTimerTicker();
      broadcastLiveState();
    }
  }

  function selectTimerPreset(seconds: number) {
    selectedPresetSeconds = seconds;
    storePreset(isKata, seconds);
    timerMs = seconds * 1000;
    resetAlarmTriggerFlags();
    if (isTimerRunning) {
      stopTimerTicker();
      isTimerRunning = false;
    }
    broadcastLiveState();
  }

  function pauseTimer() {
    if (isTimerRunning) {
      stopTimerTicker();
      isTimerRunning = false;
      broadcastLiveState();
    }
  }

  let lastToggleTimestamp = 0;

  function toggleTimer() {
    const now = performance.now();
    if (now - lastToggleTimestamp < 250) {
      return; // Debounce rapid accidental double-toggle / key-repeat bouncing
    }
    lastToggleTimestamp = now;

    if (isTimerRunning) {
      stopTimerTicker();
      isTimerRunning = false;
      broadcastLiveState();
    } else {
      if (timerMs <= 0) {
        // Do not auto-reset or jump to preset when timer is already at 00:00:00
        return;
      }
      isTimerRunning = true;
      lastTickTimestamp = performance.now();
      lastBroadcastTimestamp = lastTickTimestamp;
      broadcastLiveState();
      stopTimerTicker();
      timerTicker = startTicker(runTimerLoop);
    }
  }

  function resetTimer() {
    if (isTimerRunning) {
      stopTimerTicker();
      isTimerRunning = false;
    }
    timerMs = selectedPresetSeconds * 1000;
    resetAlarmTriggerFlags();
    broadcastLiveState();
  }

  function adjustTimer(deltaMs: number) {
    timerMs = Math.max(0, timerMs + deltaMs);
    resetAlarmTriggerFlags();
    broadcastLiveState();
  }

  // ==================== RESET ALL MATCH DATA (KATA & KUMITE) ====================
  // The top Reset clears the whole bout: ask first when there is something to lose.
  function confirmResetAllMatchData() {
    const penaltiesSet = Object.values(akaPenalties).some(Boolean) || Object.values(aoPenalties).some(Boolean);
    const hasData =
      akaScore > 0 || aoScore > 0 || akaSenshu || aoSenshu || penaltiesSet || !!matchWinner ||
      hasJudgeInput || !!manualFlags || timerMs !== selectedPresetSeconds * 1000;
    if (hasData && !confirm("Reset semua skor, pelanggaran, dan waktu partai ini?")) return;
    resetAllMatchData();
  }

  function resetAllMatchData() {
    // 1. Reset timer to current selected preset
    if (isTimerRunning) {
      stopTimerTicker();
      isTimerRunning = false;
    }
    timerMs = selectedPresetSeconds * 1000;

    // 2. Reset Kumite scores & penalties
    akaScore = 0;
    aoScore = 0;
    akaSenshu = false;
    aoSenshu = false;
    akaPenalties = { c1: false, c2: false, c3: false, hc: false, h: false };
    aoPenalties = { c1: false, c2: false, c3: false, hc: false, h: false };

    // 3. Reset Kata judge scores & bendera
    akaJudgeScores = emptyJudges();
    aoJudgeScores = emptyJudges();
    manualFlags = null;
    boardAkaScore = 0;
    boardAoScore = 0;
    isScorePublished = false;
    showKataScoreOnBoard = false;

    // 4. Reset match decision flags
    matchWinner = "";
    autoWinner = null;
    showWinnerBanner = false;
    stopPrepCountdown(false);
    clearStickState();
    clearVrChallenges();
    isMatchFinished = false;
    hasVr1 = false;
    hasVr2 = false;
    isHantei = false;

    // 5. Broadcast reset state to arena scoreboard immediately
    broadcastLiveState();
  }

  // ==================== KATA JUDGES COMPARATIVE SCORING (3 / 5 / 7) ====================
  let judgeComparisons = $derived.by(() => {
    let akaPoints = 0;
    let aoPoints = 0;
    let decisions: Array<{
      winner: "aka" | "ao" | "seri" | "empty";
      scoreAka: number | null;
      scoreAo: number | null;
    }> = [];

    for (let i = 0; i < kataJudgeCount; i++) {
      const rawAka = akaJudgeScores[i]?.trim();
      const rawAo = aoJudgeScores[i]?.trim();
      const vAka = rawAka !== "" ? parseFloat(rawAka) : NaN;
      const vAo = rawAo !== "" ? parseFloat(rawAo) : NaN;

      const hasAka = !isNaN(vAka) && ((vAka >= 5.0 && vAka <= 10.0) || vAka === 0);
      const hasAo = !isNaN(vAo) && ((vAo >= 5.0 && vAo <= 10.0) || vAo === 0);

      if (hasAka && hasAo) {
        if (vAka === 0 && vAo > 0) {
          aoPoints += 1;
          decisions.push({ winner: "ao", scoreAka: vAka, scoreAo: vAo });
        } else if (vAo === 0 && vAka > 0) {
          akaPoints += 1;
          decisions.push({ winner: "aka", scoreAka: vAka, scoreAo: vAo });
        } else if (vAka > vAo) {
          akaPoints += 1;
          decisions.push({ winner: "aka", scoreAka: vAka, scoreAo: vAo });
        } else if (vAo > vAka) {
          aoPoints += 1;
          decisions.push({ winner: "ao", scoreAka: vAka, scoreAo: vAo });
        } else {
          decisions.push({ winner: "seri", scoreAka: vAka, scoreAo: vAo });
        }
      } else if (hasAka && !hasAo) {
        decisions.push({ winner: "empty", scoreAka: vAka, scoreAo: null });
      } else if (!hasAka && hasAo) {
        decisions.push({ winner: "empty", scoreAka: null, scoreAo: vAo });
      } else {
        decisions.push({ winner: "empty", scoreAka: null, scoreAo: null });
      }
    }

    return { akaPoints, aoPoints, decisions };
  });

  // Split-Second Scoreboard Banner Notification State
  interface LiveBannerEntry {
    side: "aka" | "ao";
    title: string;
    subtitle: string;
    /** judge flags to draw (judge stick points) */
    judges?: number;
  }
  interface LiveScoreBanner extends LiveBannerEntry {
    id: number;
    durationMs: number;
    /** one or two corners shown side by side (judge sticks: AKA and AO in the same exchange) */
    entries?: LiveBannerEntry[];
  }
  let activeBanner = $state<LiveScoreBanner | null>(null);
  let bannerClearTimeout: any = null;
  let lastBannerId = 0;

  function triggerScoreBanner(
    side: "aka" | "ao",
    title: string,
    subtitle: string,
    durationMs = 1300,
    extra: { judges?: number; entries?: LiveBannerEntry[] } = {}
  ) {
    if (bannerClearTimeout) {
      clearTimeout(bannerClearTimeout);
      bannerClearTimeout = null;
    }
    // Unique, increasing id: the board only redraws a banner whose id changed, and
    // judge sticks can fire several updates within the same millisecond
    lastBannerId = Math.max(Date.now(), lastBannerId + 1);
    activeBanner = {
      id: lastBannerId,
      side,
      title,
      subtitle,
      durationMs,
      ...extra,
    };
    broadcastLiveState();

    // durationMs 0 = stays until the operator scores or closes it (judge stick banner)
    if (durationMs > 0) {
      bannerClearTimeout = setTimeout(() => {
        activeBanner = null;
        broadcastLiveState();
      }, durationMs);
    }
  }

  function dismissScoreBanner() {
    if (bannerClearTimeout) {
      clearTimeout(bannerClearTimeout);
      bannerClearTimeout = null;
    }
    activeBanner = null;
    broadcastLiveState();
  }

  // Guardrails & Input Validation for Kata Judge Score
  function sanitizeScore(val: string): string {
    const trimmed = val.trim();
    if (!trimmed) return "";
    const num = parseFloat(trimmed);
    if (isNaN(num)) return "";
    if (num === 0) return "0.0"; // Valid Kiken
    // Guardrail: cannot be less than 5.0
    let clamped = num;
    if (clamped < 5.0) clamped = 5.0;
    if (clamped > 10.0) clamped = 10.0;
    return clamped.toFixed(1);
  }

  function updateJudgeScore(side: "aka" | "ao", judgeIdx: number, val: string) {
    if (side === "aka") {
      akaJudgeScores[judgeIdx] = val;
      akaJudgeScores = [...akaJudgeScores];
    } else {
      aoJudgeScores[judgeIdx] = val;
      aoJudgeScores = [...aoJudgeScores];
    }
    // A judge score was entered: flags follow the judges again
    manualFlags = null;
    // Notice: Do NOT output to scoreboard while operator is inputting scores!
  }

  function handleScoreInputBlur(side: "aka" | "ao", judgeIdx: number, val: string) {
    const cleaned = sanitizeScore(val);
    updateJudgeScore(side, judgeIdx, cleaned);
  }

  // Kiken Handlers (Chief Referee Override)
  function applyKiken(side: "aka" | "ao") {
    if (side === "aka") {
      akaJudgeScores = emptyJudges("0.0");
      aoJudgeScores = fitJudges(aoJudgeScores).map((s) => (s && s !== "0.0" ? s : "8.0"));
      manualFlags = null;
      matchWinner = "2";
    } else {
      aoJudgeScores = emptyJudges("0.0");
      akaJudgeScores = fitJudges(akaJudgeScores).map((s) => (s && s !== "0.0" ? s : "8.0"));
      manualFlags = null;
      matchWinner = "1";
    }
    showWinnerBanner = true;
    publishScoresToBoard();
  }

  // ==================== KUMITE SCORING ====================
  // Winner the rules decided (8-point lead, hansoku). Preselected on the crown only, no
  // banner: the operator confirms or overrides. Undoing the cause undoes the preselection.
  let autoWinner: "1" | "2" | null = null;
  const KUMITE_MAX_LEAD = 8;

  function ruleWinner(): "1" | "2" | null {
    // Hansoku: the other corner wins (both disqualified: nobody decided here)
    if (akaPenalties.h !== aoPenalties.h) return akaPenalties.h ? "2" : "1";
    if (akaScore - aoScore >= KUMITE_MAX_LEAD) return "1";
    if (aoScore - akaScore >= KUMITE_MAX_LEAD) return "2";
    return null;
  }

  function applyRuleWinner() {
    if (isKata) return;
    const w = ruleWinner();
    if (w) {
      if (!matchWinner || matchWinner === autoWinner) {
        if (w !== autoWinner) pauseTimer(); // bout is over: stop the clock
        matchWinner = w;
        autoWinner = w;
      }
    } else if (autoWinner && matchWinner === autoWinner) {
      matchWinner = "";
      showWinnerBanner = false;
      autoWinner = null;
    } else {
      autoWinner = null;
    }
  }

  function addKumiteScore(side: "aka" | "ao", pts: number) {
    if (side === "aka") {
      akaScore = Math.max(0, akaScore + pts);
    } else {
      aoScore = Math.max(0, aoScore + pts);
    }
    applyRuleWinner();

    if (pts === 1) {
      triggerScoreBanner(side, "1 - POINT", "YUKO", 1300);
    } else if (pts === 2) {
      triggerScoreBanner(side, "2 - POINT", "WAZA-ARI", 1300);
    } else if (pts === 3) {
      triggerScoreBanner(side, "3 - POINT", "IPPON", 1300);
    } else if (pts < 0) {
      dismissScoreBanner();
    } else {
      broadcastLiveState();
    }
  }

  function toggleSenshu(side: "aka" | "ao") {
    if (side === "aka") {
      akaSenshu = !akaSenshu;
      if (akaSenshu) {
        aoSenshu = false;
        triggerScoreBanner("aka", "KEUNGGULAN", "SENSHU", 1300);
      } else {
        dismissScoreBanner();
      }
    } else {
      aoSenshu = !aoSenshu;
      if (aoSenshu) {
        akaSenshu = false;
        triggerScoreBanner("ao", "KEUNGGULAN", "SENSHU", 1300);
      } else {
        dismissScoreBanner();
      }
    }
  }

  // Penalties are cumulative steps: C1 → C2 → C3 → HC → H.
  // Clicking a step sets the level to it (all lower steps on, higher off);
  // clicking the current top step takes it back one step.
  const PENALTY_STEPS = ["c1", "c2", "c3", "hc", "h"] as const;
  const PENALTY_LABEL: Record<(typeof PENALTY_STEPS)[number], string> = {
    c1: "CHUI 1",
    c2: "CHUI 2",
    c3: "CHUI 3",
    hc: "HANSOKU CHUI",
    h: "HANSOKU",
  };

  function togglePenalty(side: "aka" | "ao", code: "c1" | "c2" | "c3" | "hc" | "h") {
    const current = side === "aka" ? akaPenalties : aoPenalties;
    const currentLevel = PENALTY_STEPS.filter((p) => current[p]).length;
    const idx = PENALTY_STEPS.indexOf(code);
    const newLevel = currentLevel === idx + 1 ? idx : idx + 1;

    const next = { c1: false, c2: false, c3: false, hc: false, h: false };
    PENALTY_STEPS.forEach((p, i) => (next[p] = i < newLevel));
    if (side === "aka") akaPenalties = next;
    else aoPenalties = next;
    applyRuleWinner();

    if (newLevel > currentLevel) {
      triggerScoreBanner(side, "PELANGGARAN", PENALTY_LABEL[PENALTY_STEPS[newLevel - 1]], 1500);
    } else {
      dismissScoreBanner();
    }
  }

  // Non-toggling winner setter (used by mouse hantei): shows the winner banner once
  function setWinner(winner: "1" | "2") {
    if (matchWinner === winner && showWinnerBanner) return;
    matchWinner = winner;
    showWinnerBanner = true;
    broadcastLiveState();
  }

  // ==================== KUMITE: MODE MOUSE (STICK JURI) ====================
  // The 4 judges' mice type a–x (see judgeSticks.ts). Everything is computed in
  // this window; only the resulting banner / check screen goes to the board.
  function readMouseMode(): boolean {
    try {
      return localStorage.getItem("ambrilindo.mouseMode") === "1";
    } catch {
      return false;
    }
  }
  let mouseMode = $state<boolean>(readMouseMode());
  let stickState = $state<StickState | null>(null);
  let stickCheck = $state<boolean>(false);
  let stickTested = $state<string[]>([]);
  let stickLastKey = $state<string | null>(null);
  let stickHistory = $state<StickHistoryRow[]>([]);
  // Point banner already shown for the current exchange (so it only ever goes up)
  let stickBanner = { incidentId: -1, aka: { level: 0, judges: 0 }, ao: { level: 0, judges: 0 }, bannerId: -1 };
  const stickTracker = new StickTracker(handleStickChange);

  function setMouseMode(on: boolean) {
    mouseMode = on;
    try {
      localStorage.setItem("ambrilindo.mouseMode", on ? "1" : "0");
    } catch {}
    if (!on) {
      clearStickState();
      broadcastLiveState();
    }
  }

  /**
   * The judge stick monitor as this control panel computed it, for the Admin Control page
   * on other devices (same numbers: nothing is recomputed there).
   */
  function broadcastStickMonitor() {
    if (claimStatus !== "owned" || isKata) return;
    const msg = {
      type: "stick_monitor",
      tatami: activeTatami,
      payload: $state.snapshot({
        matchId: match.id,
        mouseMode,
        votes: stickState?.votes ?? [null, null, null, null],
        aka: stickState?.aka ?? null,
        ao: stickState?.ao ?? null,
        hantei: isHantei,
        hanteiTally: stickState?.hantei ?? { aka: 0, ao: 0 },
        timerRunning: isTimerRunning,
        checkMode: stickCheck,
        tested: stickTested,
        lastKey: stickLastKey,
        history: stickHistory,
        at: Date.now(),
      }),
    };
    // Socket only: other windows on this machine treat every BroadcastChannel message as
    // live state (scoreboard) or a refresh trigger (tatami manager). The host relays the
    // socket message to every device of this tatami, this machine included.
    tatamiSocket?.send({ ...msg, origin: isHostUi() ? "host" : undefined });
  }
  // Every change of the monitor goes out at once (one message per microtask)
  let stickMonitorQueued = false;
  $effect(() => {
    void [stickState, stickHistory, stickCheck, stickTested, stickLastKey, isHantei, isTimerRunning, mouseMode];
    if (stickMonitorQueued) return;
    stickMonitorQueued = true;
    queueMicrotask(() => {
      stickMonitorQueued = false;
      untrack(() => broadcastStickMonitor());
    });
  });

  function clearStickState() {
    clearHanteiDecision();
    hanteiWinner = null;
    stickTracker.reset();
    stickState = null;
    stickHistory = [];
    stickCheck = false;
    stickTested = [];
    stickLastKey = null;
    stickBanner = { incidentId: -1, aka: { level: 0, judges: 0 }, ao: { level: 0, judges: 0 }, bannerId: -1 };
  }

  function toggleStickCheck() {
    stickCheck = !stickCheck;
    stickTested = [];
    stickLastKey = null;
    stickTracker.reset();
    stickState = null;
    broadcastLiveState();
  }

  function resetStickCheck() {
    stickTested = [];
    stickLastKey = null;
    broadcastLiveState();
  }

  function logStickIncident(state: StickState, result: string, tone: StickHistoryRow["tone"]) {
    const row: StickHistoryRow = { incidentId: state.incidentId, time: timerFormatted.full, votes: state.votes, result, tone };
    if (stickHistory[0]?.incidentId === state.incidentId) {
      stickHistory = [row, ...stickHistory.slice(1)];
    } else {
      stickHistory = [row, ...stickHistory].slice(0, 8);
    }
  }

  function handleStickChange(state: StickState) {
    stickState = state;

    // Stick test: just record which buttons work
    if (stickCheck) {
      if (!state.isDecay && state.lastKey) {
        if (!stickTested.includes(state.lastKey)) stickTested = [...stickTested, state.lastKey];
        stickLastKey = state.lastKey;
        broadcastLiveState();
      }
      return;
    }
    // Expiring votes never change what is already decided
    if (state.isDecay) return;

    if (isHantei) {
      handleHanteiVotes(state);
      return;
    }

    const voted = state.votes.filter(Boolean).length;
    const akaLv = state.aka.level;
    const aoLv = state.ao.level;
    if (akaLv || aoLv) {
      const parts = [
        akaLv ? `AKA ${LEVEL_SHORT[akaLv]}` : "",
        aoLv ? `AO ${LEVEL_SHORT[aoLv]}` : "",
      ].filter(Boolean);
      logStickIncident(state, parts.join(" / "), akaLv && aoLv ? "seri" : akaLv ? "aka" : "ao");
    } else if (voted >= 2) {
      logStickIncident(state, "-", "none");
    }

    // Point banner on the board only while the match clock runs (no output on YAME)
    if (!isTimerRunning || (!akaLv && !aoLv)) return;

    // The board follows the judges' current votes on every press: a judge who
    // switches corner moves their flag (4 AKA, then 2 of them press AO → 2 AKA + 2 AO).
    // Votes that merely expire (isDecay, handled above) never change the board.
    const nextAka = { level: state.aka.level, judges: state.aka.level ? state.aka.judges : 0 };
    const nextAo = { level: state.ao.level, judges: state.ao.level ? state.ao.judges : 0 };
    // Re-send when the result changed, or when our banner is no longer on the
    // board (closed with Esc, or replaced by the operator's own score banner):
    // the same combination pressed again must show again.
    const ourBannerShowing = activeBanner !== null && activeBanner.id === stickBanner.bannerId;
    const changed =
      !ourBannerShowing ||
      stickBanner.incidentId !== state.incidentId ||
      nextAka.level !== stickBanner.aka.level || nextAka.judges !== stickBanner.aka.judges ||
      nextAo.level !== stickBanner.ao.level || nextAo.judges !== stickBanner.ao.judges;
    if (!changed) return;

    // AKA and AO in the same exchange are shown side by side, each with its judge flags
    const entries = (
      [
        ["ao", nextAo],
        ["aka", nextAka],
      ] as const
    )
      .filter(([, r]) => r.level > 0)
      .map(([side, r]) => {
        const [title, subtitle] = LEVEL_LABEL[r.level as 1 | 2 | 3];
        return { side, title, subtitle, judges: r.judges };
      });
    const top = [...entries].sort((x, y) => levelRank(y.subtitle) - levelRank(x.subtitle))[0];
    // Alert the operator (same sound as the 15 s warning) when a new score comes in,
    // not when a judge only moves a flag within the same exchange
    if (!ourBannerShowing || stickBanner.incidentId !== state.incidentId) playWarningChime();
    // Stays up until the operator adds the score (its own banner replaces it) or closes it
    triggerScoreBanner(top.side, top.title, top.subtitle, 0, { judges: top.judges, entries });
    stickBanner = { incidentId: state.incidentId, aka: nextAka, ao: nextAo, bannerId: activeBanner?.id ?? -1 };
  }

  function levelRank(subtitle: string): number {
    return subtitle === "IPPON" ? 3 : subtitle === "WAZA-ARI" ? 2 : 1;
  }

  // ==================== HANTEI (judges' decision, as in Ambrilindo Local) ====================
  // Hantei on: the board shows "HANTEI". Each judge presses any button of AKA or
  // AO on their stick (their last press counts, votes do not expire). Nothing is
  // shown until all 4 judges have voted; then the flags per corner go up, and the
  // winner follows a few seconds later. 2–2 stays on the board for the operator.
  const HANTEI_WINNER_DELAY_MS = 4000;
  let hanteiVotes = $derived((stickState?.hantei.aka ?? 0) + (stickState?.hantei.ao ?? 0));
  let hanteiLocked = false;
  let hanteiWinnerTimer: ReturnType<typeof setTimeout> | null = null;
  // Winner picked by the hantei vote (undone when Hantei is turned off to vote again)
  let hanteiWinner: "1" | "2" | null = null;

  function clearHanteiDecision() {
    hanteiLocked = false;
    if (hanteiWinnerTimer) clearTimeout(hanteiWinnerTimer);
    hanteiWinnerTimer = null;
  }

  // Turning Hantei off (e.g. after a wrong press) and on again starts a fresh vote:
  // votes, the result banner (Esc) and the winner the vote picked are all cleared.
  function setHantei(on: boolean) {
    isHantei = on;
    clearHanteiDecision();
    if (hanteiWinner && matchWinner === hanteiWinner) {
      matchWinner = "";
      showWinnerBanner = false;
    }
    hanteiWinner = null;
    stickTracker.setHold(on);
    stickTracker.reset();
    stickState = null;
    if (bannerClearTimeout) clearTimeout(bannerClearTimeout);
    bannerClearTimeout = null;
    activeBanner = null;
    broadcastLiveState();
  }

  // Votes stay put while Hantei is on (also when a saved hantei match is reopened)
  $effect(() => {
    stickTracker.setHold(isHantei && !isKata);
  });

  function handleHanteiVotes(state: StickState) {
    const { aka, ao } = state.hantei;
    const total = aka + ao;
    if (total === 0) return;
    const tone = aka > ao ? "aka" : ao > aka ? "ao" : "seri";
    logStickIncident(state, aka > ao ? `HANTEI AKA ${aka}–${ao}` : ao > aka ? `HANTEI AO ${ao}–${aka}` : `HANTEI SERI ${aka}–${ao}`, tone);
    // Wait for all 4 judges; once decided it stays (turn Hantei off/on to vote again)
    if (hanteiLocked || total < 4) return;
    hanteiLocked = true;
    playWarningChime();
    const entries: LiveBannerEntry[] = [
      { side: "ao", title: "HANTEI", subtitle: `${ao} JURI`, judges: ao },
      { side: "aka", title: "HANTEI", subtitle: `${aka} JURI`, judges: aka },
    ];
    const top = aka >= ao ? "aka" : "ao";
    const topVotes = top === "aka" ? aka : ao;
    triggerScoreBanner(top, "HANTEI", `${topVotes} JURI`, 0, { judges: topVotes, entries });
    if (aka !== ao) {
      const winner = aka > ao ? "1" : "2";
      hanteiWinnerTimer = setTimeout(() => {
        hanteiWinnerTimer = null;
        if (!isHantei) return;
        hanteiWinner = winner;
        setWinner(winner);
      }, HANTEI_WINNER_DELAY_MS);
    }
  }

  function selectWinner(winner: "1" | "2" | "3") {
    autoWinner = null;
    if (matchWinner === winner) {
      matchWinner = "";
    } else {
      matchWinner = winner;
    }
    // Memilih pemenang memunculkan banner pemenang di board; membatalkannya menutup banner
    showWinnerBanner = matchWinner !== "";
    broadcastLiveState();
  }

  // ==================== REALTIME LOCAL IPC BROADCAST ====================
  function broadcastLiveState() {
    const payload = {
      tatami: activeTatami,
      matchId: match.id,
      baganId: match.bagan_id || baganInfo?.id || null,
      eventName: eventName || "",
      category: baganName,
      round: match.round === 4 ? "PARTAI FINAL" : match.round === 3 ? "SEMIFINAL" : `BABAK ${match.round || 1}`,
      matchNumber: match.urutan || 1,
      timerMs,
      timerSeconds: Math.ceil(timerMs / 1000),
      isTimerRunning,
      isKata,
      // Kata: number of judges (scoreboard pop up columns, jury tablet seats)
      judgeCount: kataJudgeCount,
      aka: {
        name: match.atlet1_nama || "Sudut Merah",
        contingent: match.atlet1_kontingen || "-",
        kataName: kataAkaName,
        totalPoints: isKata ? boardAkaScore : akaScore,
        judgeScores: akaJudgeScores,
        score: akaScore,
        senshu: akaSenshu,
        penalties: akaPenalties,
        vr: hasVr1,
        logo: match.atlet1_logo ? (logoMap[match.atlet1_logo] || match.atlet1_logo) : null,
      },
      ao: {
        name: match.atlet2_nama || "Sudut Biru",
        contingent: match.atlet2_kontingen || "-",
        kataName: kataAoName,
        totalPoints: isKata ? boardAoScore : aoScore,
        judgeScores: aoJudgeScores,
        score: aoScore,
        senshu: aoSenshu,
        penalties: aoPenalties,
        vr: hasVr2,
        logo: match.atlet2_logo ? (logoMap[match.atlet2_logo] || match.atlet2_logo) : null,
      },
      winner: isKata ? (isScorePublished ? matchWinner : "") : matchWinner,
      // Video Review dipakai di kategori ini: board menampilkan logo VR untuk sudut
      // yang kartunya masih ada (aka.vr / ao.vr = true berarti kartu sudah hangus)
      vrEnabled: vrEnabled && !isKata,
      // Banner pemenang layar penuh di scoring board: "1" AKA, "2" AO, "3" seri, "" tutup
      winnerBanner: showWinnerBanner ? matchWinner : "",
      // Pop up timer persiapan kata (35 detik); board menghitung sendiri dari remainingMs
      // Layar uji stick juri di TV (mode mouse)
      stickCheck: !isKata && mouseMode && stickCheck ? { tested: [...stickTested], last: stickLastKey } : null,
      // "HANTEI" screen on the board while the judges vote (gone once a winner is set)
      hanteiMode: !isKata && isHantei && !matchWinner && !isMatchFinished,
      prepCountdown: prepCountdown
        ? { id: prepCountdown.id, side: prepCountdown.side, kind: prepCountdown.kind, durationMs: prepCountdown.durationMs, remainingMs: prepRemaining() }
        : null,
      isFinished: isMatchFinished,
      showKataScoreOverlay: showKataScoreOnBoard,
      showKataTotal: isKata && isScorePublished,
      isTeamMatch: !!(teamMatchInfo && teamMatchInfo.is_team_match),
      teamAkaName: teamMatchInfo?.team_aka_name || "",
      teamAoName: teamMatchInfo?.team_ao_name || "",
      teamAkaScore: teamMatchInfo?.team_aka_score || 0,
      teamAoScore: teamMatchInfo?.team_ao_score || 0,
      teamDrawCount: teamMatchInfo?.team_draw_count || 0,
      teamAkaPoints: teamMatchInfo?.team_aka_points || 0,
      teamAoPoints: teamMatchInfo?.team_ao_points || 0,
      activeBoutRound: teamMatchInfo?.active_bout_round || match.urutan || 1,
      totalBouts: teamMatchInfo?.bouts.length || 0,
      activeBanner: activeBanner
        ? {
            id: activeBanner.id,
            side: activeBanner.side,
            title: activeBanner.title,
            subtitle: activeBanner.subtitle,
            durationMs: activeBanner.durationMs,
            judges: activeBanner.judges,
            entries: activeBanner.entries,
          }
        : null,
      // VR requests per corner (both may be open); vrChallenge kept for older screens
      vrChallenges: { aka: vrChallenges.aka, ao: vrChallenges.ao },
      vrChallenge: activeVrChallenge,
      alarmSettings: {
        alarm30s: alarm30sEnabled,
        alarm15s: alarm15sEnabled,
        alarm0s: alarm0sEnabled,
      },
    };

    // The preview in this panel draws exactly what the TV gets, without the network
    previewState = $state.snapshot(payload);

    // A window that does not hold the tatami must not overwrite the owner's scoreboard.
    if (claimStatus !== "owned") return;

    // Universal broadcast to local IPC, local BroadcastChannel, and LAN WebSocket relay
    broadcastLiveMatch($state.snapshot(payload));
    if (tatamiSocket) {
      try {
        tatamiSocket.send({
          type: "tatami_live_state",
          origin: isHostUi() ? "host" : undefined,
          payload: $state.snapshot(payload),
        });
      } catch (_) {}
    }
  }

  // ==================== SAVING & EXIT ====================
  async function handleSaveAndExit() {
    if (isSaving) return;
    if (claimStatus !== "owned") {
      alert(`Hasil tidak disimpan: Tatami ${activeTatami} sedang dikendalikan oleh ${claimHolderLabel || "operator lain"}.`);
      return;
    }
    isSaving = true;

    try {
      const finalScore1 = isKata ? String(effectiveAkaPoints) : String(akaScore);
      const finalScore2 = isKata ? String(effectiveAoPoints) : String(aoScore);

      // Auto-detect winner if operator did not click winner crown explicitly
      let effectiveWinner = matchWinner || "";
      if (!effectiveWinner) {
        if (isKata) {
          if (effectiveAkaPoints > effectiveAoPoints) effectiveWinner = "1";
          else if (effectiveAoPoints > effectiveAkaPoints) effectiveWinner = "2";
        } else {
          const decided = ruleWinner();
          if (decided) effectiveWinner = decided;
          else if (akaScore > aoScore) effectiveWinner = "1";
          else if (aoScore > akaScore) effectiveWinner = "2";
          else if (akaSenshu) effectiveWinner = "1";
          else if (aoSenshu) effectiveWinner = "2";
        }
      }

      // No winner (tie without senshu / hantei): the bout is saved as NOT finished and the
      // bracket does not advance. Say so instead of closing silently.
      if (!effectiveWinner && !isMatchFinished) {
        const ok = confirm(
          "Belum ada pemenang (skor seri). Partai akan disimpan sebagai BELUM SELESAI dan bagan tidak maju.\n\n" +
            "Pilih pemenang atau Hantei dulu untuk menyelesaikan partai. Tetap simpan sebagai belum selesai?"
        );
        if (!ok) return;
      }

      const input: SaveMatchInput = {
        match_id: match.id,
        bagan_id: match.bagan_id || 0,
        score1: finalScore1,
        score2: finalScore2,
        scorekecil1: null,
        scorekecil2: null,
        pemenang: effectiveWinner || null,
        kata1: kataAkaName || null,
        kata2: kataAoName || null,
        // Stored as "card still held", as in Ambrilindo Local and the bracket
        vr1: vrEnabled && !hasVr1,
        vr2: vrEnabled && !hasVr2,
        hantei: isHantei,
        selesai: isMatchFinished || !!effectiveWinner,
      };

      // Kata: simpan juga skor 5 juri per sudut (hanya bila ada yang diisi)
      const hasJudgeScores = [...akaJudgeScores, ...aoJudgeScores].some((s) => s.trim() !== "");
      if (isKata && hasJudgeScores) {
        input.juri_scores1 = JSON.stringify(akaJudgeScores.map((s) => s.trim()));
        input.juri_scores2 = JSON.stringify(aoJudgeScores.map((s) => s.trim()));
      }

      await onSave(input);

      // Notify ChannelSidebar, TatamiManager, and other LAN laptops that match is finished
      broadcastLiveMatch({
        tatami: activeTatami,
        matchId: match.id,
        baganId: match.bagan_id || baganInfo?.id || null,
        isFinished: true,
        isClosed: true,
        isTimerRunning: false,
      });
    } catch (err: any) {
      console.error("Gagal menyimpan hasil partai:", err);
    } finally {
      isSaving = false;
    }
  }

  // Input from venue devices (jury tablets, LO Kata, VR table, LAN scoreboards).
  function handleRemoteInput(data: any, targetTatami?: number) {
    if (!data || claimStatus !== "owned") return;
    if (targetTatami && Number(targetTatami) !== activeTatami) return;

    if (data.type === "request_tatami_state") {
      broadcastLiveState();
      broadcastStickMonitor();
      return;
    }

    if (data.type === "judge_score") {
      const juriIdx = (Number(data.juri) || 1) - 1;
      if (juriIdx >= 0 && juriIdx < kataJudgeCount) {
        if (data.score_aka !== undefined && data.score_aka !== null) {
          akaJudgeScores[juriIdx] = String(data.score_aka);
        }
        if (data.score_ao !== undefined && data.score_ao !== null) {
          aoJudgeScores[juriIdx] = String(data.score_ao);
        }
        // Scores from a jury tablet count exactly like typed ones
        manualFlags = null;
        broadcastLiveState();
      }
    } else if (data.type === "lo_kata_broadcast") {
      applyLoKata(data);
    } else if (data.type === "vr_challenge_started") {
      applyRemoteVrStarted(data);
    } else if (data.type === "vr_challenge_resolved") {
      applyRemoteVrResolved(data);
    }
  }

  /** Kata picked on the LO Kata tablet (host IPC, socket or, on this machine, BroadcastChannel) */
  function applyLoKata(data: any) {
    // Only kata for the match open here (an LO tablet may be on another match)
    if (data.match_id && Number(data.match_id) !== match.id) return;
    let changed = false;
    if (data.kata_aka !== undefined && data.kata_aka !== null && String(data.kata_aka) !== kataAkaName) {
      kataAkaName = String(data.kata_aka);
      kataFromLo = { ...kataFromLo, aka: !!kataAkaName };
      kataLoFlash = { ...kataLoFlash, aka: kataLoFlash.aka + 1 };
      changed = true;
    }
    if (data.kata_ao !== undefined && data.kata_ao !== null && String(data.kata_ao) !== kataAoName) {
      kataAoName = String(data.kata_ao);
      kataFromLo = { ...kataFromLo, ao: !!kataAoName };
      kataLoFlash = { ...kataLoFlash, ao: kataLoFlash.ao + 1 };
      changed = true;
    }
    // The same message arrives on several paths: only the first one changes anything
    // (no sound: the LO badge and highlight are enough)
    if (!changed) return;
    broadcastLiveState();
  }

  // Messages from the host over this tatami's socket. The host's own windows already
  // receive device input over Tauri IPC, so they ignore the socket copy.
  function handleSocketMessage(msg: any) {
    if (!msg || isHostUi()) return;
    handleRemoteInput(msg, msg.tatami);
  }

  // Initialize values from match prop
  onMount(() => {
    unlockAudio();
    initLiveSync(hostUrl);
    matchWinner = match.pemenang || "";
    isMatchFinished = match.selesai || false;
    hasVr1 = vrUsedFromRecord(match.vr1);
    hasVr2 = vrUsedFromRecord(match.vr2);
    isHantei = match.hantei || false;

    akaScore = parseInt(match.score1 || "0", 10) || 0;
    aoScore = parseInt(match.score2 || "0", 10) || 0;

    kataAkaName = match.kata1 || "";
    kataAoName = match.kata2 || "";

    // If match was already completed or has saved scores, restore published state
    if (match.score1 || match.score2 || match.pemenang || match.selesai) {
      boardAkaScore = parseInt(match.score1 || "0", 10) || 0;
      boardAoScore = parseInt(match.score2 || "0", 10) || 0;
      isScorePublished = true;
      manualFlags = isKata ? { aka: boardAkaScore, ao: boardAoScore } : null;
    } else {
      boardAkaScore = 0;
      boardAoScore = 0;
      isScorePublished = false;
      manualFlags = null;
    }

    // Broadcast initial state immediately
    setTimeout(() => {
      broadcastLiveState();
    }, 100);

    if (typeof BroadcastChannel !== "undefined") {
      liveBroadcastChannel = new BroadcastChannel("ambrilindo_live_sync");
      liveBroadcastChannel.onmessage = (event) => {
        const data = event.data;
        if (!data) return;
        if (data.tatami && data.tatami != activeTatami) return;
        if (data.type === "vr_challenge_started") {
          applyRemoteVrStarted(data);
        } else if (data.type === "vr_challenge_resolved") {
          applyRemoteVrResolved(data);
        } else if (data.type === "lo_kata_broadcast") {
          applyLoKata(data);
        }
      };
    }

    // Listen for requests from newly opened scoreboard windows
    let unlistenRequest: (() => void) | undefined;
    listen("request_tatami_state", (event: any) => {
      if (!event.payload?.tatami || event.payload?.tatami === activeTatami) {
        broadcastLiveState();
      }
    }).then((unsub) => {
      unlistenRequest = unsub;
    }).catch(() => {});

    // Listen for remote input from mobile venue devices (Jury Panel & LO Kata).
    // On the host these arrive over Tauri IPC; elsewhere over the tatami socket.
    let unlistenRemote: (() => void) | undefined;
    listen("tatami_remote_input", (event: any) => {
      const arenaMsg = event.payload;
      if (!arenaMsg) return;
      const data = arenaMsg.payload || arenaMsg;
      handleRemoteInput(data, arenaMsg.tatami_pk || data.tatami);
    }).then((unsub) => {
      unlistenRemote = unsub;
    }).catch(() => {});

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore OS key-repeat to prevent rapid bouncing
      if (e.repeat) return;
      // waiting for the chief referee: no timer / scoring hotkeys
      if (approvalLocked) return;

      // Escape closes activeJudgePicker or active scoreboard banner
      if (e.key === "Escape") {
        if (activeJudgePicker !== null) {
          e.preventDefault();
          activeJudgePicker = null;
          return;
        }
        if (activeBanner !== null) {
          e.preventDefault();
          dismissScoreBanner();
          return;
        }
      }
      // Spacebar for Timer Start/Stop
      if (e.code === "Space" && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) {
        e.preventDefault();
        // Blur active element so button native activation doesn't trigger duplicate click
        if (typeof document !== "undefined" && document.activeElement instanceof HTMLElement) {
          document.activeElement.blur();
        }
        toggleTimer();
        return;
      }
      // Mode Mouse: judge sticks type a–x; scored here on the laptop
      if (
        mouseMode &&
        !isKata &&
        !e.ctrlKey && !e.altKey && !e.metaKey &&
        /^[a-x]$/i.test(e.key) &&
        !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement)
      ) {
        if (stickTracker.press(e.key)) {
          e.preventDefault();
          return;
        }
      }
      // Kumite Hotkeys (1/2/3 for AO, 7/8/9 for AKA)
      if (!isKata && !(e.target instanceof HTMLInputElement)) {
        if (e.key === "1") addKumiteScore("ao", 1);
        if (e.key === "2") addKumiteScore("ao", 2);
        if (e.key === "3") addKumiteScore("ao", 3);
        if (e.key === "7") addKumiteScore("aka", 1);
        if (e.key === "8") addKumiteScore("aka", 2);
        if (e.key === "9") addKumiteScore("aka", 3);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      stopTimerTicker();
      if (unlistenRequest) unlistenRequest();
      if (unlistenRemote) unlistenRemote();
      if (claimStatus === "owned") broadcastLiveMatch({
        tatami: activeTatami,
        matchId: match.id,
        baganId: match.bagan_id || baganInfo?.id || null,
        isFinished: true,
        isClosed: true,
        isTimerRunning: false,
      });
      if (liveBroadcastChannel) liveBroadcastChannel.close();
      if (prepTicker) clearInterval(prepTicker);
      stickTracker.dispose();
    };
  });
</script>

<svelte:window onkeydown={handleKataModalKeydown} />

{#if approvalLocked && approval && claimStatus !== "blocked"}
  {@const rejected = approval.status === "draft" && !!approval.reject_note}
  <!-- Waiting for the chief referee: the panel stays empty, the scoreboard shows the bout -->
  <div class="fixed inset-0 z-[150] bg-[#0d0f12] flex flex-col" role="alertdialog" aria-modal="true" aria-labelledby="approval-title">
    <div class="flex items-center justify-between gap-3 p-3 m-3 mb-0 bg-[#181a20] border border-[#2b2d31] rounded-xl">
      <div class="flex items-center gap-2 min-w-0">
        <PortalExitButton title="Keluar dari Control Panel?" />
        <span class="px-2.5 py-1 rounded-lg bg-white/[0.06] text-zinc-200 font-mono font-black text-xs border border-white/10 shrink-0">TATAMI {activeTatami}</span>
        <span class="text-sm font-black text-white uppercase truncate">{baganName}</span>
      </div>
      <span class="text-xs font-mono text-zinc-400 shrink-0">Partai #{match.urutan || 1}</span>
    </div>
    <div class="flex-1 flex items-center justify-center p-6">
      <div class="w-full max-w-2xl text-center">
        <div class="flex items-center justify-center gap-3 text-base font-bold text-white mb-8 flex-wrap">
          <span class="px-2 py-0.5 rounded bg-red-600 text-xs font-mono font-black">AKA</span>
          <span>{teamMatchInfo?.is_team_match ? teamMatchInfo.team_aka_name : match.atlet1_nama || "-"}</span>
          <span class="text-zinc-500 text-xs font-mono">VS</span>
          <span class="px-2 py-0.5 rounded bg-sky-600 text-xs font-mono font-black">AO</span>
          <span>{teamMatchInfo?.is_team_match ? teamMatchInfo.team_ao_name : match.atlet2_nama || "-"}</span>
        </div>
        {#if rejected}
          <div class="text-[11px] font-mono font-bold uppercase tracking-widest text-red-400">Ditolak</div>
          <h2 id="approval-title" class="mt-2 text-3xl font-black uppercase text-red-300">Match ditolak Chief Referee</h2>
          <p class="mt-3 text-lg font-semibold text-red-100">{approval.reject_note}</p>
          <p class="mt-4 text-sm text-zinc-400">Menunggu Tatami Manager menetapkan ulang wasit &amp; juri, lalu permintaan dikirim lagi otomatis.</p>
        {:else if approval.status === "draft"}
          <div class="text-[11px] font-mono font-bold uppercase tracking-widest text-amber-400">Belum bisa dimulai</div>
          <h2 id="approval-title" class="mt-2 text-3xl font-black uppercase text-amber-200">Wasit belum ditetapkan</h2>
          <p class="mt-4 text-sm text-zinc-400">Tatami Manager perlu Submit wasit &amp; juri partai ini. Setelah itu permintaan ke Chief Referee dikirim otomatis.</p>
        {:else}
          <div class="mx-auto mb-5 w-12 h-12 rounded-full border-4 border-violet-400/30 border-t-violet-300 animate-spin"></div>
          <h2 id="approval-title" class="text-3xl font-black uppercase text-violet-100">Menunggu persetujuan Chief Referee</h2>
          <p class="mt-4 text-sm text-zinc-400">Wasit &amp; juri partai ini sedang diperiksa. Panel terbuka otomatis setelah disetujui.</p>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if licenseBlocked}
  <div
    class="fixed inset-0 z-[210] bg-black/85 backdrop-blur-xs flex items-center justify-center p-6"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="license-lock-title"
  >
    <div class="w-full max-w-md bg-[#161820] border border-amber-500/25 rounded-xl p-6 shadow-2xl">
      <div class="text-[10px] font-mono font-bold uppercase tracking-wider text-amber-400">Lisensi</div>
      <h2 id="license-lock-title" class="mt-2 text-lg font-black tracking-tight text-white uppercase">
        {lockedMessage(requiredModule).split(". ")[0]}
      </h2>
      {#if license.status?.message}
        <p class="mt-2 text-xs font-medium text-zinc-300">{license.status.message}</p>
      {/if}
      <p class="mt-2 text-[11px] text-zinc-500">Aktivasi lisensi di menu Lisensi pada laptop host / solo, lalu buka ulang panel ini.</p>
      {#if isFestival && manualDiscipline}
        <button
          type="button"
          onclick={() => (manualDiscipline = manualDiscipline === "kata" ? "kumite" : "kata")}
          class="mt-4 h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl"
        >
          Ganti ke {manualDiscipline === "kata" ? "Kumite" : "Kata"}
        </button>
      {/if}
    </div>
  </div>
{/if}

{#if claimStatus === "blocked"}
  <div
    class="fixed inset-0 z-[200] bg-black/80 backdrop-blur-xs flex items-center justify-center p-6"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="claim-title"
    aria-describedby="claim-desc"
  >
    <div class="w-full max-w-md bg-[#161820] border border-rose-500/20 rounded-xl p-6 shadow-2xl">
      <div class="text-[10px] font-mono font-bold uppercase tracking-wider text-rose-400">Tatami diambil alih</div>
      <h2 id="claim-title" class="mt-2 text-lg font-black tracking-tight text-white uppercase">
        Tatami <span class="font-mono tabular-nums">{activeTatami}</span> kini dikendalikan {claimHolderLabel}
      </h2>
      <p id="claim-desc" class="mt-2 text-xs font-medium text-zinc-300">
        Match dibuka di panel lain{#if claimHolderSince}{" "}pukul <span class="font-mono tabular-nums">{formatClaimTime(claimHolderSince)}</span>{/if}.
        Jendela ini berhenti mengirim skor dan timer, dan tidak bisa menyimpan hasil.
      </p>
      <p class="mt-1.5 text-[11px] text-zinc-500">Tutup jendela ini, atau ambil alih kembali.</p>
      {#if isFestival}
        <label class="mt-4 flex items-center gap-2 text-[11px] text-zinc-400">
          Pindah ke
          <select
            bind:value={activeTatami}
            class="bg-[#111216] border border-white/[0.08] text-white font-mono font-bold text-xs rounded px-1.5 py-1 focus:outline-none focus:border-blue-500/40"
          >
            {#each [1, 2, 3, 4, 5, 6, 7, 8] as t}
              <option value={t}>Tatami {t}</option>
            {/each}
          </select>
        </label>
      {/if}
      <div class="mt-5 flex justify-end">
        <button
          type="button"
          onclick={takeOverTatami}
          disabled={isTakingOver}
          class="px-3.5 py-2 rounded-lg bg-rose-500/10 text-rose-400 border border-rose-500/20 hover:border-rose-500/40 hover:text-rose-300 text-xs font-bold transition disabled:opacity-50"
        >
          {isTakingOver ? "Mengambil alih…" : `Ambil alih kembali Tatami ${activeTatami}`}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Video Review decisions for a corner's open request (as in Ambrilindo Local).
     The VR referee normally decides on the tablet; these are the operator's override. -->
{#snippet vrDecisionButtons(side: "aka" | "ao")}
  {@const asked = vrChallenges[side]?.points}
  <div class="flex items-center gap-1.5 shrink-0 flex-wrap font-mono">
    {#each [1, 2, 3] as pts}
      <button
        type="button"
        onclick={() => resolveVr(side, "accept", pts)}
        class="h-8 px-2.5 rounded-lg text-white text-xs font-black transition active:scale-95 shadow-sm {asked === pts ? 'bg-emerald-500 ring-2 ring-emerald-200' : 'bg-emerald-700/70 hover:bg-emerald-600'}"
        title="Terima {pts} poin (kartu tetap ada). Skor tetap ditambah operator."
      >
        ✓ {pts}{asked === pts ? " (diminta)" : ""}
      </button>
    {/each}
    <button
      type="button"
      onclick={() => resolveVr(side, "decline")}
      class="h-8 px-2.5 rounded-lg bg-rose-600 hover:bg-rose-500 text-white text-xs font-bold transition active:scale-95 shadow-sm"
      title="Tolak, kartu VR hangus"
    >
      ✕ Tolak
    </button>
    <button
      type="button"
      onclick={() => resolveVr(side, "minai")}
      class="h-8 px-2.5 rounded-lg bg-amber-500 hover:bg-amber-400 text-black text-xs font-bold transition active:scale-95 shadow-sm"
      title="Minai: tidak dapat disimpulkan, kartu tetap ada"
    >
      Minai
    </button>
    <button
      type="button"
      onclick={() => resolveVr(side, "cancel")}
      class="h-8 px-2 rounded-lg bg-white/10 hover:bg-white/20 text-zinc-300 text-xs font-semibold transition active:scale-95"
      title="Batalkan permohonan"
    >
      Batal
    </button>
  </div>
{/snippet}

<!-- Shared by the kata and kumite timer rows -->
{#snippet timerPresetSelect()}
  <!-- Time preset; remembered per discipline for the next match -->
  <select
    value={String(selectedPresetSeconds)}
    onchange={(e) => selectTimerPreset(Number(e.currentTarget.value))}
    class="h-9 px-2 bg-[#111216] border border-white/10 hover:border-white/20 text-zinc-200 text-xs font-mono font-bold rounded-lg outline-none cursor-pointer focus:border-[#5865f2]"
    title="Preset waktu (diingat untuk partai berikutnya)"
    aria-label="Preset waktu"
  >
    {#each TIMER_PRESETS.includes(selectedPresetSeconds) ? TIMER_PRESETS : [...TIMER_PRESETS, selectedPresetSeconds].sort((a, b) => a - b) as preset}
      <option value={String(preset)} class="bg-[#181a20]">{formatPresetLabel(preset)}</option>
    {/each}
  </select>
{/snippet}

{#snippet timerSettingsButton()}
  <!-- ⚙: alarms and arena bell. Its parent must be `relative`. -->
  <button
    type="button"
    onclick={() => (showTimerSettings = !showTimerSettings)}
    class="h-9 w-9 ml-auto shrink-0 flex items-center justify-center rounded-lg border transition {showTimerSettings ? 'bg-white/[0.08] border-white/20 text-white' : 'bg-[#1e2029] border-white/10 text-zinc-400 hover:text-white'}"
    title="Alarm & bel arena"
    aria-label="Pengaturan timer"
    aria-expanded={showTimerSettings}
  >
    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/></svg>
  </button>

  {#if showTimerSettings}
    <!-- click outside closes -->
    <button type="button" class="fixed inset-0 z-30 cursor-default" aria-label="Tutup pengaturan timer" onclick={() => (showTimerSettings = false)}></button>
    <div class="absolute right-0 top-full mt-2 z-40 w-[260px] bg-[#181a20] border border-white/10 rounded-xl p-3 shadow-2xl flex flex-col gap-3 text-xs font-mono">
      <div>
        <div class="text-[10px] text-zinc-500 uppercase font-bold mb-1.5">Alarm</div>
        <div class="flex flex-wrap gap-1.5">
          <button
            type="button"
            onclick={() => { alarm30sEnabled = !alarm30sEnabled; broadcastLiveState(); }}
            class="px-2 py-1 rounded border text-[10.5px] font-bold transition {alarm30sEnabled ? 'bg-amber-500/25 border-amber-500 text-amber-300' : 'bg-transparent border-white/10 text-zinc-500 hover:text-zinc-300'}"
            title="Alarm peringatan saat sisa 30 detik"
          >
            🔔 30s
          </button>
          <button
            type="button"
            onclick={() => { alarm15sEnabled = !alarm15sEnabled; broadcastLiveState(); }}
            class="px-2 py-1 rounded border text-[10.5px] font-bold transition {alarm15sEnabled ? 'bg-amber-500/25 border-amber-500 text-amber-300' : 'bg-transparent border-white/10 text-zinc-500 hover:text-zinc-300'}"
            title="Alarm peringatan saat sisa 15 detik (Atoshibaraku)"
          >
            🔔 15s
          </button>
          <button
            type="button"
            onclick={() => { alarm0sEnabled = !alarm0sEnabled; broadcastLiveState(); }}
            class="px-2 py-1 rounded border text-[10.5px] font-bold transition {alarm0sEnabled ? 'bg-red-500/25 border-red-500 text-red-300' : 'bg-transparent border-white/10 text-zinc-500 hover:text-zinc-300'}"
            title="Buzzer akhir pertandingan saat 0 detik"
          >
            🚨 0s
          </button>
        </div>
      </div>
      {#if isKata}
        <div>
          <div class="text-[10px] text-zinc-500 uppercase font-bold mb-1.5">Jumlah Juri Kata</div>
          <div class="flex gap-1.5" role="radiogroup" aria-label="Jumlah juri kata">
            {#each KATA_JUDGE_OPTIONS as n (n)}
              <button
                type="button"
                role="radio"
                aria-checked={kataJudgeCount === n}
                onclick={() => setKataJudgeCount(n)}
                class="flex-1 px-2 py-1 rounded border text-[10.5px] font-bold transition {kataJudgeCount === n ? 'bg-white text-black border-white' : 'bg-transparent border-white/10 text-zinc-400 hover:text-zinc-200'}"
              >
                {n} Juri
              </button>
            {/each}
          </div>
        </div>
      {/if}
      <button
        type="button"
        onclick={triggerManualBuzzer}
        class="self-start px-2.5 py-1 bg-amber-500/20 hover:bg-amber-500/30 border border-amber-500/40 text-amber-300 rounded font-bold transition active:scale-95"
        title="Bunyikan klakson arena secara manual"
      >
        🔊 Bel Arena
      </button>
      {#if !isKata}
        <!-- Judge sticks (mouse → keys a–x), scored on this laptop -->
        <div class="pt-3 border-t border-white/10">
          <button
            type="button"
            role="switch"
            aria-checked={mouseMode}
            onclick={() => setMouseMode(!mouseMode)}
            class="w-full flex items-center justify-between gap-2 text-left"
            title="Skor dari stick/mouse 4 juri (tombol a–x)"
          >
            <span class="text-[11px] font-bold text-zinc-200">Mode Mouse Juri</span>
            <span class="relative w-9 h-5 rounded-full transition {mouseMode ? 'bg-emerald-500' : 'bg-white/15'}">
              <span class="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform {mouseMode ? 'translate-x-4' : ''}"></span>
            </span>
          </button>
        </div>
      {/if}
    </div>
  {/if}
{/snippet}

<!-- Corner card extras: team score (kumite beregu) and medical time (3 min) -->
{#snippet cornerExtras(side: "aka" | "ao")}
  {@const team = teamMatchInfo?.is_team_match ? teamMatchInfo : null}
  {@const medisActive = prepCountdown?.kind === "medis" && prepCountdown.side === side}
  {@const medisSec = Math.ceil(prepRemainingMs / 1000)}
  <div class="flex items-stretch gap-2 {side === 'aka' ? 'flex-row-reverse' : ''}">
    {#if team}
      {@const logo = side === "aka" ? team.team_aka_logo : team.team_ao_logo}
      <div
        class="flex-1 min-w-0 flex items-center gap-2 rounded-lg border px-2.5 py-1 {side === 'aka' ? 'flex-row-reverse bg-red-950/40 border-red-500/40' : 'bg-blue-950/40 border-blue-500/40'}"
        title="Skor tim: partai dimenangkan · poin"
      >
        {#if logo && logoMap[logo]}
          <img src={logoMap[logo]} alt="Logo tim" class="w-6 h-6 object-contain rounded shrink-0 bg-black/40 p-0.5" />
        {/if}
        <div class="flex-1 min-w-0 {side === 'aka' ? 'text-right' : ''}">
          <div class="text-[9px] font-mono font-bold uppercase tracking-wider {side === 'aka' ? 'text-red-400' : 'text-blue-400'}">Tim {side === "aka" ? "AKA" : "AO"}</div>
          <div class="text-[11px] font-black text-white uppercase truncate">{side === "aka" ? team.team_aka_name : team.team_ao_name}</div>
        </div>
        <div class="font-mono leading-none {side === 'aka' ? 'text-left' : 'text-right'}">
          <div class="text-xl font-black {side === 'aka' ? 'text-red-400' : 'text-blue-400'}">{side === "aka" ? team.team_aka_score : team.team_ao_score}</div>
          <div class="text-[9px] font-bold text-zinc-400">{side === "aka" ? team.team_aka_points : team.team_ao_points} Pts</div>
        </div>
      </div>
    {/if}
    <button
      type="button"
      onclick={() => (medisActive ? stopPrepCountdown() : startPrepCountdown(side, "medis"))}
      class="min-h-9 px-2.5 rounded-lg font-mono text-xs font-bold transition active:scale-95 flex items-center justify-center gap-1.5 border shrink-0 {team ? '' : 'flex-1'} {medisActive
        ? (side === 'aka' ? 'bg-red-600 border-red-400 text-white shadow-lg shadow-red-600/40' : 'bg-blue-600 border-blue-400 text-white shadow-lg shadow-blue-600/40')
        : 'bg-[#1e2029] border-white/10 text-zinc-300 hover:text-white hover:bg-[#2b2d31]'}"
      title={medisActive ? "Hentikan dan tutup waktu medis" : `Mulai waktu medis 3 menit untuk ${side === "aka" ? "AKA" : "AO"}`}
    >
      <svg class="w-3.5 h-3.5 {medisActive ? '' : 'text-emerald-400'}" fill="currentColor" viewBox="0 0 24 24"><path d="M9 3h6v6h6v6h-6v6H9v-6H3V9h6z"/></svg>
      {#if medisActive}
        <span>{prepRemainingMs > 0 ? `${Math.floor(medisSec / 60)}:${String(medisSec % 60).padStart(2, "0")}` : "HABIS"}</span>
        <span class="opacity-80">✕</span>
      {:else}
        <span>Medis 3:00</span>
      {/if}
    </button>
  </div>
{/snippet}

<!-- Main Cockpit Container (Discord Palette: #0d0f12 deep background) -->
<div class="h-full flex flex-col bg-[#0d0f12] text-zinc-100 overflow-hidden font-sans select-none p-3">
  <!-- Top Cockpit Header Bar (Discord Card #181a20, Border #2b2d31) -->
  <div class="cockpit-page-header flex items-center justify-between flex-wrap gap-3 p-3 bg-[#181a20] border border-[#2b2d31] rounded-xl shadow-lg mb-3 shrink-0">
    <div class="flex items-center space-x-2">
      <PortalExitButton title="Keluar dari Control Panel?" warning="Skor dan hasil yang belum disimpan akan hilang." />
      <!-- Save & Exit Button -->
      <button
        type="button"
        onclick={handleSaveAndExit}
        disabled={isSaving}
        class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-bold rounded-lg transition flex items-center space-x-1.5 shadow-sm active:scale-95 disabled:opacity-50"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
        </svg>
        <span>{isSaving ? "Menyimpan..." : teamMatchInfo?.is_team_match ? `Simpan Partai #${teamMatchInfo.bouts.find((b) => b.bout_id === match.id)?.round ?? match.urutan ?? 1}` : "Simpan & Keluar"}</span>
      </button>

      <!-- Reset Option Button -->
      <button
        type="button"
        onclick={confirmResetAllMatchData}
        class="px-3.5 py-1.5 bg-[#2b2d31] hover:bg-rose-600/80 border border-white/10 hover:border-rose-500/50 text-zinc-300 hover:text-white text-xs font-bold rounded-lg transition flex items-center space-x-1.5 shadow-sm active:scale-95"
        title="Reset semua skor, bendera, timer, dan status kembali ke 0"
      >
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <span>Reset</span>
      </button>

      {#if teamMatchInfo?.is_team_match}
        <button
          type="button"
          onclick={openFinishTeamModal}
          class="px-3.5 py-1.5 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-400 hover:to-amber-500 text-slate-950 text-xs font-black rounded-lg transition shadow-md flex items-center gap-1.5 active:scale-95"
        >
          <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 20 20"><path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" /></svg>
          <span>Selesaikan Beregu</span>
        </button>
      {/if}
    </div>

    <!-- Match Identity Badge (Discipline locked automatically, toggleable for festival) -->
    <div class="flex items-center space-x-2 text-xs font-mono">
      {#if isFestival}
        <div class="flex items-center bg-[#111216] border border-white/10 rounded-lg p-0.5">
          <button
            type="button"
            onclick={() => { manualDiscipline = "kumite"; broadcastLiveState(); }}
            class="px-2 py-0.5 rounded text-[10px] font-bold transition {!isKata ? 'bg-blue-600 text-white shadow-xs' : 'text-zinc-400 hover:text-white'}"
          >
            KUMITE
          </button>
          <button
            type="button"
            onclick={() => { manualDiscipline = "kata"; broadcastLiveState(); }}
            class="px-2 py-0.5 rounded text-[10px] font-bold transition {isKata ? 'bg-amber-500 text-slate-950 shadow-xs' : 'text-zinc-400 hover:text-white'}"
          >
            KATA
          </button>
        </div>
      {:else}
        <span class="px-2.5 py-0.5 rounded font-black uppercase {isKata ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30' : 'bg-blue-500/20 text-blue-400 border border-blue-500/30'}">
          {isKata ? "KATA" : "KUMITE"}
        </span>
      {/if}
      <span class="text-zinc-200 font-bold uppercase truncate max-w-[320px]">
        {baganName || "Kategori Turnamen"}
      </span>
      <span class="text-zinc-500">•</span>
      <span class="text-zinc-300 font-semibold">MATCH {match.urutan || 1}</span>
      <span class="text-zinc-500">•</span>
      <span class="text-zinc-400">
        {isFestival ? (match.round === 2 ? "PARTAI FINAL" : "SEMIFINAL") : (match.round === 4 ? "PARTAI FINAL" : match.round === 3 ? "SEMIFINAL" : `BABAK ${match.round || 1}`)}
      </span>
      <span class="text-zinc-500">•</span>
      {#if isFestival}
        <div class="flex items-center space-x-1">
          <span class="text-zinc-400 text-[11px]">Tatami:</span>
          <select
            bind:value={activeTatami}
            onchange={() => broadcastLiveState()}
            class="bg-[#111216] border border-amber-500/40 text-amber-300 font-bold text-xs rounded px-1.5 py-0.5 focus:outline-none"
          >
            {#each [1, 2, 3, 4, 5, 6, 7, 8] as t}
              <option value={t}>Tatami {t}</option>
            {/each}
          </select>
        </div>
      {:else}
        <span class="text-amber-400 font-bold">Tatami {activeTatami}</span>
      {/if}
    </div>

    <!-- Active & Server Status Indicator (Clickable Server Control Modal) -->
    <div class="flex items-center space-x-2.5">
      <button
        type="button"
        onclick={() => {
          if (typeof window !== "undefined") {
            window.open(withHostParam(`/scoreboard?tatami=${activeTatami}`), `scoreboard_tatami_${activeTatami}`);
          }
        }}
        class="px-2.5 py-1.5 bg-cyan-600/30 hover:bg-cyan-600/50 border border-cyan-500/40 text-cyan-300 hover:text-white text-xs font-bold rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
        title="Buka / Proyeksikan Scoreboard Tatami {activeTatami} di jendela baru"
      >
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/>
        </svg>
        <span>Scoreboard T{activeTatami}</span>
      </button>
      <ServerStatusBadge {eventName} />
      <div class="hidden sm:flex items-center space-x-1.5 text-xs font-medium text-emerald-400 bg-emerald-500/10 border border-emerald-500/20 px-2.5 py-1.5 rounded-xl">
        <span class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></span>
        <span>Control Panel Aktif</span>
      </div>
    </div>
  </div>

  <!-- ==================== VIDEO REVIEW (VR) REQUESTS (one row per corner) ==================== -->
  {#each ["ao", "aka"] as const as vrSide (vrSide)}
    {@const ch = vrChallenges[vrSide]}
    {#if ch}
      <div class="rounded-xl border p-3 flex items-center justify-between gap-4 shadow-lg shrink-0 mb-3 {ch.status === 'accepted' ? 'bg-emerald-950/80 border-emerald-500 text-emerald-200' : ch.status === 'rejected' ? 'bg-rose-950/80 border-rose-500 text-rose-200' : ch.status === 'minai' ? 'bg-amber-950/70 border-amber-500 text-amber-100' : (vrSide === 'aka' ? 'bg-red-950/80 border-red-500 text-white' : 'bg-blue-950/80 border-blue-500 text-white')}">
        <div class="flex items-center gap-3 min-w-0">
          <span class="w-3.5 h-3.5 rounded-full {vrSide === 'aka' ? 'bg-red-500' : 'bg-blue-500'} {ch.status === 'in_progress' ? 'animate-ping' : ''} shrink-0"></span>
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <span class="px-2 py-0.5 rounded text-[10px] font-mono font-black uppercase {vrSide === 'aka' ? 'bg-red-600 text-white' : 'bg-blue-600 text-white'}">
                VIDEO REVIEW: {vrSide.toUpperCase()}
              </span>
              {#if ch.status === 'in_progress' && ch.reason}
                <span class="text-xs font-black truncate">Diminta: {ch.reason}</span>
              {/if}
            </div>
            <span class="text-[11px] opacity-90 font-mono block mt-0.5">
              {#if ch.status === 'in_progress'}
                Menunggu keputusan wasit VR di tablet (atau putuskan di sini). Timer tidak dihentikan otomatis.
              {:else if ch.status === 'accepted'}
                ✓ DITERIMA{ch.points ? ` ${ch.points} POIN · tambahkan skor ${vrSide.toUpperCase()} secara manual` : ""} (kartu tetap ada)
              {:else if ch.status === 'minai'}
                ◐ MINAI · tidak dapat disimpulkan (kartu tetap ada)
              {:else}
                ✕ DITOLAK (kartu hangus)
              {/if}
            </span>
          </div>
        </div>

        {#if ch.status === 'in_progress'}
          {@render vrDecisionButtons(vrSide)}
        {/if}
      </div>
    {/if}
  {/each}

  <!-- ==================== TEAM KUMITE (KUMITE BEREGU): bout bar ==================== -->
  {#if teamMatchInfo && teamMatchInfo.is_team_match}
    <div class="flex items-center gap-2 overflow-x-auto mb-3 shrink-0 scrollbar-thin">
      {#each teamMatchInfo.bouts as bout, bIdx}
        {@const isCurrent = bout.bout_id === match.id}
        {@const isLast = bIdx === teamMatchInfo.bouts.length - 1}
        <div class="shrink-0 flex items-center rounded-xl border transition {isCurrent ? 'bg-amber-500/20 border-amber-400 shadow-md shadow-amber-500/20' : 'bg-[#181a20] border-white/10 hover:border-white/20'}">
          <button
            type="button"
            onclick={() => onSwitchBout(bout.bout_id)}
            class="pl-3 {isCurrent ? 'pr-1.5' : 'pr-3'} py-1.5 text-xs font-mono font-bold flex items-center gap-2 {isCurrent ? 'text-amber-300' : 'text-zinc-300 hover:text-white'}"
          >
            <span>Partai {bout.round}</span>
            {#if bout.selesai}
              {#if bout.pemenang === '1'}
                <span class="px-1.5 py-0.5 rounded bg-red-600 text-white text-[9px] font-black">AKA ({bout.score1 || 0}-{bout.score2 || 0})</span>
              {:else if bout.pemenang === '2'}
                <span class="px-1.5 py-0.5 rounded bg-blue-600 text-white text-[9px] font-black">AO ({bout.score1 || 0}-{bout.score2 || 0})</span>
              {:else if bout.pemenang === '3'}
                <span class="px-1.5 py-0.5 rounded bg-amber-500 text-black text-[9px] font-black">SERI ({bout.score1 || 0}-{bout.score2 || 0})</span>
              {:else}
                <span class="px-1.5 py-0.5 rounded bg-zinc-700 text-zinc-200 text-[9px]">Selesai</span>
              {/if}
            {:else}
              <span class="px-1.5 py-0.5 rounded bg-white/5 text-zinc-400 text-[9px]">Belum</span>
            {/if}
          </button>
          {#if isCurrent}
            <!-- Atur atlet partai ini -->
            <button
              type="button"
              onclick={openRosterModal}
              class="mr-1.5 w-6 h-6 flex items-center justify-center rounded-lg text-amber-300 hover:text-white hover:bg-amber-500/30 transition"
              title="Atur atlet partai {bout.round}"
              aria-label="Atur atlet partai {bout.round}"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>
            </button>
          {/if}
        </div>
        {#if isLast}
          {#if teamMatchInfo.bouts.length > 1 && !bout.selesai}
            <button
              type="button"
              onclick={() => onDeleteBout(bout.bout_id)}
              class="shrink-0 w-7 h-7 flex items-center justify-center text-zinc-500 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition"
              title="Hapus partai {bout.round}"
              aria-label="Hapus partai {bout.round}"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
            </button>
          {/if}
          <button
            type="button"
            onclick={onAddBout}
            class="shrink-0 w-7 h-7 flex items-center justify-center text-emerald-400 hover:text-white hover:bg-emerald-600/30 border border-emerald-500/30 rounded-lg transition"
            title="Tambah partai (extra / playoff bila seri)"
            aria-label="Tambah partai"
          >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4"/></svg>
          </button>
        {/if}
      {/each}
    </div>
  {/if}

  <!-- Main View Area (Kata vs Kumite automatically) -->
  {#if isKata}
    <!-- ==================== KATA COMPARATIVE SCORING DASHBOARD ==================== -->
    <div class="flex-1 flex flex-col gap-3 min-h-0 overflow-y-auto pr-1">
      <!-- Top: compact timer + board controls (left), live board preview (right) -->
      <div class="grid grid-cols-[minmax(0,1fr)_minmax(320px,480px)] gap-3 shrink-0">
        <div class="flex flex-col gap-3 min-w-0">
          <!-- Compact kata timer (presets, alarms and arena bell live under the gear) -->
          <div class="relative bg-[#181a20] border border-[#2b2d31] rounded-xl p-3 shadow-lg flex items-center gap-2 flex-wrap">
            <div class="bg-[#111216] border px-4 py-1.5 rounded-lg flex items-baseline space-x-1 shadow-inner transition-colors {isTimerRunning && timerMs <= 15000 && timerMs > 0 ? 'border-red-500 shadow-[0_0_15px_rgba(239,68,68,0.35)] bg-red-950/20' : isTimerRunning ? 'border-amber-500/50' : 'border-[#2b2d31]'}">
              <span class="font-mono font-black text-2xl tracking-wider transition-colors {isTimerRunning && timerMs <= 15000 && timerMs > 0 ? 'text-red-500 animate-pulse' : isTimerRunning ? 'text-amber-300' : 'text-white'}">
                {timerFormatted.mmss}
              </span>
              <span class="font-mono text-sm font-bold transition-colors {isTimerRunning && timerMs <= 15000 && timerMs > 0 ? 'text-red-400' : isTimerRunning ? 'text-amber-400' : 'text-zinc-400'}">.{timerFormatted.msStr}</span>
            </div>
            <button
              type="button"
              onclick={(e) => {
                (e.currentTarget as HTMLElement)?.blur();
                toggleTimer();
              }}
              class="h-9 px-4 rounded-lg text-xs font-black uppercase tracking-wider transition shadow flex items-center space-x-1.5 {isTimerRunning ? 'bg-amber-600 hover:bg-amber-500 text-white' : 'bg-emerald-600 hover:bg-emerald-500 text-white active:scale-95'}"
            >
              {#if isTimerRunning}
                <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>
                <span>Jeda (Spasi)</span>
              {:else}
                <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>
                <span>Mulai (Spasi)</span>
              {/if}
            </button>
            <button
              type="button"
              onclick={resetTimer}
              class="h-9 px-3 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-300 text-xs font-bold rounded-lg transition"
            >
              Reset
            </button>
            <button
              type="button"
              onclick={() => adjustTimer(-1000)}
              class="h-9 px-2 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-xs font-mono font-bold text-zinc-300 rounded-lg"
              title="Kurangi 1 detik"
            >
              -1s
            </button>
            <button
              type="button"
              onclick={() => adjustTimer(1000)}
              class="h-9 px-2 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-xs font-mono font-bold text-zinc-300 rounded-lg"
              title="Tambah 1 detik"
            >
              +1s
            </button>
            {@render timerPresetSelect()}
            {@render timerSettingsButton()}
          </div>

          <!-- Board controls: everything that changes what the TV shows -->
          <div class="flex-1 bg-[#181a20] border border-[#2b2d31] rounded-xl p-3 shadow-lg flex flex-col gap-3">
            <div class="text-[11px] font-mono font-bold uppercase tracking-wider text-zinc-400">Kontrol Board</div>
            <div class="grid grid-cols-2 gap-2">
              <button
                type="button"
                onclick={toggleBoardScore}
                class="h-12 px-3 rounded-xl text-xs font-mono font-bold uppercase tracking-wider transition flex items-center justify-center gap-2 shadow active:scale-95 {isScorePublished ? 'bg-[#111216] hover:bg-[#1e2029] text-zinc-300 border border-white/10' : 'bg-cyan-600 hover:bg-cyan-500 text-white shadow-cyan-600/30'}"
                title="Tampilkan / sembunyikan skor besar di scoring board"
              >
                {#if isScorePublished}
                  <svg class="w-4 h-4 text-cyan-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l18 18"/></svg>
                  <span>Sembunyikan Skor</span>
                {:else}
                  <svg class="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>
                  <span>Tampilkan Skor</span>
                {/if}
              </button>
              <button
                type="button"
                onclick={toggleKataScorePopup}
                class="h-12 px-3 rounded-xl text-xs font-mono font-bold uppercase tracking-wider transition flex items-center justify-center gap-2 shadow active:scale-95 {showKataScoreOnBoard ? 'bg-amber-500 hover:bg-amber-400 text-black shadow-amber-500/20' : 'bg-[#111216] hover:bg-[#1e2029] text-zinc-300 border border-white/10'}"
                title="Buka / tutup pop up skor {kataJudgeCount} juri di scoring board"
              >
                {#if showKataScoreOnBoard}
                  <svg class="w-4 h-4 text-black" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l18 18"/></svg>
                  <span>Tutup Pop Up {kataJudgeCount} Juri</span>
                {:else}
                  <svg class="w-4 h-4 text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"/></svg>
                  <span>Pop Up Skor {kataJudgeCount} Juri</span>
                {/if}
              </button>
            </div>
            <!-- 35-second preparation countdown on the board (athlete must start the kata) -->
            <div class="flex items-center gap-2">
              <span class="text-[10px] font-mono font-bold uppercase text-zinc-500 mr-1 shrink-0">Persiapan</span>
              {#each [["aka", "AKA"], ["ao", "AO"]] as [side, label]}
                {@const active = prepCountdown?.side === side}
                <button
                  type="button"
                  onclick={() => startPrepCountdown(side as "aka" | "ao")}
                  class="flex-1 h-9 px-3 rounded-lg text-xs font-mono font-bold transition active:scale-95 flex items-center justify-center gap-1.5 border {side === 'aka'
                    ? (active ? 'bg-red-600 border-red-400 text-white shadow-lg shadow-red-600/40' : 'bg-red-600/20 hover:bg-red-600/30 border-red-500/50 text-red-200')
                    : (active ? 'bg-blue-600 border-blue-400 text-white shadow-lg shadow-blue-600/40' : 'bg-blue-600/20 hover:bg-blue-600/30 border-blue-500/50 text-blue-200')}"
                  title="Mulai hitung mundur persiapan 35 detik untuk {label}"
                >
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>
                  {#if active}
                    <span>{label} · {prepRemainingMs > 0 ? `${Math.ceil(prepRemainingMs / 1000)}s` : "HABIS"}</span>
                  {:else}
                    <span>35s {label}</span>
                  {/if}
                </button>
              {/each}
              <button
                type="button"
                onclick={() => stopPrepCountdown()}
                disabled={!prepCountdown}
                class="h-9 px-3 rounded-lg text-xs font-mono font-bold transition border {prepCountdown ? 'bg-[#111216] hover:bg-rose-600/80 border-white/10 hover:border-rose-500/50 text-zinc-200' : 'bg-transparent border-white/5 text-zinc-600 cursor-not-allowed'}"
                title="Hentikan dan tutup pop up persiapan"
              >
                Stop
              </button>
            </div>

            <!-- Kiken kept apart from the display buttons so it is not hit by mistake -->
            <div class="mt-auto pt-3 border-t border-[#2b2d31] flex items-center gap-2">
              <span class="text-[10px] font-mono font-bold uppercase text-zinc-500 mr-1">Kiken</span>
              <button
                type="button"
                onclick={() => applyKiken("aka")}
                class="px-3 py-1.5 bg-red-600/20 hover:bg-red-600/30 border border-red-500/50 text-red-300 text-xs font-bold rounded-lg transition active:scale-95"
                title="Kiken Sudut Merah AKA (Semua Juri 0.0)"
              >
                Kiken AKA
              </button>
              <button
                type="button"
                onclick={() => applyKiken("ao")}
                class="px-3 py-1.5 bg-blue-600/20 hover:bg-blue-600/30 border border-blue-500/50 text-blue-300 text-xs font-bold rounded-lg transition active:scale-95"
                title="Kiken Sudut Biru AO (Semua Juri 0.0)"
              >
                Kiken AO
              </button>
            </div>
          </div>
        </div>

        <!-- Live board preview: the same template the TV uses -->
        <div class="w-full aspect-video self-start rounded-xl overflow-hidden border border-[#2b2d31] bg-black shadow-lg">
          <ScoreboardDisplay liveState={previewState} tatami={activeTatami} connected={claimStatus === "owned"} muted />
        </div>
      </div>

      <!-- KATA JUDGES COMPARATIVE SCORING TABLE (3 / 5 / 7) -->
      <div class="flex-1 bg-[#181a20] border border-[#2b2d31] rounded-xl p-4 shadow-xl flex flex-col justify-between relative">
        <div>
          <!-- The Table -->
          <div class="overflow-x-auto">
            <table class="w-full text-center border-collapse">
              <thead>
                <tr class="text-[11px] font-mono font-bold uppercase tracking-wider text-zinc-400 border-b border-[#2b2d31]">
                  <th class="py-2.5 px-3 text-left w-[240px]">Sudut & Atlet</th>
                  {#each judgeIdx as idx (idx)}
                    {@const jNum = idx + 1}
                    {@const decision = judgeComparisons.decisions[idx]}
                    <th class="py-2.5 px-1.5 w-[90px] transition-colors rounded-t-lg {decision?.winner === 'aka' ? 'bg-red-500/20 text-red-300 border-t-2 border-red-500' : decision?.winner === 'ao' ? 'bg-blue-500/20 text-blue-300 border-t-2 border-blue-500' : 'bg-transparent text-zinc-400'}">
                      <div>JURI {jNum}</div>
                      {#if decision?.winner === "aka"}
                        <div class="text-[9px] font-black text-red-400 mt-0.5">AKA (+1)</div>
                      {:else if decision?.winner === "ao"}
                        <div class="text-[9px] font-black text-blue-400 mt-0.5">AO (+1)</div>
                      {:else if decision?.winner === "seri"}
                        <div class="text-[9px] font-bold text-zinc-500 mt-0.5">SERI</div>
                      {:else}
                        <div class="text-[9px] font-normal text-zinc-600 mt-0.5">-</div>
                      {/if}
                    </th>
                  {/each}
                  <th class="py-2.5 px-2 text-left w-[170px]">Pilihan Kata</th>
                  <th class="py-2.5 px-2 w-[85px] text-center" title="Sistem Bendera (Ambrilindo Local)">Bendera</th>
                  <th class="py-2.5 px-2 w-[95px] text-center">Total Poin</th>
                  <th class="py-2.5 px-2.5 w-[125px] text-center">Status</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-[#2b2d31]">
                <!-- AKA (RED) ROW -->
                <tr class="hover:bg-red-500/5 transition">
                  <td class="py-3 px-3 text-left">
                    <div class="flex items-center space-x-2">
                      <span class="px-2 py-0.5 bg-red-600 text-white font-mono font-black text-[10px] rounded shrink-0">
                        AKA
                      </span>
                      <span class="font-bold text-sm text-white leading-tight">
                        {match.atlet1_nama || "Sudut Merah"}
                      </span>
                    </div>
                    <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
                    <div class="text-xs text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                      {match.atlet1_kontingen || "-"}
                    </div>
                  </td>

                  <!-- 5 Judge Input Columns for AKA -->
                  {#each judgeIdx as idx (idx)}
                    {@const decision = judgeComparisons.decisions[idx]}
                    <td class="py-2.5 px-1.5 relative {decision?.winner === 'aka' ? 'bg-red-500/10' : ''}">
                      <input
                        type="number"
                        step="0.1"
                        min="5.0"
                        max="10.0"
                        placeholder="0.0"
                        value={akaJudgeScores[idx]}
                        onclick={() => (activeJudgePicker = idx)}
                        onblur={(e) => handleScoreInputBlur("aka", idx, e.currentTarget.value)}
                        oninput={(e) => updateJudgeScore("aka", idx, e.currentTarget.value)}
                        class="w-20 h-11 bg-[#111216] border-2 cursor-pointer {decision?.winner === 'aka' ? 'border-red-500 text-red-200 ring-2 ring-red-500/30' : 'border-[#2b2d31] text-zinc-200'} focus:border-red-400 text-center font-mono font-bold text-lg rounded-lg outline-none transition"
                        title="Pilih nilai untuk Juri {idx + 1}"
                      />
                    </td>
                  {/each}

                  <!-- Kata Selection (Searchable Discord Style) -->
                  <td class="py-2.5 px-2 text-left">
                    <button
                      type="button"
                      onclick={() => openKataModal("aka")}
                      class="w-full h-11 px-2.5 bg-[#111216] hover:bg-[#1e2029] border border-[#2b2d31] hover:border-white/20 rounded-lg text-left flex items-center justify-between gap-1.5 text-xs font-semibold text-zinc-200 transition"
                      title={kataFromLo.aka ? "Kata dari LO / Guide (klik untuk mengganti)" : "Klik untuk memilih Kata"}
                    >
                      {#key kataLoFlash.aka}
                        <span class="truncate min-w-0 flex-1 rounded px-1 -mx-1" class:cp-kata-flash={kataLoFlash.aka > 0}>{kataAkaName || "Pilih Kata..."}</span>
                      {/key}
                      {#if kataFromLo.aka && kataAkaName}
                        <span class="shrink-0 px-1.5 py-0.5 rounded text-[9px] font-mono font-black bg-purple-500/20 text-purple-300 border border-purple-500/40">LO</span>
                      {/if}
                      <svg class="w-4 h-4 text-zinc-400 shrink-0 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                      </svg>
                    </button>
                  </td>

                  <!-- Bendera Selection for AKA (Ambrilindo Local standard) -->
                  <td class="py-2.5 px-2 text-center">
                    <select
                      value={akaBendera}
                      onchange={(e) => handleAkaBenderaChange(e.currentTarget.value)}
                      class="w-16 h-11 bg-[#111216] border-2 border-red-500/70 text-red-200 font-mono font-black text-xl text-center rounded-lg outline-none cursor-pointer focus:border-red-400 focus:ring-2 focus:ring-red-500/30 transition mx-auto block shadow-inner"
                      title="Pilih jumlah bendera Sudut Merah (AKA)"
                    >
                      <option value="-" class="bg-[#181a20] text-zinc-400">-</option>
                      {#each [0, ...judgeIdx.map((i) => i + 1)] as n (n)}
                        <option value={String(n)} class="bg-[#181a20] text-white">{n}</option>
                      {/each}
                    </select>
                  </td>

                  <!-- Total Points for AKA -->
                  <td class="py-2.5 px-2">
                    <div class="h-11 bg-[#111216] border-2 {effectiveAkaPoints > effectiveAoPoints ? 'border-red-500 text-red-400 bg-red-500/10' : 'border-[#2b2d31] text-zinc-300'} font-mono font-black text-2xl flex items-center justify-center rounded-lg shadow-inner">
                      {effectiveAkaPoints}
                    </div>
                  </td>

                  <!-- Winner Status Button for AKA -->
                  <td class="py-2.5 px-2.5">
                    <button
                      type="button"
                      onclick={() => selectWinner("1")}
                      class="w-full py-2.5 rounded-lg text-xs font-black transition flex items-center justify-center gap-1.5 shadow active:scale-95 {matchWinner === '1' ? 'bg-red-600 text-white shadow-red-600/40 border border-red-400' : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-[#2b2d31]'}"
                    >
                      {#if matchWinner === "1"}
                        <svg class="w-3.5 h-3.5 text-amber-300" fill="currentColor" viewBox="0 0 20 20">
                          <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
                        </svg>
                      {/if}
                      <span>PEMENANG</span>
                    </button>
                  </td>
                </tr>

                <!-- AO (BLUE) ROW -->
                <tr class="hover:bg-blue-500/5 transition">
                  <td class="py-3 px-3 text-left">
                    <div class="flex items-center space-x-2">
                      <span class="px-2 py-0.5 bg-blue-600 text-white font-mono font-black text-[10px] rounded shrink-0">
                        AO
                      </span>
                      <span class="font-bold text-sm text-white leading-tight">
                        {match.atlet2_nama || "Sudut Biru"}
                      </span>
                    </div>
                    <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
                    <div class="text-xs text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                      {match.atlet2_kontingen || "-"}
                    </div>
                  </td>

                  <!-- 5 Judge Input Columns for AO -->
                  {#each judgeIdx as idx (idx)}
                    {@const decision = judgeComparisons.decisions[idx]}
                    <td class="py-2.5 px-1.5 relative {decision?.winner === 'ao' ? 'bg-blue-500/10' : ''}">
                      <input
                        type="number"
                        step="0.1"
                        min="5.0"
                        max="10.0"
                        placeholder="0.0"
                        value={aoJudgeScores[idx]}
                        onclick={() => (activeJudgePicker = idx)}
                        onblur={(e) => handleScoreInputBlur("ao", idx, e.currentTarget.value)}
                        oninput={(e) => updateJudgeScore("ao", idx, e.currentTarget.value)}
                        class="w-20 h-11 bg-[#111216] border-2 cursor-pointer {decision?.winner === 'ao' ? 'border-blue-500 text-blue-200 ring-2 ring-blue-500/30' : 'border-[#2b2d31] text-zinc-200'} focus:border-blue-400 text-center font-mono font-bold text-lg rounded-lg outline-none transition"
                        title="Pilih nilai untuk Juri {idx + 1}"
                      />
                    </td>
                  {/each}

                  <!-- Kata Selection (Searchable Discord Style) -->
                  <td class="py-2.5 px-2 text-left">
                    <button
                      type="button"
                      onclick={() => openKataModal("ao")}
                      class="w-full h-11 px-2.5 bg-[#111216] hover:bg-[#1e2029] border border-[#2b2d31] hover:border-white/20 rounded-lg text-left flex items-center justify-between gap-1.5 text-xs font-semibold text-zinc-200 transition"
                      title={kataFromLo.ao ? "Kata dari LO / Guide (klik untuk mengganti)" : "Klik untuk memilih Kata"}
                    >
                      {#key kataLoFlash.ao}
                        <span class="truncate min-w-0 flex-1 rounded px-1 -mx-1" class:cp-kata-flash={kataLoFlash.ao > 0}>{kataAoName || "Pilih Kata..."}</span>
                      {/key}
                      {#if kataFromLo.ao && kataAoName}
                        <span class="shrink-0 px-1.5 py-0.5 rounded text-[9px] font-mono font-black bg-purple-500/20 text-purple-300 border border-purple-500/40">LO</span>
                      {/if}
                      <svg class="w-4 h-4 text-zinc-400 shrink-0 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                      </svg>
                    </button>
                  </td>

                  <!-- Bendera Value for AO (Ambrilindo Local standard: 5 - AKA) -->
                  <td class="py-2.5 px-2 text-center">
                    <div
                      class="w-16 h-11 bg-[#111216] border-2 border-blue-500/70 text-blue-200 font-mono font-black text-xl flex items-center justify-center rounded-lg shadow-inner mx-auto select-none"
                      title="Jumlah bendera Sudut Biru (AO): juri yang memberi nilai AO lebih tinggi"
                    >
                      {aoBendera}
                    </div>
                  </td>

                  <!-- Total Points for AO -->
                  <td class="py-2.5 px-2">
                    <div class="h-11 bg-[#111216] border-2 {effectiveAoPoints > effectiveAkaPoints ? 'border-blue-500 text-blue-400 bg-blue-500/10' : 'border-[#2b2d31] text-zinc-300'} font-mono font-black text-2xl flex items-center justify-center rounded-lg shadow-inner">
                      {effectiveAoPoints}
                    </div>
                  </td>

                  <!-- Winner Status Button for AO -->
                  <td class="py-2.5 px-2.5">
                    <button
                      type="button"
                      onclick={() => selectWinner("2")}
                      class="w-full py-2.5 rounded-lg text-xs font-black transition flex items-center justify-center gap-1.5 shadow active:scale-95 {matchWinner === '2' ? 'bg-blue-600 text-white shadow-blue-600/40 border border-blue-400' : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-[#2b2d31]'}"
                    >
                      {#if matchWinner === "2"}
                        <svg class="w-3.5 h-3.5 text-amber-300" fill="currentColor" viewBox="0 0 20 20">
                          <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
                        </svg>
                      {/if}
                      <span>PEMENANG</span>
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <!-- QUICK SCORE PICKER POPOVER (SIMPEL, AKA ATAS, AO BAWAH, HORIZONTAL SCROLL) -->
        {#if activeJudgePicker !== null}
          <div
            class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 outline-none"
            onclick={(e) => { if (e.target === e.currentTarget) activeJudgePicker = null; }}
            onkeydown={(e) => { if (e.key === "Escape") activeJudgePicker = null; }}
            role="dialog"
            tabindex="-1"
            aria-modal="true"
            aria-label="Pilih Nilai Juri"
          >
            <div
              class="bg-[#181a20] border-2 border-[#5865f2] rounded-2xl p-4 shadow-2xl max-w-xl w-full animate-in fade-in zoom-in-95 duration-150"
            >
              <!-- Minimal Header -->
              <div class="flex items-center justify-between pb-2 mb-3 border-b border-[#2b2d31]">
                <div class="flex items-center space-x-2">
                  <span class="font-mono font-black text-xs text-white uppercase tracking-wider">
                    JURI {activeJudgePicker + 1}
                  </span>
                </div>
                <button
                  type="button"
                  onclick={() => (activeJudgePicker = null)}
                  class="w-7 h-7 flex items-center justify-center text-zinc-400 hover:text-white bg-[#111216] hover:bg-[#2b2d31] rounded-lg transition"
                  aria-label="Tutup"
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                </button>
              </div>

              <!-- Atas: AKA (Horizontal scrollable ke kanan-kiri) -->
              <div class="mb-3">
                <div class="flex items-center space-x-2">
                  <span class="px-2.5 py-1.5 bg-red-600 text-white font-mono font-black text-xs rounded-lg shrink-0 w-12 text-center">
                    AKA
                  </span>
                  <div
                    class="flex-1 overflow-x-auto whitespace-nowrap flex space-x-1.5 py-1 px-1 scrollbar-thin scroll-smooth"
                    onwheel={(e) => {
                      if (e.deltaY !== 0) {
                        e.preventDefault();
                        e.currentTarget.scrollLeft += e.deltaY;
                      }
                    }}
                  >
                    {#each KATA_SCORE_LIST as scoreVal}
                      <button
                        type="button"
                        onclick={() => updateJudgeScore("aka", activeJudgePicker!, scoreVal)}
                        class="px-3 py-1.5 rounded-lg font-mono font-bold text-xs shrink-0 transition {akaJudgeScores[activeJudgePicker] === scoreVal ? 'bg-red-600 text-white shadow-md shadow-red-600/40 ring-2 ring-red-400' : 'bg-[#111216] text-zinc-300 hover:bg-white/10 hover:text-white border border-[#2b2d31]'}"
                      >
                        {scoreVal}
                      </button>
                    {/each}
                  </div>
                </div>
              </div>

              <!-- Bawah: AO (Horizontal scrollable ke kanan-kiri) -->
              <div>
                <div class="flex items-center space-x-2">
                  <span class="px-2.5 py-1.5 bg-blue-600 text-white font-mono font-black text-xs rounded-lg shrink-0 w-12 text-center">
                    AO
                  </span>
                  <div
                    class="flex-1 overflow-x-auto whitespace-nowrap flex space-x-1.5 py-1 px-1 scrollbar-thin scroll-smooth"
                    onwheel={(e) => {
                      if (e.deltaY !== 0) {
                        e.preventDefault();
                        e.currentTarget.scrollLeft += e.deltaY;
                      }
                    }}
                  >
                    {#each KATA_SCORE_LIST as scoreVal}
                      <button
                        type="button"
                        onclick={() => updateJudgeScore("ao", activeJudgePicker!, scoreVal)}
                        class="px-3 py-1.5 rounded-lg font-mono font-bold text-xs shrink-0 transition {aoJudgeScores[activeJudgePicker] === scoreVal ? 'bg-blue-600 text-white shadow-md shadow-blue-600/40 ring-2 ring-blue-400' : 'bg-[#111216] text-zinc-300 hover:bg-white/10 hover:text-white border border-[#2b2d31]'}"
                      >
                        {scoreVal}
                      </button>
                    {/each}
                  </div>
                </div>
              </div>
            </div>
          </div>
        {/if}
      </div>
    </div>

  {:else}
    <!-- ==================== KUMITE 3-COLUMN COCKPIT ==================== -->
    <div class="flex-1 grid grid-cols-1 lg:grid-cols-12 gap-3 min-h-0 overflow-hidden">
      <!-- POD KIRI: AO (BLUE) -->
      <div class="lg:col-span-4 bg-[#181a20] border-t-4 border-t-blue-600 border border-[#2b2d31] rounded-xl p-4 flex flex-col shadow-xl min-h-0 overflow-y-auto cp-pod">
        <div>
          <!-- Header AO -->
          <div class="flex items-start justify-between gap-2 pb-3 mb-3 border-b border-[#2b2d31] cp-pod-head">
            <div class="min-w-0 flex-1">
              <span class="px-2 py-0.5 bg-blue-600 text-white font-mono font-black text-[10px] rounded tracking-wider uppercase inline-block mb-1">
                AO • BIRU
              </span>
              <div class="text-xl md:text-2xl font-black text-white uppercase leading-tight cp-pod-name">
                {match.atlet2_nama || "Atlet Biru"}
              </div>
              <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
              <div class="text-xs font-semibold text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                {match.atlet2_kontingen || "-"}
              </div>
            </div>

            <button
              type="button"
              onclick={() => selectWinner("2")}
              class="px-3 py-1.5 rounded-lg text-xs font-black uppercase tracking-wider transition shrink-0 flex items-center gap-1.5 {matchWinner === '2' ? 'bg-gradient-to-r from-blue-600 to-blue-700 text-white border-2 border-blue-400 shadow-lg shadow-blue-600/60' : 'bg-[#1e2029] border border-[#2b2d31] text-zinc-300 hover:text-white'}"
            >
              <svg class="w-4 h-4 {matchWinner === '2' ? 'text-amber-300' : 'text-zinc-400'}" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
              </svg>
              <span>AO Menang</span>
            </button>
          </div>

          <!-- Score & Senshu Row -->
          <div class="flex items-center gap-3 mb-4 cp-score-row">
            <div class="cp-score-box flex-1 h-24 bg-[#111216] border-2 border-blue-600 rounded-xl flex items-center justify-center shadow-inner">
              <span class="cp-score-num text-6xl md:text-7xl font-mono font-black text-blue-400 tracking-tight">
                {aoScore}
              </span>
            </div>

            <!-- Senshu Ribbon -->
            <button
              type="button"
              onclick={() => toggleSenshu("ao")}
              class="cp-senshu w-24 h-24 rounded-xl flex flex-col items-center justify-center gap-1 font-black transition border-2 {aoSenshu ? 'bg-gradient-to-br from-amber-400 to-amber-600 text-slate-950 border-white shadow-lg shadow-amber-500/50' : 'bg-[#1e2029] text-amber-500 border-amber-600/50 hover:bg-[#2b2d31]'}"
            >
              <svg class="cp-senshu-icon w-7 h-7" fill="currentColor" viewBox="0 0 24 24">
                <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
              </svg>
              <span class="text-[11px] tracking-wider font-mono">SENSHU</span>
            </button>
          </div>

          <!-- Fast Score Deck (mirror of AKA: values read from the right) -->
          <div class="grid grid-cols-4 gap-2 mb-4 cp-deck">
            <button
              type="button"
              onclick={() => addKumiteScore("ao", 3)}
              class="h-12 bg-gradient-to-br from-blue-600 to-blue-800 hover:from-blue-500 hover:to-blue-700 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +3 IPPON
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", 2)}
              class="h-12 bg-gradient-to-br from-blue-600 to-blue-800 hover:from-blue-500 hover:to-blue-700 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +2 WAZA
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", 1)}
              class="h-12 bg-gradient-to-br from-blue-600 to-blue-800 hover:from-blue-500 hover:to-blue-700 border border-blue-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +1 YUKO
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("ao", -1)}
              class="h-12 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-300 font-mono font-black text-base rounded-lg active:scale-95 transition"
            >
              -1
            </button>
          </div>

          <!-- Penalties (C1..H) -->
          <div class="bg-[#111216] border border-[#2b2d31] rounded-xl p-3 mb-3 cp-pen">
            <div class="text-[10px] font-mono font-bold text-zinc-400 uppercase tracking-wider mb-2">Pelanggaran</div>
            <div class="grid grid-cols-5 gap-1.5 font-mono font-bold text-xs">
              {#each ["h", "hc", "c3", "c2", "c1"] as pCode}
                {@const isMarked = aoPenalties[pCode as keyof typeof aoPenalties]}
                <button
                  type="button"
                  onclick={() => togglePenalty("ao", pCode as any)}
                  class="h-11 rounded-md transition flex items-center justify-center uppercase {isMarked ? (pCode === 'h' ? 'bg-red-600 text-white shadow-lg' : 'bg-amber-500 text-black font-black') : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-white/5'}"
                >
                  {pCode.toUpperCase()}
                </button>
              {/each}
            </div>
          </div>

          <!-- Video Review AO: right under the penalties -->
          {#if vrEnabled}
            {@const reviewing = vrChallenges.ao?.status === "in_progress"}
            <div class="px-1 flex flex-col gap-2 text-xs text-zinc-400">
              <div class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-2">
                  <img src="/img/vr-ao.png" alt="VR" class="w-6 h-6 {hasVr2 ? 'opacity-30 grayscale' : ''}" />
                  <span>Video Review</span>
                  <button
                    type="button"
                    onclick={() => toggleVrCard("ao")}
                    class="px-2 py-0.5 rounded font-mono font-bold text-[10px] border transition {reviewing ? 'bg-amber-500/20 border-amber-500 text-amber-300 animate-pulse' : hasVr2 ? 'bg-rose-600/20 border-rose-500/60 text-rose-300' : 'bg-emerald-600/15 border-emerald-500/50 text-emerald-300'}"
                    title="Klik untuk mengoreksi status kartu VR secara manual"
                  >
                    {reviewing ? "DITINJAU" : hasVr2 ? "HANGUS" : "TERSEDIA"}
                  </button>
                </span>
                {#if !reviewing}
                  <button
                    type="button"
                    onclick={() => requestVr("ao")}
                    disabled={hasVr2}
                    class="h-9 px-3 rounded-lg text-[11px] font-bold font-mono transition border disabled:opacity-40 disabled:cursor-not-allowed bg-[#1e2029] border-white/10 text-zinc-200 hover:text-white hover:border-white/30"
                    title="Ajukan Video Review (timer dijeda)"
                  >
                    Ajukan VR
                  </button>
                {/if}
              </div>
              {#if reviewing}
                {@render vrDecisionButtons("ao")}
              {/if}
            </div>
          {/if}

          <!-- Team score (beregu) + medical time, under the VR row -->
          <div class="mt-3 cp-extras">
            {@render cornerExtras("ao")}
          </div>
        </div>
      </div>

      <!-- HUB TENGAH: LIVE PREVIEW (the board already shows the timer) + CONTROLS -->
      <div class="lg:col-span-4 bg-[#181a20] border-t-4 border-t-amber-500 border border-[#2b2d31] rounded-xl p-3 flex flex-col gap-2.5 shadow-xl min-h-0 overflow-y-auto">
        <!-- Live board preview: the same template the TV uses. Its box is exactly the
             board (16:9 of the column width), so the controls sit right under it on a
             tall window; on a short window it shrinks so the column never scrolls. -->
        <div class="w-full aspect-video shrink min-h-[110px] flex items-start justify-center [container-type:size]">
          <div class="cp-preview rounded-lg overflow-hidden border border-[#2b2d31] bg-black">
            <ScoreboardDisplay liveState={previewState} tatami={activeTatami} connected={claimStatus === "owned"} muted />
          </div>
        </div>

        <button
          type="button"
          onclick={(e) => {
            (e.currentTarget as HTMLElement)?.blur();
            toggleTimer();
          }}
          class="w-full py-3 shrink-0 rounded-xl font-mono font-black text-sm uppercase tracking-wider transition shadow-lg active:scale-95 flex items-center justify-center gap-2 {isTimerRunning ? 'bg-amber-600 hover:bg-amber-500 text-white' : 'bg-emerald-600 hover:bg-emerald-500 text-white'}"
        >
          {#if isTimerRunning}
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>
            <span>PAUSE (SPASI)</span>
          {:else}
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>
            <span>START (SPASI)</span>
          {/if}
          <span class="px-2 py-0.5 rounded-md text-xs font-black tabular-nums {isTimerRunning && timerMs <= 15000 && timerMs > 0 ? 'bg-red-600 text-white animate-pulse' : 'bg-black/25'}">{timerFormatted.mmss}</span>
        </button>

        <!-- -1s / +1s / Reset / preset / settings -->
        <div class="relative flex items-center gap-1.5 font-mono text-xs shrink-0">
          <button type="button" onclick={() => adjustTimer(-1000)} class="h-9 px-2.5 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 rounded-lg font-bold text-zinc-300" title="Kurangi 1 detik">-1s</button>
          <button type="button" onclick={() => adjustTimer(1000)} class="h-9 px-2.5 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 rounded-lg font-bold text-zinc-300" title="Tambah 1 detik">+1s</button>
          <button type="button" onclick={resetTimer} class="h-9 px-3 font-bold text-red-400 bg-red-600/10 hover:bg-red-600/20 border border-red-500/30 rounded-lg transition">Reset</button>
          {@render timerPresetSelect()}
          {@render timerSettingsButton()}
        </div>

        {#if mouseMode}
          <div class="shrink-0">
            <JudgeStickMonitor
              votes={stickState?.votes ?? [null, null, null, null]}
              aka={stickState?.aka ?? null}
              ao={stickState?.ao ?? null}
              hantei={isHantei}
              hanteiTally={stickState?.hantei ?? { aka: 0, ao: 0 }}
              timerRunning={isTimerRunning}
              checkMode={stickCheck}
              tested={stickTested}
              lastKey={stickLastKey}
              history={stickHistory}
              onToggleCheck={toggleStickCheck}
              onResetCheck={resetStickCheck}
            />
          </div>
        {/if}

        {#if activeBanner}
          <button
            type="button"
            onclick={dismissScoreBanner}
            class="w-full py-1.5 px-2 shrink-0 bg-amber-500 hover:bg-amber-400 text-slate-950 font-black text-xs rounded-lg transition uppercase tracking-wider animate-pulse shadow-lg flex items-center justify-center gap-1.5"
          >
            <span>Tutup Banner Score (Esc)</span>
          </button>
        {/if}

        <!-- Hantei (judges' decision) and, for kumite beregu, Hikiwake -->
        <div class="flex items-stretch gap-2 shrink-0">
          <button
            type="button"
            onclick={() => setHantei(!isHantei)}
            aria-pressed={isHantei}
            class="flex-1 py-2 px-3 rounded-xl text-xs font-black uppercase tracking-wider transition flex items-center justify-center gap-1.5 shadow {isHantei ? 'bg-gradient-to-r from-indigo-500 to-violet-600 text-white border-2 border-white/80 shadow-indigo-500/40' : 'bg-[#1e2029] border border-indigo-400/40 text-indigo-200 hover:bg-[#2b2d31]'}"
            title={isHantei ? "Matikan mode Hantei" : "Mode Hantei: 4 juri memilih AKA / AO lewat stick"}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 6l3 1m0 0l-3 9a5.002 5.002 0 006.001 0M6 7l3 9M6 7l6-2m6 2l3-1m-3 1l-3 9a5.002 5.002 0 006.001 0M18 7l3 9m-3-9l-6-2m0-2v2m0 16V5m0 16H9m3 0h3"/></svg>
            {#if isHantei}
              <span>Hantei aktif{mouseMode ? ` · ${hanteiVotes}/4 juri` : ""}</span>
            {:else}
              <span>Hantei</span>
            {/if}
          </button>
          {#if teamMatchInfo?.is_team_match}
            <button
              type="button"
              onclick={() => selectWinner("3")}
              class="flex-1 py-2 px-3 rounded-xl text-xs font-black uppercase tracking-wider transition flex items-center justify-center gap-1.5 shadow {matchWinner === '3' ? 'bg-gradient-to-r from-amber-500 to-amber-600 text-slate-950 border-2 border-white shadow-amber-500/50' : 'bg-[#1e2029] border border-amber-500/30 text-amber-300 hover:bg-[#2b2d31]'}"
            >
              <span>Hikiwake (Seri)</span>
            </button>
          {/if}
        </div>
      </div>

      <!-- POD KANAN: AKA (RED) -->
      <div class="lg:col-span-4 bg-[#181a20] border-t-4 border-t-red-600 border border-[#2b2d31] rounded-xl p-4 flex flex-col shadow-xl min-h-0 overflow-y-auto cp-pod">
        <div>
          <!-- Header AKA -->
          <div class="flex items-start justify-between gap-2 pb-3 mb-3 border-b border-[#2b2d31] cp-pod-head">
            <button
              type="button"
              onclick={() => selectWinner("1")}
              class="px-3 py-1.5 rounded-lg text-xs font-black uppercase tracking-wider transition shrink-0 flex items-center gap-1.5 {matchWinner === '1' ? 'bg-gradient-to-r from-red-600 to-red-700 text-white border-2 border-red-400 shadow-lg shadow-red-600/60' : 'bg-[#1e2029] border border-[#2b2d31] text-zinc-300 hover:text-white'}"
            >
              <svg class="w-4 h-4 {matchWinner === '1' ? 'text-amber-300' : 'text-zinc-400'}" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
              </svg>
              <span>AKA Menang</span>
            </button>

            <div class="min-w-0 flex-1 text-right">
              <span class="px-2 py-0.5 bg-red-600 text-white font-mono font-black text-[10px] rounded tracking-wider uppercase inline-block mb-1">
                AKA • MERAH
              </span>
              <div class="text-xl md:text-2xl font-black text-white uppercase leading-tight cp-pod-name">
                {match.atlet1_nama || "Sudut Merah"}
              </div>
              <!-- NO ELLIPSIS: FULL PERGURUAN & KONTINGEN -->
              <div class="text-xs font-semibold text-zinc-300 mt-1 whitespace-normal break-words leading-snug">
                {match.atlet1_kontingen || "-"}
              </div>
            </div>
          </div>

          <!-- Score & Senshu Row -->
          <div class="flex items-center gap-3 mb-4 cp-score-row">
            <!-- Senshu Ribbon -->
            <button
              type="button"
              onclick={() => toggleSenshu("aka")}
              class="cp-senshu w-24 h-24 rounded-xl flex flex-col items-center justify-center gap-1 font-black transition border-2 {akaSenshu ? 'bg-gradient-to-br from-amber-400 to-amber-600 text-slate-950 border-white shadow-lg shadow-amber-500/50' : 'bg-[#1e2029] text-amber-500 border-amber-600/50 hover:bg-[#2b2d31]'}"
            >
              <svg class="cp-senshu-icon w-7 h-7" fill="currentColor" viewBox="0 0 24 24">
                <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
              </svg>
              <span class="text-[11px] tracking-wider font-mono">SENSHU</span>
            </button>

            <div class="cp-score-box flex-1 h-24 bg-[#111216] border-2 border-red-600 rounded-xl flex items-center justify-center shadow-inner">
              <span class="cp-score-num text-6xl md:text-7xl font-mono font-black text-red-400 tracking-tight">
                {akaScore}
              </span>
            </div>
          </div>

          <!-- Fast Score Deck -->
          <div class="grid grid-cols-4 gap-2 mb-4 cp-deck">
            <button
              type="button"
              onclick={() => addKumiteScore("aka", -1)}
              class="h-12 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-zinc-300 font-mono font-black text-base rounded-lg active:scale-95 transition"
            >
              -1
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("aka", 1)}
              class="h-12 bg-gradient-to-br from-red-600 to-red-800 hover:from-red-500 hover:to-red-700 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +1 YUKO
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("aka", 2)}
              class="h-12 bg-gradient-to-br from-red-600 to-red-800 hover:from-red-500 hover:to-red-700 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +2 WAZA
            </button>
            <button
              type="button"
              onclick={() => addKumiteScore("aka", 3)}
              class="h-12 bg-gradient-to-br from-red-600 to-red-800 hover:from-red-500 hover:to-red-700 border border-red-400 text-white font-mono font-black text-sm rounded-lg active:scale-95 transition shadow"
            >
              +3 IPPON
            </button>
          </div>

          <!-- Penalties (C1..H) -->
          <div class="bg-[#111216] border border-[#2b2d31] rounded-xl p-3 mb-3 cp-pen">
            <div class="text-[10px] font-mono font-bold text-zinc-400 uppercase tracking-wider mb-2 text-right">Pelanggaran</div>
            <div class="grid grid-cols-5 gap-1.5 font-mono font-bold text-xs">
              {#each ["c1", "c2", "c3", "hc", "h"] as pCode}
                {@const isMarked = akaPenalties[pCode as keyof typeof akaPenalties]}
                <button
                  type="button"
                  onclick={() => togglePenalty("aka", pCode as any)}
                  class="h-11 rounded-md transition flex items-center justify-center uppercase {isMarked ? (pCode === 'h' ? 'bg-red-600 text-white shadow-lg' : 'bg-amber-500 text-black font-black') : 'bg-[#1e2029] text-zinc-400 hover:text-white border border-white/5'}"
                >
                  {pCode.toUpperCase()}
                </button>
              {/each}
            </div>
          </div>

          <!-- Video Review AKA: right under the penalties -->
          {#if vrEnabled}
            {@const reviewing = vrChallenges.aka?.status === "in_progress"}
            <div class="px-1 flex flex-col gap-2 text-xs text-zinc-400">
              <div class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-2">
                  <img src="/img/vr-aka.png" alt="VR" class="w-6 h-6 {hasVr1 ? 'opacity-30 grayscale' : ''}" />
                  <span>Video Review</span>
                  <button
                    type="button"
                    onclick={() => toggleVrCard("aka")}
                    class="px-2 py-0.5 rounded font-mono font-bold text-[10px] border transition {reviewing ? 'bg-amber-500/20 border-amber-500 text-amber-300 animate-pulse' : hasVr1 ? 'bg-rose-600/20 border-rose-500/60 text-rose-300' : 'bg-emerald-600/15 border-emerald-500/50 text-emerald-300'}"
                    title="Klik untuk mengoreksi status kartu VR secara manual"
                  >
                    {reviewing ? "DITINJAU" : hasVr1 ? "HANGUS" : "TERSEDIA"}
                  </button>
                </span>
                {#if !reviewing}
                  <button
                    type="button"
                    onclick={() => requestVr("aka")}
                    disabled={hasVr1}
                    class="h-9 px-3 rounded-lg text-[11px] font-bold font-mono transition border disabled:opacity-40 disabled:cursor-not-allowed bg-[#1e2029] border-white/10 text-zinc-200 hover:text-white hover:border-white/30"
                    title="Ajukan Video Review (timer dijeda)"
                  >
                    Ajukan VR
                  </button>
                {/if}
              </div>
              {#if reviewing}
                {@render vrDecisionButtons("aka")}
              {/if}
            </div>
          {/if}

          <!-- Team score (beregu) + medical time, under the VR row -->
          <div class="mt-3 cp-extras">
            {@render cornerExtras("aka")}
          </div>
        </div>
      </div>
    </div>
  {/if}

</div>

<!-- ==================== DISCORD-STYLED KATA SELECT MODAL ==================== -->
{#if isKataModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-xs p-4">
    <button
      type="button"
      class="absolute inset-0 w-full h-full cursor-default"
      onclick={() => (isKataModalOpen = false)}
      tabindex="-1"
      aria-label="Tutup modal"
    ></button>

    <div class="relative w-full max-w-[500px] bg-[#181a20] border border-[#2b2d31] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[85vh] z-10 animate-in fade-in zoom-in-95 duration-100">
      <!-- Modal Header -->
      <div class="p-4 border-b border-[#2b2d31] flex items-center justify-between bg-[#14161b]">
        <div>
          <h3 class="text-sm font-black text-white uppercase tracking-wider flex items-center gap-2">
            <span class="px-2 py-0.5 text-[10px] rounded font-mono font-bold {kataModalTarget === 'aka' ? 'bg-red-600 text-white' : 'bg-blue-600 text-white'}">
              {kataModalTarget === "aka" ? "AKA • SUDUT MERAH" : "AO • SUDUT BIRU"}
            </span>
            <span>Pilih Gerakan Kata (WKF)</span>
          </h3>
          <p class="text-[11px] text-zinc-400 mt-0.5">Ketik nomor kode (e.g. 99) atau nama kata untuk mencari.</p>
        </div>
        <button
          type="button"
          onclick={() => (isKataModalOpen = false)}
          aria-label="Tutup modal"
          class="w-7 h-7 flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/10 rounded-lg transition"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <!-- Search Input Box -->
      <div class="p-3 bg-[#111216] border-b border-[#2b2d31]">
        <div class="relative flex items-center">
          <svg class="w-4 h-4 text-zinc-400 absolute left-3 pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            bind:this={searchInputEl}
            type="text"
            bind:value={kataSearchQuery}
            placeholder="Cari kode nomor (99) atau nama (Unsu)..."
            class="w-full h-10 pl-9 pr-8 bg-[#181a20] border border-[#2b2d31] focus:border-[#5865f2] focus:ring-2 focus:ring-[#5865f2]/30 text-white text-xs font-semibold rounded-xl outline-none transition placeholder-zinc-500"
          />
          {#if kataSearchQuery}
            <button
              type="button"
              onclick={() => (kataSearchQuery = "")}
              aria-label="Bersihkan pencarian"
              class="absolute right-2.5 w-5 h-5 flex items-center justify-center text-zinc-400 hover:text-white"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
            </button>
          {/if}
        </div>
      </div>

      <!-- Kata Items List -->
      <div class="flex-1 overflow-y-auto p-2 divide-y divide-[#2b2d31]/40 scrollbar-thin max-h-[380px]">
        {#if filteredKataList.length === 0}
          <div class="p-8 text-center text-zinc-500 text-xs font-medium">
            Tidak ada kata yang cocok dengan "{kataSearchQuery}"
          </div>
        {:else}
          {#each filteredKataList as item, idx}
            <button
              type="button"
              onclick={() => selectKataItem(item)}
              class="w-full px-3 py-2.5 rounded-lg flex items-center justify-between text-left transition {idx === kataModalSelectedIndex ? 'bg-[#5865f2]/20 border border-[#5865f2] text-white' : 'hover:bg-white/[0.04] text-zinc-300'}"
            >
              <div class="flex items-center space-x-2.5">
                <span class="px-2 py-0.5 bg-[#111216] border border-[#2b2d31] text-amber-400 font-mono font-bold text-xs rounded">
                  {item.code}
                </span>
                <span class="font-bold text-sm text-white">
                  {item.name}
                </span>
              </div>
              <span class="text-[11px] font-mono text-zinc-500">
                WKF #{item.code}
              </span>
            </button>
          {/each}
        {/if}
      </div>

      <!-- Modal Footer -->
      <div class="p-2.5 bg-[#14161b] border-t border-[#2b2d31] flex items-center justify-between text-[11px] text-zinc-400 font-mono">
        <span>Gunakan tombol ↑ ↓ dan Enter untuk navigasi</span>
        <button
          type="button"
          onclick={() => {
            if (kataModalTarget === "aka") kataAkaName = "";
            else kataAoName = "";
            kataFromLo = { ...kataFromLo, [kataModalTarget]: false };
            isKataModalOpen = false;
            broadcastLiveState();
          }}
          class="text-red-400 hover:underline font-bold"
        >
          Kosongkan Pilihan
        </button>
      </div>
    </div>
  </div>
{/if}


<!-- ==================== ROSTER ATLET PARTAI MODAL ==================== -->
<!-- One list per corner (AO left, AKA right as on this panel): members of the team
     category + kontingen first, other athletes of the kontingen folded below. -->
{#snippet rosterColumn(side: "aka" | "ao")}
  {@const info = teamMatchInfo!}
  {@const all = side === "aka" ? info.candidate_atlets_aka : info.candidate_atlets_ao}
  {@const members = all.filter((c) => c.same_nomor)}
  {@const others = all.filter((c) => !c.same_nomor)}
  {@const selectedId = side === "aka" ? rosterAkaId : rosterAoId}
  {@const currentName = side === "aka" ? match.atlet1_nama : match.atlet2_nama}
  {@const selected = all.find((c) => c.id === selectedId)}
  <div class="min-w-0 flex flex-col rounded-xl border p-3 {side === 'aka' ? 'bg-red-950/20 border-red-500/30' : 'bg-blue-950/20 border-blue-500/30'}">
    <div class="flex items-center justify-between gap-2 mb-2">
      <span class="px-2 py-0.5 rounded text-[10px] font-mono font-black text-white {side === 'aka' ? 'bg-red-600' : 'bg-blue-600'}">{side === "aka" ? "AKA" : "AO"}</span>
      <span class="text-[11px] font-bold text-zinc-300 truncate">{side === "aka" ? info.team_aka_name : info.team_ao_name}</span>
    </div>
    <div class="text-[11px] text-zinc-400 mb-2 truncate">
      Dipilih: <span class="font-bold text-white">{selected?.nama_atlet ?? currentName ?? "-"}</span>
    </div>
    <div class="flex-1 min-h-0 max-h-[46vh] overflow-y-auto flex flex-col gap-1.5 pr-0.5">
      {#if all.length === 0}
        <div class="text-xs text-zinc-400 py-6 text-center">Tidak ada atlet terdaftar untuk kontingen ini.</div>
      {:else}
        {#if members.length === 0}
          <div class="text-[11px] text-amber-300/90 px-1 pb-1">Belum ada anggota terdaftar di nomor tanding ini.</div>
        {/if}
        {#each members as cand (cand.id)}
          {@render rosterOption(side, cand, selectedId === cand.id)}
        {/each}
        {#if others.length > 0}
          <details class="group" open={members.length === 0}>
            <summary class="cursor-pointer select-none text-[11px] font-bold text-zinc-400 hover:text-zinc-200 px-1 py-1.5">
              Atlet lain di kontingen ({others.length})
            </summary>
            <div class="flex flex-col gap-1.5 pt-1">
              {#each others as cand (cand.id)}
                {@render rosterOption(side, cand, selectedId === cand.id)}
              {/each}
            </div>
          </details>
        {/if}
      {/if}
    </div>
  </div>
{/snippet}

{#snippet rosterOption(side: "aka" | "ao", cand: TeamCandidateAthlete, active: boolean)}
  {@const played = rosterPlayedIn(side, cand.id)}
  <button
    type="button"
    onclick={() => (side === "aka" ? (rosterAkaId = cand.id) : (rosterAoId = cand.id))}
    aria-pressed={active}
    class="w-full text-left px-3 py-2 rounded-lg border transition flex items-center gap-2 {active
      ? (side === 'aka' ? 'bg-red-600 border-red-300 text-white shadow-md' : 'bg-blue-600 border-blue-300 text-white shadow-md')
      : 'bg-[#111216] border-white/10 text-zinc-200 hover:border-white/30 hover:bg-[#1e2029]'}"
  >
    <span class="min-w-0 flex-1">
      <span class="block text-xs font-black uppercase truncate">{cand.nama_atlet}</span>
      {#if cand.perguruan_nama}
        <span class="block text-[10px] truncate {active ? 'text-white/80' : 'text-zinc-500'}">{cand.perguruan_nama}</span>
      {/if}
    </span>
    {#if played.length > 0}
      <span class="shrink-0 px-1.5 py-0.5 rounded text-[9px] font-mono font-bold {active ? 'bg-black/30 text-white' : 'bg-amber-500/15 text-amber-300 border border-amber-500/30'}" title="Sudah bertanding di partai {played.join(', ')}">P{played.join(", P")}</span>
    {/if}
    {#if active}
      <svg class="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7"/></svg>
    {/if}
  </button>
{/snippet}

{#if isRosterModalOpen && teamMatchInfo}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-xs p-4">
    <div class="relative w-full max-w-3xl max-h-[92vh] bg-[#181a20] border border-white/10 rounded-2xl shadow-2xl overflow-hidden flex flex-col">
      <div class="px-5 py-4 border-b border-[#2b2d31] flex items-center justify-between bg-[#14161b] shrink-0">
        <h3 class="text-sm font-black text-white uppercase tracking-wider">
          Atur Atlet Partai #{teamMatchInfo.active_bout_round || match.urutan || 1}
        </h3>
        <button type="button" onclick={() => (isRosterModalOpen = false)} aria-label="Tutup modal roster" class="w-7 h-7 flex items-center justify-center text-zinc-400 hover:text-white rounded-lg">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="p-4 grid grid-cols-2 gap-3 min-h-0 overflow-hidden">
        {@render rosterColumn("ao")}
        {@render rosterColumn("aka")}
      </div>

      <div class="px-4 py-3 bg-[#14161b] border-t border-[#2b2d31] flex items-center justify-end gap-2 shrink-0">
        <button
          type="button"
          onclick={() => (isRosterModalOpen = false)}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleSaveRoster}
          disabled={isSavingRoster}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition disabled:opacity-50"
        >
          {isSavingRoster ? "Menyimpan..." : "Simpan Atlet"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ==================== FINISH TEAM MATCH MODAL ==================== -->
{#if isFinishTeamModalOpen && teamMatchInfo}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-xs p-4">
    <div class="relative w-full max-w-lg bg-[#181a20] border border-[#2b2d31] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
      <div class="p-4 border-b border-[#2b2d31] flex items-center justify-between bg-[#14161b]">
        <div>
          <h3 class="text-sm font-black text-white uppercase tracking-wider">
            Konfirmasi Penyelesaian Pertandingan Beregu
          </h3>
          <p class="text-xs text-zinc-400 mt-0.5">Finalisasi skor tim dan pemenang yang lolos di bagan kejuaraan.</p>
        </div>
        <button type="button" onclick={() => (isFinishTeamModalOpen = false)} aria-label="Tutup modal finalisasi" class="w-7 h-7 flex items-center justify-center text-zinc-400 hover:text-white rounded-lg">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="p-5 space-y-4">
        <!-- Summary Scorecard -->
        <div class="grid grid-cols-2 gap-3">
          <div class="bg-red-950/30 border-2 {selectedTeamWinner === '1' ? 'border-red-500 shadow-md shadow-red-500/20' : 'border-red-500/30'} rounded-xl p-3 text-center">
            <span class="text-[10px] font-mono font-bold text-red-400 uppercase">TIM AKA</span>
            <div class="text-sm font-black text-white uppercase mt-0.5 truncate">{teamMatchInfo.team_aka_name}</div>
            <div class="text-3xl font-mono font-black text-red-400 mt-2">{teamMatchInfo.team_aka_score}</div>
            <div class="text-[11px] font-mono text-zinc-400">{teamMatchInfo.team_aka_points} Total Poin</div>
          </div>

          <div class="bg-blue-950/30 border-2 {selectedTeamWinner === '2' ? 'border-blue-500 shadow-md shadow-blue-500/20' : 'border-blue-500/30'} rounded-xl p-3 text-center">
            <span class="text-[10px] font-mono font-bold text-blue-400 uppercase">TIM AO</span>
            <div class="text-sm font-black text-white uppercase mt-0.5 truncate">{teamMatchInfo.team_ao_name}</div>
            <div class="text-3xl font-mono font-black text-blue-400 mt-2">{teamMatchInfo.team_ao_score}</div>
            <div class="text-[11px] font-mono text-zinc-400">{teamMatchInfo.team_ao_points} Total Poin</div>
          </div>
        </div>

        <!-- Winner Selection Option -->
        <div>
          <div class="block text-xs font-mono font-bold text-zinc-300 mb-2">TENTUKAN TIM PEMENANG AKHIR:</div>
          <div class="grid grid-cols-2 gap-2">
            <button
              type="button"
              onclick={() => (selectedTeamWinner = "1")}
              class="py-3 px-3 rounded-xl border text-xs font-bold transition flex items-center justify-center gap-1.5 {selectedTeamWinner === '1' ? 'bg-red-600 text-white border-red-300 shadow-lg shadow-red-600/40' : 'bg-[#111216] border-[#2b2d31] text-zinc-400 hover:text-white'}"
            >
              <span>AKA ({teamMatchInfo.team_aka_name})</span>
            </button>
            <button
              type="button"
              onclick={() => (selectedTeamWinner = "2")}
              class="py-3 px-3 rounded-xl border text-xs font-bold transition flex items-center justify-center gap-1.5 {selectedTeamWinner === '2' ? 'bg-blue-600 text-white border-blue-300 shadow-lg shadow-blue-600/40' : 'bg-[#111216] border-[#2b2d31] text-zinc-400 hover:text-white'}"
            >
              <span>AO ({teamMatchInfo.team_ao_name})</span>
            </button>
          </div>
        </div>
      </div>

      <div class="p-3 bg-[#14161b] border-t border-[#2b2d31] flex items-center justify-end gap-2">
        <button
          type="button"
          onclick={() => (isFinishTeamModalOpen = false)}
          class="px-4 py-2 bg-[#1e2029] hover:bg-[#2b2d31] text-zinc-300 text-xs font-bold rounded-lg transition"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={handleConfirmFinishTeam}
          class="px-5 py-2 bg-gradient-to-r from-emerald-600 to-emerald-700 hover:from-emerald-500 hover:to-emerald-600 text-white text-xs font-black rounded-lg transition shadow-md flex items-center gap-1.5"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/></svg>
          <span>Finalisasi & Loloskan Pemenang</span>
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Board preview in the kumite center column: as large as fits, 16:9, never taller
     than the space the controls leave (no scrolling in the center column) */
  .cp-preview {
    aspect-ratio: 16 / 9;
    width: min(100cqw, calc(100cqh * 16 / 9));
  }

  /* Kata just filled in by the LO Kata tablet */
  .cp-kata-flash {
    animation: cp-kata-flash 2.4s ease-out;
  }
  @keyframes cp-kata-flash {
    0%, 30% { background-color: rgba(168, 85, 247, 0.45); color: #ffffff; }
    100% { background-color: transparent; }
  }

  /* Short screens (small operator laptops): compact AO / AKA cards so the whole
     card fits without scrolling. Component CSS is unlayered, so it wins over the
     Tailwind utilities on the same elements. */
  @media (max-height: 760px) {
    .cp-pod { padding: 0.75rem; }
    .cp-pod-head { padding-bottom: 0.5rem; margin-bottom: 0.5rem; }
    .cp-pod-name { font-size: 1.125rem; line-height: 1.25; }
    .cp-score-row { margin-bottom: 0.625rem; }
    .cp-score-box { height: 4.25rem; }
    .cp-score-num { font-size: 3.25rem; line-height: 1; }
    .cp-senshu { width: 4.25rem; height: 4.25rem; }
    .cp-senshu-icon { width: 1.25rem; height: 1.25rem; }
    .cp-deck { margin-bottom: 0.625rem; }
    .cp-deck > button { height: 2.5rem; }
    .cp-pen { padding: 0.5rem; margin-bottom: 0.5rem; }
    .cp-pen > div:first-child { margin-bottom: 0.25rem; }
    .cp-pen button { height: 2.25rem; }
    .cp-extras { margin-top: 0.5rem; }
  }
</style>
