<!--
  A conversation: messages, tool calls, and the composer. Used by the AI Chat tab and the spec
  editor's assistant panel, each with its own ChatState.
-->
<script lang="ts">
  import Icon from "../../lib/ui/Icon.svelte";
  import type { ChatState } from "./chat.svelte";
  import Markdown from "./Markdown.svelte";
  import ToolCall from "./ToolCall.svelte";

  let {
    session,
    title,
    subtitle,
    examples,
    placeholder,
  }: { session: ChatState; title: string; subtitle: string; examples: string[]; placeholder: string } =
    $props();

  let draft = $state("");
  let textarea = $state<HTMLTextAreaElement>();
  let scroller = $state<HTMLDivElement>();
  let inner = $state<HTMLDivElement>();
  /** Follow new content only while the user is at the bottom. */
  let stick = true;

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
    if (!t || session.streaming) return;
    draft = "";
    stick = true;
    queueMicrotask(autosize);
    session.send(t).then(() => textarea?.focus());
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    }
  }
</script>

<div class="scroller" bind:this={scroller} onscroll={onScroll}>
  <div class="thread" bind:this={inner}>
    {#if session.messages.length === 0}
      <div class="welcome">
        <div class="icon"><Icon name="chat" size={24} /></div>
        <h3>{title}</h3>
        <p class="muted">{subtitle}</p>
        <div class="examples">
          {#each examples as ex (ex)}
            <button class="btn btn-sm" onclick={() => send(ex)}>{ex}</button>
          {/each}
        </div>
      </div>
    {/if}

    {#each session.messages as m, i (i)}
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
                    pending={session.streaming && i === session.messages.length - 1}
                  />
                {:else}
                  <div class:error-text={p.error} class:muted={!p.error}>{p.text}</div>
                {/if}
              </div>
            {/each}
            {#if session.streaming && i === session.messages.length - 1}
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
    {placeholder}
    aria-label="Chat message"
  ></textarea>
  {#if session.streaming}
    <button class="btn btn-danger" onclick={() => session.stop()}>
      <Icon name="stop" size={14} /> Stop
    </button>
  {:else}
    <button class="btn btn-primary" onclick={() => send()} disabled={!draft.trim()}>
      <Icon name="send" size={14} /> Send
    </button>
  {/if}
</div>

<style>
  .scroller {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
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
