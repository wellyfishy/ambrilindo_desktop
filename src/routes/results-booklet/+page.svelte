<script lang="ts">
  import { page } from "$app/stores";
  import ResultsBookletView from "$lib/components/ResultsBookletView.svelte";

  let eventId = $derived(
    parseInt($page.url.searchParams.get("event_id") || "1") || 1
  );
</script>

<svelte:head>
  <title>Buku Hasil & Berita Acara Resmi</title>
</svelte:head>

<div class="h-screen w-screen overflow-hidden">
  <ResultsBookletView {eventId} />
</div>
