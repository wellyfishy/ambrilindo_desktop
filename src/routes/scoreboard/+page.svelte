<script lang="ts">
  // Arena TV scoreboard page. Receives the tatami's live state (Tauri IPC on the
  // host, LAN WebSocket on other devices, BroadcastChannel between windows) and
  // hands every message to the shared ScoreboardDisplay template, which is also
  // used for the preview inside the control panel.
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import { listen, emit } from "@tauri-apps/api/event";
  import { playFinishSound, unlockAudio } from "$lib/sounds";
  import { isHostUi, setConnectionConfig } from "$lib/api_client";
  import { connectTatamiSocket, type TatamiSocket } from "$lib/liveSync";
  import ScoreboardDisplay from "$lib/components/ScoreboardDisplay.svelte";
  import PortalExitButton from "$lib/components/PortalExitButton.svelte";

  // Tatami number from query string, defaults to 1
  let tatamiNumber = $derived(Number(page.url.searchParams.get("tatami") || "1"));
  let isLocalSyncConnected = $state(false);
  let display: ReturnType<typeof ScoreboardDisplay> | undefined = $state();

  function applyLiveState(data: any) {
    if (!data) return;
    isLocalSyncConnected = true;
    display?.apply(data);
  }

  function playEndBuzzer() {
    try {
      playFinishSound();
    } catch {}
  }

  let unlistenSync: (() => void) | null = null;
  let tatamiSocket: TatamiSocket | null = null;
  let bc: BroadcastChannel | null = null;

  onMount(() => {
    unlockAudio();

    // Opened by an operator-tambahan laptop: keep talking to its host.
    const qHost = page.url.searchParams.get("hostUrl");
    if (qHost) {
      setConnectionConfig("client", qHost);
    }

    // On a TV/browser device or an operator-tambahan laptop, live state arrives from
    // the host over WebSocket (the host's own windows get it over Tauri IPC instead).
    if (!isHostUi()) {
      tatamiSocket = connectTatamiSocket(
        "scoring-board",
        tatamiNumber,
        (msg) => {
          if (msg?.type === "tatami_live_state") {
            const data = msg.payload || msg.data;
            if (data && data.tatami == tatamiNumber) applyLiveState(data);
          } else if (msg?.type === "manual_buzzer" && (!msg.tatami || msg.tatami == tatamiNumber)) {
            playEndBuzzer();
          }
        },
        // Ask the control panel for its current state so a freshly opened or
        // reconnected scoreboard is not blank until the next change.
        (sock) => sock.send({ type: "request_tatami_state", tatami: tatamiNumber }),
        undefined,
        // A TV on Wi-Fi can lose the link without a close event: re-ask after 20 s of
        // silence and reconnect if the host does not answer.
        { staleMs: 20000, onStale: (sock) => sock.send({ type: "request_tatami_state", tatami: tatamiNumber }) }
      );
    }
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

    if (typeof BroadcastChannel !== "undefined") {
      bc = new BroadcastChannel("ambrilindo_live_sync");
      bc.onmessage = (event) => {
        if (event.data) {
          if (event.data.type === "manual_buzzer") {
            if (!event.data.tatami || event.data.tatami == tatamiNumber) {
              playEndBuzzer();
            }
          } else if (!event.data.tatami || event.data.tatami == tatamiNumber) {
            applyLiveState(event.data);
          }
        }
      };
    }
  });

  onDestroy(() => {
    if (unlistenSync) unlistenSync();
    tatamiSocket?.close();
    bc?.close();
  });
</script>

<div class="sb-page">
  <ScoreboardDisplay bind:this={display} tatami={tatamiNumber} connected={isLocalSyncConnected} />
</div>
<PortalExitButton title="Keluar dari Scoreboard?" autohide />

<style>
  .sb-page {
    width: 100vw;
    height: 100vh;
    height: 100dvh;
    overflow: hidden;
    background-color: #000;
  }
</style>
