<!--
  LLM provider setup (POST /api/chat/configure). Credentials live in torc-dash memory only, and the
  provider is shared by every conversation; `session` is the one whose form this is.
-->
<script lang="ts">
  import Icon from "../../lib/ui/Icon.svelte";
  import { chat, type ChatState } from "./chat.svelte";

  let { session = chat }: { session?: ChatState } = $props();

  const DOCS =
    "https://natlabrockies.github.io/torc/v0.20.4/core/monitoring/dashboard.html#ai-chat-tab";

  type Provider = "anthropic" | "openai" | "foundry" | "ollama" | "github" | "custom";
  const PROVIDERS: { id: Provider; label: string }[] = [
    { id: "anthropic", label: "Anthropic (Direct API)" },
    { id: "openai", label: "OpenAI" },
    { id: "foundry", label: "Azure AI Foundry" },
    { id: "ollama", label: "Ollama (Local)" },
    { id: "github", label: "GitHub Models" },
    { id: "custom", label: "Custom via gateway (LiteLLM, proxy)" },
  ];
  const KEY_PLACEHOLDER: Record<Provider, string> = {
    anthropic: "sk-ant-…",
    openai: "sk-… (OpenAI API key)",
    foundry: "Foundry API key",
    ollama: "",
    github: "ghp_… (GitHub token)",
    custom: "API key or token",
  };
  const URL_PLACEHOLDER: Partial<Record<Provider, string>> = {
    openai: "https://api.openai.com/v1",
    ollama: "http://localhost:11434/v1",
    github: "https://models.inference.ai.azure.com",
    custom: "https://litellm.example.org",
  };
  const MODEL_PLACEHOLDER: Partial<Record<Provider, string>> = {
    anthropic: "claude-sonnet-4-20250514",
    foundry: "claude-sonnet-4-20250514",
    custom: "Model name on the gateway",
    openai: "gpt-4o",
    ollama: "llama3.2",
    github: "gpt-4o",
  };

  // The last provider used and, per provider, everything but the key (kept in this browser only).
  type Fields = { baseUrl: string; model: string; foundryResource: string; authHeader: string };
  const STORE = "torc-dash.chat-setup";
  function loadSaved(): { provider?: Provider; fields?: Partial<Record<Provider, Partial<Fields>>> } {
    try {
      return JSON.parse(localStorage.getItem(STORE) ?? "{}");
    } catch {
      return {};
    }
  }
  const saved = loadSaved();

  let provider = $state<Provider>(saved.provider ?? "anthropic");
  let apiKey = $state("");
  let model = $state("");
  let foundryResource = $state("");
  let baseUrl = $state("");
  let authHeader = $state("x-api-key");
  fill();

  /** Load the remembered fields for the selected provider. */
  function fill() {
    const f = saved.fields?.[provider] ?? {};
    baseUrl = f.baseUrl ?? "";
    model = f.model ?? "";
    foundryResource = f.foundryResource ?? "";
    authHeader = f.authHeader ?? "x-api-key";
  }

  function remember() {
    saved.provider = provider;
    saved.fields = { ...saved.fields, [provider]: { baseUrl, model, foundryResource, authHeader } };
    try {
      localStorage.setItem(STORE, JSON.stringify(saved));
    } catch {
      // Remembering the form is a convenience only.
    }
  }
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const key = apiKey.trim();
    if (provider !== "ollama" && !key)
      return (error = `Please enter ${provider === "github" ? "a GitHub token" : "an API key"}.`);
    if (provider === "foundry" && !foundryResource.trim())
      return (error = "Please enter a Foundry resource name.");
    if (provider === "custom" && !baseUrl.trim()) return (error = "Please enter a base URL.");

    const body: Record<string, string> = { provider, api_key: provider === "ollama" ? "ollama" : key };
    if (model.trim() && MODEL_PLACEHOLDER[provider]) body.model = model.trim();
    if (provider === "foundry") body.foundry_resource = foundryResource.trim();
    if (baseUrl.trim() && URL_PLACEHOLDER[provider]) body.base_url = baseUrl.trim();
    if (provider === "custom") body.auth_header = authHeader;

    busy = true;
    error = null;
    try {
      await session.configure(body);
      remember();
      apiKey = "";
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }
</script>

<form class="card setup" onsubmit={submit}>
  <div class="icon"><Icon name="chat" size={22} /></div>
  <h3>Connect an AI provider</h3>
  {#if session.reason}<p class="muted">{session.reason}</p>{/if}

  <label class="field">
    <span>Provider</span>
    <select class="select" bind:value={provider} onchange={() => {
        error = null;
        fill();
      }}>
      {#each PROVIDERS as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
    </select>
  </label>

  {#if provider === "foundry"}
    <label class="field">
      <span>Foundry resource name</span>
      <input class="input" bind:value={foundryResource} placeholder="my-resource" />
    </label>
  {/if}
  {#if provider === "custom"}
    <label class="field">
      <span>Base URL <span class="hint">(a gateway with the Anthropic Messages API, as LiteLLM provides for any model)</span></span>
      <input class="input" bind:value={baseUrl} placeholder={URL_PLACEHOLDER.custom} />
    </label>
  {/if}
  {#if provider !== "ollama"}
    <label class="field">
      <span>{provider === "github" ? "GitHub token" : "API key"}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        type="password"
        autocomplete="off"
        autofocus
        bind:value={apiKey}
        placeholder={KEY_PLACEHOLDER[provider]}
      />
    </label>
  {/if}
  {#if MODEL_PLACEHOLDER[provider]}
    <label class="field">
      <span>Model <span class="hint">(optional)</span></span>
      <input class="input" bind:value={model} placeholder={MODEL_PLACEHOLDER[provider]} />
    </label>
  {/if}
  {#if provider !== "custom" && URL_PLACEHOLDER[provider]}
    <label class="field">
      <span>
        {provider === "openai" ? "API URL" : provider === "ollama" ? "Ollama URL" : "GitHub Models URL"}
        <span class="hint">(optional{provider === "openai" ? ", for compatible services" : ""})</span>
      </span>
      <input class="input" bind:value={baseUrl} placeholder={URL_PLACEHOLDER[provider]} />
    </label>
  {/if}
  {#if provider === "custom"}
    <label class="field">
      <span>Auth header</span>
      <input class="input" bind:value={authHeader} list="auth-headers" placeholder="x-api-key" />
      <datalist id="auth-headers">
        <option value="x-api-key">API key, like ANTHROPIC_API_KEY</option>
        <option value="Authorization">Bearer token, like ANTHROPIC_AUTH_TOKEN</option>
      </datalist>
    </label>
  {/if}

  {#if error}<p class="error-text">{error}</p>{/if}

  <div class="actions">
    {#if session.available}
      <button type="button" class="btn" onclick={() => (session.setupOpen = false)}>Cancel</button>
    {/if}
    <button class="btn btn-primary" disabled={busy}>
      {#if busy}<span class="spinner"></span>{/if} Connect
    </button>
  </div>
  <p class="faint note">
    Credentials are kept in memory for this session only. For persistent configuration, use CLI
    flags or environment variables. <a href={DOCS} target="_blank" rel="noopener">Documentation</a>
  </p>
</form>

<style>
  .setup {
    width: min(440px, 100%);
    margin: auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }
  h3 {
    font-size: 16px;
  }
  p {
    margin: 0;
  }
  .note {
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
