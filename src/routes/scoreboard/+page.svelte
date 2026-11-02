<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import { listen, emit } from "@tauri-apps/api/event";

  // Tatami number from query string, defaults to 1
  let tatamiNumber = $derived(Number(page.url.searchParams.get("tatami") || "1"));
  let isLocalSyncConnected = $state(false);

  // Match information state
  let eventName = $state("AMBRILINDO KARATE CHAMPIONSHIP");
  let categoryName = $state("KATA PERORANGAN SENIOR PUTRA");
  let matchRound = $state("BABAK PENYISIHAN");
  let matchNumber = $state("1");
  let isKata = $state(true);
  let matchWinner = $state("");
  let isMatchFinished = $state(false);
  let showKataScoreOverlay = $state(false);

  // Team Kumite (Kumite Beregu) state
  let isTeamMatch = $state(false);
  let teamAkaName = $state("");
  let teamAoName = $state("");
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

  function runScoreboardTimerLoop() {
    if (!isTimerRunning) return;
    const now = performance.now();
    const delta = now - lastTickTimestamp;
    lastTickTimestamp = now;

    timerMs = Math.max(0, timerMs - delta);
    if (timerMs <= 0) {
      timerMs = 0;
      isTimerRunning = false;
      return;
    }

    animFrameId = requestAnimationFrame(runScoreboardTimerLoop);
  }

  // Calculate judge highlights for 5-judge table
  let judgeDecisions = $derived.by(() => {
    return [0, 1, 2, 3, 4].map((idx) => {
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
    isLocalSyncConnected = true;
    if (data.category) categoryName = data.category;
    if (data.round) matchRound = data.round;
    if (data.matchNumber !== undefined) matchNumber = String(data.matchNumber);
    
    if (data.timerMs !== undefined) {
      if (isTimerRunning) {
        if (Math.abs(timerMs - data.timerMs) > 150) {
          timerMs = data.timerMs;
        }
      } else {
        timerMs = data.timerMs;
      }
    } else if (data.timerSeconds !== undefined) {
      if (!isTimerRunning) {
        timerMs = data.timerSeconds * 1000;
      }
    }

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

    if (data.isKata !== undefined) isKata = data.isKata;
    if (data.winner !== undefined) matchWinner = data.winner;
    if (data.isFinished !== undefined) isMatchFinished = data.isFinished;
    if (data.showKataScoreOverlay !== undefined) showKataScoreOverlay = data.showKataScoreOverlay;

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
  }

  let unlistenSync: (() => void) | null = null;

  onMount(() => {
    async function setupIpc() {
      try {
        unlistenSync = await listen("tatami_live_state", (event: any) => {
          if (event.payload && event.payload.tatami == tatamiNumber) {
            applyLiveState(event.payload);
          }
        });
        await emit("request_tatami_state", { tatami: tatamiNumber });
      } catch (err) {
        console.warn("Tauri IPC listen failed, using HTTP/WS fallback:", err);
      }
    }
    setupIpc();
  });

  onDestroy(() => {
    if (unlistenSync) unlistenSync();
    if (animFrameId !== null) cancelAnimationFrame(animFrameId);
  });
</script>

<div class="sb-root">
  <!-- ==================== TOP BROADCAST HEADER BAR WITH SCROLLING MARQUEE ==================== -->
  <header class="sb-header">
    <div class="sb-header-left">
      <div class="sb-tatami-badge">
        TATAMI {tatamiNumber}
      </div>

      {#if isTeamMatch}
        <div class="sb-team-tag-group">
          <span class="sb-team-badge-tag">BEREGU</span>
          <span class="sb-team-badge-partai">P{activeBoutRound}/{totalBouts}</span>
          <span class="sb-team-score-num aka">{teamAkaScore}</span>
          <span class="sb-team-score-sep">:</span>
          <span class="sb-team-score-num ao">{teamAoScore}</span>
        </div>
      {/if}
    </div>

    <!-- Scrolling Marquee Ticker (Full-width, large, high visibility) -->
    <div class="sb-header-marquee-wrap">
      <!-- svelte-ignore a11y_distracting_elements -->
      <marquee behavior="scroll" direction="left" scrollamount="10" class="sb-header-marquee">
        <span class="sb-marquee-sep">•</span>
        <span class="sb-marquee-cat">{categoryName}</span>
        <span class="sb-marquee-sep">•</span>
        <span class="sb-marquee-round">{matchRound}</span>
        <span class="sb-marquee-sep">•</span>
        <span class="sb-marquee-partai">PARTAI #{matchNumber}</span>
        <span class="sb-marquee-sep">•</span>
        <span class="sb-marquee-event">{eventName}</span>
      </marquee>
    </div>

    <div class="sb-header-right">
      <div class="sb-sync-dot" class:is-active={isLocalSyncConnected} title={isLocalSyncConnected ? "Live LAN Connected" : "Connecting..."}></div>
    </div>
  </header>

  <!-- ==================== MAIN ARENA (EXACT 50/50 VERTICAL PROPORTIONS) ==================== -->
  <main class="sb-arena">
    <!-- TOP CARD: AKA (RED) -->
    <section class="sb-card sb-card-aka" class:is-winner={matchWinner === "1"}>
      <div class="sb-card-body">
        <!-- Contingent / Club Logo Frame -->
        <div class="sb-logo-frame">
          <img
            src={akaLogo || "/img/idn-flag.png"}
            alt="Logo AKA"
            class="sb-logo-media"
            onerror={(e) => { (e.currentTarget as HTMLImageElement).src = "/img/idn-flag.png"; }}
          />
        </div>

        <!-- Competitor Details -->
        <div class="sb-details">
          <div class="sb-status-strip">
            <span class="sb-corner-tag aka">AKA</span>

            {#if isTeamMatch && teamAkaName}
              <span class="sb-team-name-tag">TIM: {teamAkaName}</span>
            {/if}

            {#if matchWinner === "1"}
              <div class="sb-winner-pill">
                <svg class="sb-winner-svg" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                  <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
                </svg>
                <span>PEMENANG</span>
              </div>
            {/if}
          </div>

          <h1 class="sb-athlete-name">
            {akaName}
          </h1>

          <p class="sb-club-name">
            {akaClub}
          </p>

          {#if isKata && akaKataName}
            <div class="sb-kata-title">
              {akaKataName}
            </div>
          {/if}
        </div>

        <!-- Score Chamber -->
        <div class="sb-score-chamber">
          {#if !isKata && akaSenshu}
            <span class="sb-senshu-ribbon">
              Senshu ★
            </span>
          {/if}

          <div class="sb-score-box" class:is-leading={akaScore > aoScore || akaTotalPoints > aoTotalPoints}>
            <span class="sb-score-val">
              {isKata ? akaTotalPoints : akaScore}
            </span>
          </div>
        </div>
      </div>

      <!-- Kumite Penalties -->
      {#if !isKata}
        <div class="sb-penalties">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            {@const active = akaPenalties[penalty as keyof typeof akaPenalties]}
            <div class="sb-penalty-item" class:is-active={active}>
              {penalty.toUpperCase()}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- BOTTOM CARD: AO (BLUE) -->
    <section class="sb-card sb-card-ao" class:is-winner={matchWinner === "2"}>
      <div class="sb-card-body">
        <!-- Contingent / Club Logo Frame -->
        <div class="sb-logo-frame">
          <img
            src={aoLogo || "/img/idn-flag.png"}
            alt="Logo AO"
            class="sb-logo-media"
            onerror={(e) => { (e.currentTarget as HTMLImageElement).src = "/img/idn-flag.png"; }}
          />
        </div>

        <!-- Competitor Details -->
        <div class="sb-details">
          <div class="sb-status-strip">
            <span class="sb-corner-tag ao">AO</span>

            {#if isTeamMatch && teamAoName}
              <span class="sb-team-name-tag">TIM: {teamAoName}</span>
            {/if}

            {#if matchWinner === "2"}
              <div class="sb-winner-pill">
                <svg class="sb-winner-svg" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                  <path d="M10 2a1 1 0 011 1v1h3a1 1 0 011 1v2a3 3 0 01-3 3h-1.1a4.99 4.99 0 01-1.9 3.1V15h2a1 1 0 110 2H8a1 1 0 110-2h2v-1.9A4.99 4.99 0 018.1 10H7a3 3 0 01-3-3V5a1 1 0 011-1h3V3a1 1 0 011-1z" />
                </svg>
                <span>PEMENANG</span>
              </div>
            {/if}
          </div>

          <h1 class="sb-athlete-name">
            {aoName}
          </h1>

          <p class="sb-club-name">
            {aoClub}
          </p>

          {#if isKata && aoKataName}
            <div class="sb-kata-title">
              {aoKataName}
            </div>
          {/if}
        </div>

        <!-- Score Chamber -->
        <div class="sb-score-chamber">
          {#if !isKata && aoSenshu}
            <span class="sb-senshu-ribbon">
              Senshu ★
            </span>
          {/if}

          <div class="sb-score-box" class:is-leading={aoScore > akaScore || aoTotalPoints > akaTotalPoints}>
            <span class="sb-score-val">
              {isKata ? aoTotalPoints : aoScore}
            </span>
          </div>
        </div>
      </div>

      <!-- Kumite Penalties -->
      {#if !isKata}
        <div class="sb-penalties">
          {#each ['c1', 'c2', 'c3', 'hc', 'h'] as penalty}
            {@const active = aoPenalties[penalty as keyof typeof aoPenalties]}
            <div class="sb-penalty-item" class:is-active={active}>
              {penalty.toUpperCase()}
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </main>

  <!-- ==================== KATA 5-JUDGE SCORE OVERLAY ==================== -->
  {#if isKata && showKataScoreOverlay}
    <div class="sb-overlay">
      <div class="sb-overlay-card">
        <div class="sb-judges-grid">
          {#each judgeDecisions as decision, idx}
            {@const sAka = akaJudgeScores[idx] || "0.0"}
            {@const sAo = aoJudgeScores[idx] || "0.0"}
            <div
              class="sb-judge-pod"
              class:is-aka-win={decision === 'aka'}
              class:is-ao-win={decision === 'ao'}
            >
              <div class="sb-jscore aka" class:leading={decision === 'aka'}>
                {sAka}
              </div>

              <div class="sb-jlabel">
                JURI {idx + 1}
              </div>

              <div class="sb-jscore ao" class:leading={decision === 'ao'}>
                {sAo}
              </div>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  <!-- ==================== BOTTOM ARENA FOOTER BAR (GIANT DIGITAL TIMER & HD LOGO) ==================== -->
  <footer class="sb-footer">
    <!-- Left: Official HD Ambrilindo Sponsor Logo -->
    <div class="sb-footer-left">
      <img
        src="/img/ambrilindo-logo-dark.png"
        alt="Ambrilindo"
        class="sb-sponsor-brand"
      />
    </div>

    <!-- Center: Giant Digital Timer Display (30m Arena Visibility) -->
    <div class="sb-footer-center">
      <div class="sb-timer-vault" class:is-running={isTimerRunning}>
        <span class="sb-timer-digits">
          {timerFormatted.full}
        </span>
      </div>
    </div>

    <!-- Right: Big Golden Team Score or Arena Label -->
    <div class="sb-footer-right">
      {#if isTeamMatch}
        <div class="sb-team-standings">
          <div class="sb-team-meta">
            <span class="sb-team-meta-title">Skor Tim Beregu</span>
            <span class="sb-team-meta-pts">Partai {activeBoutRound}/{totalBouts} • Agg {teamAkaPoints}-{teamAoPoints} Pts</span>
          </div>
          <div class="sb-team-score-display">
            <span class="sb-team-num aka">{teamAkaScore}</span>
            <span class="sb-team-num-dash">-</span>
            <span class="sb-team-num ao">{teamAoScore}</span>
          </div>
        </div>
      {:else}
        <div class="sb-arena-badge">
          <span class="sb-arena-label">TATAMI {tatamiNumber}</span>
          <span class="sb-arena-sub">Arena Scoring</span>
        </div>
      {/if}
    </div>
  </footer>
</div>

<style>
/* Hallmark · checked: 2026-11-02 · genre: modern-minimal
 * floor: 0 FAIL · fixed: 63 (3, 7a, 7b, 24b, 29, 34, 40, 41, 48, 50, 64, 67, 74, F3)
 * built: Hallmark · scope: app · redesign: arena-compact · v3.0.0 */

:root {
  /* Obsidian Surface Palette */
  --sb-bg: #06080d;
  --sb-header-bg: #090c13;
  --sb-footer-bg: #080b11;
  --sb-card-base: #0c1018;
  --sb-card-aka: #150c12;
  --sb-card-ao: #0b111c;
  --sb-pod-bg: #040609;
  --sb-vault-bg: #0a0d13;
  --sb-overlay-bg: #080c14;
  --sb-overlay-backdrop: rgba(4, 6, 10, 0.88);
  --sb-penalty-slot: rgba(248, 250, 252, 0.04);

  /* Hairlines & Elevation Borders */
  --sb-line-subtle: rgba(248, 250, 252, 0.08);
  --sb-line-accent: rgba(248, 250, 252, 0.16);
  --sb-border-aka: rgba(205, 32, 32, 0.45);
  --sb-border-ao: rgba(28, 82, 216, 0.45);
  --sb-border-gold: rgba(226, 155, 18, 0.55);

  /* Tournament Hues (Balanced Contrast >= 4.5:1) */
  --sb-aka: #c51b1b;
  --sb-aka-tint: rgba(197, 27, 27, 0.16);
  --sb-aka-text: #fca5a5;
  --sb-ao: #1c52d8;
  --sb-ao-tint: rgba(28, 82, 216, 0.16);
  --sb-ao-text: #93c5fd;

  --sb-gold: #e29b12;
  --sb-gold-bright: #f5af24;
  --sb-gold-tint: rgba(226, 155, 18, 0.16);
  --sb-emerald: #0d9488;

  /* Typography Tones (Custom Tinted Off-White & Deep Ink) */
  --sb-text-bright: #f7f9fd;
  --sb-text: #e1e7f2;
  --sb-text-sub: #92a1b7;
  --sb-text-dim: #63738b;
  --sb-ink: #07090e;

  /* Shadows */
  --sb-shadow-drop: 0 4px 12px rgba(0, 0, 0, 0.5);
  --sb-overlay-shadow: 0 16px 40px rgba(0, 0, 0, 0.85);

  --sb-font: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --sb-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
}

/* ==================== ROOT CONTAINER (EXACT VIEWPORT FIT, NO OVERFLOW) ==================== */
.sb-root {
  height: 100vh;
  height: 100dvh;
  width: 100%;
  max-height: 100vh;
  max-width: 100%;
  overflow: clip;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background-color: var(--sb-bg);
  color: var(--sb-text);
  font-family: var(--sb-font);
  user-select: none;
  box-sizing: border-box;
}

/* ==================== TOP BROADCAST HEADER BAR WITH MARQUEE ==================== */
.sb-header {
  height: clamp(44px, 6vh, 60px);
  min-height: clamp(44px, 6vh, 60px);
  max-height: clamp(44px, 6vh, 60px);
  background-color: var(--sb-header-bg);
  border-bottom: 1px solid var(--sb-line-subtle);
  padding: 0 clamp(8px, 1.4vw, 20px);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: clamp(8px, 1.2vw, 16px);
  flex-shrink: 0;
  box-sizing: border-box;
  overflow: hidden;
}

.sb-header-left {
  display: flex;
  align-items: center;
  gap: clamp(8px, 1vw, 12px);
  flex-shrink: 0;
}

.sb-tatami-badge {
  background: linear-gradient(135deg, var(--sb-gold), var(--sb-gold-bright));
  color: var(--sb-ink);
  font-weight: 900;
  font-size: clamp(1.2rem, 2.6vh, 2rem);
  padding: clamp(4px, 0.8vh, 8px) clamp(12px, 1.6vw, 24px);
  border-radius: 8px;
  line-height: 1.1;
  text-wrap: balance;
}

.sb-team-tag-group {
  display: flex;
  align-items: center;
  gap: clamp(4px, 0.8vw, 8px);
  background-color: var(--sb-pod-bg);
  border: 1px solid var(--sb-line-accent);
  padding: clamp(4px, 0.6vh, 8px) clamp(8px, 1vw, 16px);
  border-radius: 6px;
  font-size: clamp(0.95rem, 1.8vh, 1.4rem);
}

.sb-team-badge-tag {
  color: var(--sb-text-sub);
  font-weight: 700;
}

.sb-team-badge-partai {
  background-color: var(--sb-card-base);
  color: var(--sb-gold-bright);
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 0.9em;
  font-weight: 700;
}

.sb-team-score-num {
  font-family: var(--sb-mono);
  font-weight: 900;
}
.sb-team-score-num.aka { color: var(--sb-aka); }
.sb-team-score-num.ao { color: var(--sb-ao); }
.sb-team-score-sep { color: var(--sb-text-dim); }

/* Continuous Scrolling Marquee */
.sb-header-marquee-wrap {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
}

.sb-header-marquee {
  width: 100%;
  font-family: var(--sb-font);
  font-size: clamp(1.2rem, 2.6vh, 1.9rem);
  font-weight: 900;
  line-height: 1;
  text-transform: uppercase;
}

.sb-marquee-sep {
  color: var(--sb-gold);
  padding: 0 clamp(8px, 1.2vw, 16px);
}

.sb-marquee-cat { color: var(--sb-text-bright); }
.sb-marquee-round { color: var(--sb-gold-bright); }
.sb-marquee-partai { color: var(--sb-text-bright); }
.sb-marquee-event { color: var(--sb-text-sub); }

.sb-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.sb-sync-dot {
  width: 10px;
  height: 10px;
  border-radius: 9999px;
  background-color: var(--sb-text-dim);
  transition: background-color 0.2s ease;
}
.sb-sync-dot.is-active {
  background-color: var(--sb-emerald);
}

/* ==================== MAIN ARENA (EXACT 50/50 VERTICAL PROPORTIONS) ==================== */
.sb-arena {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-rows: 1fr 1fr;
  gap: clamp(4px, 0.8vh, 8px);
  padding: clamp(4px, 0.8vh, 8px);
  box-sizing: border-box;
}

.sb-card {
  min-height: 0;
  border-radius: 12px;
  border: 1px solid var(--sb-line-subtle);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: clamp(4px, 0.8vh, 8px) clamp(12px, 1.6vw, 24px);
  position: relative;
  overflow: hidden;
  box-sizing: border-box;
}

.sb-card-aka {
  background: radial-gradient(ellipse at 15% 50%, var(--sb-aka-tint), transparent 75%), var(--sb-card-aka);
  border-color: var(--sb-border-aka);
}
.sb-card-aka.is-winner {
  outline: 2px solid var(--sb-gold);
  outline-offset: -2px;
}

.sb-card-ao {
  background: radial-gradient(ellipse at 15% 50%, var(--sb-ao-tint), transparent 75%), var(--sb-card-ao);
  border-color: var(--sb-border-ao);
}
.sb-card-ao.is-winner {
  outline: 2px solid var(--sb-gold);
  outline-offset: -2px;
}

.sb-card-body {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: clamp(12px, 1.8vw, 28px);
}

/* Flag / Logo Box */
.sb-logo-frame {
  height: clamp(84px, 20vh, 200px);
  width: clamp(84px, 20vh, 200px);
  background-color: var(--sb-pod-bg);
  border: 1px solid var(--sb-line-accent);
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: clamp(4px, 1vh, 12px);
  flex-shrink: 0;
  box-sizing: border-box;
}

.sb-logo-media {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  border-radius: 6px;
}

/* Competitor Details */
.sb-details {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.sb-status-strip {
  display: flex;
  align-items: center;
  gap: clamp(8px, 1vw, 12px);
  margin-bottom: clamp(2px, 0.4vh, 4px);
  flex-wrap: wrap;
}

.sb-corner-tag {
  font-size: clamp(1.1rem, 2.4vh, 1.8rem);
  font-weight: 900;
  padding: 4px 16px;
  border-radius: 6px;
  color: var(--sb-text-bright);
}
.sb-corner-tag.aka { background-color: var(--sb-aka); }
.sb-corner-tag.ao { background-color: var(--sb-ao); }

.sb-team-name-tag {
  font-size: clamp(1rem, 2vh, 1.6rem);
  font-weight: 700;
  color: var(--sb-gold-bright);
}

.sb-winner-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background-color: var(--sb-gold);
  color: var(--sb-ink);
  font-size: clamp(0.95rem, 2vh, 1.5rem);
  font-weight: 900;
  padding: 4px 12px;
  border-radius: 6px;
}

.sb-winner-svg {
  width: clamp(14px, 2vh, 20px);
  height: clamp(14px, 2vh, 20px);
}

.sb-athlete-name {
  font-size: clamp(2.8rem, 8.4vh, 7.5rem);
  line-height: 1.02;
  font-weight: 900;
  color: var(--sb-text-bright);
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-wrap: balance;
}

.sb-club-name {
  font-size: clamp(1.4rem, 3.8vh, 3.4rem);
  font-weight: 800;
  margin: clamp(2px, 0.4vh, 4px) 0 0 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sb-card-aka .sb-club-name { color: var(--sb-aka-text); }
.sb-card-ao .sb-club-name { color: var(--sb-ao-text); }

.sb-kata-title {
  font-size: clamp(1.5rem, 3.8vh, 3.4rem);
  font-weight: 800;
  color: var(--sb-gold-bright);
  margin-top: clamp(2px, 0.4vh, 4px);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Score Chamber */
.sb-score-chamber {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  height: 90%;
}

.sb-senshu-ribbon {
  position: absolute;
  top: clamp(-12px, -2vh, -16px);
  right: 0;
  background-color: var(--sb-gold);
  color: var(--sb-ink);
  font-size: clamp(0.9rem, 2vh, 1.4rem);
  font-weight: 900;
  padding: 2px 12px;
  border-radius: 4px;
  z-index: 2;
  box-shadow: var(--sb-shadow-drop);
}

.sb-score-box {
  height: 100%;
  max-height: clamp(130px, 26vh, 260px);
  min-height: clamp(100px, 18vh, 220px);
  min-width: clamp(140px, 17vw, 320px);
  background-color: var(--sb-pod-bg);
  border: 2px solid var(--sb-line-subtle);
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 clamp(16px, 2vw, 32px);
  transition: border-color 0.2s ease;
  box-sizing: border-box;
}

.sb-card-aka .sb-score-box.is-leading {
  border-color: var(--sb-border-aka);
}
.sb-card-ao .sb-score-box.is-leading {
  border-color: var(--sb-border-ao);
}

.sb-score-val {
  font-family: var(--sb-mono);
  font-weight: 900;
  font-size: clamp(7rem, 24vh, 18rem);
  line-height: 1;
  color: var(--sb-text-bright);
}

/* Kumite Penalties */
.sb-penalties {
  display: flex;
  align-items: center;
  justify-content: space-around;
  gap: clamp(4px, 0.8vw, 12px);
  padding-top: clamp(4px, 0.8vh, 8px);
  margin-top: clamp(4px, 0.6vh, 8px);
  border-top: 1px solid var(--sb-line-subtle);
  flex-shrink: 0;
}

.sb-penalty-item {
  flex: 1;
  height: clamp(28px, 4.4vh, 44px);
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--sb-mono);
  font-weight: 900;
  font-size: clamp(1rem, 2.4vh, 1.7rem);
  background-color: var(--sb-penalty-slot);
  color: var(--sb-text-dim);
  border: 1px solid var(--sb-line-subtle);
  transition: background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease;
}

.sb-card-aka .sb-penalty-item.is-active {
  background-color: var(--sb-aka);
  color: var(--sb-text-bright);
  border-color: var(--sb-aka);
}

.sb-card-ao .sb-penalty-item.is-active {
  background-color: var(--sb-ao);
  color: var(--sb-text-bright);
  border-color: var(--sb-ao);
}

/* ==================== BOTTOM FOOTER (GIANT ARENA TIMER VAULT) ==================== */
.sb-footer {
  height: clamp(84px, 17vh, 152px);
  min-height: clamp(84px, 17vh, 152px);
  max-height: clamp(84px, 17vh, 152px);
  background-color: var(--sb-footer-bg);
  color: var(--sb-text);
  border-top: 1px solid var(--sb-line-subtle);
  padding: 0 clamp(16px, 2vw, 32px);
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  flex-shrink: 0;
  box-sizing: border-box;
}

.sb-footer-left {
  display: flex;
  align-items: center;
  justify-content: flex-start;
}

.sb-sponsor-brand {
  height: clamp(48px, 10.5vh, 96px);
  max-width: clamp(160px, 24vw, 360px);
  object-fit: contain;
}

.sb-footer-center {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.sb-timer-vault {
  height: clamp(72px, 15vh, 136px);
  background-color: var(--sb-vault-bg);
  border: 3px solid var(--sb-line-accent);
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 clamp(24px, 3.5vw, 56px);
  transition: border-color 0.2s ease, background-color 0.2s ease;
  box-sizing: border-box;
}
.sb-timer-vault.is-running {
  border-color: var(--sb-gold);
  background-color: var(--sb-pod-bg);
}

.sb-timer-digits {
  font-family: var(--sb-mono);
  font-weight: 900;
  font-size: clamp(3.8rem, 13.5vh, 10.5rem);
  line-height: 1;
  color: var(--sb-text-bright);
  font-variant-numeric: tabular-nums;
}
.sb-timer-vault.is-running .sb-timer-digits {
  color: var(--sb-gold-bright);
}

.sb-footer-right {
  display: flex;
  align-items: center;
  justify-content: flex-end;
}

.sb-team-standings {
  display: flex;
  align-items: center;
  gap: clamp(8px, 1.2vw, 16px);
  background-color: var(--sb-vault-bg);
  border: 1px solid var(--sb-border-gold);
  border-radius: 8px;
  padding: clamp(4px, 1vh, 12px) clamp(12px, 1.4vw, 20px);
}

.sb-team-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.sb-team-meta-title {
  font-size: clamp(0.9rem, 1.6vh, 1.25rem);
  font-weight: 700;
  color: var(--sb-gold-bright);
}

.sb-team-meta-pts {
  font-size: clamp(0.85rem, 1.4vh, 1.1rem);
  color: var(--sb-text-sub);
}

.sb-team-score-display {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--sb-mono);
  font-weight: 900;
  font-size: clamp(2rem, 5vh, 4rem);
  line-height: 1;
  text-wrap: balance;
}
.sb-team-num.aka { color: var(--sb-aka); }
.sb-team-num.ao { color: var(--sb-ao); }
.sb-team-num-dash { color: var(--sb-text-dim); }

.sb-arena-badge {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  color: var(--sb-text-sub);
}
.sb-arena-label {
  font-size: clamp(1rem, 2vh, 1.5rem);
  font-weight: 700;
  color: var(--sb-text-bright);
}
.sb-arena-sub {
  font-size: clamp(0.8rem, 1.5vh, 1.15rem);
}

/* ==================== KATA 5-JUDGE OVERLAY ==================== */
.sb-overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: clamp(8px, 2vw, 24px);
  background-color: var(--sb-overlay-backdrop);
}

.sb-overlay-card {
  width: 100%;
  max-width: clamp(340px, 86vw, 1050px);
  background-color: var(--sb-overlay-bg);
  color: var(--sb-text);
  border: 2px solid var(--sb-border-gold);
  border-radius: 16px;
  padding: clamp(8px, 1.8vh, 20px);
  box-shadow: var(--sb-overlay-shadow);
}

.sb-judges-grid {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: clamp(8px, 1.2vw, 16px);
}

.sb-judge-pod {
  flex: 1;
  background-color: var(--sb-card-base);
  border: 1px solid var(--sb-line-subtle);
  border-radius: 8px;
  padding: clamp(4px, 1vh, 12px);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: clamp(4px, 0.8vh, 8px);
  box-sizing: border-box;
}
.sb-judge-pod.is-aka-win { border-color: var(--sb-border-aka); }
.sb-judge-pod.is-ao-win { border-color: var(--sb-border-ao); }

.sb-jscore {
  width: 100%;
  padding: clamp(4px, 1vh, 12px) 0;
  border-radius: 8px;
  font-family: var(--sb-mono);
  font-weight: 900;
  font-size: clamp(2rem, 5vh, 4.6rem);
  line-height: 1;
  text-align: center;
  color: var(--sb-text-bright);
  background-color: var(--sb-pod-bg);
  border: 1px solid var(--sb-line-subtle);
}
.sb-jscore.aka.leading {
  background-color: var(--sb-aka);
  border-color: var(--sb-aka);
}
.sb-jscore.ao.leading {
  background-color: var(--sb-ao);
  border-color: var(--sb-ao);
}

.sb-jlabel {
  font-size: clamp(0.95rem, 1.8vh, 1.4rem);
  font-weight: 700;
  color: var(--sb-gold-bright);
}

/* ==================== ACCESSIBILITY & PREFERS-REDUCED-MOTION ==================== */
@media (prefers-reduced-motion: reduce) {
  * {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
</style>
