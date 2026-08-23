<script lang="ts">
  import HistoryList from "$lib/HistoryList.svelte";
</script>

<section class="hist-wrap">
  <header class="section-head">
    <h2>history</h2>
    <p>your recent reads. replay anything, anytime.</p>
  </header>
  <div class="card pref-card"><HistoryList /></div>
</section>
