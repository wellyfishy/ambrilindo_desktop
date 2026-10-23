<script lang="ts">
  import { tick } from "svelte";

  export interface SelectOption {
    value: any;
    label: string;
    subLabel?: string | null;
    badge?: string | null;
  }

  interface Props {
    items: SelectOption[];
    value: any;
    placeholder?: string;
    allowClear?: boolean;
    clearLabel?: string;
    clearValue?: any;
    disabled?: boolean;
    compact?: boolean;
    id?: string;
    class?: string;
  }

  let {
    items = [],
    value = $bindable(),
    placeholder = "Pilih...",
    allowClear = false,
    clearLabel = "Semua",
    clearValue = "all",
    disabled = false,
    compact = false,
    id,
    class: customClass = "",
  }: Props = $props();

  let isOpen = $state(false);
  let searchQuery = $state("");
  let highlightedIndex = $state(0);
  let searchInputEl = $state<HTMLInputElement | null>(null);
  let containerEl = $state<HTMLDivElement | null>(null);
  let listEl = $state<HTMLUListElement | null>(null);

  function isSelected(optVal: any): boolean {
    if (value === optVal) return true;
    if (value === null || value === undefined || optVal === null || optVal === undefined) return false;
    return String(value) === String(optVal);
  }

  // Selected Option Finder (robust against string vs number mismatch)
  let selectedOption = $derived(items.find((item) => isSelected(item.value)));

  // Fuzzy match scoring function
  function fuzzyScore(query: string, text: string): number {
    const q = query.toLowerCase().trim();
    const t = text.toLowerCase().trim();
    if (!q) return 1;
    if (t === q) return 100;
    if (t.startsWith(q)) return 85;
    if (t.includes(q)) return 70;

    // Token matching (all query words exist anywhere in target text)
    const tokens = q.split(/\s+/).filter(Boolean);
    if (tokens.length > 1) {
      const allFound = tokens.every((tok) => t.includes(tok));
      if (allFound) return 60;
    }

    // Subsequence fuzzy match (letters appear in sequence)
    let qIdx = 0;
    for (let i = 0; i < t.length && qIdx < q.length; i++) {
      if (t[i] === q[qIdx]) {
        qIdx++;
      }
    }
    if (qIdx === q.length) return 40;

    return 0;
  }

  // Filtered and ranked options
  let filteredItems = $derived.by(() => {
    const q = searchQuery.trim();
    if (!q) return items;

    const scored = items.map((item) => {
      const labelScore = fuzzyScore(q, item.label);
      const subScore = item.subLabel ? fuzzyScore(q, item.subLabel) * 0.8 : 0;
      const score = Math.max(labelScore, subScore);
      return { item, score };
    });

    return scored
      .filter((s) => s.score > 0)
      .sort((a, b) => b.score - a.score)
      .map((s) => s.item);
  });

  async function openDropdown() {
    if (disabled) return;
    isOpen = true;
    searchQuery = "";
    // Set initial highlighted index to currently selected item if visible
    const curIdx = items.findIndex((item) => isSelected(item.value));
    highlightedIndex = curIdx >= 0 ? curIdx : 0;
    await tick();
    if (searchInputEl) {
      searchInputEl.focus();
    }
    scrollToHighlighted();
  }

  function closeDropdown() {
    isOpen = false;
    searchQuery = "";
  }

  function selectItem(optVal: any) {
    value = optVal;
    closeDropdown();
  }

  function handleClear(e: MouseEvent) {
    e.stopPropagation();
    value = clearValue;
    closeDropdown();
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (!isOpen) {
      if (e.key === "Enter" || e.key === "ArrowDown" || e.key === " ") {
        e.preventDefault();
        openDropdown();
      }
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (filteredItems.length > 0) {
        highlightedIndex = (highlightedIndex + 1) % filteredItems.length;
        scrollToHighlighted();
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (filteredItems.length > 0) {
        highlightedIndex = (highlightedIndex - 1 + filteredItems.length) % filteredItems.length;
        scrollToHighlighted();
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filteredItems.length > 0 && filteredItems[highlightedIndex]) {
        selectItem(filteredItems[highlightedIndex].value);
      }
    } else if (e.key === "Escape" || e.key === "Tab") {
      closeDropdown();
    }
  }

  function scrollToHighlighted() {
    if (!listEl) return;
    const item = listEl.children[highlightedIndex] as HTMLElement;
    if (item) {
      item.scrollIntoView({ block: "nearest" });
    }
  }

  // Click outside listener
  function handleDocumentClick(e: MouseEvent) {
    if (isOpen && containerEl && !containerEl.contains(e.target as Node)) {
      closeDropdown();
    }
  }

  $effect(() => {
    if (isOpen) {
      window.addEventListener("click", handleDocumentClick);
      return () => window.removeEventListener("click", handleDocumentClick);
    }
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={containerEl}
  class="relative inline-block text-left {customClass}"
  onkeydown={handleKeyDown}
>
  <!-- Trigger Button -->
  <button
    type="button"
    {id}
    role="combobox"
    aria-expanded={isOpen}
    aria-haspopup="listbox"
    aria-controls={id ? `${id}-listbox` : undefined}
    onclick={() => (isOpen ? closeDropdown() : openDropdown())}
    {disabled}
    class="w-full flex items-center justify-between text-left transition bg-[#181a20] hover:bg-[#1f222a] border {isOpen ? 'border-white' : 'border-white/[0.1] hover:border-white/20'} rounded-xl text-white outline-none {compact ? 'px-2.5 py-1.5 text-xs' : 'px-3 py-2 text-xs font-medium'} disabled:opacity-50 disabled:pointer-events-none"
  >
    <div class="flex items-center space-x-1.5 truncate mr-2 min-w-0">
      {#if selectedOption}
        <span class="truncate font-semibold {isSelected(clearValue) ? 'text-zinc-400' : 'text-zinc-100'}">
          {selectedOption.label}
        </span>
        {#if selectedOption.badge}
          <span class="text-[9.5px] font-mono px-1 py-0.2 rounded bg-white/[0.06] text-zinc-400 border border-white/[0.1] shrink-0">
            {selectedOption.badge}
          </span>
        {/if}
      {:else}
        <span class="text-zinc-500 truncate">{placeholder}</span>
      {/if}
    </div>

    <div class="flex items-center space-x-1 shrink-0 text-zinc-400">
      {#if allowClear && selectedOption && !isSelected(clearValue)}
        <span
          role="button"
          tabindex="0"
          onclick={handleClear}
          onkeydown={(e) => { if (e.key === 'Enter') handleClear(e as any); }}
          class="p-0.5 hover:text-white rounded hover:bg-white/[0.1] transition"
          title="Reset pilihan"
        >
          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </span>
      {/if}
      <svg class="w-3.5 h-3.5 transition-transform {isOpen ? 'rotate-180 text-white' : 'text-zinc-500'}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
      </svg>
    </div>
  </button>

  <!-- Dropdown Menu Box -->
  {#if isOpen}
    <div
      class="absolute z-50 left-0 mt-1.5 w-full min-w-[240px] max-w-lg bg-[#181a20] border border-white/20 rounded-xl shadow-2xl overflow-hidden animate-fadeIn"
    >
      <!-- Search Input Header -->
      <div class="p-2 border-b border-white/[0.08] bg-[#111215]">
        <div class="relative">
          <input
            bind:this={searchInputEl}
            type="text"
            bind:value={searchQuery}
            placeholder="Ketik untuk mencari..."
            class="w-full bg-[#181a20] text-xs text-white pl-7 pr-7 py-1.5 rounded-lg border border-white/[0.12] focus:border-white outline-none"
            oninput={() => { highlightedIndex = 0; }}
          />
          <svg class="w-3.5 h-3.5 text-zinc-500 absolute left-2 top-1/2 -translate-y-1/2 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          {#if searchQuery}
            <button
              type="button"
              onclick={() => { searchQuery = ""; highlightedIndex = 0; }}
              class="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-white text-xs"
            >
              ✕
            </button>
          {/if}
        </div>
      </div>

      <!-- Option List -->
      <ul
        bind:this={listEl}
        role="listbox"
        id={id ? `${id}-listbox` : undefined}
        class="max-h-56 overflow-y-auto p-1 divide-y divide-white/[0.02] scrollbar-thin text-xs"
      >
        <!-- Optional Clear / All Option -->
        {#if allowClear && !searchQuery}
          <li
            role="option"
            aria-selected={isSelected(clearValue)}
            onclick={() => selectItem(clearValue)}
            onkeydown={(e) => { if (e.key === 'Enter') selectItem(clearValue); }}
            class="px-3 py-1.5 rounded-lg cursor-pointer flex items-center justify-between text-zinc-400 hover:text-white hover:bg-white/[0.06] transition {isSelected(clearValue) ? 'bg-white/[0.08] font-bold text-white' : ''}"
          >
            <span>{clearLabel}</span>
            {#if isSelected(clearValue)}
              <svg class="w-3.5 h-3.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
              </svg>
            {/if}
          </li>
        {/if}

        {#if filteredItems.length === 0}
          <li class="px-3 py-4 text-center text-zinc-500 text-xs font-normal">
            Pilihan tidak ditemukan
          </li>
        {:else}
          {#each filteredItems as opt, idx (opt.value)}
            <li
              role="option"
              aria-selected={isSelected(opt.value)}
              onclick={() => selectItem(opt.value)}
              onkeydown={(e) => { if (e.key === 'Enter') selectItem(opt.value); }}
              class="px-3 py-1.5 rounded-lg cursor-pointer flex items-center justify-between transition {isSelected(opt.value) ? 'bg-white/[0.1] font-bold text-white' : idx === highlightedIndex ? 'bg-white/[0.06] text-white' : 'text-zinc-300 hover:bg-white/[0.04]'}"
            >
              <div class="min-w-0 truncate mr-2">
                <div class="truncate font-medium">{opt.label}</div>
                {#if opt.subLabel}
                  <div class="text-[10px] text-zinc-500 truncate">{opt.subLabel}</div>
                {/if}
              </div>

              <div class="flex items-center space-x-1.5 shrink-0">
                {#if opt.badge}
                  <span class="text-[9px] font-mono font-bold px-1 py-0.2 rounded bg-white/[0.06] text-zinc-400 border border-white/[0.08]">
                    {opt.badge}
                  </span>
                {/if}
                {#if isSelected(opt.value)}
                  <svg class="w-3.5 h-3.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
                  </svg>
                {/if}
              </div>
            </li>
          {/each}
        {/if}
      </ul>
    </div>
  {/if}
</div>

<style>
  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .animate-fadeIn {
    animation: fadeIn 0.1s ease-out forwards;
  }
</style>
