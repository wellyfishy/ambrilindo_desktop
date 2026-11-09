<!--
  One printed bracket page (A4 landscape): banner + bracket, always in the light
  "paper" palette. The single template for printing a bagan: used by the bagan
  channel's print, the results booklet and the tatami booklet.

  The content is laid out at a fixed natural size and scaled to the page box. The
  scale is computed from the box size in millimetres (not measured from the DOM),
  so it is identical on screen, in a hidden print-only copy and on paper.
-->
<script lang="ts">
  import type { DetailBaganItem } from "$lib/types";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import BracketTree from "$lib/components/BracketTree.svelte";
  import FestivalBracketSheet from "$lib/components/FestivalBracketSheet.svelte";
  import { TREE_HEIGHT, TREE_WIDTH, type BracketMeta, type PodiumResult } from "$lib/bracket_layout";

  interface Props {
    kind: "elim" | "festival";
    matches: DetailBaganItem[];
    categoryName: string;
    baganKode?: string | null;
    eventName?: string;
    eventVenue?: string;
    eventDate?: string;
    logoMap?: Record<string, string>;
    /** Tournament bracket only. */
    meta?: BracketMeta | null;
    podium?: PodiumResult | null;
    /** Festival only. */
    festivalGroupCount?: number;
    festivalLayout?: string | null;
    /** Page box; defaults fit A4 landscape with 5 mm margins. */
    widthMm?: number;
    heightMm?: number;
  }

  let {
    kind,
    matches,
    categoryName,
    baganKode = null,
    eventName = "",
    eventVenue = "",
    eventDate = "",
    logoMap = {},
    meta = null,
    podium = null,
    festivalGroupCount = 1,
    festivalLayout = "quadrant",
    widthMm = 285,
    heightMm = 196,
  }: Props = $props();

  // Natural layout size in CSS px.
  const BANNER_H = 58;
  const GAP = 8;
  // The tree grid adds a few px of padding (pb-2 pr-2) around its 1064 x 692 geometry.
  const TREE_BOX_W = TREE_WIDTH + 8;
  const TREE_BOX_H = TREE_HEIGHT + 8;
  const NATURAL_W = TREE_BOX_W;
  const NATURAL_H = BANNER_H + GAP + TREE_BOX_H;
  const PX_PER_MM = 96 / 25.4;

  let scale = $derived(Math.min((widthMm * PX_PER_MM) / NATURAL_W, (heightMm * PX_PER_MM) / NATURAL_H));
  let offsetX = $derived(Math.max(0, (widthMm * PX_PER_MM - NATURAL_W * scale) / 2));

  let discipline = $derived(categoryName.toLowerCase().includes("kata") ? "kata" : "kumite");
  let athletesCount = $derived(
    new Set(matches.flatMap((m) => [m.atlet1_id, m.atlet2_id].filter(Boolean))).size
  );
  let subtitle = $derived([eventName, eventVenue, eventDate].filter((x) => x && x.trim()).join("  •  "));
</script>

<div class="paper-page" style="width: {widthMm}mm; height: {heightMm}mm;">
  <div
    class="paper-content"
    style="width: {NATURAL_W}px; height: {NATURAL_H}px; transform: translateX({offsetX}px) scale({scale});"
  >
    <!-- div, not <header>: app.css hides every <header> in print -->
    <div class="paper-banner" style="height: {BANNER_H}px; margin-bottom: {GAP}px;">
      <div class="paper-banner-logos">
        <img src={FORKI_LOGO_DATA_URL} alt="FORKI" class="paper-logo-forki" />
        <span class="paper-logo-divider"></span>
        <img src={AMBRILINDO_LOGO_DATA_URL} alt="Ambrilindo" class="paper-logo-ambrilindo" />
      </div>
      <div class="paper-banner-title">
        <h1>{baganKode ? `(${baganKode}) ` : ""}{categoryName}</h1>
        {#if kind === "elim" && subtitle}
          <p>{subtitle}</p>
        {/if}
      </div>
    </div>

    <div style="width: {TREE_BOX_W}px; height: {TREE_BOX_H}px; position: relative;">
      {#if kind === "festival"}
        <FestivalBracketSheet
          {categoryName}
          {baganKode}
          {discipline}
          poolCount={festivalGroupCount}
          {athletesCount}
          venue={eventVenue}
          {matches}
          layoutTemplate={festivalLayout || "quadrant"}
          showHeader={false}
          interactive={false}
          isDarkMode={false}
        />
      {:else if meta && podium}
        <BracketTree {matches} {meta} {podium} theme="paper" {logoMap} />
      {/if}
    </div>
  </div>
</div>

<style>
  .paper-page {
    position: relative;
    overflow: hidden;
    background: #ffffff;
    color: #0f172a;
    box-sizing: border-box;
    -webkit-print-color-adjust: exact;
    print-color-adjust: exact;
  }

  .paper-content {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: top left;
  }

  .paper-banner {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 2px solid #0f172a;
    box-sizing: border-box;
  }

  .paper-banner-logos {
    position: absolute;
    left: 4px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .paper-logo-forki {
    height: 36px;
    width: auto;
    max-width: 40px;
    object-fit: contain;
  }

  .paper-logo-divider {
    width: 1.2px;
    height: 26px;
    background: #cbd5e1;
  }

  .paper-logo-ambrilindo {
    height: 22px;
    width: auto;
    max-width: 110px;
    object-fit: contain;
  }

  .paper-banner-title {
    width: 100%;
    padding: 0 170px;
    text-align: center;
    box-sizing: border-box;
  }

  .paper-banner-title h1 {
    margin: 0;
    font-size: 18px;
    line-height: 1.15;
    font-weight: 900;
    letter-spacing: -0.01em;
    text-transform: uppercase;
    color: #020617;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .paper-banner-title p {
    margin: 3px 0 0;
    font-size: 11px;
    font-weight: 700;
    color: #1e293b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
