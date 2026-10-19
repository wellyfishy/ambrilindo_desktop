<script lang="ts">
  import type { ActiveChannel } from "$lib/types";

  interface Props {
    activeChannel: ActiveChannel;
  }

  let { activeChannel }: Props = $props();
</script>

<header class="h-14 bg-[#111216] border-b border-white/[0.08] px-5 flex items-center justify-between shadow-sm select-none shrink-0 z-10 font-sans">
  <!-- Left: Channel Title & Description -->
  <div class="flex items-center space-x-3 truncate">
    <div class="flex items-center space-x-2 font-bold text-white text-sm truncate">
      <span class="text-zinc-500 font-mono text-base">#</span>
      <span class="truncate tracking-wide">{activeChannel.name}</span>
    </div>

    {#if activeChannel.subtitle}
      <div class="hidden sm:flex items-center space-x-3 text-xs text-zinc-400 truncate border-l border-white/[0.08] pl-3 font-normal">
        <span class="truncate">{activeChannel.subtitle}</span>
      </div>
    {/if}
  </div>
</header>
