/**
 * Torc Dashboard Application
 * Main application logic and UI management
 */

class TorcDashboard {
    constructor() {
        this.currentTab = 'workflows';
        this.selectedWorkflowId = null;
        this.selectedSubTab = 'jobs';
        this.workflows = [];
        this.events = [];
        this.lastEventId = null;
        this.eventPollInterval = null;
        this.autoRefreshInterval = null;
        this.isConnected = false;
    }

    async init() {
        // Load saved settings
        this.loadSettings();

        // Setup event listeners
        this.setupNavigation();
        this.setupWorkflowsTab();
        this.setupDetailsTab();
        this.setupDAGTab();
        this.setupEventsTab();
        this.setupSettingsTab();
        this.setupModal();

        // Initial data load
        await this.testConnection();
        if (this.isConnected) {
            await this.loadWorkflows();
            this.startAutoRefresh();
        }
    }

    // ==================== Settings ====================

    loadSettings() {
        const darkMode = localStorage.getItem('torc-dark-mode') === 'true';
        if (darkMode) {
            document.body.classList.add('dark-mode');
            const checkbox = document.getElementById('dark-mode');
            if (checkbox) checkbox.checked = true;
        }

        const refreshInterval = localStorage.getItem('torc-refresh-interval') || '30';
        const intervalInput = document.getElementById('refresh-interval');
        if (intervalInput) intervalInput.value = refreshInterval;

        const apiUrl = api.getBaseUrl();
        const apiInput = document.getElementById('api-url');
        if (apiInput) apiInput.value = apiUrl;
    }

    saveSettings() {
        const darkMode = document.getElementById('dark-mode')?.checked || false;
        const refreshInterval = document.getElementById('refresh-interval')?.value || '30';
        const apiUrl = document.getElementById('api-url')?.value || '/torc-service/v1';

        localStorage.setItem('torc-dark-mode', darkMode);
        localStorage.setItem('torc-refresh-interval', refreshInterval);
        api.setBaseUrl(apiUrl);

        if (darkMode) {
            document.body.classList.add('dark-mode');
        } else {
            document.body.classList.remove('dark-mode');
        }

        this.showToast('Settings saved', 'success');

        // Restart auto-refresh with new interval
        this.stopAutoRefresh();
        this.startAutoRefresh();
    }

    // ==================== Navigation ====================

    setupNavigation() {
        const navItems = document.querySelectorAll('.nav-item');
        navItems.forEach(item => {
            item.addEventListener('click', () => {
                const tab = item.dataset.tab;
                this.switchTab(tab);
            });
        });
    }

    switchTab(tabName) {
        // Update nav items
        document.querySelectorAll('.nav-item').forEach(item => {
            item.classList.toggle('active', item.dataset.tab === tabName);
        });

        // Update tab content
        document.querySelectorAll('.tab-content').forEach(content => {
            content.classList.toggle('active', content.id === `tab-${tabName}`);
        });

        this.currentTab = tabName;

        // Tab-specific initialization
        if (tabName === 'dag' && dagVisualizer && this.selectedWorkflowId) {
            dagVisualizer.initialize();
            dagVisualizer.loadJobDependencies(this.selectedWorkflowId);
        }
    }

    // ==================== Connection ====================

    async testConnection() {
        const result = await api.testConnection();
        this.isConnected = result.success;
        this.updateConnectionStatus(result.success);

        const serverInfo = document.getElementById('server-info');
        if (serverInfo) {
            if (result.success) {
                serverInfo.innerHTML = `<p style="color: var(--success-color)">Connected to ${api.getBaseUrl()}</p>`;
            } else {
                serverInfo.innerHTML = `<p style="color: var(--danger-color)">Connection failed: ${this.escapeHtml(result.error)}</p>`;
            }
        }

        return result;
    }

    updateConnectionStatus(connected) {
        const statusEl = document.getElementById('connection-status');
        if (statusEl) {
            const dot = statusEl.querySelector('.status-dot');
            const text = statusEl.querySelector('.status-text');
            if (connected) {
                dot.classList.remove('disconnected');
                dot.classList.add('connected');
                text.textContent = 'Connected';
            } else {
                dot.classList.remove('connected');
                dot.classList.add('disconnected');
                text.textContent = 'Disconnected';
            }
        }
    }

    // ==================== Auto Refresh ====================

    startAutoRefresh() {
        const interval = parseInt(localStorage.getItem('torc-refresh-interval') || '30') * 1000;
        this.autoRefreshInterval = setInterval(() => {
            if (this.currentTab === 'workflows') {
                this.loadWorkflows();
            } else if (this.currentTab === 'details' && this.selectedWorkflowId) {
                this.loadWorkflowDetails(this.selectedWorkflowId);
            } else if (this.currentTab === 'dag' && this.selectedWorkflowId) {
                dagVisualizer.refresh();
            }
        }, interval);
    }

