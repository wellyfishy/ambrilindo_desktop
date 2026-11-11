<script lang="ts">
  // Referee photo (webp from the host's media folder) or initials when there is none.
  import { getHostBaseUrl } from "$lib/api_client";
  import { resolveMediaLogos } from "$lib/media_helper";

  interface Props {
    foto?: string | null;
    nama?: string | null;
    /** px */
    size?: number;
    rounded?: string;
  }
  let { foto = null, nama = "", size = 40, rounded = "rounded-lg" }: Props = $props();

  let src = $state<string | null>(null);
  $effect(() => {
    const path = foto;
    src = null;
    if (!path) return;
    let alive = true;
    resolveMediaLogos([path], getHostBaseUrl()).then((m) => {
      if (alive) src = m[path] ?? null;
    });
    return () => (alive = false);
  });

  let initials = $derived(
    (nama || "?")
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0])
      .join("")
      .toUpperCase()
  );
</script>

<div
  class="shrink-0 overflow-hidden bg-white/[0.06] border border-white/[0.1] flex items-center justify-center {rounded}"
  style="width: {size}px; height: {size}px;"
>
  {#if src}
    <img {src} alt={nama || "Wasit"} class="w-full h-full object-cover object-top" loading="lazy" onerror={() => (src = null)} />
  {:else}
    <span class="font-black font-mono text-zinc-400" style="font-size: {Math.max(9, size * 0.34)}px;">{initials}</span>
  {/if}
</div>
