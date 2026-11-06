/**
 * Ambrilindo Sound System
 * Utilizes the official Ambrilindo audio assets:
 * - warning.mp3 (Atoshibaraku / 15s warning chime)
 * - finish.mp3  (Arena match-end buzzer & manual buzzer)
 */

let warningAudio: HTMLAudioElement | null = null;
let finishAudio: HTMLAudioElement | null = null;
let audioUnlocked = false;

function initAudio() {
  if (typeof window === "undefined") return;
  if (!warningAudio) {
    try {
      warningAudio = new Audio("/sound/warning.mp3");
      warningAudio.preload = "auto";
    } catch (e) {
      console.warn("Failed to initialize warningAudio:", e);
    }
  }
  if (!finishAudio) {
    try {
      finishAudio = new Audio("/sound/finish.mp3");
      finishAudio.preload = "auto";
    } catch (e) {
      console.warn("Failed to initialize finishAudio:", e);
    }
  }
}

/**
 * Unlock audio context/playback on first user interaction
 */
export function unlockAudio() {
  if (typeof window === "undefined" || audioUnlocked) return;
  initAudio();

  const tryUnlock = (audio: HTMLAudioElement | null) => {
    if (!audio) return Promise.resolve();
    try {
      audio.muted = true;
      const p = audio.play();
      if (p && typeof p.then === "function") {
        return p
          .then(() => {
            audio.pause();
            audio.currentTime = 0;
            audio.muted = false;
          })
          .catch(() => {
            audio.muted = false;
          });
      }
      audio.muted = false;
      return Promise.resolve();
    } catch {
      if (audio) audio.muted = false;
      return Promise.resolve();
    }
  };

  Promise.allSettled([tryUnlock(warningAudio), tryUnlock(finishAudio)]).then(() => {
    audioUnlocked = true;
  });
}

/**
 * Fallback Web Audio oscillator in case HTMLAudioElement is blocked
 */
function playOscillatorFallback(freqs: number[], durationSec: number, wave: OscillatorType = "sine") {
  try {
    const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
    if (!AudioCtx) return;
    const ctx = new AudioCtx();
    const now = ctx.currentTime;
    freqs.forEach((freq) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = wave;
      osc.frequency.setValueAtTime(freq, now);
      gain.gain.setValueAtTime(0.35, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + durationSec);
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start(now);
      osc.stop(now + durationSec);
    });
  } catch {}
}

/**
 * Play warning alarm (warning.mp3)
 */
export function playWarningSound() {
  if (typeof window === "undefined") return;
  initAudio();

  if (warningAudio) {
    try {
      warningAudio.pause();
      warningAudio.currentTime = 0;
      warningAudio.volume = 1.0;
      const p = warningAudio.play();
      if (p && typeof p.catch === "function") {
        p.catch(() => {
          // Play oscillator fallback if browser blocked the file playback
          playOscillatorFallback([880, 1046], 0.25, "sine");
        });
      }
    } catch {
      playOscillatorFallback([880, 1046], 0.25, "sine");
    }
  } else {
    playOscillatorFallback([880, 1046], 0.25, "sine");
  }
}

/**
 * Play finish buzzer / arena bell (finish.mp3)
 */
export function playFinishSound() {
  if (typeof window === "undefined") return;
  initAudio();

  if (finishAudio) {
    try {
      finishAudio.pause();
      finishAudio.currentTime = 0;
      finishAudio.volume = 1.0;
      const p = finishAudio.play();
      if (p && typeof p.catch === "function") {
        p.catch(() => {
          // Play oscillator fallback if browser blocked the file playback
          playOscillatorFallback([220, 440], 1.2, "sawtooth");
        });
      }
    } catch {
      playOscillatorFallback([220, 440], 1.2, "sawtooth");
    }
  } else {
    playOscillatorFallback([220, 440], 1.2, "sawtooth");
  }
}

// Auto-register unlock listeners
if (typeof window !== "undefined") {
  ["click", "pointerdown", "mousedown", "keydown", "touchstart"].forEach((evt) => {
    window.addEventListener(evt, () => unlockAudio(), { passive: true, once: true });
  });
}