    stopAutoRefresh() {
        if (this.autoRefreshInterval) {
            clearInterval(this.autoRefreshInterval);
            this.autoRefreshInterval = null;
        }
    }

    // ==================== Workflows Tab ====================

    setupWorkflowsTab() {
        document.getElementById('btn-refresh-workflows')?.addEventListener('click', () => {
            this.loadWorkflows();
        });

        document.getElementById('btn-create-workflow')?.addEventListener('click', () => {
            this.showModal('create-workflow-modal');
        });
    }

    async loadWorkflows() {
        try {
            const workflows = await api.listWorkflows(0, 100);
            this.workflows = workflows || [];
            this.renderWorkflowsTable(this.workflows);
            this.updateWorkflowSelectors(this.workflows);
        } catch (error) {
            console.error('Error loading workflows:', error);
            this.showToast('Error loading workflows: ' + error.message, 'error');
        }
    }

    renderWorkflowsTable(workflows) {
        const tbody = document.getElementById('workflows-body');
        if (!tbody) return;

        if (!workflows || workflows.length === 0) {
            tbody.innerHTML = '<tr><td colspan="7" class="placeholder-message">No workflows found</td></tr>';
            return;
        }

        tbody.innerHTML = workflows.map(workflow => `
            <tr data-workflow-id="${workflow.id}">
                <td><code>${this.truncateId(workflow.id)}</code></td>
                <td>${this.escapeHtml(workflow.name || 'Unnamed')}</td>
                <td>${this.escapeHtml(workflow.owner || '-')}</td>
                <td>${this.getStatusBadge(workflow)}</td>
                <td>${workflow.job_count || '-'}</td>
                <td>${this.formatDate(workflow.created_at)}</td>
                <td>
                    <div class="action-buttons">
                        <button class="btn btn-sm btn-secondary" onclick="app.viewWorkflow('${workflow.id}')" title="View Details">View</button>
                        <button class="btn btn-sm btn-secondary" onclick="app.viewDAG('${workflow.id}')" title="View DAG">DAG</button>
                        <button class="btn btn-sm btn-danger" onclick="app.deleteWorkflow('${workflow.id}')" title="Delete">Del</button>
                    </div>
                </td>
            </tr>
        `).join('');
    }

    getStatusBadge(workflow) {
        // Compute workflow status from job counts if available
        let statusClass = 'status-uninitialized';
        let statusText = 'Unknown';

        if (workflow.status) {
            statusText = workflow.status;
            statusClass = `status-${workflow.status.toLowerCase()}`;
        } else if (workflow.completed_count !== undefined) {
            const total = workflow.job_count || 0;
            const completed = workflow.completed_count || 0;
            const failed = workflow.failed_count || 0;
            const running = workflow.running_count || 0;

            if (failed > 0) {
                statusClass = 'status-failed';
                statusText = `Failed (${failed})`;
            } else if (completed === total && total > 0) {
                statusClass = 'status-completed';
                statusText = 'Completed';
            } else if (running > 0) {
                statusClass = 'status-running';
                statusText = `Running (${running})`;
            } else if (completed > 0) {
                statusClass = 'status-pending';
                statusText = `${completed}/${total}`;
            } else {
                statusClass = 'status-ready';
                statusText = 'Ready';
            }
        }

        return `<span class="status-badge ${statusClass}">${statusText}</span>`;
    }

    updateWorkflowSelectors(workflows) {
        const selectors = [
            'workflow-selector',
            'dag-workflow-selector',
            'events-workflow-selector',
        ];

        selectors.forEach(id => {
            const select = document.getElementById(id);
            if (!select) return;

            const currentValue = select.value;
            const options = workflows.map(w =>
                `<option value="${w.id}">${this.escapeHtml(w.name || w.id)}</option>`
            ).join('');

            if (id === 'events-workflow-selector') {
                select.innerHTML = `<option value="">All Workflows</option>${options}`;
            } else {
                select.innerHTML = `<option value="">Select a workflow...</option>${options}`;
            }

            // Restore selection if still valid
            if (currentValue && workflows.find(w => w.id === currentValue)) {
                select.value = currentValue;
            }
        });
    }

    async viewWorkflow(workflowId) {
        this.selectedWorkflowId = workflowId;
        document.getElementById('workflow-selector').value = workflowId;
        this.switchTab('details');
        await this.loadWorkflowDetails(workflowId);
    }

