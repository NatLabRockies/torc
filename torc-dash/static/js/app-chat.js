/**
 * Torc Dashboard - AI Chat Module
 * Provides AI assistant functionality using Claude via the MCP server
 */

Object.assign(TorcDashboard.prototype, {
    setupChatTab() {
        // Initialize chat state
        this.chatMessages = [];
        this.chatAbortController = null;
        this.chatStreaming = false;

        // Check if chat is available
        this.checkChatAvailability();

        // Send button
        const sendBtn = document.getElementById('chat-send-btn');
        const input = document.getElementById('chat-input');

        if (sendBtn) {
            sendBtn.addEventListener('click', () => this.sendChatMessage());
        }

        if (input) {
            input.addEventListener('keydown', (e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    this.sendChatMessage();
                }
            });
        }

        // Clear button
        const clearBtn = document.getElementById('chat-clear-btn');
        if (clearBtn) {
            clearBtn.addEventListener('click', () => this.clearChat());
        }

        // Stop button
        const stopBtn = document.getElementById('chat-stop-btn');
        if (stopBtn) {
            stopBtn.addEventListener('click', () => this.stopChat());
        }

        // API key setup form
        const apiKeySubmit = document.getElementById('chat-api-key-submit');
        const apiKeyInput = document.getElementById('chat-api-key-input');
        if (apiKeySubmit) {
            apiKeySubmit.addEventListener('click', () => this.submitApiKey());
        }
        if (apiKeyInput) {
            apiKeyInput.addEventListener('keydown', (e) => {
                if (e.key === 'Enter') {
                    e.preventDefault();
                    this.submitApiKey();
                }
            });
        }

        // Provider selector
        const providerSelect = document.getElementById('chat-provider-select');
        if (providerSelect) {
            providerSelect.addEventListener('change', () => this.updateProviderFields());
        }
    },

    updateProviderFields() {
        const provider = document.getElementById('chat-provider-select')?.value || 'anthropic';
        const apiKeyField = document.getElementById('chat-api-key-field');
        const apiKeyInput = document.getElementById('chat-api-key-input');
        const modelField = document.getElementById('chat-model-field');
        const modelInput = document.getElementById('chat-model-input');
        const foundryField = document.getElementById('chat-foundry-field');
        const openaiUrlField = document.getElementById('chat-openai-url-field');
        const ollamaUrlField = document.getElementById('chat-ollama-url-field');
        const githubUrlField = document.getElementById('chat-github-url-field');
        const customUrlField = document.getElementById('chat-custom-url-field');
        const customHeaderField = document.getElementById('chat-custom-header-field');

        // Hide all provider-specific fields
        if (foundryField) foundryField.style.display = 'none';
        if (openaiUrlField) openaiUrlField.style.display = 'none';
        if (ollamaUrlField) ollamaUrlField.style.display = 'none';
        if (githubUrlField) githubUrlField.style.display = 'none';
        if (customUrlField) customUrlField.style.display = 'none';
        if (customHeaderField) customHeaderField.style.display = 'none';
        if (modelField) modelField.style.display = 'none';
        if (apiKeyField) apiKeyField.style.display = '';

        // Show fields for selected provider and update placeholder
        if (provider === 'foundry') {
            if (foundryField) foundryField.style.display = '';
            if (apiKeyInput) apiKeyInput.placeholder = 'Foundry API key';
        } else if (provider === 'openai') {
            if (apiKeyInput) apiKeyInput.placeholder = 'sk-... (OpenAI API key)';
            if (openaiUrlField) openaiUrlField.style.display = '';
            if (modelField) modelField.style.display = '';
            if (modelInput) modelInput.placeholder = 'gpt-4o';
        } else if (provider === 'ollama') {
            // Ollama doesn't need an API key
            if (apiKeyField) apiKeyField.style.display = 'none';
            if (ollamaUrlField) ollamaUrlField.style.display = '';
            if (modelField) modelField.style.display = '';
            if (modelInput) modelInput.placeholder = 'llama3.2';
        } else if (provider === 'github') {
            if (apiKeyInput) apiKeyInput.placeholder = 'ghp_... (GitHub token)';
            if (githubUrlField) githubUrlField.style.display = '';
            if (modelField) modelField.style.display = '';
            if (modelInput) modelInput.placeholder = 'gpt-4o';
        } else if (provider === 'custom') {
            if (customUrlField) customUrlField.style.display = '';
            if (customHeaderField) customHeaderField.style.display = '';
            if (apiKeyInput) apiKeyInput.placeholder = 'API key';
        } else {
            // anthropic
            if (apiKeyInput) apiKeyInput.placeholder = 'sk-ant-...';
        }
    },

    async checkChatAvailability() {
        try {
            const response = await fetch('/api/chat/status');
            const data = await response.json();

            // Always keep the nav item enabled so users can discover the feature
            const navItem = document.querySelector('.nav-item[data-tab="chat"]');
            if (navItem) {
                navItem.classList.remove('nav-item-disabled');
                navItem.title = '';
            }

            if (!data.available) {
                this.showChatSetup();
            } else {
                this.hideChatSetup();
            }
        } catch (e) {
            console.debug('Chat status check failed:', e);
        }
    },

    showChatSetup() {
        const setup = document.getElementById('chat-setup');
        const inputArea = document.querySelector('.chat-input-area');
        const messages = document.getElementById('chat-messages');
        if (setup) setup.style.display = 'flex';
        if (inputArea) inputArea.style.display = 'none';
        if (messages) messages.style.display = 'none';
    },

    hideChatSetup() {
        const setup = document.getElementById('chat-setup');
        const inputArea = document.querySelector('.chat-input-area');
        const messages = document.getElementById('chat-messages');
        if (setup) setup.style.display = 'none';
        if (inputArea) inputArea.style.display = '';
        if (messages) messages.style.display = '';
    },

    async submitApiKey() {
        const input = document.getElementById('chat-api-key-input');
        const errorEl = document.getElementById('chat-setup-error');
        const submitBtn = document.getElementById('chat-api-key-submit');

        const provider = document.getElementById('chat-provider-select')?.value || 'anthropic';
        const key = input?.value?.trim() || '';

        // Validate API key (not required for Ollama)
        if (provider !== 'ollama' && !key) {
            if (errorEl) {
                const keyName = provider === 'github' ? 'GitHub token' : 'API key';
                errorEl.textContent = `Please enter ${keyName}.`;
                errorEl.style.display = '';
            }
            return;
        }

        const body = { api_key: provider === 'ollama' ? 'ollama' : key, provider };

        // Get model if specified
        const modelInput = document.getElementById('chat-model-input');
        const model = modelInput?.value?.trim();
        if (model) body.model = model;

        if (provider === 'foundry') {
            body.foundry_resource = document.getElementById('chat-foundry-resource')?.value?.trim() || '';
            if (!body.foundry_resource) {
                if (errorEl) {
                    errorEl.textContent = 'Please enter a Foundry resource name.';
                    errorEl.style.display = '';
                }
                return;
            }
        } else if (provider === 'openai') {
            const baseUrl = document.getElementById('chat-openai-base-url')?.value?.trim();
            if (baseUrl) body.base_url = baseUrl;
        } else if (provider === 'ollama') {
            const baseUrl = document.getElementById('chat-ollama-base-url')?.value?.trim();
            if (baseUrl) body.base_url = baseUrl;
        } else if (provider === 'github') {
            const baseUrl = document.getElementById('chat-github-base-url')?.value?.trim();
            if (baseUrl) body.base_url = baseUrl;
        } else if (provider === 'custom') {
            body.base_url = document.getElementById('chat-custom-base-url')?.value?.trim() || '';
            if (!body.base_url) {
                if (errorEl) {
                    errorEl.textContent = 'Please enter a base URL.';
                    errorEl.style.display = '';
                }
                return;
            }
            const authHeader = document.getElementById('chat-custom-auth-header')?.value?.trim();
            if (authHeader) body.auth_header = authHeader;
        }

        if (submitBtn) submitBtn.disabled = true;
        if (errorEl) errorEl.style.display = 'none';

        try {
            const response = await fetch('/api/chat/configure', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(body),
            });

            if (!response.ok) {
                const text = await response.text();
                throw new Error(text || 'Failed to configure provider');
            }

            // Clear the inputs and show the chat
            if (input) input.value = '';
            if (modelInput) modelInput.value = '';
            this.hideChatSetup();
        } catch (e) {
            if (errorEl) {
                errorEl.textContent = e.message;
                errorEl.style.display = '';
            }
        } finally {
            if (submitBtn) submitBtn.disabled = false;
        }
    },

    async sendChatMessage() {
        const input = document.getElementById('chat-input');
        if (!input) return;

        const text = input.value.trim();
        if (!text || this.chatStreaming) return;

        input.value = '';
        input.style.height = 'auto';

        // Add user message
        this.chatMessages.push({ role: 'user', content: text });
        this.renderChatMessage('user', text);

        // Prepare request
        const workflowId = this.selectedWorkflowId;
        this.chatStreaming = true;
        this.updateChatControls();

        // Create assistant message placeholder
        const assistantDiv = this.createAssistantMessageDiv();

        // Abort controller for cancellation
        this.chatAbortController = new AbortController();

        try {
            const response = await fetch('/api/chat', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    messages: this.chatMessages,
                    workflow_id: workflowId ? parseInt(workflowId) : null,
                }),
                signal: this.chatAbortController.signal,
            });

            if (!response.ok) {
                const errText = await response.text();
                this.appendToAssistantDiv(assistantDiv, `Error: ${errText}`, 'error');
                this.finishStreaming(assistantDiv);
                return;
            }

            // Parse SSE stream
            const reader = response.body.getReader();
            const decoder = new TextDecoder();
            let buffer = '';
            let fullText = '';

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });
                // SSE events are separated by blank lines (\n\n)
                const events = buffer.split('\n\n');
                buffer = events.pop() || '';

                for (const eventBlock of events) {
                    if (!eventBlock.trim()) continue;
                    let eventType = null;
                    const dataLines = [];
                    for (const line of eventBlock.split('\n')) {
                        if (line.startsWith('event: ')) {
                            eventType = line.slice(7).trim();
                        } else if (line.startsWith('data: ')) {
                            dataLines.push(line.slice(6));
                        } else if (line.startsWith('data:')) {
                            dataLines.push(line.slice(5));
                        }
                    }
                    if (dataLines.length === 0) continue;
                    const data = dataLines.join('\n');

                    // For text events, data is JSON-encoded to preserve newlines
                    let textData = data;
                    if (eventType === 'text') {
                        try { textData = JSON.parse(data); } catch (_) { /* use raw */ }
                    }

                    this.handleChatSSEEvent(eventType || 'text', textData, assistantDiv);
                    if (eventType === 'text') {
                        fullText += textData;
                    }
                }
            }

            // Record assistant response
            if (fullText) {
                this.chatMessages.push({
                    role: 'assistant',
                    content: fullText,
                });
            }
        } catch (e) {
            if (e.name === 'AbortError') {
                this.appendToAssistantDiv(assistantDiv, '\n\n(stopped)', 'info');
            } else {
                this.appendToAssistantDiv(assistantDiv, `\nError: ${e.message}`, 'error');
            }
        } finally {
            this.finishStreaming(assistantDiv);
        }
    },

    handleChatSSEEvent(eventType, data, assistantDiv) {
        switch (eventType) {
            case 'text':
                this.appendTextToAssistantDiv(assistantDiv, data);
                break;
            case 'tool_use': {
                try {
                    const tool = JSON.parse(data);
                    this.appendToolCallToAssistantDiv(assistantDiv, tool);
                } catch (_) { /* ignore */ }
                break;
            }
            case 'tool_result': {
                try {
                    const result = JSON.parse(data);
                    this.appendToolResultToAssistantDiv(assistantDiv, result);
                } catch (_) { /* ignore */ }
                break;
            }
            case 'error':
                this.appendToAssistantDiv(assistantDiv, `Error: ${data}`, 'error');
                break;
            case 'done':
                break;
        }
    },

    renderChatMessage(role, text) {
        const container = document.getElementById('chat-messages');
        if (!container) return;

        const msgDiv = document.createElement('div');
        msgDiv.className = `chat-message chat-message-${role}`;

        const content = document.createElement('div');
        content.className = 'chat-message-content';
        content.textContent = text;

        msgDiv.appendChild(content);
        container.appendChild(msgDiv);
        container.scrollTop = container.scrollHeight;
    },

    createAssistantMessageDiv() {
        const container = document.getElementById('chat-messages');
        if (!container) return null;

        const msgDiv = document.createElement('div');
        msgDiv.className = 'chat-message chat-message-assistant';

        const content = document.createElement('div');
        content.className = 'chat-message-content';

        const textSpan = document.createElement('div');
        textSpan.className = 'chat-text-content';

        content.appendChild(textSpan);
        msgDiv.appendChild(content);
        container.appendChild(msgDiv);
        container.scrollTop = container.scrollHeight;

        return msgDiv;
    },

    appendTextToAssistantDiv(div, text) {
        if (!div) return;
        const textSpan = div.querySelector('.chat-text-content');
        if (textSpan) {
            textSpan.textContent += text;
        }
        const container = document.getElementById('chat-messages');
        if (container) container.scrollTop = container.scrollHeight;
    },

    appendToAssistantDiv(div, text, className) {
        if (!div) return;
        const content = div.querySelector('.chat-message-content');
        if (content) {
            const span = document.createElement('span');
            span.className = `chat-${className}`;
            span.textContent = text;
            content.appendChild(span);
        }
        const container = document.getElementById('chat-messages');
        if (container) container.scrollTop = container.scrollHeight;
    },

    appendToolCallToAssistantDiv(div, tool) {
        if (!div) return;
        const content = div.querySelector('.chat-message-content');
        if (!content) return;

        const toolDiv = document.createElement('details');
        toolDiv.className = 'chat-tool-call';
        toolDiv.id = `tool-call-${tool.id}`;

        const summary = document.createElement('summary');
        summary.className = 'chat-tool-name';
        summary.textContent = `Tool: ${tool.name}`;
        toolDiv.appendChild(summary);

        const inputPre = document.createElement('pre');
        inputPre.className = 'chat-tool-input';
        inputPre.textContent = JSON.stringify(tool.input, null, 2);
        toolDiv.appendChild(inputPre);

        content.appendChild(toolDiv);

        const container = document.getElementById('chat-messages');
        if (container) container.scrollTop = container.scrollHeight;
    },

    appendToolResultToAssistantDiv(div, result) {
        if (!div) return;

        // Find the corresponding tool_call details element
        const toolDiv = div.querySelector(`#tool-call-${CSS.escape(result.id)}`);
        if (toolDiv) {
            const resultPre = document.createElement('pre');
            resultPre.className = result.is_error ? 'chat-tool-result chat-tool-error' : 'chat-tool-result';

            // Truncate very long results for display
            let text = result.result || '';
            if (text.length > 5000) {
                text = text.substring(0, 5000) + '\n... (truncated)';
            }
            resultPre.textContent = text;
            toolDiv.appendChild(resultPre);
        }

        const container = document.getElementById('chat-messages');
        if (container) container.scrollTop = container.scrollHeight;
    },

    finishStreaming(assistantDiv) {
        this.chatStreaming = false;
        this.chatAbortController = null;
        this.updateChatControls();

        // Try to render markdown in the text content
        if (assistantDiv) {
            const textSpan = assistantDiv.querySelector('.chat-text-content');
            if (textSpan && textSpan.textContent) {
                textSpan.innerHTML = this.renderMarkdown(textSpan.textContent);
            }
        }

        const input = document.getElementById('chat-input');
        if (input) input.focus();
    },

    /** Render markdown to HTML using the marked library (with raw HTML sanitized). */
    renderMarkdown(text) {
        if (typeof marked !== 'undefined' && marked.parse) {
            // Escape ampersands first to prevent entity-encoded XSS (e.g. &#x3C;script&#x3E;),
            // then escape raw HTML tags. This preserves markdown syntax like > for blockquotes
            // since those appear at line start without <.
            const sanitized = text
                .replace(/&/g, '&amp;')
                .replace(/<\/?[a-zA-Z][^>]*>/g, (match) => {
                    return match.replace(/</g, '&lt;').replace(/>/g, '&gt;');
                });
            const renderer = new marked.Renderer();
            // marked v15 uses token objects, not positional (href, title, text) args.
            const isSafeUrl = (url) => {
                if (!url) return false;
                const trimmed = url.trim();
                if (/^(https?:|mailto:)/i.test(trimmed)) return true;
                if (/^(\/|\.\/|\.\.\/|#)/.test(trimmed)) return true;
                return false;
            };
            const baseLinkRenderer = renderer.link.bind(renderer);
            renderer.link = function (token) {
                const href = token && typeof token.href === 'string' ? token.href : '';
                if (!isSafeUrl(href)) {
                    return token.text || '';
                }
                return baseLinkRenderer(token);
            };
            const baseImageRenderer = renderer.image.bind(renderer);
            renderer.image = function (token) {
                const href = token && typeof token.href === 'string' ? token.href : '';
                if (!isSafeUrl(href)) {
                    return token.text || '';
                }
                return baseImageRenderer(token);
            };
            return marked.parse(sanitized, { breaks: true, renderer });
        }
        // Fallback: escape HTML and preserve line breaks
        return text
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/\n/g, '<br>');
    },

    updateChatControls() {
        const sendBtn = document.getElementById('chat-send-btn');
        const stopBtn = document.getElementById('chat-stop-btn');
        const input = document.getElementById('chat-input');
        const clearBtn = document.getElementById('chat-clear-btn');

        if (sendBtn) sendBtn.style.display = this.chatStreaming ? 'none' : 'inline-flex';
        if (stopBtn) stopBtn.style.display = this.chatStreaming ? 'inline-flex' : 'none';
        if (input) input.disabled = this.chatStreaming;
        if (clearBtn) clearBtn.disabled = this.chatStreaming;
    },

    stopChat() {
        if (this.chatAbortController) {
            this.chatAbortController.abort();
        }
    },

    clearChat() {
        this.chatMessages = [];
        const container = document.getElementById('chat-messages');
        if (container) {
            container.innerHTML = `
                <div class="chat-welcome">
                    <p><strong>Torc AI Assistant</strong></p>
                    <p>Ask questions about your workflows, job status, logs, and more.</p>
                    <p class="chat-examples">Try: "Help me create a workflow" or "Show me the failed jobs"</p>
                </div>
            `;
        }
    },
});
