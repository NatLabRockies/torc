<!-- AI assistant chat over the torc MCP tools, streamed from POST /api/chat. -->
<script lang="ts">
  import { app } from "../../lib/state.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { chat } from "./chat.svelte";
  import ChatSetup from "./ChatSetup.svelte";
  import Markdown from "./Markdown.svelte";
  import ToolCall from "./ToolCall.svelte";

  const EXAMPLES = [
    "Show me the failed jobs",
    "Why did my last workflow fail?",
    "Help me create a workflow",
    "Summarize resource usage",
  ];

  let draft = $state("");
  let textarea = $state<HTMLTextAreaElement>();
  let scroller = $state<HTMLDivElement>();
  let inner = $state<HTMLDivElement>();
  /** Follow new content only while the user is at the bottom. */
  let stick = true;

  $effect(() => {
    chat.checkStatus();
  });

  // Content grows as tokens, tool results, and lazily rendered markdown arrive.
  $effect(() => {
    if (!inner || !scroller) return;
    const el = scroller;
    const ro = new ResizeObserver(() => {
      if (stick) el.scrollTop = el.scrollHeight;
    });
    ro.observe(inner);
    return () => ro.disconnect();
  });

  function onScroll() {
    if (scroller) stick = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
  }

  function autosize() {
    if (!textarea) return;
    textarea.style.height = "auto";
    textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`;
  }

  function send(text = draft) {
    const t = text.trim();
    if (!t || chat.streaming) return;
    draft = "";
    stick = true;
    queueMicrotask(autosize);
    chat.send(t).then(() => textarea?.focus());
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    }
  }

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
    <div class="scroller" bind:this={scroller} onscroll={onScroll}>
      <div class="thread" bind:this={inner}>
        {#if chat.messages.length === 0}
          <div class="welcome">
            <div class="icon"><Icon name="chat" size={24} /></div>
            <h3>Torc AI Assistant</h3>
            <p class="muted">Ask about your workflows, job status, logs, and more.</p>
            <div class="examples">
              {#each EXAMPLES as ex (ex)}
                <button class="btn btn-sm" onclick={() => send(ex)}>{ex}</button>
              {/each}
            </div>
          </div>
        {/if}

        {#each chat.messages as m, i (i)}
          {#if m.role === "user"}
            <div class="msg user"><div class="bubble">{m.text}</div></div>
          {:else}
            <div class="msg assistant">
              <div class="avatar"><Icon name="chat" size={14} /></div>
              <div class="bubble">
                {#each m.parts as p, j (j)}
                  <div class="part">
                    {#if p.kind === "text"}
                      <Markdown text={p.text} />
                    {:else if p.kind === "tool"}
                      <ToolCall
                        name={p.name}
                        input={p.input}
                        result={p.result}
                        isError={p.isError}
                        pending={chat.streaming && i === chat.messages.length - 1}
                      />
                    {:else}
                      <div class:error-text={p.error} class:muted={!p.error}>{p.text}</div>
                    {/if}
                  </div>
                {/each}
                {#if chat.streaming && i === chat.messages.length - 1}
                  <span class="dots" aria-label="Working"><span></span><span></span><span></span></span>
                {/if}
              </div>
            </div>
          {/if}
        {/each}
      </div>
    </div>

    <div class="composer">
      <textarea
        bind:this={textarea}
        bind:value={draft}
        oninput={autosize}
        onkeydown={onKeydown}
        rows="1"
        placeholder="Ask about your workflows…  (Enter to send, Shift+Enter for a new line)"
        aria-label="Chat message"
      ></textarea>
      {#if chat.streaming}
        <button class="btn btn-danger" onclick={() => chat.stop()}>
          <Icon name="stop" size={14} /> Stop
        </button>
      {:else}
        <button class="btn btn-primary" onclick={() => send()} disabled={!draft.trim()}>
          <Icon name="send" size={14} /> Send
        </button>
      {/if}
    </div>
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
  }
  .scroller {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin: 0 -24px;
    padding: 0 24px;
  }
  .thread {
    max-width: 860px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-bottom: 8px;
  }
  .welcome {
    display: grid;
    justify-items: center;
    gap: 8px;
    padding: 56px 16px 24px;
    text-align: center;
  }
  .welcome p {
    margin: 0;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .examples {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 8px;
  }
  .examples .btn {
    border-radius: 14px;
  }
  .msg {
    display: flex;
    gap: 10px;
    animation: rise 0.18s ease-out;
  }
  .msg.user {
    justify-content: flex-end;
  }
  .bubble {
    min-width: 0;
    max-width: 85%;
    padding: 10px 14px;
    border-radius: var(--radius-lg);
    line-height: 1.55;
  }
  .user .bubble {
    background: var(--accent);
    color: var(--accent-text);
    border-bottom-right-radius: 4px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .assistant .bubble {
    flex: 1;
    background: var(--surface);
    border: 1px solid var(--border);
    border-top-left-radius: 4px;
    box-shadow: var(--shadow-sm);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .part {
    animation: fade 0.2s ease-out;
  }
  .dots {
    display: inline-flex;
    gap: 4px;
    padding: 4px 0;
  }
  .dots span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-faint);
    animation: blink 1.2s infinite ease-in-out;
  }
  .dots span:nth-child(2) {
    animation-delay: 0.15s;
  }
  .dots span:nth-child(3) {
    animation-delay: 0.3s;
  }
  .composer {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    max-width: 860px;
    width: 100%;
    margin: 0 auto;
    padding: 8px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
  }
  .composer:focus-within {
    border-color: var(--accent);
  }
  textarea {
    flex: 1;
    min-height: 32px;
    max-height: 200px;
    padding: 6px 6px;
    border: 0;
    outline: none;
    resize: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    line-height: 1.45;
  }
  textarea:focus-visible {
    outline: none;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes blink {
    0%,
    80%,
    100% {
      opacity: 0.25;
    }
    40% {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .msg,
    .part,
    .dots span {
      animation: none;
    }
  }
</style>