    async viewDAG(workflowId) {
        this.selectedWorkflowId = workflowId;
        document.getElementById('dag-workflow-selector').value = workflowId;
        this.switchTab('dag');
        dagVisualizer.initialize();
        await dagVisualizer.loadJobDependencies(workflowId);
    }

    async deleteWorkflow(workflowId) {
        if (!confirm('Are you sure you want to delete this workflow? This action cannot be undone.')) {
            return;
        }

        try {
            await api.deleteWorkflow(workflowId);
            this.showToast('Workflow deleted', 'success');
            await this.loadWorkflows();
        } catch (error) {
            this.showToast('Error deleting workflow: ' + error.message, 'error');
        }
    }

    // ==================== Details Tab ====================

    setupDetailsTab() {
        document.getElementById('workflow-selector')?.addEventListener('change', async (e) => {
            const workflowId = e.target.value;
            if (workflowId) {
                this.selectedWorkflowId = workflowId;
                await this.loadWorkflowDetails(workflowId);
            } else {
                this.clearWorkflowDetails();
            }
        });

        // Sub-tab navigation
        document.querySelectorAll('.sub-tab').forEach(tab => {
            tab.addEventListener('click', () => {
                this.switchSubTab(tab.dataset.subtab);
            });
        });
    }

    async loadWorkflowDetails(workflowId) {
        try {
            const workflow = await api.getWorkflow(workflowId);

            // Show workflow summary
            const container = document.getElementById('details-container');
            container.innerHTML = `
                <div class="workflow-summary">
                    <div class="summary-card">
                        <div class="value">${workflow.id ? this.truncateId(workflow.id) : '-'}</div>
                        <div class="label">ID</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">${this.escapeHtml(workflow.name || 'Unnamed')}</div>
                        <div class="label">Name</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">${this.escapeHtml(workflow.owner || '-')}</div>
                        <div class="label">Owner</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">${this.formatDate(workflow.created_at)}</div>
                        <div class="label">Created</div>
                    </div>
                </div>
            `;

            // Show sub-tabs
            document.getElementById('details-sub-tabs').style.display = 'flex';

            // Load current sub-tab content
            await this.loadSubTabContent(workflowId, this.selectedSubTab);
        } catch (error) {
            console.error('Error loading workflow details:', error);
            this.showToast('Error loading workflow details: ' + error.message, 'error');
        }
    }

    clearWorkflowDetails() {
        document.getElementById('details-container').innerHTML = `
            <div class="placeholder-message">Select a workflow to view details</div>
        `;
        document.getElementById('details-sub-tabs').style.display = 'none';
        document.getElementById('details-content').innerHTML = '';
    }

