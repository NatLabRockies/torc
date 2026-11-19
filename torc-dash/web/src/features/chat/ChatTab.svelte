<!-- AI assistant chat over the torc MCP tools, streamed from POST /api/chat. -->
<script lang="ts">
  import { app } from "../../lib/state.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { chat } from "./chat.svelte";
  import ChatSetup from "./ChatSetup.svelte";
  import ChatThread from "./ChatThread.svelte";

  const EXAMPLES = [
    "Show me the failed jobs",
    "Why did my last workflow fail?",
    "Help me create a workflow",
    "Summarize resource usage",
  ];

  $effect(() => {
    chat.checkStatus();
  });

  let showSetup = $derived(chat.available === false || chat.setupOpen);
</script>

<div class="page chat">
  <div class="page-header">
    <h2>AI Assistant</h2>
    {#if app.workflowId !== null}
      <span class="ctx" title="The assistant is told which workflow is selected">
        Context: workflow <span class="mono">#{app.workflowId}</span>
      </span>
    {/if}
    <div class="toolbar">
      {#if chat.available && !chat.setupOpen}
        <button class="btn btn-sm btn-ghost" onclick={() => (chat.setupOpen = true)}>Provider</button>
      {/if}
      <a
        class="btn btn-sm btn-ghost"
        href="https://natlabrockies.github.io/torc/v0.20.4/core/monitoring/dashboard.html#ai-chat-tab"
        target="_blank"
        rel="noopener"
        title="AI Chat documentation"><Icon name="external" size={13} /> Docs</a
      >
      <button
        class="btn btn-sm"
        onclick={() => chat.clear()}
        disabled={chat.streaming || chat.messages.length === 0}
      >
        <Icon name="trash" size={13} /> Clear
      </button>
    </div>
  </div>

  {#if chat.available === null}
    <div class="empty"><span class="spinner"></span></div>
  {:else if showSetup}
    <div class="setup-wrap"><ChatSetup /></div>
  {:else}
    <ChatThread
      session={chat}
      title="Torc AI Assistant"
      subtitle="Ask about your workflows, job status, logs, and more."
      examples={EXAMPLES}
      placeholder="Ask about your workflows…  (Enter to send, Shift+Enter for a new line)"
    />
  {/if}
</div>

<style>
  .chat {
    gap: 12px;
  }
  .page-header h2 {
    margin-right: 0;
  }
  .page-header .toolbar {
    margin-left: auto;
  }
  .ctx {
    padding: 2px 10px;
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
  }
  .setup-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
  }</style>
