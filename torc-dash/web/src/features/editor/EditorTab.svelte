<!-- Loads the spec editor (CodeMirror, YAML parser, schema validator) only when the tab opens. -->
<script lang="ts">
  const page = import("./EditorPage.svelte");
</script>

{#await page}
  <div class="empty"><span class="spinner"></span></div>
{:then m}
  <m.default />
{:catch e}
  <div class="empty error-text">Could not load the editor: {e}</div>
{/await}
