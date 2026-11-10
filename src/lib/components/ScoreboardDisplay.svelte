<script lang="ts">
  // The arena scoreboard itself, shared by the TV page (/scoreboard) and the
  // control panel preview. It only draws live state it is given; fetching that
  // state (Tauri IPC, LAN socket, BroadcastChannel) is the caller's job.
  // Everything is sized in cqh (% of this component's own height), so the same
  // template scales from a full TV down to a small preview box.
  import { onDestroy, untrack } from "svelte";
  import { playWarningSound, playFinishSound } from "$lib/sounds";
  import { JUDGE_KEYS } from "$lib/judgeSticks";
  import "@fontsource/oswald/500.css";
  import "@fontsource/oswald/700.css";
  import { fly, fade } from "svelte/transition";
  import { backOut, cubicInOut, cubicOut } from "svelte/easing";

  // Winner banner reveal: AKA wipes down from the top (its card), AO wipes up
  // from the bottom, a draw opens from the centre. Closing plays it in reverse.
  function wipe(node: Element, { from = "top", duration = 750 }: { from?: "top" | "bottom" | "center"; duration?: number } = {}) {
    return {
      duration,
      easing: cubicInOut,
      css: (t: number) => {
        const r = (1 - t) * 100;
        const clip =
          from === "top" ? `inset(0 0 ${r}% 0)` :
          from === "bottom" ? `inset(${r}% 0 0 0)` :
          `inset(0 ${r / 2}% 0 ${r / 2}%)`;
        return `clip-path: ${clip};`;
      },
    };
  }

  // Action banner (point, penalty, senshu, alarm): pops in from slightly small
  // and fades in; closing plays the same in reverse (shrinks a little and fades)
  function bannerPop(node: Element, { duration = 280 }: { duration?: number } = {}) {
    return {
      duration,
      easing: cubicOut,
      css: (t: number) => `opacity: ${t}; transform: scale(${0.9 + t * 0.1});`,
    };
  }

  // Preparation countdown pop up: springs in with a slight overshoot and
  // shrinks away the same way when closed
  function prepPop(node: Element, { duration = 450 }: { duration?: number } = {}) {
    return {
      duration,
      easing: backOut,
      css: (t: number) => `opacity: ${Math.min(1, t * 1.6)}; transform: scale(${0.82 + t * 0.18});`,
    };
  }

  // Content of the winner banner rises in one row after another
  const vh = (n: number) => (frameHeight * n) / 100;
  const riseIn = (i: number) => ({ y: vh(6), duration: 650, delay: 380 + i * 140, easing: cubicOut });

  // Shrinks a single-line label (athlete name, contingent) until it fits its
  // column, down to 55% of its CSS size, so long names stay as large as possible
  // instead of being cut off early. Re-runs when the text or the layout changes.
  function fitText(node: HTMLElement, _text: unknown) {
    const fit = () => {
      node.style.fontSize = "";
      const max = parseFloat(getComputedStyle(node).fontSize);
      const min = max * 0.55;
      let size = max;
      while (node.scrollWidth > node.clientWidth + 1 && size > min) {
        size -= max * 0.04;
        node.style.fontSize = `${size}px`;
      }
    };
    const ro = new ResizeObserver(fit);
    if (node.parentElement) ro.observe(node.parentElement);
    document.fonts?.ready.then(fit);
    fit();
    return {
      // run after the new text is in the DOM
      update() { setTimeout(fit, 0); },
      destroy() { ro.disconnect(); },
    };
  }

  // Like fitText, but lets the text wrap onto up to two lines first; only
  // shrinks when it still needs a third line or a single word is too wide.
  function fitTwoLines(node: HTMLElement, _text: unknown) {
    const fit = () => {
      node.style.fontSize = "";
      const max = parseFloat(getComputedStyle(node).fontSize);
      const min = max * 0.5;
      let size = max;
      const tooBig = () => {
        const lh = parseFloat(getComputedStyle(node).lineHeight) || size;
        // Oswald's tall ascenders overflow the line box a little; allow for that
        return node.scrollHeight > lh * 2.4 || node.scrollWidth > node.clientWidth + 1;
      };
      while (tooBig() && size > min) {
        size -= max * 0.04;
        node.style.fontSize = `${size}px`;
      }
    };
    const ro = new ResizeObserver(fit);
    if (node.parentElement) ro.observe(node.parentElement);
    document.fonts?.ready.then(fit);
    fit();
    return {
      // run after the new text is in the DOM
      update() { setTimeout(fit, 0); },
      destroy() { ro.disconnect(); },
    };
  }

  let {
    liveState = null,
    connected = false,
    muted = false,
    tatami = 1,
  }: {
    /** Latest live payload from the control panel; a new object per update */
    liveState?: any;
    /** Shows the green sync dot */
    connected?: boolean;
    /** Preview mode: no alarm, buzzer or VR chime sounds */
    muted?: boolean;
    /** Tatami shown in the header until the live state says otherwise */
    tatami?: number;
  } = $props();

  // Height of the frame in px, for JS animations measured in "vh" of the board
  let frameHeight = $state(0);
  let tatamiNumber = $state(1);
  $effect.pre(() => {
    if (!liveState?.tatami) tatamiNumber = tatami;
  });

  // Callers either pass liveState, or push each message through apply()
  // (keeps partial updates that arrive in the same tick)
  export function apply(data: any) {
    applyLiveState(data);
  }
  // Only liveState itself may trigger this effect. applyLiveState reads and
  // writes the board's own state, so it must run untracked; otherwise every
  // write re-runs the effect and it loops until Svelte aborts it.
  $effect(() => {
    const data = liveState;
    if (data) untrack(() => applyLiveState(data));
  });

  // Match information state
  let eventName = $state("");
  let categoryName = $state("KATA PERORANGAN SENIOR PUTRA");
  let matchRound = $state("BABAK PENYISIHAN");
  let matchNumber = $state("1");
  let isKata = $state(true);
  let matchWinner = $state("");
  // Full-screen winner banner: "1" AKA, "2" AO, "3" draw (hikiwake), "" closed.
  // winnerView keeps the last shown side so the banner keeps its content while
  // its closing animation plays.
  let winnerBanner = $state("");
  let winnerView = $state<"1" | "2" | "3">("1");

  // Kata preparation countdown (35 s). The board counts down on its own from
  // the remaining time it is sent and only re-syncs when it drifts.
  // Judge stick test screen (mouse mode): which of the 24 buttons have been pressed
  let stickCheck = $state<{ tested: string[]; last: string | null } | null>(null);
  let stickCheckView = $derived(stickCheck ?? { tested: [], last: null });

  // kind "kata" = preparation (35 s), "medis" = kumite medical / injury time (3 min)
  type PrepInfo = { id: number; side: "aka" | "ao"; kind: "kata" | "medis"; durationMs: number };
  let prep = $state<PrepInfo | null>(null);
  // Last countdown shown, so the pop up keeps its content while it closes
  let prepView = $state<PrepInfo>({ id: 0, side: "aka", kind: "kata", durationMs: 35000 });
  let prepRemainingMs = $state(0);
  let prepEndAt = 0;
  let prepFrame: number | null = null;
  let prepBuzzed = false;

  function runPrepLoop() {
    prepFrame = null;
    if (!prep) return;
    prepRemainingMs = Math.max(0, prepEndAt - performance.now());
    if (prepRemainingMs <= 0) {
      if (!prepBuzzed) {
        prepBuzzed = true;
        playEndBuzzer();
      }
      return;
    }
    prepFrame = requestAnimationFrame(runPrepLoop);
  }

  function applyPrepCountdown(p: any) {
    if (p && p.id) {
      const isNew = !prep || prep.id !== p.id;
      const remaining = Math.max(0, Number(p.remainingMs) || 0);
      const wantEnd = performance.now() + remaining;
      prep = {
        id: p.id,
        side: p.side === "ao" ? "ao" : "aka",
        kind: p.kind === "medis" ? "medis" : "kata",
        durationMs: Number(p.durationMs) || 35000,
      };
      prepView = prep;
      if (isNew || Math.abs(wantEnd - prepEndAt) > 250) prepEndAt = wantEnd;
      // Opened after it already ran out: show "time up" without ringing again
      if (isNew) prepBuzzed = remaining <= 0;
      prepRemainingMs = Math.max(0, prepEndAt - performance.now());
      if (prepFrame === null) prepFrame = requestAnimationFrame(runPrepLoop);
    } else {
      prep = null;
      if (prepFrame !== null) cancelAnimationFrame(prepFrame);
      prepFrame = null;
    }
  }

  let prepDigits = $derived.by(() => {
    const ms = Math.max(0, Math.ceil(prepRemainingMs / 10) * 10);
    const s = Math.floor(ms / 1000);
    const hs = Math.floor((ms % 1000) / 10);
    return { sec: String(s).padStart(2, "0"), hs: String(hs).padStart(2, "0") };
  });
  // Medical time is long: shown as M:SS (rounded up, so it starts at 3:00)
  let prepClock = $derived.by(() => {
    const s = Math.max(0, Math.ceil(prepRemainingMs / 1000));
    return { min: String(Math.floor(s / 60)), sec: String(s % 60).padStart(2, "0") };
  });
  let prepSecondsLabel = $derived(
    prepView.durationMs % 60000 === 0 ? `${prepView.durationMs / 60000} MENIT` : `${Math.round(prepView.durationMs / 1000)} DETIK`
  );
  let isMatchFinished = $state(false);
  let showKataScoreOverlay = $state(false);
  // Big kata totals read 0 until the operator turns them on in the control panel
  let showKataTotal = $state(false);
  // Holds the kata total back while the 5 judge scores are still being revealed.
  // Matches the CSS: slot opens, first judge at 0.6s, +0.5s each, 0.6s animation.
  const JUDGE_REVEAL_MS = 600 + 4 * 500 + 600 + 200;
  let holdKataTotal = $state(false);
  let holdKataTotalTimer: ReturnType<typeof setTimeout> | null = null;

  // Split-Second Scoreboard Banner Notification
  interface ScoreboardBannerEntry {
    side: "aka" | "ao";
    title: string;
    subtitle: string;
    /** judge flags to draw (judge stick points) */
    judges?: number;
  }
  interface ScoreboardBannerData extends ScoreboardBannerEntry {
    id: number;
    durationMs?: number;
    /** one or two corners side by side (AKA and AO in the same exchange) */
    entries?: ScoreboardBannerEntry[];
  }
  let currentBanner = $state<ScoreboardBannerData | null>(null);
  // Last banner shown, so its text stays put while it animates out
  let lastBanner: ScoreboardBannerData | null = null;
  let bannerView = $derived.by(() => {
    if (currentBanner) lastBanner = currentBanner;
    return currentBanner ?? lastBanner;
  });
  // Columns to draw: AO on the left, AKA on the right (as in Ambrilindo Local)
  let bannerEntries = $derived.by((): ScoreboardBannerEntry[] => {
    const b = bannerView;
    if (!b) return [];
    const list = Array.isArray(b.entries) && b.entries.length > 0 ? b.entries : [b];
    return [...list].sort((x, y) => (x.side === y.side ? 0 : x.side === "ao" ? -1 : 1));
  });
  let bannerDismissTimer: any = null;

  // Video Review (VR) Subsystem Overlay state
  interface ScoreboardVrChallenge {
    side: "aka" | "ao";
    reason: string;
    status: "in_progress" | "accepted" | "rejected" | "minai";
    /** points awarded when accepted */
    points?: number;
  }
  // One request per corner: AKA and AO can be under review at the same time
  let vrRequests = $state<{ aka: ScoreboardVrChallenge | null; ao: ScoreboardVrChallenge | null }>({ aka: null, ao: null });
  // Cards to draw: AO on the left, AKA on the right
  let vrCards = $derived([vrRequests.ao, vrRequests.aka].filter((r): r is ScoreboardVrChallenge => !!r));
  let vrSafetyTimer: any = null;
  let vrChimeTimer: any = null;
  // VR logo next to the score while a corner still has its card (kumite with VR only)
  let vrEnabled = $state(false);
  let akaVrUsed = $state(false);
  let aoVrUsed = $state(false);

  // Team Kumite (Kumite Beregu) state
  let isTeamMatch = $state(false);
  let teamAkaName = $state("");
  let teamAoName = $state("");
  // Hantei: "HANTEI" screen while the 4 judges vote (result comes as a banner)
  let hanteiMode = $state(false);
  let teamAkaScore = $state(0);
  let teamAoScore = $state(0);
  let teamDrawCount = $state(0);
  let teamAkaPoints = $state(0);
  let teamAoPoints = $state(0);
  let activeBoutRound = $state(1);
  let totalBouts = $state(5);

  // Timer state (with high-precision milliseconds)
  let timerMs = $state(120000);
  let isTimerRunning = $state(false);

  // Alarm threshold options (synced from Control Panel)
  let alarmSettings = $state({
    alarm30s: false,
    alarm15s: true, // Default ON
    alarm0s: true,  // Default ON
  });
  let alarmTriggered30s = false;
  let alarmTriggered15s = false;
  let alarmTriggered0s = false;

  function resetScoreboardAlarmFlags() {
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

  function playWarningAlarm(seconds: number = 15) {
    if (muted) return;
    try {
      playWarningSound();
    } catch {}
  }

  function playEndBuzzer() {
    if (muted) return;
    try {
      playFinishSound();
    } catch {}
  }
  let animFrameId: number | null = null;
  let lastTickTimestamp: number = 0;

  // AKA (Red) combatant state
  let akaName = $state("SUDUT MERAH (AKA)");
  let akaClub = $state("KONTINGEN A");
  let akaKataName = $state("");
  let akaLogo = $state<string | null>(null);
  let akaTotalPoints = $state(0);
  let akaJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let akaScore = $state(0);
  let akaSenshu = $state(false);
  let akaPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // AO (Blue) combatant state
  let aoName = $state("SUDUT BIRU (AO)");
  let aoClub = $state("KONTINGEN B");
  let aoKataName = $state("");
  let aoLogo = $state<string | null>(null);
  let aoTotalPoints = $state(0);
  let aoJudgeScores = $state<string[]>(["", "", "", "", ""]);
  let aoScore = $state(0);
  let aoSenshu = $state(false);
  let aoPenalties = $state({ c1: false, c2: false, c3: false, hc: false, h: false });

  // Formatting minutes:seconds and hundredths
  let timerFormatted = $derived.by(() => {
    const safeMs = Math.max(0, Math.floor(timerMs));
    const totalSec = Math.floor(safeMs / 1000);
    const mins = Math.floor(totalSec / 60);
    const secs = totalSec % 60;
    const hundredths = Math.floor((safeMs % 1000) / 10);
    const mStr = mins < 10 ? `0${mins}` : `${mins}`;
    const sStr = secs < 10 ? `0${secs}` : `${secs}`;
    const msStr = hundredths < 10 ? `0${hundredths}` : `${hundredths}`;
    return {
      mmss: `${mStr}:${sStr}`,
      msStr,
      full: `${mStr}:${sStr}.${msStr}`,
    };
  });

  function formatLogoUrl(logo: string | null | undefined): string {
    if (!logo) return "/img/idn-flag.png";
    if (logo.startsWith("http://") || logo.startsWith("https://") || logo.startsWith("data:")) {
      return logo;
    }
    const clean = logo.replace(/^[/\\]+/, "");
    return `/media/${clean}`;
  }

  function runScoreboardTimerLoop() {
    if (!isTimerRunning) return;
    const now = performance.now();
    const delta = now - lastTickTimestamp;
    lastTickTimestamp = now;

    const prevMs = timerMs;
    timerMs = Math.max(0, timerMs - delta);

    // Alarm thresholds checking with auto-rearm if time increases
    if (alarmSettings.alarm30s && prevMs > 30000 && timerMs <= 30000 && !alarmTriggered30s) {
      playWarningAlarm(30);
      alarmTriggered30s = true;
      currentBanner = {
        id: Date.now(),
        side: "aka",
        title: "PERINGATAN",
        subtitle: "30 DETIK TERSISA",
        durationMs: 1800,
      };
      if (bannerDismissTimer) clearTimeout(bannerDismissTimer);
      bannerDismissTimer = setTimeout(() => { currentBanner = null; bannerDismissTimer = null; }, 1800);
    } else if (timerMs > 30000) {
      alarmTriggered30s = false;
    }

    if (alarmSettings.alarm15s && prevMs > 15000 && timerMs <= 15000 && !alarmTriggered15s) {
      playWarningAlarm(15);
      alarmTriggered15s = true;
      currentBanner = {
        id: Date.now(),
        side: "aka",
        title: "ATOSHIBARAKU",
        subtitle: "15 DETIK TERSISA",
        durationMs: 1800,
      };
      if (bannerDismissTimer) clearTimeout(bannerDismissTimer);
      bannerDismissTimer = setTimeout(() => { currentBanner = null; bannerDismissTimer = null; }, 1800);
    } else if (timerMs > 15000) {
      alarmTriggered15s = false;
    }

    if (alarmSettings.alarm0s && prevMs > 0 && timerMs <= 0 && !alarmTriggered0s) {
      playEndBuzzer();
      alarmTriggered0s = true;
      currentBanner = {
        id: Date.now(),
        side: "aka",
        title: "WAKTU HABIS",
        subtitle: "PARTAI SELESAI",
        durationMs: 2200,
      };
      if (bannerDismissTimer) clearTimeout(bannerDismissTimer);
      bannerDismissTimer = setTimeout(() => { currentBanner = null; bannerDismissTimer = null; }, 2200);
    } else if (timerMs > 0) {
      alarmTriggered0s = false;
    }

    if (timerMs <= 0) {
      timerMs = 0;
      isTimerRunning = false;
      return;
    }

    animFrameId = requestAnimationFrame(runScoreboardTimerLoop);
  }

  // Which side is ahead, using the score that is actually shown for this mode
  let akaShownScore = $derived(isKata ? akaTotalPoints : akaScore);
  let aoShownScore = $derived(isKata ? aoTotalPoints : aoScore);
  let showJudgePanel = $derived(isKata && showKataScoreOverlay);

  // Data shown on the winner banner (AKA or AO, from winnerView)
  let winnerInfo = $derived.by(() => {
    const aka = winnerView !== "2";
    return {
      side: aka ? "aka" : "ao",
      label: aka ? "AKA" : "AO",
      name: aka ? akaName : aoName,
      club: aka ? akaClub : aoClub,
      kataName: aka ? akaKataName : aoKataName,
      logo: aka ? akaLogo : aoLogo,
      teamName: aka ? teamAkaName : teamAoName,
      score: aka ? akaShownScore : aoShownScore,
      otherScore: aka ? aoShownScore : akaShownScore,
    };
  });
  // Kata scores only appear on the banner when the operator has shown them
  let winnerScoreVisible = $derived(!isKata || (showKataTotal && !holdKataTotal));

  // Kata judges shown in the pop up (3 / 5 / 7, set on the control panel)
  let judgeCount = $state<number>(5);

  // Calculate judge highlights for the judges table
  let judgeDecisions = $derived.by(() => {
    return Array.from({ length: judgeCount }, (_, idx) => idx).map((idx) => {
      const rawAka = akaJudgeScores[idx]?.trim();
      const rawAo = aoJudgeScores[idx]?.trim();
      const vAka = rawAka !== "" && rawAka !== undefined ? parseFloat(rawAka) : NaN;
      const vAo = rawAo !== "" && rawAo !== undefined ? parseFloat(rawAo) : NaN;

      const hasAka = !isNaN(vAka) && ((vAka >= 5.0 && vAka <= 10.0) || vAka === 0);
      const hasAo = !isNaN(vAo) && ((vAo >= 5.0 && vAo <= 10.0) || vAo === 0);

      if (hasAka && hasAo) {
        if (vAka === 0 && vAo > 0) return "ao";
        if (vAo === 0 && vAka > 0) return "aka";
        if (vAka > vAo) return "aka";
        if (vAo > vAka) return "ao";
        return "seri";
      }
      return "empty";
    });
  });

  function applyLiveState(data: any) {
    if (!data) return;
    if (data.tatami) tatamiNumber = Number(data.tatami);
    const wasJudgePanel = isKata && showKataScoreOverlay;
    const wasTotalVisible = showKataTotal && !holdKataTotal;
    if (data.category) categoryName = data.category;
    if ([3, 5, 7].includes(Number(data.judgeCount))) judgeCount = Number(data.judgeCount);
    if (data.round) matchRound = data.round;
    if (data.matchNumber !== undefined) matchNumber = String(data.matchNumber);
    if (data.eventName) eventName = data.eventName;
    if (data.alarmSettings) {
      alarmSettings = {
        alarm30s: data.alarmSettings.alarm30s ?? false,
        alarm15s: data.alarmSettings.alarm15s ?? true,
        alarm0s: data.alarmSettings.alarm0s ?? true,
      };
    }
    
    // 1. Process running state first so cancelAnimationFrame happens immediately on pause
    if (data.isTimerRunning !== undefined) {
      const wasRunning = isTimerRunning;
      isTimerRunning = data.isTimerRunning;
      if (!wasRunning && isTimerRunning) {
        lastTickTimestamp = performance.now();
        animFrameId = requestAnimationFrame(runScoreboardTimerLoop);
      } else if (!isTimerRunning && animFrameId !== null) {
        cancelAnimationFrame(animFrameId);
        animFrameId = null;
      }
    }

    // 2. Process timerMs: when paused, always snap immediately to authoritative value
    if (data.timerMs !== undefined) {
      if (isTimerRunning) {
        if (Math.abs(timerMs - data.timerMs) > 150) {
          timerMs = data.timerMs;
        }
      } else {
        timerMs = data.timerMs;
      }
      resetScoreboardAlarmFlags();
    } else if (data.timerSeconds !== undefined) {
      if (!isTimerRunning) {
        timerMs = data.timerSeconds * 1000;
        resetScoreboardAlarmFlags();
      }
    }

    if (data.isKata !== undefined) isKata = data.isKata;
    if (data.winner !== undefined) matchWinner = data.winner;
    if (data.prepCountdown !== undefined) applyPrepCountdown(data.prepCountdown);
    if (data.hanteiMode !== undefined) hanteiMode = !!data.hanteiMode;
    if (data.stickCheck !== undefined) {
      if (data.stickCheck) {
        stickCheck = {
          tested: Array.isArray(data.stickCheck.tested) ? data.stickCheck.tested.map(String) : [],
          last: data.stickCheck.last ?? null,
        };
      } else {
        stickCheck = null;
      }
    }
    if (data.winnerBanner !== undefined) {
      const w = String(data.winnerBanner || "");
      if (w === "1" || w === "2" || w === "3") {
        winnerView = w;
        winnerBanner = w;
      } else {
        winnerBanner = "";
      }
    }
    if (data.isFinished !== undefined) isMatchFinished = data.isFinished;
    if (data.showKataScoreOverlay !== undefined) showKataScoreOverlay = data.showKataScoreOverlay;
    if (data.showKataTotal !== undefined) showKataTotal = !!data.showKataTotal;

    // Opening the judge pop up also turns the big kata total on; hold it back
    // until all 5 judge scores have appeared, so the result lands last.
    const isJudgePanel = isKata && showKataScoreOverlay;
    if (isJudgePanel && !wasJudgePanel && showKataTotal && !wasTotalVisible) {
      holdKataTotal = true;
      if (holdKataTotalTimer) clearTimeout(holdKataTotalTimer);
      holdKataTotalTimer = setTimeout(() => {
        holdKataTotal = false;
        holdKataTotalTimer = null;
      }, JUDGE_REVEAL_MS);
    } else if (holdKataTotal && (!isJudgePanel || !showKataTotal)) {
      // Pop up closed early, or the total was hidden: stop holding
      if (holdKataTotalTimer) clearTimeout(holdKataTotalTimer);
      holdKataTotalTimer = null;
      holdKataTotal = false;
    }

    if (data.isTeamMatch !== undefined) isTeamMatch = data.isTeamMatch;
    if (data.teamAkaName !== undefined) teamAkaName = data.teamAkaName;
    if (data.teamAoName !== undefined) teamAoName = data.teamAoName;
    if (data.teamAkaScore !== undefined) teamAkaScore = data.teamAkaScore;
    if (data.teamAoScore !== undefined) teamAoScore = data.teamAoScore;
    if (data.teamDrawCount !== undefined) teamDrawCount = data.teamDrawCount;
    if (data.teamAkaPoints !== undefined) teamAkaPoints = data.teamAkaPoints;
    if (data.teamAoPoints !== undefined) teamAoPoints = data.teamAoPoints;
    if (data.activeBoutRound !== undefined) activeBoutRound = data.activeBoutRound;
    if (data.totalBouts !== undefined) totalBouts = data.totalBouts;

    if (data.aka) {
      if (data.aka.name) akaName = data.aka.name;
      if (data.aka.contingent) akaClub = data.aka.contingent;
      if (data.aka.kataName !== undefined) akaKataName = data.aka.kataName;
      if (data.aka.logo !== undefined) akaLogo = data.aka.logo;
      if (data.aka.score !== undefined) akaScore = data.aka.score;
      if (data.aka.totalPoints !== undefined) akaTotalPoints = data.aka.totalPoints;
      if (data.aka.senshu !== undefined) akaSenshu = data.aka.senshu;
      if (data.aka.penalties) akaPenalties = { ...akaPenalties, ...data.aka.penalties };
      if (data.aka.judgeScores) akaJudgeScores = data.aka.judgeScores;
    }

    if (data.ao) {
      if (data.ao.name) aoName = data.ao.name;
      if (data.ao.contingent) aoClub = data.ao.contingent;
      if (data.ao.kataName !== undefined) aoKataName = data.ao.kataName;
      if (data.ao.logo !== undefined) aoLogo = data.ao.logo;
      if (data.ao.score !== undefined) aoScore = data.ao.score;
      if (data.ao.totalPoints !== undefined) aoTotalPoints = data.ao.totalPoints;
      if (data.ao.senshu !== undefined) aoSenshu = data.ao.senshu;
      if (data.ao.penalties) aoPenalties = { ...aoPenalties, ...data.ao.penalties };
      if (data.ao.judgeScores) aoJudgeScores = data.ao.judgeScores;
    }

    if (data.activeBanner !== undefined) {
      if (data.activeBanner && data.activeBanner.id) {
        if (!currentBanner || currentBanner.id !== data.activeBanner.id) {
          currentBanner = data.activeBanner;
          if (bannerDismissTimer) clearTimeout(bannerDismissTimer);
          bannerDismissTimer = null;
          // durationMs 0 = stays until the control panel clears it (judge stick point)
          if (data.activeBanner.durationMs !== 0) {
            const dur = data.activeBanner.durationMs || 1400;
            bannerDismissTimer = setTimeout(() => {
              currentBanner = null;
              bannerDismissTimer = null;
            }, dur);
          }
        }
      } else if (data.activeBanner === null) {
        currentBanner = null;
        if (bannerDismissTimer) {
          clearTimeout(bannerDismissTimer);
          bannerDismissTimer = null;
        }
      }
    }

    if (data.vrEnabled !== undefined) vrEnabled = !!data.vrEnabled;
    if (data.aka && data.aka.vr !== undefined) akaVrUsed = !!data.aka.vr;
    if (data.ao && data.ao.vr !== undefined) aoVrUsed = !!data.ao.vr;

    // Video Review requests per corner (older control panels send a single one)
    if (data.vrChallenges !== undefined || data.vrChallenge !== undefined) {
      let incoming: { aka: any; ao: any };
      if (data.vrChallenges && typeof data.vrChallenges === "object") {
        incoming = { aka: data.vrChallenges.aka ?? null, ao: data.vrChallenges.ao ?? null };
      } else {
        const single = data.vrChallenge;
        incoming = { aka: null, ao: null };
        if (single) incoming[single.side === "ao" ? "ao" : "aka"] = single;
      }
      const next = { ...vrRequests };
      for (const side of ["aka", "ao"] as const) {
        const req = incoming[side];
        const prev = vrRequests[side];
        if (!req || !["in_progress", "accepted", "rejected", "minai"].includes(req.status)) {
          next[side] = null;
          continue;
        }
        // New request: same sound as the 15 s warning
        if (req.status === "in_progress" && (!prev || prev.status !== "in_progress")) playWarningAlarm();
        next[side] = { side, reason: req.reason || "", status: req.status, points: Number(req.points) || undefined };
      }
      vrRequests = next;
      // Decided results stay while the other corner is still under review. Once none
      // is open, a safety net clears them in case the control panel's clear never arrives.
      if (vrSafetyTimer) clearTimeout(vrSafetyTimer);
      vrSafetyTimer = null;
      const shown = [next.aka, next.ao];
      if (shown.some(Boolean) && !shown.some((r) => r?.status === "in_progress")) {
        vrSafetyTimer = setTimeout(() => {
          vrRequests = { aka: null, ao: null };
          vrSafetyTimer = null;
        }, 4500);
      }
    }
  }

  onDestroy(() => {
    if (animFrameId !== null) cancelAnimationFrame(animFrameId);
    if (bannerDismissTimer) clearTimeout(bannerDismissTimer);
    if (vrChimeTimer) clearTimeout(vrChimeTimer);
    if (vrSafetyTimer) clearTimeout(vrSafetyTimer);
    if (holdKataTotalTimer) clearTimeout(holdKataTotalTimer);
    if (prepFrame !== null) cancelAnimationFrame(prepFrame);
  });
</script>

{#snippet combatantCard(c: {
  side: "aka" | "ao";
  label: string;
  name: string;
  club: string;
  kataName: string;
  logo: string | null;
  score: number;
  otherScore: number;
  senshu: boolean;
  penalties: Record<string, boolean>;
  teamName: string;
})}
  {@const scoreVisible = !isKata || (showKataTotal && !holdKataTotal)}
  {@const shownValue = scoreVisible ? c.score : 0}
  <section
    class="sb-card sb-card-{c.side}"
    class:is-kata={isKata}
    class:has-vr={!isKata && vrEnabled}
  >
    <div class="sb-corner" aria-hidden="true">{c.label}</div>

    <div class="sb-logo-frame">
      <img
        src={formatLogoUrl(c.logo)}
        alt="Logo {c.label}"
        class="sb-logo-media"
        onerror={(e) => { (e.currentTarget as HTMLImageElement).src = "/img/idn-flag.png"; }}
      />
    </div>

    <div class="sb-details">
      {#if isTeamMatch && c.teamName}
        <div class="sb-status-strip">
          <span class="sb-team-name-tag">TIM {c.teamName}</span>
        </div>
      {/if}

      <h1 class="sb-athlete-name" use:fitText={c.name}>{c.name}</h1>
      <p class="sb-club-name" use:fitText={c.club}>{c.club}</p>
      {#if isKata && c.kataName}
        <p class="sb-kata-title" use:fitText={c.kataName}>{c.kataName}</p>
      {/if}
    </div>

    {#if !isKata}
      <div class="sb-penalties" aria-label="Penalti {c.label}">
        {#each ["c1", "c2", "c3", "hc", "h"] as penalty}
          <div class="sb-penalty-item" class:is-active={c.penalties[penalty]}>
            {penalty.toUpperCase()}
          </div>
        {/each}
      </div>
    {/if}

    <!-- Kata totals read 0 until the operator shows them; the real number pops
         in when it is revealed (keyed so the pop replays on each change) -->
    {#if !isKata && vrEnabled}
      <!-- Video Review card, left of the score box (as in Ambrilindo Local). The
           slot stays in both cards so their layout matches; the logo hides once
           the card is used up. -->
      <div class="sb-vr-slot">
        <img
          class="sb-vr-logo"
          class:is-used={c.side === "aka" ? akaVrUsed : aoVrUsed}
          src="/img/vr-{c.side}.png"
          alt="VR"
        />
      </div>
    {/if}

    <div class="sb-score-box" class:is-leading={scoreVisible && c.score > c.otherScore}>
      {#if !isKata && c.senshu}
        <span class="sb-senshu" title="Senshu">S</span>
      {/if}
      {#key shownValue}
        <span class="sb-score-val">{shownValue}</span>
      {/key}
    </div>
  </section>
{/snippet}

<div class="sb-frame" bind:clientHeight={frameHeight}>
<div class="sb-root">
  <!-- Header + arena; the winner banner covers this whole stage, never the footer -->
  <div class="sb-stage">
  {#if winnerBanner}
    {#if winnerView === "3"}
      <!-- Draw (hikiwake) -->
      <div class="sb-winner sb-winner-draw" role="status" transition:wipe|global={{ from: "center" }}>
        <div class="sb-winner-ribbon" in:fly|global={riseIn(0)} out:fade|global={{ duration: 180 }}>HIKIWAKE</div>
        <div class="sb-draw-title" in:fly|global={riseIn(1)} out:fade|global={{ duration: 180 }}>SERI</div>
        <div class="sb-draw-names" in:fly|global={riseIn(2)} out:fade|global={{ duration: 180 }}>
          <span class="sb-draw-chip aka">{akaName}</span>
          {#if winnerScoreVisible}
            <span class="sb-draw-score">{akaShownScore} – {aoShownScore}</span>
          {:else}
            <span class="sb-draw-vs">VS</span>
          {/if}
          <span class="sb-draw-chip ao">{aoName}</span>
        </div>
        <div class="sb-winner-foot" in:fly|global={riseIn(3)} out:fade|global={{ duration: 180 }}>
          {categoryName} · {matchRound} · PARTAI {matchNumber}
        </div>
      </div>
    {:else}
      <div
        class="sb-winner sb-winner-{winnerInfo.side}"
        role="status"
        transition:wipe|global={{ from: winnerView === "1" ? "top" : "bottom" }}
      >
        <div class="sb-winner-shine" aria-hidden="true"></div>

        <div class="sb-winner-head" in:fly|global={riseIn(0)} out:fade|global={{ duration: 180 }}>
          <span class="sb-winner-ribbon">
            <svg class="sb-winner-trophy" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
              <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
            </svg>
            PEMENANG
          </span>
          <span class="sb-winner-corner">{winnerInfo.label}</span>
        </div>

        <div class="sb-winner-body">
          <div class="sb-winner-logo" in:fly|global={riseIn(1)} out:fade|global={{ duration: 180 }}>
            <img
              src={formatLogoUrl(winnerInfo.logo)}
              alt="Logo {winnerInfo.label}"
              onerror={(e) => { (e.currentTarget as HTMLImageElement).src = "/img/idn-flag.png"; }}
            />
          </div>

          <div class="sb-winner-info" in:fly|global={riseIn(2)} out:fade|global={{ duration: 180 }}>
            {#if isTeamMatch && winnerInfo.teamName}
              <div class="sb-winner-team">TIM {winnerInfo.teamName}</div>
            {/if}
            <div class="sb-winner-name" use:fitTwoLines={winnerInfo.name}>{winnerInfo.name}</div>
            <div class="sb-winner-club" use:fitText={winnerInfo.club}>{winnerInfo.club}</div>
            {#if isKata && winnerInfo.kataName}
              <div class="sb-winner-kata" use:fitText={winnerInfo.kataName}>{winnerInfo.kataName}</div>
            {/if}
          </div>

          <!-- Same width as the logo so the name stays centred on the screen -->
          <div class="sb-winner-side" in:fly|global={riseIn(1)} out:fade|global={{ duration: 180 }}>
            {#if winnerScoreVisible}
              <div class="sb-winner-score">
                <span class="sb-winner-score-main">{winnerInfo.score}</span>
                <span class="sb-winner-score-sep">–</span>
                <span class="sb-winner-score-other">{winnerInfo.otherScore}</span>
              </div>
            {/if}
          </div>
        </div>

        <div class="sb-winner-foot" in:fly|global={riseIn(3)} out:fade|global={{ duration: 180 }}>
          {categoryName} · {matchRound} · PARTAI {matchNumber}
        </div>
      </div>
    {/if}
  {/if}

  <!-- ==================== CHECK STICK JURI (mouse mode test) ==================== -->
  {#if stickCheck}
    <div class="sb-check" role="status" transition:prepPop|global>
      <div class="sb-check-head">
        <div class="sb-check-title">CHECK STICK JURI <span class="sb-check-tatami">· TATAMI {tatamiNumber}</span></div>
        <div class="sb-check-count" class:is-done={stickCheckView.tested.length === 24}>{stickCheckView.tested.length} / 24</div>
      </div>
      <div class="sb-check-grid">
        {#each JUDGE_KEYS as keys, j}
          {@const done = keys.filter((k) => stickCheckView.tested.includes(k)).length}
          <div class="sb-check-judge" class:is-done={done === 6}>
            <div class="sb-check-judge-head">
              <span>JURI {j + 1}</span>
              <span class="sb-check-judge-count">{done}/6</span>
            </div>
            {#each [keys.slice(0, 3), keys.slice(3)] as row, r}
              <div class="sb-check-row">
                <span class="sb-check-side sb-check-side-{r === 0 ? 'aka' : 'ao'}">{r === 0 ? "AKA" : "AO"}</span>
                {#each row as k, i}
                  <span
                    class="sb-check-cell sb-check-cell-{r === 0 ? 'aka' : 'ao'}"
                    class:is-hit={stickCheckView.tested.includes(k)}
                    class:is-last={stickCheckView.last === k}
                  >{i + 1} {["YUKO", "WAZA", "IPPON"][i]}</span>
                {/each}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- ==================== COUNTDOWN POP UP: KATA PREPARATION (35 s) / MEDICAL TIME (3 min) ==================== -->
  {#if prep}
    {@const timeUp = prepRemainingMs <= 0}
    <div
      class="sb-prep sb-prep-{prepView.side}"
      class:is-medis={prepView.kind === "medis"}
      class:is-ending={!timeUp && prepRemainingMs <= 10000}
      class:is-up={timeUp}
      role="timer"
      aria-live="polite"
      transition:prepPop|global
    >
      <div class="sb-prep-eyebrow">
        {#if prepView.kind === "medis"}
          <svg class="sb-prep-icon sb-prep-cross" fill="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 3h6v6h6v6h-6v6H9v-6H3V9h6z"/></svg>
          WAKTU MEDIS (CIDERA) · {prepSecondsLabel}
        {:else}
          <svg class="sb-prep-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.4" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>
          WAKTU PERSIAPAN KATA · {prepSecondsLabel}
        {/if}
      </div>
      <div class="sb-prep-athlete">
        <span class="sb-prep-side">{prepView.side === "aka" ? "AKA" : "AO"}</span>
        <span class="sb-prep-name" use:fitText={prepView.side === "aka" ? akaName : aoName}>{prepView.side === "aka" ? akaName : aoName}</span>
      </div>
      {#if prepView.kind === "medis"}
        <div class="sb-prep-digits">{#each prepClock.min.split("") as ch}<span class="sb-tdig">{ch}</span>{/each}<span class="sb-tsep">:</span>{#each prepClock.sec.split("") as ch}<span class="sb-tdig">{ch}</span>{/each}</div>
      {:else}
        <div class="sb-prep-digits">{#each prepDigits.sec.split("") as ch}<span class="sb-tdig">{ch}</span>{/each}<span class="sb-prep-hs"><span class="sb-tsep">.</span>{#each prepDigits.hs.split("") as ch}<span class="sb-tdig">{ch}</span>{/each}</span></div>
      {/if}
      <div class="sb-prep-track">
        <div class="sb-prep-bar" style="width: {Math.max(0, Math.min(100, (prepRemainingMs / prepView.durationMs) * 100))}%"></div>
      </div>
      <div class="sb-prep-status">{timeUp ? "WAKTU HABIS" : prepView.kind === "medis" ? "PEMERIKSAAN MEDIS" : "MOHON SEGERA MEMASUKI MATRAS"}</div>
    </div>
  {/if}

  <!-- ==================== HEADER (for people near the screen) ==================== -->
  <header class="sb-header">
    <div class="sb-tatami-badge">TATAMI {tatamiNumber}</div>

    <div class="sb-header-center">
      <span class="sb-header-cat">{categoryName}</span>
      <span class="sb-header-sep">·</span>
      <span class="sb-header-round">{matchRound}</span>
      <span class="sb-header-sep">·</span>
      <span class="sb-header-partai">PARTAI {matchNumber}</span>
    </div>

    <div class="sb-header-right">
      {#if eventName && eventName !== "AMBRILINDO KARATE CHAMPIONSHIP"}
        <span class="sb-event-name">{eventName}</span>
      {/if}
      <div class="sb-sync-dot" class:is-active={connected} title={connected ? "Live LAN Connected" : "Connecting..."}></div>
    </div>
  </header>

  <!-- ==================== ARENA: AKA (top) / AO (bottom) ==================== -->
  <main class="sb-arena" class:has-judges={showJudgePanel}>
    {#if hanteiMode && !currentBanner}
      <div class="sb-hantei" role="status" transition:bannerPop|global={{ duration: 280 }}>
        <div class="sb-hantei-title">HANTEI</div>
        <div class="sb-hantei-sub">KEPUTUSAN JURI</div>
      </div>
    {/if}
    {#if currentBanner && bannerView}
      <!-- bannerView keeps the last banner's content while it animates out -->
      <div
        class="sb-banner-overlay"
        class:is-dual={bannerEntries.length > 1}
        role="alert"
        aria-live="assertive"
        transition:bannerPop|global={{ duration: 280 }}
      >
        {#each bannerEntries as entry (entry.side)}
          <div
            class="sb-banner-col is-{entry.side}"
            class:has-flags={(entry.judges ?? 0) > 0}
            in:bannerPop={{ duration: 280 }}
          >
            <div class="sb-banner-side-badge">
              <span>{entry.side === "aka" ? "AKA" : "AO"}</span>
              <span class="sb-banner-side-sep">/</span>
              <span>{entry.side === "aka" ? akaName : aoName}</span>
            </div>
            <div class="sb-banner-title">{entry.title}</div>
            <div class="sb-banner-subtitle">{entry.subtitle}</div>
            {#if (entry.judges ?? 0) > 0}
              <!-- One flag per judge who called this point -->
              <div class="sb-banner-flags">
                {#each Array.from({ length: Math.min(4, entry.judges ?? 0) }) as _, i (i)}
                  <img
                    src="/img/{entry.side}_flag.png"
                    alt="Bendera {entry.side === 'aka' ? 'AKA' : 'AO'}"
                    class="sb-banner-flag"
                    style="--i: {i}"
                  />
                {/each}
              </div>
              {#if entry.title !== "HANTEI"}
                <div class="sb-banner-judges">{entry.judges} JURI</div>
              {/if}
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {@render combatantCard({
      side: "aka",
      label: "AKA",
      name: akaName,
      club: akaClub,
      kataName: akaKataName,
      logo: akaLogo,
      score: akaShownScore,
      otherScore: aoShownScore,
      senshu: akaSenshu,
      penalties: akaPenalties,
      teamName: teamAkaName,
    })}

    <!-- Kata judge scores sit between AKA and AO (the timer footer gives up its
         space, kata is not timed). The slot is always in the grid so its row can
         ease open and push AO down smoothly. Judges are revealed one by one; each
         judge's higher score is lit in its corner colour, the other stays readable. -->
    <div class="sb-judges-slot">
    {#if showJudgePanel}
      <div class="sb-judges" aria-label="Skor juri" style="--jcount: {judgeCount}">
        {#each judgeDecisions as decision, idx}
          <div class="sb-judge-pod" style="--i: {idx}">
            <div class="sb-jscore aka" class:leading={decision === "aka"}>
              {akaJudgeScores[idx]?.trim() || "–"}
            </div>
            <div class="sb-jlabel">JURI {idx + 1}</div>
            <div class="sb-jscore ao" class:leading={decision === "ao"}>
              {aoJudgeScores[idx]?.trim() || "–"}
            </div>
          </div>
        {/each}
      </div>
    {/if}
    </div>

    {@render combatantCard({
      side: "ao",
      label: "AO",
      name: aoName,
      club: aoClub,
      kataName: aoKataName,
      logo: aoLogo,
      score: aoShownScore,
      otherScore: akaShownScore,
      senshu: aoSenshu,
      penalties: aoPenalties,
      teamName: teamAoName,
    })}
  </main>
  </div>

  <!-- ==================== FOOTER: TIMER (hidden while the kata judges show) ==================== -->
  <footer class="sb-footer" class:is-collapsed={showJudgePanel} aria-hidden={showJudgePanel}>
      <div class="sb-footer-left">
        <img src="/img/ambrilindo-logo-dark.png" alt="Ambrilindo" class="sb-sponsor-brand" />
      </div>

      <div
        class="sb-timer-vault"
        class:is-running={isTimerRunning}
        class:is-warning={isTimerRunning && timerMs <= 15000 && timerMs > 0}
      >
        <!-- Oswald has no tabular figures ("1" is much narrower than "0"), so every
             character sits in a fixed-width cell; otherwise the timer would change
             width and shift sideways on every tick. -->
        <span class="sb-timer-digits">{#each timerFormatted.mmss.split("") as ch}<span class={ch === ":" ? "sb-tsep" : "sb-tdig"}>{ch}</span>{/each}<span class="sb-timer-ms"><span class="sb-tsep">.</span>{#each timerFormatted.msStr.split("") as ch}<span class="sb-tdig">{ch}</span>{/each}</span></span>
      </div>

      <div class="sb-footer-right">
        {#if isTeamMatch}
          <div class="sb-team-standings">
            <!-- AO left, AKA right: the operator's view of the tatami -->
            <div class="sb-team-meta">BEREGU P{activeBoutRound}/{totalBouts} · POIN {teamAoPoints}–{teamAkaPoints}</div>
            <div class="sb-team-score-display">
              <span class="sb-team-num ao">{teamAoScore}</span>
              <span class="sb-team-num-dash">–</span>
              <span class="sb-team-num aka">{teamAkaScore}</span>
            </div>
          </div>
        {/if}
      </div>
  </footer>

  <!-- ==================== VIDEO REVIEW (VR) OVERLAY: one card per corner ==================== -->
  {#if vrCards.length > 0}
    <div class="sb-vr-backdrop" class:is-dual={vrCards.length > 1}>
      {#each vrCards as vr (vr.side)}
        <div class="sb-vr-card is-{vr.status} sb-vr-side-{vr.side}">
          <div class="sb-vr-pill">
            <span class="sb-vr-dot"></span>
            VIDEO REVIEW
          </div>
          <div class="sb-vr-corner">{vr.side === "aka" ? "AKA" : "AO"}</div>
          <div class="sb-vr-athlete">{vr.side === "aka" ? akaName : aoName}</div>
          {#if vr.status === "in_progress" && (vr.reason || vr.points)}
            <div class="sb-vr-reason">{vr.reason || `${["", "YUKO", "WAZA-ARI", "IPPON"][vr.points ?? 0]} (${vr.points} POINT)`}</div>
          {/if}
          <div class="sb-vr-status">
            {#if vr.status === "in_progress"}
              <span class="sb-vr-pending">SEDANG DITINJAU</span>
            {:else if vr.status === "accepted"}
              <span class="sb-vr-accepted">✓ DITERIMA{vr.points ? ` · ${vr.points} POINT` : ""}</span>
            {:else if vr.status === "minai"}
              <span class="sb-vr-minai">MINAI</span>
            {:else}
              <span class="sb-vr-rejected">✕ DITOLAK</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
</div>

<style>
/* Arena scoreboard, sized for a 30" TV read from ~20 m.
 * Everything scales with the frame height (cqh), never fixed px, so the same
 * template works on the TV and in the control panel preview.
 * At 20 m a glyph needs to be roughly 4–7 cm tall, i.e. 11–19% of a 30" screen's
 * height, so only the score, timer, corner colour and penalty lamps are sized
 * to that; names come next; the header is for people close to the screen. */

/* Size container: every cqh below is a % of this frame's height */
.sb-frame {
  container-type: size;
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.sb-root {
  --sb-bg: #000000;
  --sb-bar-bg: #0a0b0f;

  --sb-aka: #c8102e;
  --sb-aka-deep: #8f0b20;
  --sb-ao: #0b3fbf;
  --sb-ao-deep: #072a80;

  --sb-gold: #facc15;
  --sb-warn: #ff3b3b;
  --sb-white: #ffffff;
  --sb-ink: #0a0a0a;
  --sb-dim: rgba(255, 255, 255, 0.55);
  --sb-line: rgba(255, 255, 255, 0.14);

  --sb-display: "Oswald", "Arial Narrow", "Segoe UI", sans-serif;
  --sb-font: "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;

  height: 100cqh;
  width: 100%;
  overflow: clip;
  display: flex;
  flex-direction: column;
  background-color: var(--sb-bg);
  color: var(--sb-white);
  font-family: var(--sb-font);
  user-select: none;
  box-sizing: border-box;
}

.sb-root *,
.sb-root *::before,
.sb-root *::after {
  box-sizing: border-box;
}

/* Header + arena, the area the winner banner covers */
.sb-stage {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  position: relative;
}

/* ==================== CHECK STICK JURI ==================== */
.sb-check {
  position: absolute;
  inset: 0;
  z-index: 67;
  display: flex;
  flex-direction: column;
  gap: 2cqh;
  padding: 3cqh 4cqh;
  overflow: hidden;
  font-family: var(--sb-display);
  text-transform: uppercase;
  background: radial-gradient(ellipse at 50% 30%, #23262f 0%, #0d0e12 75%);
  border-top: 1cqh solid var(--sb-gold);
  border-bottom: 1cqh solid var(--sb-gold);
}

.sb-check-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 2cqh;
}
.sb-check-title {
  font-size: 7cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-white);
  white-space: nowrap;
}
.sb-check-tatami { color: var(--sb-gold); }
.sb-check-count {
  font-size: 6cqh;
  font-weight: 700;
  line-height: 1;
  padding: 1cqh 2.4cqh;
  border-radius: 1.2cqh;
  background-color: rgba(255, 255, 255, 0.08);
  color: var(--sb-white);
}
.sb-check-count.is-done { background-color: #059669; }

.sb-check-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-template-rows: repeat(2, minmax(0, 1fr));
  gap: 2cqh;
}

.sb-check-judge {
  min-height: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 1.2cqh;
  padding: 1.6cqh 2cqh;
  border-radius: 1.6cqh;
  border: 0.4cqh solid rgba(255, 255, 255, 0.12);
  background-color: rgba(0, 0, 0, 0.3);
}
.sb-check-judge.is-done { border-color: #10b981; }

.sb-check-judge-head {
  display: flex;
  justify-content: space-between;
  font-size: 4.4cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-white);
}
.sb-check-judge-count { color: rgba(255, 255, 255, 0.55); }
.sb-check-judge.is-done .sb-check-judge-count { color: #34d399; }

.sb-check-row {
  display: grid;
  grid-template-columns: 9cqh repeat(3, minmax(0, 1fr));
  gap: 1cqh;
  align-items: center;
}
.sb-check-side {
  font-size: 3.4cqh;
  font-weight: 700;
  line-height: 1;
  text-align: center;
  padding: 0.8cqh 0;
  border-radius: 0.8cqh;
}
.sb-check-side-aka { background-color: var(--sb-aka); }
.sb-check-side-ao { background-color: var(--sb-ao); }

.sb-check-cell {
  font-size: 3.2cqh;
  font-weight: 700;
  line-height: 1;
  text-align: center;
  white-space: nowrap;
  padding: 1cqh 0;
  border-radius: 0.8cqh;
  color: rgba(255, 255, 255, 0.35);
  border: 0.3cqh solid rgba(255, 255, 255, 0.15);
}
.sb-check-cell-aka.is-hit { background-color: var(--sb-aka); border-color: #fca5a5; color: var(--sb-white); }
.sb-check-cell-ao.is-hit { background-color: var(--sb-ao); border-color: #93c5fd; color: var(--sb-white); }
.sb-check-cell.is-last { animation: sbCheckPulse 0.6s ease-in-out infinite alternate; box-shadow: 0 0 0 0.5cqh var(--sb-gold); }
@keyframes sbCheckPulse {
  from { transform: scale(1); }
  to { transform: scale(1.06); }
}

/* ==================== KATA PREPARATION COUNTDOWN ==================== */
.sb-prep {
  position: absolute;
  inset: 0;
  z-index: 66;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2.4cqh;
  padding: 3cqh 5cqh;
  overflow: hidden;
  text-align: center;
  font-family: var(--sb-display);
  text-transform: uppercase;
  border-top: 1cqh solid var(--sb-gold);
  border-bottom: 1cqh solid var(--sb-gold);
  transform-origin: center;
}
.sb-prep-aka { background: radial-gradient(ellipse at 50% 45%, #e0233f 0%, var(--sb-aka) 45%, var(--sb-aka-deep) 100%); }
.sb-prep-ao { background: radial-gradient(ellipse at 50% 45%, #1f5ae0 0%, var(--sb-ao) 45%, var(--sb-ao-deep) 100%); }

.sb-prep-eyebrow {
  display: inline-flex;
  align-items: center;
  gap: 1.4cqh;
  font-size: 5cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-gold);
}
.sb-prep-icon { width: 5cqh; height: 5cqh; }

.sb-prep-athlete {
  display: flex;
  align-items: center;
  gap: 2cqh;
  max-width: 100%;
  min-width: 0;
  padding: 1cqh 3cqh;
  border-radius: 1.6cqh;
  background-color: rgba(0, 0, 0, 0.35);
}
.sb-prep-side {
  flex-shrink: 0;
  font-size: 5.5cqh;
  font-weight: 700;
  line-height: 1;
  padding: 0.8cqh 2cqh;
  border-radius: 1cqh;
  background-color: rgba(255, 255, 255, 0.92);
}
.sb-prep-aka .sb-prep-side { color: var(--sb-aka); }
.sb-prep-ao .sb-prep-side { color: var(--sb-ao); }
.sb-prep-name {
  min-width: 0;
  font-size: 7cqh;
  font-weight: 700;
  line-height: 1.05;
  color: var(--sb-white);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sb-prep-digits {
  font-size: 30cqh;
  font-weight: 700;
  line-height: 0.85;
  color: var(--sb-white);
  white-space: nowrap;
}
.sb-prep-hs { font-size: 0.5em; opacity: 0.85; }
.sb-prep.is-ending .sb-prep-digits { color: var(--sb-gold); }
.sb-prep.is-up .sb-prep-digits { color: #ffd2d2; }

.sb-prep-track {
  width: min(70%, 120cqh);
  height: 1.8cqh;
  border-radius: 9999px;
  background-color: rgba(0, 0, 0, 0.35);
  overflow: hidden;
}
.sb-prep-bar {
  height: 100%;
  border-radius: inherit;
  background-color: var(--sb-white);
}
.sb-prep.is-ending .sb-prep-bar { background-color: var(--sb-gold); }

.sb-prep-status {
  font-size: 4.6cqh;
  font-weight: 700;
  line-height: 1;
  color: rgba(255, 255, 255, 0.92);
}
.sb-prep.is-up .sb-prep-status {
  color: var(--sb-ink);
  background-color: var(--sb-gold);
  padding: 1cqh 3cqh;
  border-radius: 1cqh;
  animation: sbTimerAlert 0.6s infinite alternate;
}

/* ==================== WINNER BANNER ==================== */
.sb-winner {
  position: absolute;
  inset: 0;
  z-index: 65;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  text-align: center;
  gap: 2.4cqh;
  padding: 3cqh 5cqh;
  overflow: hidden;
  font-family: var(--sb-display);
  text-transform: uppercase;
  border-top: 1cqh solid var(--sb-gold);
  border-bottom: 1cqh solid var(--sb-gold);
}
.sb-winner-aka { background: radial-gradient(ellipse at 30% 40%, #e0233f 0%, var(--sb-aka) 45%, var(--sb-aka-deep) 100%); }
.sb-winner-ao { background: radial-gradient(ellipse at 30% 40%, #1f5ae0 0%, var(--sb-ao) 45%, var(--sb-ao-deep) 100%); }
.sb-winner-draw {
  background: radial-gradient(ellipse at 50% 40%, #2a2d36 0%, #121318 70%);
  align-items: center;
  text-align: center;
}

/* Slow light sweep across the banner */
.sb-winner-shine {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: linear-gradient(105deg, transparent 35%, rgba(255, 255, 255, 0.14) 50%, transparent 65%);
  background-size: 250% 100%;
  animation: sbWinnerShine 3.2s ease-in-out 0.9s infinite;
}
@keyframes sbWinnerShine {
  0% { background-position: 120% 0; }
  60%, 100% { background-position: -120% 0; }
}

.sb-winner-head {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 2cqh;
}

.sb-winner-ribbon {
  display: inline-flex;
  align-items: center;
  gap: 1.4cqh;
  background-color: var(--sb-gold);
  color: var(--sb-ink);
  font-size: 7cqh;
  font-weight: 700;
  line-height: 1;
  padding: 1.2cqh 3cqh;
  border-radius: 1.2cqh;
}

.sb-winner-trophy {
  width: 6cqh;
  height: 6cqh;
}

.sb-winner-corner {
  font-size: 7cqh;
  font-weight: 700;
  line-height: 1;
  padding: 1.2cqh 2.4cqh;
  border-radius: 1.2cqh;
  background-color: rgba(0, 0, 0, 0.3);
}

/* logo | name block (centred) | score — side columns equal width */
.sb-winner-body {
  width: 100%;
  display: grid;
  /* side columns wide enough for a two-digit score like "12 – 10" */
  grid-template-columns: 30cqh minmax(0, 1fr) 30cqh;
  justify-items: center;
  align-items: center;
  gap: 3cqh;
  min-height: 0;
}

.sb-winner-side {
  display: flex;
  justify-content: center;
}

.sb-winner-logo {
  width: 24cqh;
  height: 24cqh;
  background-color: rgba(0, 0, 0, 0.28);
  border-radius: 1.8cqh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.6cqh;
}
.sb-winner-logo img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  border-radius: 0.8cqh;
}

.sb-winner-info {
  justify-self: stretch;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  text-align: center;
  gap: 1cqh;
}

.sb-winner-team {
  font-size: 4.4cqh;
  font-weight: 500;
  line-height: 1;
  color: rgba(255, 255, 255, 0.85);
}

.sb-winner-name,
.sb-winner-club,
.sb-winner-kata {
  line-height: 1.02;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
/* Name may take two balanced lines so long names keep a big size */
.sb-winner-name {
  font-size: 12.5cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-white);
  white-space: normal;
  text-wrap: balance;
  overflow-wrap: normal;
}
.sb-winner-club { font-size: 7cqh; font-weight: 500; color: rgba(255, 255, 255, 0.9); }
.sb-winner-kata { font-size: 6.5cqh; font-weight: 700; color: var(--sb-gold); }

/* Final score, right of the name: winner's score big, opponent's smaller */
.sb-winner-score {
  display: flex;
  align-items: baseline;
  gap: 1cqh;
  padding: 1.6cqh 2.4cqh;
  border-radius: 1.6cqh;
  background-color: rgba(0, 0, 0, 0.45);
  line-height: 0.9;
  font-weight: 700;
}
.sb-winner-score-main { font-size: 14cqh; color: var(--sb-white); }
.sb-winner-score-sep { font-size: 5cqh; color: rgba(255, 255, 255, 0.5); }
.sb-winner-score-other { font-size: 7cqh; color: rgba(255, 255, 255, 0.65); }

.sb-winner-foot {
  font-size: 4cqh;
  font-weight: 500;
  line-height: 1;
  color: rgba(255, 255, 255, 0.8);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Draw */
.sb-draw-title {
  font-size: 24cqh;
  font-weight: 700;
  line-height: 0.9;
  color: var(--sb-white);
}

.sb-draw-names {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3cqh;
  max-width: 100%;
}

.sb-draw-chip {
  min-width: 0;
  font-size: 6cqh;
  font-weight: 700;
  line-height: 1;
  padding: 1.4cqh 3cqh;
  border-radius: 1.2cqh;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sb-draw-chip.aka { background-color: var(--sb-aka); }
.sb-draw-chip.ao { background-color: var(--sb-ao); }

.sb-draw-vs,
.sb-draw-score {
  flex-shrink: 0;
  font-size: 9cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-gold);
}

/* ==================== HEADER ==================== */
.sb-header {
  height: 6cqh;
  flex-shrink: 0;
  background-color: var(--sb-bar-bg);
  padding: 0 1.4cqh;
  display: flex;
  align-items: center;
  gap: 1.6cqh;
  font-family: var(--sb-display);
}

.sb-tatami-badge {
  background-color: var(--sb-gold);
  color: var(--sb-ink);
  font-weight: 700;
  font-size: 3.6cqh;
  line-height: 1;
  padding: 0.7cqh 1.6cqh;
  border-radius: 0.8cqh;
  flex-shrink: 0;
}

.sb-header-center {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 3.6cqh;
  font-weight: 500;
  line-height: 1;
  text-align: center;
  text-transform: uppercase;
}

.sb-header-round {
  color: var(--sb-gold);
}

.sb-header-sep {
  color: var(--sb-dim);
  padding: 0 1cqh;
}

.sb-header-right {
  display: flex;
  align-items: center;
  gap: 1.2cqh;
  flex-shrink: 0;
}

.sb-event-name {
  font-size: 2.8cqh;
  font-weight: 500;
  color: var(--sb-dim);
  max-width: 50cqh;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sb-sync-dot {
  width: 1.2cqh;
  height: 1.2cqh;
  border-radius: 9999px;
  background-color: rgba(255, 255, 255, 0.3);
}
.sb-sync-dot.is-active {
  background-color: #22c55e;
}

/* ==================== ARENA ==================== */
.sb-arena {
  flex: 1;
  min-height: 0;
  display: grid;
  /* AKA | judge slot (closed = just the gap between cards) | AO */
  grid-template-rows: 1fr 0.8cqh 1fr;
  padding: 0.8cqh;
  position: relative;
  transition: grid-template-rows 0.8s cubic-bezier(0.65, 0, 0.35, 1);
}

/* Kata judges open between the cards while the timer footer folds away, so AO
   eases down and the cards keep almost their normal height */
.sb-arena.has-judges {
  grid-template-rows: 1fr 30cqh 1fr;
}

.sb-judges-slot {
  min-height: 0;
  overflow: hidden;
}

/* ==================== COMBATANT CARD ==================== */
.sb-card {
  min-height: 0;
  border-radius: 1.4cqh;
  display: grid;
  /* corner | logo | name block | score
     kumite adds a second row under logo + name for the penalty lamps */
  grid-template-columns: 7cqh auto minmax(0, 1fr) auto;
  grid-template-rows: minmax(0, 1fr) auto;
  align-items: center;
  column-gap: 2cqh;
  row-gap: 1cqh;
  padding: 1.2cqh 1.6cqh 1.2cqh 0;
  position: relative;
  overflow: hidden;
}

.sb-card.is-kata {
  grid-template-rows: minmax(0, 1fr);
}

.sb-corner { grid-column: 1; grid-row: 1 / -1; }
.sb-logo-frame { grid-column: 2; grid-row: 1; }
.sb-details { grid-column: 3; grid-row: 1; }
.sb-penalties { grid-column: 2 / 4; grid-row: 2; }
.sb-score-box { grid-column: -2; grid-row: 1 / -1; }

/* With Video Review: an extra column for the VR logo, left of the score */
.sb-card.has-vr { grid-template-columns: 7cqh auto minmax(0, 1fr) auto auto; }
.sb-vr-slot { grid-column: 4; grid-row: 1 / -1; display: flex; align-items: center; justify-content: center; }
/* Kumite: the VR logo sits in the top row only, the penalty row runs under it */
.sb-card.has-vr:not(.is-kata) .sb-vr-slot { grid-row: 1; }
.sb-card.has-vr:not(.is-kata) .sb-penalties { grid-column: 2 / 5; }

/* Kumite: the penalty row takes ~7cqh, so the top row is a bit smaller */
.sb-card:not(.is-kata) .sb-logo-frame { height: 17cqh; width: 17cqh; }
.sb-card:not(.is-kata) .sb-athlete-name { font-size: 11cqh; }
.sb-card:not(.is-kata) .sb-club-name { font-size: 6cqh; }

/* Solid corner colour on the left (read from far away), fading to transparent
   on the right so the VR logo and score box stand out against the dark board */
.sb-card-aka {
  background: linear-gradient(
    90deg,
    var(--sb-aka) 0%,
    var(--sb-aka) 50%,
    color-mix(in srgb, var(--sb-aka) 35%, transparent) 78%,
    color-mix(in srgb, var(--sb-aka) 10%, transparent) 100%
  );
}
.sb-card-ao {
  background: linear-gradient(
    90deg,
    var(--sb-ao) 0%,
    var(--sb-ao) 50%,
    color-mix(in srgb, var(--sb-ao) 35%, transparent) 78%,
    color-mix(in srgb, var(--sb-ao) 10%, transparent) 100%
  );
}

/* Vertical AKA / AO strip on the left edge */
.sb-corner {
  align-self: stretch;
  margin: -1.2cqh 0;
  display: flex;
  align-items: center;
  justify-content: center;
  writing-mode: vertical-rl;
  transform: rotate(180deg);
  font-family: var(--sb-display);
  font-weight: 700;
  font-size: 5cqh;
  letter-spacing: 0.4cqh;
  line-height: 1;
}
.sb-card-aka .sb-corner { background-color: var(--sb-aka-deep); }
.sb-card-ao .sb-corner { background-color: var(--sb-ao-deep); }

.sb-logo-frame {
  height: 20cqh;
  width: 20cqh;
  background-color: rgba(0, 0, 0, 0.28);
  border-radius: 1.2cqh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1cqh;
}

.sb-logo-media {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  border-radius: 0.6cqh;
}

.sb-details {
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 0.6cqh;
}

.sb-status-strip {
  display: flex;
  align-items: center;
  gap: 1.2cqh;
  min-width: 0;
}

.sb-team-name-tag {
  font-family: var(--sb-display);
  font-size: 3.4cqh;
  font-weight: 500;
  line-height: 1;
  color: rgba(255, 255, 255, 0.85);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sb-athlete-name,
.sb-club-name,
.sb-kata-title {
  margin: 0;
  font-family: var(--sb-display);
  line-height: 1.02;
  text-transform: uppercase;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sb-athlete-name {
  font-size: 12cqh;
  font-weight: 700;
  color: var(--sb-white);
}

.sb-club-name {
  font-size: 6.5cqh;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.88);
}

.sb-kata-title {
  font-size: 6.5cqh;
  font-weight: 700;
  color: var(--sb-gold);
}

/* Kata cards have no penalties: give the name more presence */
.sb-card.is-kata .sb-athlete-name { font-size: 11cqh; }

/* Penalty lamps, one horizontal row along the bottom of the card:
   read from distance as "how many are lit" */
.sb-penalties {
  display: grid;
  /* spans logo + name columns, i.e. the full card up to the score box */
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 1cqh;
}

.sb-penalty-item {
  height: 6.5cqh;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 0.8cqh;
  font-family: var(--sb-display);
  font-weight: 700;
  font-size: 4.4cqh;
  line-height: 1;
  color: rgba(255, 255, 255, 0.35);
  border: 0.3cqh solid rgba(255, 255, 255, 0.3);
  background-color: rgba(0, 0, 0, 0.18);
}

.sb-penalty-item.is-active {
  background-color: var(--sb-gold);
  border-color: var(--sb-gold);
  color: var(--sb-ink);
}

/* Score */
.sb-score-box {
  position: relative;
  align-self: stretch;
  min-width: 36cqh;
  background-color: rgba(0, 0, 0, 0.55);
  border: 0.5cqh solid transparent;
  border-radius: 1.6cqh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 2.4cqh;
  /* clips the senshu corner tag to the rounded corner */
  overflow: hidden;
}

.sb-score-box.is-leading {
  border-color: rgba(255, 255, 255, 0.9);
}

.sb-score-val {
  font-family: var(--sb-display);
  font-weight: 700;
  font-size: 30cqh;
  line-height: 0.85;
  font-variant-numeric: tabular-nums;
  color: var(--sb-white);
}

/* Kata total pops in when it is revealed */
.sb-card.is-kata .sb-score-val {
  animation: sbScorePop 0.7s cubic-bezier(0.16, 1, 0.3, 1) both;
}

@keyframes sbScorePop {
  0% { opacity: 0; transform: scale(0.6); }
  100% { opacity: 1; transform: none; }
}

/* Senshu: bottom-left corner of the score box */
/* Senshu: a gold corner tag (triangle) in the bottom-right of the score box, "S" in it */
.sb-senshu {
  position: absolute;
  right: 0;
  bottom: 0;
  width: 9cqh;
  height: 9cqh;
  background-color: var(--sb-gold);
  clip-path: polygon(100% 0, 100% 100%, 0 100%);
  color: var(--sb-ink);
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  padding: 0 0.9cqh 0.5cqh 0;
  font-family: var(--sb-display);
  font-weight: 700;
  font-size: 4.6cqh;
  line-height: 1;
}

/* ==================== FOOTER / TIMER ==================== */
.sb-footer {
  height: 27cqh;
  flex-shrink: 0;
  background-color: var(--sb-bar-bg);
  padding: 1.2cqh 1.6cqh;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: 2cqh;
  overflow: hidden;
  transition:
    height 0.8s cubic-bezier(0.65, 0, 0.35, 1),
    padding 0.8s cubic-bezier(0.65, 0, 0.35, 1);
}

/* Folds away while the kata judge panel is open */
.sb-footer.is-collapsed {
  height: 0;
  padding-top: 0;
  padding-bottom: 0;
}

.sb-footer-left {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  min-width: 0;
}

.sb-sponsor-brand {
  height: 11cqh;
  max-width: 100%;
  object-fit: contain;
}

.sb-timer-vault {
  height: 100%;
  border: 0.5cqh solid var(--sb-line);
  border-radius: 1.8cqh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 3cqh;
  background-color: #000;
}

.sb-timer-vault.is-running {
  border-color: var(--sb-gold);
}

.sb-timer-digits {
  font-family: var(--sb-display);
  font-weight: 700;
  font-size: 25cqh;
  line-height: 0.85;
  color: var(--sb-white);
  white-space: nowrap;
}

/* Fixed cells: widest Oswald digit is "0" at 0.55em, ":" is 0.28em */
.sb-tdig,
.sb-tsep {
  display: inline-block;
  text-align: center;
}
.sb-tdig { width: 0.56em; }
.sb-tsep { width: 0.28em; }

.sb-timer-ms {
  font-size: 0.6em;
  color: inherit;
  opacity: 0.85;
}

.sb-timer-vault.is-running .sb-timer-digits {
  color: var(--sb-gold);
}

.sb-timer-vault.is-warning {
  border-color: var(--sb-warn);
  background-color: rgba(255, 59, 59, 0.16);
}
.sb-timer-vault.is-warning .sb-timer-digits {
  color: var(--sb-warn);
  animation: sbTimerAlert 0.7s infinite alternate;
}

@keyframes sbTimerAlert {
  from { opacity: 1; }
  to { opacity: 0.55; }
}

.sb-footer-right {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  min-width: 0;
}

.sb-team-standings {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 0.8cqh;
  min-width: 0;
}

.sb-team-meta {
  font-family: var(--sb-display);
  font-size: 3.2cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-gold);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sb-team-score-display {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1cqh;
  font-family: var(--sb-display);
  font-weight: 700;
  font-size: 19cqh;
  line-height: 0.95;
}

.sb-team-num {
  min-width: 13cqh;
  text-align: center;
  border-radius: 1.4cqh;
  padding: 0 1cqh;
}
.sb-team-num.aka { background-color: var(--sb-aka); }
.sb-team-num.ao { background-color: var(--sb-ao); }
.sb-team-num-dash {
  font-size: 0.6em;
  color: var(--sb-dim);
}

/* ==================== BANNER (alarm / point call) ==================== */
/* One column per corner; two columns side by side when AKA and AO both scored */
.sb-banner-overlay {
  position: absolute;
  inset: 0.8cqh;
  z-index: 60;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 0.8cqh;
  pointer-events: none;
  font-family: var(--sb-display);
  text-transform: uppercase;
}
.sb-banner-overlay.is-dual { grid-template-columns: repeat(2, minmax(0, 1fr)); }

/* Hantei: waiting for the 4 judges (as in Ambrilindo Local) */
.sb-hantei {
  position: absolute;
  inset: 0.8cqh;
  z-index: 59;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1.6cqh;
  border-radius: 1.4cqh;
  background: radial-gradient(circle at center, #1e293b 0%, #0f172a 100%);
  border: 0.5cqh solid var(--sb-gold);
  font-family: var(--sb-display);
  text-transform: uppercase;
  text-align: center;
  pointer-events: none;
}
.sb-hantei-title {
  font-size: 24cqh;
  font-weight: 700;
  line-height: 0.9;
  color: var(--sb-gold);
  letter-spacing: 0.04em;
}
.sb-hantei-sub {
  font-size: 7cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-white);
  letter-spacing: 0.12em;
}

.sb-banner-col {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 1.4cqh;
  padding: 2cqh 3cqh;
  border-radius: 1.4cqh;
  overflow: hidden;
}
.sb-banner-col.is-aka { background-color: var(--sb-aka); }
.sb-banner-col.is-ao { background-color: var(--sb-ao); }


.sb-banner-side-badge {
  display: inline-flex;
  align-items: center;
  gap: 1cqh;
  max-width: 100%;
  font-size: 4.4cqh;
  font-weight: 500;
  line-height: 1;
  background-color: rgba(0, 0, 0, 0.4);
  padding: 1cqh 2.4cqh;
  border-radius: 9999px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sb-banner-side-sep { opacity: 0.5; }

.sb-banner-title {
  font-size: 11cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-gold);
}

.sb-banner-subtitle {
  font-size: 22cqh;
  font-weight: 700;
  line-height: 0.95;
  color: var(--sb-white);
  white-space: nowrap;
}

/* Judge flags (as in Ambrilindo Local): white outline, pop in one by one, slow wave */
.sb-banner-flags {
  display: flex;
  align-items: flex-end;
  justify-content: center;
  gap: 2cqh;
}
.sb-banner-flag {
  height: 13cqh;
  width: auto;
  transform-origin: bottom center;
  filter:
    drop-shadow(0 0 0.4cqh #ffffff)
    drop-shadow(0 0 0.8cqh rgba(255, 255, 255, 0.9))
    drop-shadow(0 0.8cqh 2cqh rgba(0, 0, 0, 0.85));
  animation:
    sbFlagPop 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275) both,
    sbFlagWave 1.8s ease-in-out 0.4s infinite alternate;
  animation-delay: calc(var(--i) * 0.08s), calc(0.4s + var(--i) * 0.08s);
}
@keyframes sbFlagPop {
  0% { opacity: 0; transform: scale(0.6) translateY(2cqh); }
  100% { opacity: 1; transform: none; }
}
@keyframes sbFlagWave {
  from { transform: rotate(-2deg) scale(1); }
  to { transform: rotate(2deg) scale(1.03); }
}
.sb-banner-judges {
  font-size: 3.6cqh;
  font-weight: 700;
  line-height: 1;
  letter-spacing: 0.08em;
  padding: 0.6cqh 2cqh;
  border-radius: 0.8cqh;
  background-color: rgba(0, 0, 0, 0.4);
  border: 0.15cqh solid rgba(255, 255, 255, 0.2);
}

/* With flags the point name shrinks a little so everything fits the arena */
.sb-banner-col.has-flags .sb-banner-subtitle { font-size: 18cqh; }
.sb-banner-col.has-flags .sb-banner-title { font-size: 9cqh; }

/* AKA and AO side by side: everything smaller to fit half the width */
.sb-banner-overlay.is-dual .sb-banner-side-badge { font-size: 3.4cqh; }
.sb-banner-overlay.is-dual .sb-banner-title { font-size: 7.5cqh; }
.sb-banner-overlay.is-dual .sb-banner-subtitle { font-size: 14cqh; }
.sb-banner-overlay.is-dual .sb-banner-flag { height: 10cqh; }

/* ==================== KATA JUDGES PANEL (3 / 5 / 7, between AKA and AO) ==================== */
.sb-judges {
  --jcount: 5;
  height: 100%;
  min-height: 30cqh;
  padding: 0.8cqh 0;
  display: grid;
  grid-template-columns: repeat(var(--jcount), minmax(0, 1fr));
  gap: 1.6cqh;
}

/* AKA score on top, AO below, mirroring the cards above */
.sb-judge-pod {
  min-height: 0;
  display: grid;
  grid-template-rows: 1fr auto 1fr;
  gap: 0.8cqh;
  animation: sbJudgeIn 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
  /* first judge appears once the slot has mostly opened */
  animation-delay: calc(0.6s + var(--i) * 0.5s);
}

@keyframes sbJudgeIn {
  0% { opacity: 0; transform: translateY(3cqh) scale(0.9); }
  100% { opacity: 1; transform: none; }
}

.sb-jscore {
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 1.2cqh;
  font-family: var(--sb-display);
  font-weight: 700;
  /* 7 narrower pods: smaller digits so "10.0" still fits */
  font-size: calc(11cqh * 5 / max(var(--jcount), 5));
  line-height: 1;
  color: rgba(255, 255, 255, 0.9);
  background-color: #1b1d23;
  border: 0.4cqh solid transparent;
  transition: background-color 0.25s ease;
}
.sb-jscore.aka { border-color: var(--sb-aka); }
.sb-jscore.ao { border-color: var(--sb-ao); }
.sb-jscore.aka.leading { background-color: var(--sb-aka); color: var(--sb-white); }
.sb-jscore.ao.leading { background-color: var(--sb-ao); color: var(--sb-white); }

.sb-jlabel {
  font-family: var(--sb-display);
  font-size: 3.4cqh;
  font-weight: 700;
  line-height: 1;
  text-align: center;
  color: var(--sb-gold);
}

/* Cards are slightly shorter while the judge panel is up */
.sb-arena.has-judges .sb-athlete-name { font-size: 10cqh; }
.sb-arena.has-judges .sb-club-name,
.sb-arena.has-judges .sb-kata-title { font-size: 6cqh; }

/* ==================== VIDEO REVIEW OVERLAY ==================== */
.sb-vr-backdrop {
  position: absolute;
  inset: 0;
  z-index: 70;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3cqh;
  padding: 3cqh;
  background-color: rgba(0, 0, 0, 0.9);
  font-family: var(--sb-display);
  text-transform: uppercase;
}

.sb-vr-card {
  width: 100%;
  max-width: 160cqh;
  padding: 4cqh 5cqh;
  border-radius: 2.4cqh;
  border: 0.8cqh solid;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2cqh;
}
.sb-vr-side-aka { background-color: var(--sb-aka-deep); border-color: var(--sb-aka); }
.sb-vr-side-ao { background-color: var(--sb-ao-deep); border-color: var(--sb-ao); }
.sb-vr-card.is-accepted { background-color: #065f46; border-color: #34d399; }
.sb-vr-card.is-rejected { background-color: #4c0519; border-color: #fb7185; }
.sb-vr-card.is-minai { background-color: #451a03; border-color: var(--sb-gold); }
.sb-vr-minai { color: var(--sb-gold); }

/* VR card logo, left of the score box while the card is still available */
.sb-vr-logo {
  width: 11cqh;
  height: 11cqh;
  object-fit: contain;
  filter: drop-shadow(0 0 1.2cqh rgba(255, 255, 255, 0.45));
  transition: opacity 0.25s ease, transform 0.25s ease;
}
.sb-vr-logo.is-used {
  opacity: 0;
  transform: scale(0.8);
}

.sb-vr-pill {
  display: inline-flex;
  align-items: center;
  gap: 1.2cqh;
  font-size: 4cqh;
  font-weight: 700;
  line-height: 1;
  padding: 1cqh 2.4cqh;
  border-radius: 9999px;
  background-color: rgba(0, 0, 0, 0.45);
}

.sb-vr-dot {
  width: 2cqh;
  height: 2cqh;
  border-radius: 9999px;
  background-color: var(--sb-warn);
  animation: sbTimerAlert 0.7s infinite alternate;
}

.sb-vr-corner {
  font-size: 16cqh;
  font-weight: 700;
  line-height: 0.9;
}

.sb-vr-athlete {
  font-size: 6cqh;
  font-weight: 500;
  line-height: 1;
  max-width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sb-vr-reason {
  font-size: 8cqh;
  font-weight: 700;
  line-height: 1;
  color: var(--sb-gold);
  background-color: rgba(0, 0, 0, 0.45);
  padding: 2cqh 4cqh;
  border-radius: 1.6cqh;
  max-width: 100%;
}

.sb-vr-status {
  font-size: 7cqh;
  font-weight: 700;
  line-height: 1;
}

/* AKA and AO under review at the same time: two cards side by side */
.sb-vr-backdrop.is-dual .sb-vr-card { flex: 1; max-width: none; min-width: 0; padding: 3cqh 3cqh; }
.sb-vr-backdrop.is-dual .sb-vr-corner { font-size: 12cqh; }
.sb-vr-backdrop.is-dual .sb-vr-athlete { font-size: 4.6cqh; }
.sb-vr-backdrop.is-dual .sb-vr-reason { font-size: 6cqh; padding: 1.6cqh 2.4cqh; }
.sb-vr-backdrop.is-dual .sb-vr-status { font-size: 5.4cqh; }
.sb-vr-pending { color: var(--sb-gold); animation: sbTimerAlert 0.9s infinite alternate; }
.sb-vr-accepted { color: #6ee7b7; }
.sb-vr-rejected { color: #fda4af; }

@media (prefers-reduced-motion: reduce) {
  .sb-root * {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
</style>
