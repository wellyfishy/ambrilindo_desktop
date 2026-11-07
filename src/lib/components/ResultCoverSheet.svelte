<!--
  Landscape cover of the results booklet, a 1:1 rebuild of "sampul turney.pdf".
  The design is laid out on a 960 × 540 grid (the PDF's own coordinates) and scales
  with the sheet width through container query units, so the preview and the
  printed page match at any size.
-->
<script lang="ts">
  import "@fontsource/oswald/700.css";
  import "@fontsource/roboto/900.css";
  import "@fontsource/inter/600.css";
  import { DEFAULT_COVER_BACKGROUND } from "$lib/cover_settings";

  interface Props {
    federasi: string;
    badge: string;
    judul: string;
    namaEvent: string;
    tempat: string;
    tanggal: string;
    showSupportedBy: boolean;
    /** Resolved image URL for the logo card, or null for no card. */
    logoUrl: string | null;
    backgroundUrl?: string | null;
  }

  let {
    federasi,
    badge,
    judul,
    namaEvent,
    tempat,
    tanggal,
    showSupportedBy,
    logoUrl,
    backgroundUrl = null,
  }: Props = $props();
</script>

<div class="cover-sheet">
  <div class="cover-grid">
    <img class="cover-bg" src={backgroundUrl || DEFAULT_COVER_BACKGROUND} alt="" />

    {#if federasi}
      <div class="cover-federasi">{federasi}</div>
    {/if}

    {#if logoUrl}
      <div class="cover-logo-card">
        <img src={logoUrl} alt="Logo event" />
      </div>
    {/if}

    <div class="cover-text">
      {#if badge}
        <div class="cover-badge">{badge}</div>
      {/if}
      {#if judul}
        <div class="cover-judul">{judul}</div>
      {/if}
      {#if namaEvent}
        <div class="cover-event">{namaEvent}</div>
      {/if}
      {#if tempat || tanggal}
        <div class="cover-meta">
          {#if tempat}<div>{tempat}</div>{/if}
          {#if tanggal}<div>{tanggal}</div>{/if}
        </div>
      {/if}
    </div>

    {#if showSupportedBy}
      <div class="cover-supported">SUPPORTED BY:</div>
    {/if}
  </div>
</div>

<style>
  /* The sheet is exactly 16:9, like the source PDF (720 × 405 pt). */
  .cover-sheet {
    container-type: inline-size;
    width: 100%;
    aspect-ratio: 960 / 540;
    background: #111827;
    overflow: hidden;
    position: relative;
    -webkit-print-color-adjust: exact;
    print-color-adjust: exact;
  }

  .cover-grid {
    /* 1 design unit = 1/960 of the sheet width. */
    --u: calc(100cqw / 960);
    position: absolute;
    inset: 0;
  }

  .cover-bg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .cover-federasi {
    position: absolute;
    left: calc(543.5 * var(--u));
    top: calc(19.5 * var(--u));
    font-family: "Oswald", "Arial Narrow", sans-serif;
    font-weight: 700;
    font-size: calc(16 * var(--u));
    line-height: 1.2;
    letter-spacing: 0.01em;
    color: #ffffff;
    white-space: nowrap;
    text-shadow: 0 calc(1 * var(--u)) calc(3 * var(--u)) rgba(0, 0, 0, 0.8);
  }

  /* Box of the logo image in the PDF: x 665.6, y 83.2, 291.8 × 165.4. */
  .cover-logo-card {
    position: absolute;
    left: calc(675 * var(--u));
    top: calc(92 * var(--u));
    width: calc(273 * var(--u));
    height: calc(146 * var(--u));
    background: #ffffff;
    box-shadow: 0 calc(3 * var(--u)) calc(9 * var(--u)) rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: calc(10 * var(--u));
    box-sizing: border-box;
  }

  .cover-logo-card img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    display: block;
  }

  .cover-text {
    position: absolute;
    left: calc(541 * var(--u));
    top: calc(247 * var(--u));
    width: calc(405 * var(--u));
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  .cover-badge {
    height: calc(27 * var(--u));
    padding: 0 calc(9.8 * var(--u));
    border-radius: calc(3 * var(--u));
    background: linear-gradient(180deg, #f8a832 0%, #e0820f 55%, #b8620a 100%);
    font-family: "Oswald", "Arial Narrow", sans-serif;
    font-weight: 700;
    font-size: calc(14 * var(--u));
    line-height: calc(27 * var(--u));
    letter-spacing: 0.02em;
    color: #111111;
    white-space: nowrap;
  }

  .cover-judul {
    margin-top: calc(4.6 * var(--u));
    font-family: "Roboto", "Arial Black", sans-serif;
    font-weight: 900;
    font-size: calc(34 * var(--u));
    line-height: calc(37 * var(--u));
    color: #ffffff;
    text-shadow: 0 calc(2 * var(--u)) calc(4 * var(--u)) rgba(0, 0, 0, 0.6);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .cover-event {
    margin-top: calc(4.4 * var(--u));
    font-family: "Roboto", "Arial Black", sans-serif;
    font-weight: 900;
    font-size: calc(20 * var(--u));
    line-height: calc(24 * var(--u));
    color: #facc15;
    text-shadow: 0 calc(1 * var(--u)) calc(3 * var(--u)) rgba(0, 0, 0, 0.6);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .cover-meta {
    margin-top: calc(11.6 * var(--u));
    font-family: "Inter", "Segoe UI", sans-serif;
    font-weight: 600;
    font-size: calc(14 * var(--u));
    line-height: calc(19 * var(--u));
    color: #ffff00;
    text-shadow: 0 calc(1 * var(--u)) calc(2 * var(--u)) rgba(0, 0, 0, 0.6);
  }

  .cover-supported {
    position: absolute;
    left: calc(781.6 * var(--u));
    top: calc(457 * var(--u));
    font-family: Calibri, Carlito, "Segoe UI", sans-serif;
    font-weight: 700;
    font-size: calc(13 * var(--u));
    line-height: 1.2;
    color: #ffffff;
    white-space: nowrap;
    text-shadow: 0 calc(1 * var(--u)) calc(2 * var(--u)) rgba(0, 0, 0, 0.7);
  }
</style>