    switchSubTab(subtab) {
        this.selectedSubTab = subtab;

        document.querySelectorAll('.sub-tab').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.subtab === subtab);
        });

        if (this.selectedWorkflowId) {
            this.loadSubTabContent(this.selectedWorkflowId, subtab);
        }
    }

    async loadSubTabContent(workflowId, subtab) {
        const content = document.getElementById('details-content');

        try {
            switch (subtab) {
                case 'jobs':
                    const jobs = await api.listJobs(workflowId);
                    content.innerHTML = this.renderJobsTable(jobs);
                    break;
                case 'files':
                    const files = await api.listFiles(workflowId);
                    content.innerHTML = this.renderFilesTable(files);
                    break;
                case 'user-data':
                    const userData = await api.listUserData(workflowId);
                    content.innerHTML = this.renderUserDataTable(userData);
                    break;
                case 'results':
                    const results = await api.listResults(workflowId);
                    content.innerHTML = this.renderResultsTable(results);
                    break;
            }
        } catch (error) {
            content.innerHTML = `<div class="placeholder-message">Error loading ${this.escapeHtml(subtab)}: ${this.escapeHtml(error.message)}</div>`;
        }
    }

    renderJobsTable(jobs) {
        if (!jobs || jobs.length === 0) {
            return '<div class="placeholder-message">No jobs in this workflow</div>';
        }

        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];

        return `
            <table class="data-table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                        <th>Status</th>
                        <th>Command</th>
                        <th>Started</th>
                        <th>Completed</th>
                    </tr>
                </thead>
                <tbody>
                    ${jobs.map(job => `
                        <tr>
                            <td><code>${this.truncateId(job.id)}</code></td>
                            <td>${this.escapeHtml(job.name || '-')}</td>
                            <td><span class="status-badge status-${statusNames[job.status]?.toLowerCase() || 'unknown'}">${statusNames[job.status] || job.status}</span></td>
                            <td><code>${this.escapeHtml(this.truncate(job.command || '-', 50))}</code></td>
                            <td>${this.formatDate(job.start_time)}</td>
                            <td>${this.formatDate(job.end_time)}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    }

    renderFilesTable(files) {
        if (!files || files.length === 0) {
            return '<div class="placeholder-message">No files in this workflow</div>';
        }

        return `
            <table class="data-table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                        <th>Path</th>
                        <th>Type</th>
                    </tr>
                </thead>
                <tbody>
                    ${files.map(file => `
                        <tr>
                            <td><code>${this.truncateId(file.id)}</code></td>
                            <td>${this.escapeHtml(file.name || '-')}</td>
                            <td><code>${this.escapeHtml(file.path || '-')}</code></td>
                            <td>${this.escapeHtml(file.file_type || '-')}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    }

    renderUserDataTable(userData) {
        if (!userData || userData.length === 0) {
            return '<div class="placeholder-message">No user data in this workflow</div>';
        }

        return `
            <table class="data-table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                        <th>Data</th>
                    </tr>
                </thead>
                <tbody>
                    ${userData.map(ud => `
                        <tr>
                            <td><code>${this.truncateId(ud.id)}</code></td>
                            <td>${this.escapeHtml(ud.name || '-')}</td>
                            <td><code>${this.escapeHtml(this.truncate(JSON.stringify(ud.data) || '-', 100))}</code></td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    }

    renderResultsTable(results) {
        if (!results || results.length === 0) {
            return '<div class="placeholder-message">No results in this workflow</div>';
        }

        return `
            <table class="data-table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Job ID</th>
                        <th>Return Code</th>
                        <th>Stdout</th>
                        <th>Stderr</th>
                    </tr>
                </thead>
                <tbody>
                    ${results.map(result => `
                        <tr>
                            <td><code>${this.truncateId(result.id)}</code></td>
                            <td><code>${this.truncateId(result.job_id)}</code></td>
                            <td>${result.return_code ?? '-'}</td>
                            <td><code>${this.escapeHtml(this.truncate(result.stdout || '-', 50))}</code></td>
                            <td><code>${this.escapeHtml(this.truncate(result.stderr || '-', 50))}</code></td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    }

    // ==================== DAG Tab ====================

    setupDAGTab() {
        document.getElementById('dag-workflow-selector')?.addEventListener('change', async (e) => {
            const workflowId = e.target.value;
            if (workflowId) {
                this.selectedWorkflowId = workflowId;
                dagVisualizer.initialize();
                await this.loadDAG(workflowId);
            }
        });

        document.getElementById('dag-type-selector')?.addEventListener('change', async (e) => {
            if (this.selectedWorkflowId) {
                await this.loadDAG(this.selectedWorkflowId);
            }
        });

        document.getElementById('btn-fit-dag')?.addEventListener('click', () => {
            dagVisualizer.fitToView();
        });
    }

    async loadDAG(workflowId) {
        const type = document.getElementById('dag-type-selector')?.value || 'jobs';

        switch (type) {
            case 'jobs':
                await dagVisualizer.loadJobDependencies(workflowId);
                break;
            case 'files':
                await dagVisualizer.loadFileRelationships(workflowId);
                break;
            case 'userdata':
                await dagVisualizer.loadUserDataRelationships(workflowId);
                break;
        }
    }

    // ==================== Events Tab ====================

    setupEventsTab() {
        document.getElementById('events-workflow-selector')?.addEventListener('change', () => {
            this.events = [];
            this.lastEventId = null;
            this.loadEvents();
        });

        document.getElementById('btn-clear-events')?.addEventListener('click', () => {
            this.events = [];
            this.lastEventId = null;
            this.renderEvents();
        });

        document.getElementById('auto-refresh-events')?.addEventListener('change', (e) => {
            if (e.target.checked) {
                this.startEventPolling();
            } else {
                this.stopEventPolling();
            }
        });

        // Start polling if auto-refresh is checked
        const autoRefresh = document.getElementById('auto-refresh-events');
        if (autoRefresh?.checked) {
            this.startEventPolling();
        }
    }

    startEventPolling() {
        this.stopEventPolling();
        this.loadEvents();
        this.eventPollInterval = setInterval(() => this.loadEvents(), 10000);
    }

    stopEventPolling() {
        if (this.eventPollInterval) {
            clearInterval(this.eventPollInterval);
            this.eventPollInterval = null;
        }
    }

    async loadEvents() {
        try {
            const workflowId = document.getElementById('events-workflow-selector')?.value || null;
            const newEvents = await api.listEvents(workflowId, 0, 50, this.lastEventId);

            if (newEvents && newEvents.length > 0) {
                // Prepend new events
                this.events = [...newEvents, ...this.events].slice(0, 200); // Keep last 200 events
                this.lastEventId = newEvents[0].id;
                this.updateEventBadge(newEvents.length);
            }

            this.renderEvents();
        } catch (error) {
            console.error('Error loading events:', error);
        }
    }

    renderEvents() {
        const container = document.getElementById('events-list');
        if (!container) return;

        if (this.events.length === 0) {
            container.innerHTML = '<div class="placeholder-message">No events yet</div>';
            return;
        }

        container.innerHTML = this.events.map(event => `
            <div class="event-item">
                <span class="event-time">${this.formatDate(event.timestamp)}</span>
                <span class="event-type">${this.escapeHtml(event.event_type || '-')}</span>
                <span class="event-message">${this.escapeHtml(event.message || '-')}</span>
            </div>
        `).join('');
    }

    updateEventBadge(count) {
        const badge = document.getElementById('event-badge');
        if (badge) {
            if (count > 0 && this.currentTab !== 'events') {
                badge.textContent = count;
                badge.style.display = 'inline';
            } else {
                badge.style.display = 'none';
            }
        }
    }

    // ==================== Settings Tab ====================

    setupSettingsTab() {
        document.getElementById('btn-save-settings')?.addEventListener('click', () => {
            this.saveSettings();
        });

        document.getElementById('btn-test-connection')?.addEventListener('click', async () => {
            const apiUrl = document.getElementById('api-url')?.value;
            if (apiUrl) {
                api.setBaseUrl(apiUrl);
            }
            await this.testConnection();
        });

        document.getElementById('dark-mode')?.addEventListener('change', (e) => {
            if (e.target.checked) {
                document.body.classList.add('dark-mode');
            } else {
                document.body.classList.remove('dark-mode');
            }
        });
    }

    // ==================== Modal ====================

    setupModal() {
        document.getElementById('modal-close')?.addEventListener('click', () => {
            this.hideModal('create-workflow-modal');
        });

        document.getElementById('btn-cancel-create')?.addEventListener('click', () => {
            this.hideModal('create-workflow-modal');
        });

        document.getElementById('btn-submit-workflow')?.addEventListener('click', async () => {
            await this.createWorkflow();
        });

        // Close modal on background click
        document.getElementById('create-workflow-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('create-workflow-modal');
            }
        });
    }

    showModal(modalId) {
        document.getElementById(modalId)?.classList.add('active');
    }

    hideModal(modalId) {
        document.getElementById(modalId)?.classList.remove('active');
    }

    async createWorkflow() {
        const nameInput = document.getElementById('workflow-name');
        const descInput = document.getElementById('workflow-description');

        const name = nameInput?.value?.trim();
        const description = descInput?.value?.trim();

        if (!name) {
            this.showToast('Please provide a workflow name', 'warning');
            return;
        }

        try {
            // Create a workflow with just name and description
            // For full workflow specs with jobs, use the CLI: torc workflows create <spec.yaml>
            const workflow = {
                name: name,
                description: description || null,
            };

            const result = await api.createWorkflow(workflow);
            this.showToast('Workflow created: ' + (result.name || result.id || 'Success'), 'success');
            this.hideModal('create-workflow-modal');

            // Clear form
            if (nameInput) nameInput.value = '';
            if (descInput) descInput.value = '';

            await this.loadWorkflows();
        } catch (error) {
            this.showToast('Error creating workflow: ' + error.message, 'error');
        }
    }

    // ==================== Utilities ====================

    showToast(message, type = 'info') {
        const container = document.getElementById('toast-container');
        if (!container) return;

        const toast = document.createElement('div');
        toast.className = `toast ${type}`;
        toast.textContent = message;
        container.appendChild(toast);

        setTimeout(() => {
            toast.remove();
        }, 5000);
    }

    escapeHtml(str) {
        if (str === null || str === undefined) return '';
        const div = document.createElement('div');
        div.textContent = String(str);
        return div.innerHTML;
    }

    truncateId(id) {
        if (!id) return '-';
        return id.length > 8 ? id.substring(0, 8) + '...' : id;
    }

    truncate(str, maxLen) {
        if (!str) return '';
        return str.length > maxLen ? str.substring(0, maxLen) + '...' : str;
    }

    formatDate(dateStr) {
        if (!dateStr) return '-';
        try {
            const date = new Date(dateStr);
            return date.toLocaleString();
        } catch {
            return dateStr;
        }
    }
}

// Initialize application
const app = new TorcDashboard();
document.addEventListener('DOMContentLoaded', () => app.init());
