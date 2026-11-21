<script lang="ts">
  /**
   * Signature under a results recap (klasemen kontingen / perguruan, daftar juara): place and
   * date, the role (e.g. Technical Delegate), the scanned signature and the name, right-aligned.
   * The scan's white paper is blended away (multiply), so only the ink shows on the page.
   */
  import type { SignatureInfo } from "$lib/cover_settings";

  let { signature }: { signature: SignatureInfo } = $props();
</script>

<div class="signature-block mt-8 flex justify-end break-inside-avoid">
  <div class="w-[70mm] text-center text-[11px] text-black leading-snug">
    {#if signature.tempatTanggal}
      <div>{signature.tempatTanggal}</div>
    {/if}
    <div class="font-semibold">{signature.jabatan}</div>
    <div class="h-[26mm] flex items-center justify-center">
      {#if signature.image}
        <img src={signature.image} alt="Tanda tangan {signature.nama}" class="max-h-[26mm] max-w-[60mm] object-contain mix-blend-multiply" />
      {/if}
    </div>
    <div class="font-bold underline underline-offset-2">{signature.nama}</div>
  </div>
</div>

<style>
  .signature-block {
    page-break-inside: avoid;
  }
</style>
