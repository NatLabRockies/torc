/**
 * Torc Dashboard - Modal Handling
 * Create workflow modal, execution plan modal, job details modal, file viewer modal
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Modal ====================

    setupModal() {
        document.getElementById('modal-close')?.addEventListener('click', () => {
            this.hideModal('create-workflow-modal');
            this.resetWizard();
        });

        document.getElementById('btn-cancel-create')?.addEventListener('click', () => {
            this.hideModal('create-workflow-modal');
            this.resetWizard();
        });

        document.getElementById('btn-submit-workflow')?.addEventListener('click', async () => {
            await this.createWorkflow();
        });

        // Close modal on background click (use mousedown to avoid closing when selecting text)
        document.getElementById('create-workflow-modal')?.addEventListener('mousedown', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('create-workflow-modal');
                this.resetWizard();
            }
        });

        // Create source tabs
        document.querySelectorAll('.sub-tab[data-createtab]').forEach(tab => {
            tab.addEventListener('click', () => {
                this.switchCreateTab(tab.dataset.createtab);
            });
        });

        // File upload zone
        this.setupFileUpload();

        // Slurm checkbox toggle
        this.setupSlurmOptions();
    },

    setupSlurmOptions() {
        const slurmCheckbox = document.getElementById('create-option-slurm');
        const accountSection = document.getElementById('slurm-account-section');

        slurmCheckbox?.addEventListener('change', () => {
            if (accountSection) {
                accountSection.style.display = slurmCheckbox.checked ? 'block' : 'none';
            }
            // Re-render wizard steps if we're on them (to update the messages/preview)
            if (this.currentCreateTab === 'wizard') {
                if (this.wizardStep === 3) {
                    this.wizardRenderSchedulers();
                } else if (this.wizardStep === 4) {
                    this.wizardRenderActions();
                } else if (this.wizardStep === 6) {
                    this.wizardGeneratePreview();
                }
            }
        });
    },

    async checkHpcProfiles() {
        // Check for available HPC profiles
        const slurmGroup = document.getElementById('slurm-options-group');
        const slurmCheckbox = document.getElementById('create-option-slurm');
        const slurmBadge = document.getElementById('slurm-profile-badge');
        const slurmHint = document.getElementById('slurm-disabled-hint');
        const accountSection = document.getElementById('slurm-account-section');

        if (!slurmGroup) return;

        try {
            const result = await api.getHpcProfiles();

            if (result.success && result.detected_profile) {
                // HPC profile detected - enable the option
                this.detectedHpcProfile = result.detected_profile;
                slurmGroup.style.display = 'block';
                slurmCheckbox.disabled = false;
                slurmHint.style.display = 'none';
                slurmBadge.style.display = 'inline';
                slurmBadge.textContent = result.detected_profile;
            } else if (result.success && result.profiles && result.profiles.length > 0) {
                // Profiles available but none detected - show disabled
                this.detectedHpcProfile = null;
                slurmGroup.style.display = 'block';
                slurmCheckbox.disabled = true;
                slurmCheckbox.checked = false;
                slurmHint.style.display = 'block';
                slurmBadge.style.display = 'none';
                accountSection.style.display = 'none';
            } else {
                // No profiles available - hide the section entirely
                this.detectedHpcProfile = null;
                slurmGroup.style.display = 'none';
            }
        } catch (error) {
            console.error('Error checking HPC profiles:', error);
            slurmGroup.style.display = 'none';
            this.detectedHpcProfile = null;
        }
    },

    setupFileUpload() {
        const zone = document.getElementById('file-upload-zone');
        const input = document.getElementById('spec-file-input');

        if (!zone || !input) return;

        zone.addEventListener('click', () => input.click());

        zone.addEventListener('dragover', (e) => {
            e.preventDefault();
            zone.classList.add('drag-over');
        });

        zone.addEventListener('dragleave', () => {
            zone.classList.remove('drag-over');
        });

        zone.addEventListener('drop', (e) => {
            e.preventDefault();
            zone.classList.remove('drag-over');
            const file = e.dataTransfer.files[0];
            if (file) this.handleFileUpload(file);
        });

        input.addEventListener('change', (e) => {
            const file = e.target.files[0];
            if (file) this.handleFileUpload(file);
        });
    },

    handleFileUpload(file) {
        const reader = new FileReader();
        reader.onload = (e) => {
            this.uploadedSpecContent = e.target.result;
            // Extract the file extension to preserve format when creating temp file
            const dotIndex = file.name.lastIndexOf('.');
            this.uploadedSpecExtension = dotIndex >= 0 ? file.name.substring(dotIndex) : '.json';
            document.getElementById('upload-status').innerHTML = `
                <p style="color: var(--success-color)">Uploaded: ${this.escapeHtml(file.name)} (${(file.size / 1024).toFixed(1)} KB)</p>
            `;
        };
        reader.onerror = () => {
            this.showToast('Error reading file', 'error');
        };
        reader.readAsText(file);
    },

    switchCreateTab(tabName) {
        this.currentCreateTab = tabName;

        document.querySelectorAll('.sub-tab[data-createtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.createtab === tabName);
        });

        document.querySelectorAll('.create-panel').forEach(panel => {
            panel.classList.toggle('active', panel.id === `create-panel-${tabName}`);
        });
    },

    showModal(modalId) {
        document.getElementById(modalId)?.classList.add('active');

        // Check HPC profiles when create workflow modal is opened
        if (modalId === 'create-workflow-modal') {
            this.checkHpcProfiles();
        }
    },

    hideModal(modalId) {
        document.getElementById(modalId)?.classList.remove('active');
    },

    async createWorkflow() {
        let specContent = null;
        let isFilePath = false;
        let fileExtension = null;

        switch (this.currentCreateTab) {
            case 'upload':
                if (!this.uploadedSpecContent) {
                    this.showToast('Please upload a workflow spec file', 'warning');
                    return;
                }
                specContent = this.uploadedSpecContent;
                fileExtension = this.uploadedSpecExtension;
                break;
            case 'path':
                const pathInput = document.getElementById('workflow-spec-path')?.value?.trim();
                if (!pathInput) {
                    this.showToast('Please enter a spec file path', 'warning');
                    return;
                }
                specContent = pathInput;
                isFilePath = true;
                break;
            case 'inline':
                const textInput = document.getElementById('workflow-spec-text')?.value?.trim();
                if (!textInput) {
                    this.showToast('Please enter a workflow spec', 'warning');
                    return;
                }
                specContent = textInput;
                break;
            case 'wizard':
                // Use the wizard's create workflow function
                await this.wizardCreateWorkflow();
                return;
        }

        // Check if Slurm option is selected
        const useSlurmCheckbox = document.getElementById('create-option-slurm');
        const useSlurm = useSlurmCheckbox && !useSlurmCheckbox.disabled && useSlurmCheckbox.checked;
        const slurmAccount = document.getElementById('create-slurm-account')?.value?.trim();

        if (useSlurm && !slurmAccount) {
            this.showToast('Please enter a Slurm account name', 'warning');
            return;
        }

        try {
            let result;
            if (useSlurm) {
                result = await api.cliCreateSlurmWorkflow(
                    specContent,
                    isFilePath,
                    fileExtension,
                    slurmAccount,
                    this.detectedHpcProfile
                );
            } else {
                result = await api.cliCreateWorkflow(specContent, isFilePath, fileExtension);
            }

            if (result.success) {
                this.showToast('Workflow created successfully', 'success');
                this.hideModal('create-workflow-modal');

                // Clear form
                this.uploadedSpecContent = null;
                this.uploadedSpecExtension = null;
                document.getElementById('upload-status').innerHTML = '';
                const pathInput = document.getElementById('workflow-spec-path');
                const textInput = document.getElementById('workflow-spec-text');
                if (pathInput) pathInput.value = '';
                if (textInput) textInput.value = '';

                await this.loadWorkflows();

                // Check if we should initialize
                const shouldInit = document.getElementById('create-option-initialize')?.checked;

                // Try to extract workflow ID from JSON output
                const workflowId = this.extractWorkflowId(result.stdout);
                if (workflowId && shouldInit) {
                    await this.initializeWorkflow(workflowId);
                }
            } else {
                const errorMsg = result.stderr || result.stdout || 'Unknown error';
                this.showToast('Error: ' + errorMsg, 'error');
            }
        } catch (error) {
            this.showToast('Error creating workflow: ' + error.message, 'error');
        }
    },

    // ==================== Execution Plan Modal ====================

    setupExecutionPlanModal() {
        document.getElementById('plan-modal-close')?.addEventListener('click', () => {
            this.hideModal('execution-plan-modal');
        });

        document.getElementById('btn-close-plan')?.addEventListener('click', () => {
            this.hideModal('execution-plan-modal');
        });

        document.getElementById('execution-plan-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('execution-plan-modal');
            }
        });
    },

    setupInitConfirmModal() {
        document.getElementById('init-confirm-modal-close')?.addEventListener('click', () => {
            this.hideModal('init-confirm-modal');
        });

        document.getElementById('btn-cancel-init')?.addEventListener('click', () => {
            this.hideModal('init-confirm-modal');
        });

        document.getElementById('btn-confirm-init')?.addEventListener('click', async () => {
            this.hideModal('init-confirm-modal');
            if (this.pendingInitializeWorkflowId) {
                await this.initializeWorkflow(this.pendingInitializeWorkflowId, true);
                this.pendingInitializeWorkflowId = null;
            }
        });

        document.getElementById('init-confirm-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('init-confirm-modal');
            }
        });
    },

    setupReinitConfirmModal() {
        document.getElementById('reinit-confirm-modal-close')?.addEventListener('click', () => {
            this.hideModal('reinit-confirm-modal');
        });

        document.getElementById('btn-cancel-reinit')?.addEventListener('click', () => {
            this.hideModal('reinit-confirm-modal');
        });

        document.getElementById('btn-confirm-reinit')?.addEventListener('click', async () => {
            this.hideModal('reinit-confirm-modal');
            if (this.pendingReinitializeWorkflowId) {
                await this.reinitializeWorkflow(this.pendingReinitializeWorkflowId);
                this.pendingReinitializeWorkflowId = null;
            }
        });

        document.getElementById('reinit-confirm-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('reinit-confirm-modal');
            }
        });
    },

    setupRecoverModal() {
        document.getElementById('recover-modal-close')?.addEventListener('click', () => {
            this.hideModal('recover-modal');
        });

        document.getElementById('btn-cancel-recover')?.addEventListener('click', () => {
            this.hideModal('recover-modal');
        });

        document.getElementById('btn-confirm-recover')?.addEventListener('click', async () => {
            if (this.pendingRecoverWorkflowId) {
                await this.executeRecovery(this.pendingRecoverWorkflowId);
                this.pendingRecoverWorkflowId = null;
            }
        });

        document.getElementById('recover-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('recover-modal');
            }
        });
    },

    setupSyncStatusModal() {
        document.getElementById('sync-status-modal-close')?.addEventListener('click', () => {
            this.hideModal('sync-status-modal');
        });

        document.getElementById('btn-cancel-sync-status')?.addEventListener('click', () => {
            this.hideModal('sync-status-modal');
        });

        document.getElementById('btn-confirm-sync-status')?.addEventListener('click', async () => {
            if (this.pendingSyncStatusWorkflowId) {
                await this.executeSyncStatus(this.pendingSyncStatusWorkflowId);
                this.pendingSyncStatusWorkflowId = null;
            }
        });

        document.getElementById('sync-status-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('sync-status-modal');
            }
        });
    },

    setupFileViewerModal() {
        // Close button handlers
        document.getElementById('file-viewer-modal-close')?.addEventListener('click', () => {
            this.hideModal('file-viewer-modal');
        });

        document.getElementById('btn-close-file-viewer')?.addEventListener('click', () => {
            this.hideModal('file-viewer-modal');
        });

        // Close on background click
        document.getElementById('file-viewer-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('file-viewer-modal');
            }
        });

        // Delegate click events for "View" buttons in the files table
        document.addEventListener('click', async (e) => {
            if (e.target.classList.contains('btn-view-file')) {
                const path = e.target.dataset.path;
                const name = e.target.dataset.name;
                if (path) {
                    await this.viewFile(path, name);
                }
            }
        });
    },

    async viewFile(path, name) {
        this.showModal('file-viewer-modal');
        const titleEl = document.getElementById('file-viewer-title');
        const pathEl = document.getElementById('file-viewer-path');
        const contentEl = document.getElementById('file-viewer-content');

        titleEl.textContent = name || 'File Contents';
        pathEl.textContent = path;
        contentEl.innerHTML = '<span class="placeholder-message">Loading file contents...</span>';

        try {
            const response = await fetch('/api/cli/read-file', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path }),
            });

            const result = await response.json();

            if (!result.exists) {
                contentEl.innerHTML = '<span class="file-not-found">File does not exist</span>';
                return;
            }

            if (!result.success) {
                contentEl.innerHTML = `<span class="file-not-found">Error: ${this.escapeHtml(result.error || 'Unknown error')}</span>`;
                return;
            }

            if (result.is_json) {
                // Apply JSON syntax highlighting
                contentEl.innerHTML = this.highlightJson(result.content);
            } else {
                contentEl.textContent = result.content;
            }
        } catch (error) {
            contentEl.innerHTML = `<span class="file-not-found">Error loading file: ${this.escapeHtml(error.message)}</span>`;
        }
    },

    // ==================== Export Modal ====================

    setupExportModal() {
        document.getElementById('export-modal-close')?.addEventListener('click', () => {
            this.hideModal('export-workflow-modal');
        });

        document.getElementById('btn-cancel-export')?.addEventListener('click', () => {
            this.hideModal('export-workflow-modal');
        });

        document.getElementById('btn-confirm-export')?.addEventListener('click', async () => {
            await this.executeExport();
        });

        document.getElementById('export-workflow-modal')?.addEventListener('mousedown', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('export-workflow-modal');
            }
        });
    },

    // ==================== Import Modal ====================

    setupImportModal() {
        this.currentImportTab = 'path';

        document.getElementById('import-modal-close')?.addEventListener('click', () => {
            this.hideModal('import-workflow-modal');
            this.clearImportState();
        });

        document.getElementById('btn-cancel-import')?.addEventListener('click', () => {
            this.hideModal('import-workflow-modal');
            this.clearImportState();
        });

        document.getElementById('btn-confirm-import')?.addEventListener('click', async () => {
            await this.executeImport();
        });

        document.getElementById('import-workflow-modal')?.addEventListener('mousedown', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('import-workflow-modal');
                this.clearImportState();
            }
        });

        // Import source tabs
        document.querySelectorAll('.sub-tab[data-importtab]').forEach(tab => {
            tab.addEventListener('click', () => {
                this.switchImportTab(tab.dataset.importtab);
            });
        });

        this.setupImportFileUpload();
    },

    switchImportTab(tabName) {
        this.currentImportTab = tabName;
        document.querySelectorAll('.sub-tab[data-importtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.importtab === tabName);
        });
        document.getElementById('import-panel-path').style.display = tabName === 'path' ? 'block' : 'none';
        document.getElementById('import-panel-upload').style.display = tabName === 'upload' ? 'block' : 'none';
    },

    setupImportFileUpload() {
        const zone = document.getElementById('import-file-upload-zone');
        const input = document.getElementById('import-file-input');
        if (!zone || !input) return;

        zone.addEventListener('click', () => input.click());

        zone.addEventListener('dragover', (e) => {
            e.preventDefault();
            zone.classList.add('drag-over');
        });

        zone.addEventListener('dragleave', () => {
            zone.classList.remove('drag-over');
        });

        zone.addEventListener('drop', (e) => {
            e.preventDefault();
            zone.classList.remove('drag-over');
            const file = e.dataTransfer.files[0];
            if (file) this.handleImportFileUpload(file);
        });

        input.addEventListener('change', (e) => {
            const file = e.target.files[0];
            if (file) this.handleImportFileUpload(file);
        });
    },

    handleImportFileUpload(file) {
        const reader = new FileReader();
        reader.onload = (e) => {
            this.importFileContent = e.target.result;
            document.getElementById('import-upload-status').innerHTML = `
                <p style="color: var(--success-color)">Loaded: ${this.escapeHtml(file.name)} (${(file.size / 1024).toFixed(1)} KB)</p>
            `;
        };
        reader.onerror = () => {
            this.showToast('Error reading file', 'error');
        };
        reader.readAsText(file);
    },

    clearImportState() {
        this.importFileContent = null;
        this.currentImportTab = 'path';
        const statusEl = document.getElementById('import-upload-status');
        if (statusEl) statusEl.innerHTML = '';
        const importStatusEl = document.getElementById('import-status');
        if (importStatusEl) importStatusEl.innerHTML = '';
        const nameInput = document.getElementById('import-name-override');
        if (nameInput) nameInput.value = '';
        const pathInput = document.getElementById('import-file-path');
        if (pathInput) pathInput.value = '';
        const fileInput = document.getElementById('import-file-input');
        if (fileInput) fileInput.value = '';
        const skipResults = document.getElementById('import-skip-results');
        if (skipResults) skipResults.checked = false;
        const skipEvents = document.getElementById('import-skip-events');
        if (skipEvents) skipEvents.checked = false;
        // Reset tabs to default
        this.switchImportTab('path');
    },

    async showExecutionPlan(workflowId) {
        this.showModal('execution-plan-modal');
        const content = document.getElementById('execution-plan-content');
        content.innerHTML = '<div class="placeholder-message">Loading execution plan...</div>';

        try {
            // Get execution plan from the CLI
            const response = await api.getExecutionPlan(workflowId);

            if (!response.success) {
                content.innerHTML = `<div class="placeholder-message">Error: ${response.error || 'Unknown error'}</div>`;
                return;
            }

            const plan = response.data;
            content.innerHTML = this.renderExecutionPlan(plan);
        } catch (error) {
            content.innerHTML = `<div class="placeholder-message">Error loading execution plan: ${this.escapeHtml(error.message)}</div>`;
        }
    },

    renderExecutionPlan(plan) {
        if (!plan || !plan.events || plan.events.length === 0) {
            return '<div class="placeholder-message">No execution events computed</div>';
        }

        const events = plan.events;
        const rootEvents = plan.root_events || [];
        const leafEvents = plan.leaf_events || [];

        // Build event map for quick lookup
        const eventMap = {};
        events.forEach(e => eventMap[e.id] = e);

        // Count total jobs
        let totalJobs = 0;
        events.forEach(e => totalJobs += (e.jobs_becoming_ready || []).length);

        // Render events in a topological order using BFS from roots
        const visited = new Set();
        const orderedEvents = [];
        const queue = [...rootEvents];

        while (queue.length > 0) {
            const eventId = queue.shift();
            if (visited.has(eventId)) continue;
            visited.add(eventId);

            const event = eventMap[eventId];
            if (event) {
                orderedEvents.push(event);
                // Add unlocked events to queue
                (event.unlocks_events || []).forEach(next => {
                    if (!visited.has(next)) {
                        queue.push(next);
                    }
                });
            }
        }

        // Also add any events not reachable from roots (shouldn't happen, but just in case)
        events.forEach(e => {
            if (!visited.has(e.id)) {
                orderedEvents.push(e);
            }
        });

        return `
            <div class="plan-summary" style="margin-bottom: 16px;">
                <strong>Total Events:</strong> ${events.length} |
                <strong>Total Jobs:</strong> ${totalJobs}
            </div>
            <div class="plan-events">
                ${orderedEvents.map((event, idx) => this.renderExecutionEvent(event, idx, rootEvents, leafEvents)).join('')}
            </div>
        `;
    },

    renderExecutionEvent(event, index, rootEvents, leafEvents) {
        const isRoot = rootEvents.includes(event.id);
        const isLeaf = leafEvents.includes(event.id);
        const jobs = event.jobs_becoming_ready || [];
        const schedulers = event.scheduler_allocations || [];
        const unlocks = event.unlocks_events || [];

        // Determine event type icon and style
        let eventIcon = '→';
        let eventClass = '';
        if (isRoot) {
            eventIcon = '▶';
            eventClass = 'event-root';
        } else if (isLeaf) {
            eventIcon = '◆';
            eventClass = 'event-leaf';
        }

        // Format trigger description
        const triggerDesc = event.trigger_description || this.formatEventTrigger(event.trigger);

        return `
            <div class="plan-stage ${eventClass}">
                <div class="plan-stage-header">
                    <div class="plan-stage-number">${eventIcon}</div>
                    <div class="plan-stage-trigger">${this.escapeHtml(triggerDesc)}</div>
                </div>
                <div class="plan-stage-content">
                    ${schedulers.length > 0 ? `
                        <div class="plan-section">
                            <h5>Scheduler Allocations</h5>
                            <ul>
                                ${schedulers.map(s => `
                                    <li>
                                        <strong>${this.escapeHtml(s.scheduler)}</strong>
                                        (${this.escapeHtml(s.scheduler_type)})
                                        - ${s.num_allocations} allocation${s.num_allocations !== 1 ? 's' : ''}
                                    </li>
                                `).join('')}
                            </ul>
                        </div>
                    ` : ''}
                    <div class="plan-section">
                        <h5>Jobs Becoming Ready (${jobs.length})</h5>
                        <ul>
                            ${jobs.slice(0, 10).map(job => `
                                <li>${this.escapeHtml(job)}</li>
                            `).join('')}
                            ${jobs.length > 10 ? `<li>... and ${jobs.length - 10} more</li>` : ''}
                        </ul>
                    </div>
                    ${unlocks.length > 0 ? `
                        <div class="plan-section plan-flow">
                            <span class="flow-arrow">↓</span>
                            <span class="flow-text">unlocks ${unlocks.length} event${unlocks.length !== 1 ? 's' : ''}</span>
                        </div>
                    ` : ''}
                </div>
            </div>
        `;
    },

    formatEventTrigger(trigger) {
        if (!trigger) return 'Unknown trigger';

        if (trigger.type === 'WorkflowStart') {
            return 'Workflow Start';
        } else if (trigger.type === 'JobsComplete') {
            const jobs = trigger.data?.jobs || [];
            if (jobs.length === 0) return 'Jobs Complete';
            if (jobs.length === 1) return `When job '${jobs[0]}' completes`;
            if (jobs.length <= 3) return `When jobs complete: ${jobs.map(j => `'${j}'`).join(', ')}`;
            return `When ${jobs.length} jobs complete ('${jobs[0]}', '${jobs[1]}'...)`;
        }

        return 'Unknown trigger';
    },
});
