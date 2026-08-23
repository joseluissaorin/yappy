<script lang="ts">
  import { onMount } from "svelte";
  import { type Voice, type Settings, listVoices, getSettings, isModelReady, setVoice, sampleVoice } from "$lib/ipc";

  let voices: Voice[] = $state([]);
  let settings: Settings | null = $state(null);
  let modelReady = $state(false);
  let voiceSearch = $state("");

  onMount(async () => {
    voices = await listVoices();
    settings = await getSettings();
    modelReady = await isModelReady();
  });

  const filteredVoices = $derived(
    voices.filter((v) => {
      const q = voiceSearch.trim().toLowerCase();
      if (!q) return true;
      return (
        v.name.toLowerCase().includes(q) ||
        v.description.toLowerCase().includes(q) ||
        v.tags.some((t) => t.toLowerCase().includes(q))
      );
    }),
  );

  async function pickVoice(v: Voice) {
    if (!settings) return;
    settings = { ...settings, voice: v.name };
    await setVoice(v.name);
  }
  async function playVoiceSample(v: Voice, e?: Event) {
    e?.stopPropagation();
    if (modelReady) await sampleVoice(v.name);
  }
</script>

<section class="voices-grid-wrap">
  <header class="section-head">
    <h2>voices</h2>
    <p>click a card to select. tap the speaker icon to hear a sample.</p>
  </header>
  <div class="voice-controls">
    <input class="search" type="search" placeholder="search voices…" bind:value={voiceSearch} />
  </div>
  <div class="voices-grid">
    {#each filteredVoices as v}
      <div class="voice-card" class:active={settings?.voice === v.name} role="button" tabindex="0"
        onclick={() => pickVoice(v)}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pickVoice(v); } }}>
        <div class="vc-head">
          <div class="voice-avatar" data-id={v.id}>{v.name[0]}</div>
          <button class="sample-btn" onclick={(e) => playVoiceSample(v, e)} title="sample">
            <svg width="11" height="11" viewBox="0 0 14 14" fill="currentColor"><path d="M3 1.5C3 0.7 3.85 0.25 4.5 0.7L12.5 6.2c0.6 0.4 0.6 1.3 0 1.7l-8 5.5c-0.7 0.4-1.5 0-1.5-0.8V1.5Z"/></svg>
          </button>
        </div>
        <div class="voice-meta">
          <div class="voice-name">{v.name}</div>
          <div class="voice-tags">{#each v.tags as t}<span class="voice-tag">{t}</span>{/each}</div>
        </div>
        <p class="voice-desc">{v.description}</p>
        <div class="voice-actions">
          <span class="voice-id">{v.id} · {v.gender.toLowerCase()}</span>
          {#if settings?.voice === v.name}<span class="voice-active-pill">selected</span>{/if}
        </div>
      </div>
    {/each}
  </div>
</section>
