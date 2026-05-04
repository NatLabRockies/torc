/**
 * Torc Dashboard - Events Tab
 * Real-time event streaming via SSE (Server-Sent Events)
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Events Tab ====================

    setupEventsTab() {
        this._lastEventsWorkflowId = null;
        this._eventSource = null;  // SSE connection

        document.getElementById('events-workflow-selector')?.addEventListener('change', (e) => {
            const newWorkflowId = e.target.value;
            if (newWorkflowId !== this._lastEventsWorkflowId) {
                this._lastEventsWorkflowId = newWorkflowId;
                this.events = [];
                this.renderEvents();
                // Start SSE connection for the new workflow
                if (newWorkflowId) {
                    this.startEventStream(newWorkflowId);
                } else {
                    this.stopEventStream();
                }
            }
        });

        document.getElementById('btn-clear-events')?.addEventListener('click', () => {
            // Clear displayed events
            this.events = [];
            this.renderEvents();
        });

        // Auto-start if a workflow is selected
        const workflowId = document.getElementById('events-workflow-selector')?.value;
        if (workflowId) {
            this.startEventStream(workflowId);
        }
    },

    startEventStream(workflowId) {
        this.stopEventStream();

        if (!workflowId) {
            return;
        }

        this._lastEventsWorkflowId = workflowId;
        this.events = [];
        this.renderEvents();

        // Connect to SSE endpoint
        const sseUrl = `${api.getBaseUrl()}/workflows/${workflowId}/events/stream`;
        this._eventSource = new EventSource(sseUrl);

        this._eventSource.onopen = () => {
            console.log('SSE connection opened for workflow', workflowId);
            this.updateEventStatus('connected');
        };

        this._eventSource.onmessage = (event) => {
            try {
                const sseEvent = JSON.parse(event.data);
                // Add event to the beginning (newest first)
                this.events.unshift(sseEvent);
                // Limit to 1000 events to prevent memory issues
                if (this.events.length > 1000) {
                    this.events = this.events.slice(0, 1000);
                }
                this.updateEventBadge(1);
                this.renderEvents();
            } catch (error) {
                console.error('Error parsing SSE event:', error, event.data);
            }
        };

        this._eventSource.onerror = (error) => {
            console.error('SSE connection error:', error);
            this.updateEventStatus('error');
            // EventSource will automatically try to reconnect
        };

        // Handle specific event types
        ['job_started', 'job_completed', 'job_failed', 'job_canceled', 'job_terminated',
         'job_blocked', 'job_ready', 'job_uninitialized',
         'compute_node_started', 'compute_node_stopped', 'workflow_started',
         'workflow_reinitialized', 'scheduler_node_created', 'warning'].forEach(eventType => {
            this._eventSource.addEventListener(eventType, (event) => {
                try {
                    const sseEvent = JSON.parse(event.data);
                    // Override event_type from the SSE event: field
                    sseEvent.event_type = eventType;
                    this.events.unshift(sseEvent);
                    if (this.events.length > 1000) {
                        this.events = this.events.slice(0, 1000);
                    }
                    this.updateEventBadge(1);
                    this.renderEvents();
                } catch (error) {
                    console.error('Error parsing SSE event:', error, event.data);
                }
            });
        });
    },

    stopEventStream() {
        if (this._eventSource) {
            this._eventSource.close();
            this._eventSource = null;
            this.updateEventStatus('disconnected');
        }
    },

    updateEventStatus(status) {
        const statusIndicator = document.getElementById('events-status');
        if (statusIndicator) {
            switch (status) {
                case 'connected':
                    statusIndicator.textContent = '● Live';
                    statusIndicator.className = 'status-connected';
                    break;
                case 'error':
                    statusIndicator.textContent = '● Reconnecting...';
                    statusIndicator.className = 'status-error';
                    break;
                case 'disconnected':
                    statusIndicator.textContent = '○ Disconnected';
                    statusIndicator.className = 'status-disconnected';
                    break;
            }
        }
    },

    renderEvents() {
        const container = document.getElementById('events-list');
        if (!container) return;

        const workflowId = document.getElementById('events-workflow-selector')?.value;

        if (!workflowId) {
            container.innerHTML = '<div class="placeholder-message">Select a workflow to view events</div>';
            return;
        }

        if (this.events.length === 0) {
            const connected = this._eventSource && this._eventSource.readyState !== EventSource.CLOSED;
            const statusText = connected ? 'Waiting for events... (SSE connected)' : 'No events yet (SSE not connected)';
            container.innerHTML = `<div class="placeholder-message">${statusText}</div>`;
            return;
        }

        container.innerHTML = `
            <table class="data-table">
                <thead>
                    <tr>
                        <th>Timestamp</th>
                        <th>Level</th>
                        <th>Event Type</th>
                        <th>Data</th>
                    </tr>
                </thead>
                <tbody>
                    ${this.events.map(event => {
                        let severityClass = '';
                        const severity = (event.severity || 'info').toLowerCase();
                        if (severity === 'error') severityClass = 'status-failed';
                        else if (severity === 'warning') severityClass = 'status-pending'; // Yellow
                        else if (severity === 'info') severityClass = 'status-success'; // Green

                        return `
                        <tr>
                            <td>${this.formatTimestamp(event.timestamp)}</td>
                            <td><span class="status-badge ${severityClass}">${this.escapeHtml(severity)}</span></td>
                            <td><code>${this.escapeHtml(event.event_type || '-')}</code></td>
                            <td><pre class="event-data">${this.escapeHtml(JSON.stringify(event.data, null, 2))}</pre></td>
                        </tr>
                        `;
                    }).join('')}
                </tbody>
            </table>
        `;
    },

    updateEventBadge(count) {
        const badge = document.getElementById('event-badge');
        if (badge) {
            if (count > 0 && this.currentTab !== 'events') {
                // Increment the badge count
                const currentCount = parseInt(badge.textContent) || 0;
                badge.textContent = currentCount + count;
                badge.style.display = 'inline';
            } else if (this.currentTab === 'events') {
                // Clear badge when viewing events tab
                badge.style.display = 'none';
            }
        }
    },
});
