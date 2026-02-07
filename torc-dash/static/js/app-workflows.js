/**
 * Torc Dashboard - Workflows Tab
 * Workflow listing, running, and management
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Workflows Tab ====================

    setupWorkflowsTab() {
        document.getElementById('btn-refresh-workflows')?.addEventListener('click', () => {
            this.loadWorkflows();
        });

        document.getElementById('btn-create-workflow')?.addEventListener('click', () => {
            this.showModal('create-workflow-modal');
        });

        // Workflow filter
        document.getElementById('workflow-filter')?.addEventListener('input', (e) => {
            this.filterWorkflows(e.target.value);
        });

        // Select all checkbox
        document.getElementById('workflows-select-all')?.addEventListener('change', (e) => {
            this.toggleSelectAllWorkflows(e.target.checked);
        });

        // Bulk action buttons
        document.getElementById('btn-bulk-delete')?.addEventListener('click', () => {
            this.bulkDeleteWorkflows();
        });

        document.getElementById('btn-clear-selection')?.addEventListener('click', () => {
            this.clearWorkflowSelection();
        });
    },

    async loadWorkflows() {
        try {
            const workflows = await api.listWorkflows(0, 100, this.currentUser);
            this.workflows = workflows || [];
            // Sort by id descending (newer workflows first)
            this.workflows.sort((a, b) => {
                const idA = parseInt(a.id) || 0;
                const idB = parseInt(b.id) || 0;
                return idB - idA;
            });
            this.renderWorkflowsTable(this.workflows);
            this.updateWorkflowSelectors(this.workflows);
        } catch (error) {
            console.error('Error loading workflows:', error);
            this.showToast('Error loading workflows: ' + error.message, 'error');
        }
    },

    filterWorkflows(filterText) {
        if (!filterText) {
            this.renderWorkflowsTable(this.workflows);
            return;
        }

        const lowerFilter = filterText.toLowerCase();
        const filtered = this.workflows.filter(w =>
            (w.name || '').toLowerCase().includes(lowerFilter) ||
            (w.user || '').toLowerCase().includes(lowerFilter) ||
            (w.project || '').toLowerCase().includes(lowerFilter) ||
            String(w.id || '').toLowerCase().includes(lowerFilter) ||
            (w.description || '').toLowerCase().includes(lowerFilter)
        );
        this.renderWorkflowsTable(filtered);
    },

    renderWorkflowsTable(workflows) {
        const tbody = document.getElementById('workflows-body');
        if (!tbody) return;

        if (!workflows || workflows.length === 0) {
            tbody.innerHTML = '<tr><td colspan="8" class="placeholder-message">No workflows found</td></tr>';
            this.updateBulkActionBar();
            return;
        }

        tbody.innerHTML = workflows.map(workflow => {
            const isSelected = this.selectedWorkflowIds.has(workflow.id);
            return `
            <tr data-workflow-id="${workflow.id}" class="clickable-row ${isSelected ? 'selected' : ''}" onclick="app.viewWorkflow('${workflow.id}')">
                <td class="checkbox-column" onclick="event.stopPropagation()">
                    <input type="checkbox"
                           class="workflow-checkbox"
                           data-workflow-id="${workflow.id}"
                           ${isSelected ? 'checked' : ''}
                           onchange="app.toggleWorkflowSelection('${workflow.id}', this.checked)">
                </td>
                <td><code>${workflow.id ?? '-'}</code></td>
                <td>${this.escapeHtml(workflow.name || 'Unnamed')}</td>
                <td>${this.formatTimestamp(workflow.timestamp)}</td>
                <td>${this.escapeHtml(workflow.user || '-')}</td>
                <td>${this.escapeHtml(workflow.project || '-')}</td>
                <td title="${this.escapeHtml(workflow.description || '')}">${this.escapeHtml(this.truncate(workflow.description || '-', 40))}</td>
                <td class="actions-column" onclick="event.stopPropagation()">
                    <div class="action-buttons">
                        <button class="btn btn-sm btn-success" onclick="app.runWorkflow('${workflow.id}')" title="Run Locally">Run</button>
                        <button class="btn btn-sm btn-primary" onclick="app.submitWorkflow('${workflow.id}')" title="Submit to Scheduler">Submit</button>
                        <button class="btn btn-sm btn-secondary" onclick="app.viewWorkflow('${workflow.id}')" title="View Details">View</button>
                        <button class="btn btn-sm btn-secondary" onclick="app.viewDAG('${workflow.id}')" title="View DAG">DAG</button>
                        <button class="btn btn-sm btn-danger" onclick="app.deleteWorkflow('${workflow.id}')" title="Delete">Del</button>
                    </div>
                </td>
            </tr>
        `;
        }).join('');

        this.updateSelectAllCheckbox();
        this.updateBulkActionBar();
    },

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
    },

    updateWorkflowSelectors(workflows) {
        const selectors = [
            'workflow-selector',
            'dag-workflow-selector',
            'events-workflow-selector',
            'debug-workflow-selector',
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
    },

    async viewWorkflow(workflowId) {
        this.selectedWorkflowId = workflowId;
        document.getElementById('workflow-selector').value = workflowId;
        this.switchTab('details');
        await this.loadWorkflowDetails(workflowId);
    },

    async viewDAG(workflowId) {
        this.selectedWorkflowId = workflowId;
        document.getElementById('dag-workflow-selector').value = workflowId;
        this.switchTab('dag');
        dagVisualizer.initialize();
        await dagVisualizer.loadJobDependencies(workflowId);
    },

    async deleteWorkflow(workflowId) {
        if (!confirm('Are you sure you want to delete this workflow? This action cannot be undone.')) {
            return;
        }

        try {
            const result = await api.cliDeleteWorkflow(workflowId);
            if (result.success) {
                this.showToast('Workflow deleted', 'success');
                await this.loadWorkflows();
            } else {
                this.showToast('Error: ' + (result.stderr || result.stdout), 'error');
            }
        } catch (error) {
            this.showToast('Error deleting workflow: ' + error.message, 'error');
        }
    },

    async runWorkflow(workflowId) {
        // Show the execution output panel
        this.showExecutionPanel();
        this.appendExecutionOutput(`Starting workflow ${workflowId}...\n`, 'info');

        // Create EventSource for streaming
        const eventSource = new EventSource(`/api/cli/run-stream?workflow_id=${workflowId}`);
        this.currentEventSource = eventSource;

        eventSource.addEventListener('start', (e) => {
            this.appendExecutionOutput(`${e.data}\n`, 'info');
        });

        eventSource.addEventListener('stdout', (e) => {
            this.appendExecutionOutput(`${e.data}\n`, 'stdout');
        });

        eventSource.addEventListener('stderr', (e) => {
            this.appendExecutionOutput(`${e.data}\n`, 'stderr');
        });

        eventSource.addEventListener('status', (e) => {
            // Status updates from periodic API polling - shown in a different color
            this.appendExecutionOutput(`[Status] ${e.data}\n`, 'info');
        });

        eventSource.addEventListener('error', (e) => {
            if (e.data) {
                this.appendExecutionOutput(`Error: ${e.data}\n`, 'error');
            }
        });

        eventSource.addEventListener('end', (e) => {
            const status = e.data;
            if (status === 'success') {
                this.appendExecutionOutput(`\n✓ Workflow completed successfully\n`, 'success');
                this.showToast('Workflow completed successfully', 'success');
            } else {
                this.appendExecutionOutput(`\n✗ Workflow ${status}\n`, 'error');
                this.showToast(`Workflow ${status}`, 'error');
            }
            eventSource.close();
            this.currentEventSource = null;
            this.hideExecutionCancelButton();
            // Refresh workflow details
            this.loadWorkflows();
            this.loadWorkflowDetails(workflowId);
        });

        eventSource.onerror = (e) => {
            if (eventSource.readyState === EventSource.CLOSED) {
                // Normal close
                return;
            }
            this.appendExecutionOutput(`\nConnection error\n`, 'error');
            eventSource.close();
            this.currentEventSource = null;
            this.hideExecutionCancelButton();
        };
    },

    async recoverWorkflow(workflowId) {
        // Store for use in confirm handler
        this.pendingRecoverWorkflowId = workflowId;

        // Show modal with loading state
        const content = document.getElementById('recover-content');
        const footer = document.getElementById('recover-modal-footer');
        if (content) {
            content.innerHTML = '<div class="placeholder-message">Analyzing workflow for recovery...</div>';
        }
        if (footer) {
            footer.style.display = 'none'; // Hide buttons until we have data
        }
        this.showModal('recover-modal');

        try {
            const response = await fetch('/api/cli/recover', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    workflow_id: workflowId.toString(),
                    dry_run: true
                })
            });

            const result = await response.json();

            if (!result.success) {
                content.innerHTML = `<div class="recover-error">Error: ${this.escapeHtml(result.error || 'Recovery check failed')}</div>`;
                return;
            }

            const data = result.data;
            if (!data || !data.result) {
                content.innerHTML = '<div class="recover-error">No recovery data available.</div>';
                return;
            }

            // Build the formatted content
            content.innerHTML = this.buildRecoveryContent(data);

            // Always show footer (cancel button available), but only show confirm if jobs to retry
            footer.style.display = 'flex';
            const confirmBtn = document.getElementById('btn-confirm-recover');
            if (confirmBtn) {
                confirmBtn.style.display = (data.result.jobs_to_retry && data.result.jobs_to_retry.length > 0) ? 'inline-block' : 'none';
            }

        } catch (error) {
            content.innerHTML = `<div class="recover-error">Error: ${this.escapeHtml(error.message)}</div>`;
        }
    },

    buildRecoveryContent(data) {
        const r = data.result;
        const diagnosis = data.diagnosis;
        let html = '';

        // Summary section
        html += '<div class="recover-section">';
        html += '<h4>Summary</h4>';
        html += '<div class="recover-summary">';
        html += `<div class="recover-stat">
            <div class="recover-stat-value ${r.jobs_to_retry?.length > 0 ? 'warning' : ''}">${r.jobs_to_retry?.length || 0}</div>
            <div class="recover-stat-label">Jobs to Retry</div>
        </div>`;
        html += `<div class="recover-stat">
            <div class="recover-stat-value ${r.oom_fixed > 0 ? 'danger' : ''}">${r.oom_fixed || 0}</div>
            <div class="recover-stat-label">OOM Fixes</div>
        </div>`;
        html += `<div class="recover-stat">
            <div class="recover-stat-value ${r.timeout_fixed > 0 ? 'warning' : ''}">${r.timeout_fixed || 0}</div>
            <div class="recover-stat-label">Timeout Fixes</div>
        </div>`;
        html += `<div class="recover-stat">
            <div class="recover-stat-value">${r.other_failures || 0}</div>
            <div class="recover-stat-label">Unknown Failures</div>
        </div>`;
        html += '</div></div>';

        // Failed Jobs section
        if (diagnosis?.failed_jobs?.length > 0) {
            html += '<div class="recover-section">';
            html += '<h4>Failed Jobs</h4>';
            html += '<table class="recover-table">';
            html += '<thead><tr><th>Job</th><th>Return Code</th><th>Reason</th><th>Details</th></tr></thead>';
            html += '<tbody>';
            for (const job of diagnosis.failed_jobs) {
                let reasonText = '';
                let reasonStyle = '';
                let details = '';
                if (job.likely_oom) {
                    reasonText = 'OOM';
                    reasonStyle = 'color: var(--danger-color)';
                    details = job.oom_reason || '';
                    if (job.peak_memory_formatted && job.peak_memory_formatted !== '0.0 MB') {
                        details += ` (peak: ${job.peak_memory_formatted})`;
                    }
                } else if (job.likely_timeout) {
                    reasonText = 'Timeout';
                    reasonStyle = 'color: var(--warning-color)';
                    details = job.timeout_reason || '';
                } else {
                    reasonText = 'Unknown';
                }
                const reasonStyleAttr = reasonStyle ? ` style="${reasonStyle}"` : '';
                html += `<tr>
                    <td><code>${this.escapeHtml(job.job_name)}</code></td>
                    <td>${job.return_code}</td>
                    <td${reasonStyleAttr}>${this.escapeHtml(reasonText)}</td>
                    <td>${this.escapeHtml(details)}</td>
                </tr>`;
            }
            html += '</tbody></table></div>';
        }

        // Resource Adjustments section
        if (r.adjustments?.length > 0) {
            html += '<div class="recover-section">';
            html += '<h4>Resource Adjustments</h4>';
            html += '<table class="recover-table">';
            html += '<thead><tr><th>Jobs</th><th>Memory</th><th>Runtime</th></tr></thead>';
            html += '<tbody>';
            for (const adj of r.adjustments) {
                const jobNames = adj.job_names.slice(0, 3).map(n => this.escapeHtml(n)).join(', ');
                const moreJobsHtml = adj.job_names.length > 3 ? ` <em>(+${adj.job_names.length - 3} more)</em>` : '';

                let memoryCell = '-';
                if (adj.memory_adjusted) {
                    memoryCell = `<div class="recover-adjustment">
                        <code>${adj.original_memory}</code>
                        <span class="arrow">→</span>
                        <code>${adj.new_memory}</code>
                    </div>`;
                }

                let runtimeCell = '-';
                if (adj.runtime_adjusted) {
                    runtimeCell = `<div class="recover-adjustment">
                        <code>${adj.original_runtime}</code>
                        <span class="arrow">→</span>
                        <code>${adj.new_runtime}</code>
                    </div>`;
                }

                html += `<tr>
                    <td>${jobNames}${moreJobsHtml}</td>
                    <td>${memoryCell}</td>
                    <td>${runtimeCell}</td>
                </tr>`;
            }
            html += '</tbody></table></div>';
        }

        // Slurm Schedulers section
        if (r.slurm_dry_run?.planned_schedulers?.length > 0) {
            html += '<div class="recover-section">';
            html += '<h4>Slurm Allocations to Create</h4>';
            for (const sched of r.slurm_dry_run.planned_schedulers) {
                html += `<div style="margin-bottom: 10px;"><strong>${this.escapeHtml(sched.name)}</strong>`;
                html += ` <span style="color: var(--text-secondary)">(${sched.job_count} job(s), ${sched.num_allocations} allocation(s))</span></div>`;
                html += '<div class="recover-slurm-info">';
                html += `<div><span>Account:</span> <strong>${this.escapeHtml(sched.account || '-')}</strong></div>`;
                html += `<div><span>Partition:</span> <strong>${this.escapeHtml(sched.partition || 'default')}</strong></div>`;
                html += `<div><span>Walltime:</span> <strong>${this.escapeHtml(sched.walltime || '-')}</strong></div>`;
                html += `<div><span>Memory:</span> <strong>${this.escapeHtml(sched.mem || '-')}</strong></div>`;
                html += `<div><span>Nodes:</span> <strong>${sched.nodes || 1}</strong></div>`;
                html += '</div>';
            }
            html += `<div style="margin-top: 12px; color: var(--text-secondary);">Total: ${r.slurm_dry_run.total_allocations} allocation(s) would be submitted</div>`;
            html += '</div>';
        }

        // No recoverable jobs message
        if (!r.jobs_to_retry || r.jobs_to_retry.length === 0) {
            html += '<div class="recover-section">';
            html += '<div class="recover-no-data">';
            if (r.other_failures > 0) {
                html += `${r.other_failures} job(s) failed with unknown causes. Use the CLI with --retry-unknown to retry these jobs.`;
            } else {
                html += 'No recoverable jobs found. The workflow may have completed successfully or all failures require manual intervention.';
            }
            html += '</div></div>';
        }

        return html;
    },

    async executeRecovery(workflowId) {
        const content = document.getElementById('recover-content');
        const footer = document.getElementById('recover-modal-footer');

        // Show processing state
        if (content) {
            content.innerHTML = '<div class="placeholder-message">Applying recovery...</div>';
        }
        if (footer) {
            footer.style.display = 'none';
        }

        try {
            const response = await fetch('/api/cli/recover', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    workflow_id: workflowId.toString(),
                    dry_run: false
                })
            });

            const result = await response.json();

            if (!result.success) {
                content.innerHTML = `<div class="recover-error">Recovery failed: ${this.escapeHtml(result.error || 'Unknown error')}</div>`;
                this.showToast('Recovery failed', 'error');
                return;
            }

            const r = result.data.result;

            // Show success
            let successHtml = '<div class="recover-section" style="border-color: var(--success-color);">';
            successHtml += '<h4 style="color: var(--success-color);">✓ Recovery Complete</h4>';
            successHtml += '<ul style="margin: 0; padding-left: 20px;">';
            if (r.oom_fixed > 0) {
                successHtml += `<li>${r.oom_fixed} job(s) had memory increased</li>`;
            }
            if (r.timeout_fixed > 0) {
                successHtml += `<li>${r.timeout_fixed} job(s) had runtime increased</li>`;
            }
            if (r.unknown_retried > 0) {
                successHtml += `<li>${r.unknown_retried} job(s) with unknown failures reset</li>`;
            }
            if (r.jobs_to_retry?.length > 0) {
                successHtml += `<li>Reset ${r.jobs_to_retry.length} job(s)</li>`;
                successHtml += '<li>Slurm schedulers regenerated and submitted</li>';
            }
            successHtml += '</ul></div>';

            content.innerHTML = successHtml;
            this.showToast('Recovery complete', 'success');

            // Refresh workflow data
            this.loadWorkflows();
            this.loadWorkflowDetails(workflowId);

            // Auto-close after a delay
            setTimeout(() => {
                this.hideModal('recover-modal');
            }, 3000);

        } catch (error) {
            content.innerHTML = `<div class="recover-error">Error: ${this.escapeHtml(error.message)}</div>`;
            this.showToast('Recovery failed', 'error');
        }
    },

    showExecutionPanel() {
        const panel = document.getElementById('execution-output-panel');
        const output = document.getElementById('execution-output');
        if (panel) {
            panel.style.display = 'block';
            output.textContent = '';
        }
        // Show cancel button
        const cancelBtn = document.getElementById('btn-cancel-execution');
        if (cancelBtn) cancelBtn.style.display = 'inline-block';
    },

    hideExecutionPanel() {
        const panel = document.getElementById('execution-output-panel');
        if (panel) {
            panel.style.display = 'none';
        }
        // Close any active event source
        if (this.currentEventSource) {
            this.currentEventSource.close();
            this.currentEventSource = null;
        }
    },

    hideExecutionCancelButton() {
        const cancelBtn = document.getElementById('btn-cancel-execution');
        if (cancelBtn) cancelBtn.style.display = 'none';
    },

    appendExecutionOutput(text, type = 'stdout') {
        const output = document.getElementById('execution-output');
        if (!output) return;

        const span = document.createElement('span');
        span.textContent = text;
        span.className = `output-${type}`;
        output.appendChild(span);

        // Auto-scroll to bottom
        output.scrollTop = output.scrollHeight;
    },

    setupExecutionPanel() {
        document.getElementById('btn-close-output')?.addEventListener('click', () => {
            this.hideExecutionPanel();
        });

        document.getElementById('btn-cancel-execution')?.addEventListener('click', () => {
            if (this.currentEventSource) {
                this.currentEventSource.close();
                this.currentEventSource = null;
                this.appendExecutionOutput(`\n⚠ Execution cancelled by user\n`, 'warning');
                this.hideExecutionCancelButton();
            }
        });
    },

    async submitWorkflow(workflowId) {
        if (!confirm('Submit this workflow to the scheduler (e.g., Slurm)?')) {
            return;
        }

        this.showToast('Submitting workflow...', 'info');
        try {
            const result = await api.cliSubmitWorkflow(workflowId);
            if (result.success) {
                this.showToast('Workflow submitted successfully', 'success');
            } else {
                this.showToast('Error: ' + (result.stderr || result.stdout), 'error');
            }
        } catch (error) {
            this.showToast('Error submitting workflow: ' + error.message, 'error');
        }
    },

    async initializeWorkflow(workflowId, force = false) {
        try {
            // If not forcing, first check if there are existing output files
            if (!force) {
                const checkResult = await api.cliCheckInitialize(workflowId);

                // Parse the JSON response from stdout
                if (checkResult.success && checkResult.stdout) {
                    try {
                        const dryRunData = JSON.parse(checkResult.stdout);
                        const fileCount = dryRunData.existing_output_file_count || 0;

                        if (fileCount > 0) {
                            // Show confirmation modal
                            this.showInitializeConfirmModal(workflowId, fileCount, dryRunData.existing_output_files || []);
                            return;
                        }
                    } catch (parseError) {
                        // JSON parse failed, continue with initialization
                        console.warn('Could not parse dry-run response:', parseError);
                    }
                }
            }

            // Proceed with actual initialization
            const result = await api.cliInitializeWorkflow(workflowId, force);
            if (result.success) {
                this.showToast('Workflow initialized', 'success');
                await this.loadWorkflows();
                await this.loadWorkflowDetails(workflowId);
            } else {
                this.showToast('Error: ' + (result.stderr || result.stdout), 'error');
            }
        } catch (error) {
            this.showToast('Error initializing workflow: ' + error.message, 'error');
        }
    },

    showInitializeConfirmModal(workflowId, fileCount, files) {
        // Store for use by confirm button
        this.pendingInitializeWorkflowId = workflowId;

        // Update modal content
        const content = document.getElementById('init-confirm-content');
        if (content) {
            const fileList = files.slice(0, 10).map(f => `<li><code>${this.escapeHtml(f)}</code></li>`).join('');
            const moreFiles = files.length > 10 ? `<li>... and ${files.length - 10} more</li>` : '';

            content.innerHTML = `
                <p>This workflow has <strong>${fileCount}</strong> existing output file(s) that will be deleted:</p>
                <ul class="file-list">${fileList}${moreFiles}</ul>
                <p>Do you want to proceed and delete these files?</p>
            `;
        }

        this.showModal('init-confirm-modal');
    },

    // ==================== Multi-Select and Bulk Operations ====================

    toggleWorkflowSelection(workflowId, isSelected) {
        if (isSelected) {
            this.selectedWorkflowIds.add(workflowId);
        } else {
            this.selectedWorkflowIds.delete(workflowId);
        }

        // Update row styling
        const row = document.querySelector(`tr[data-workflow-id="${workflowId}"]`);
        if (row) {
            row.classList.toggle('selected', isSelected);
        }

        this.updateSelectAllCheckbox();
        this.updateBulkActionBar();
    },

    toggleSelectAllWorkflows(selectAll) {
        const checkboxes = document.querySelectorAll('.workflow-checkbox');

        if (selectAll) {
            // Select all currently visible workflows
            checkboxes.forEach(cb => {
                const workflowId = cb.dataset.workflowId;
                this.selectedWorkflowIds.add(workflowId);
                cb.checked = true;
                cb.closest('tr')?.classList.add('selected');
            });
        } else {
            // Deselect all
            this.selectedWorkflowIds.clear();
            checkboxes.forEach(cb => {
                cb.checked = false;
                cb.closest('tr')?.classList.remove('selected');
            });
        }

        this.updateBulkActionBar();
    },

    clearWorkflowSelection() {
        this.selectedWorkflowIds.clear();
        document.querySelectorAll('.workflow-checkbox').forEach(cb => {
            cb.checked = false;
            cb.closest('tr')?.classList.remove('selected');
        });
        const selectAll = document.getElementById('workflows-select-all');
        if (selectAll) selectAll.checked = false;
        this.updateBulkActionBar();
    },

    updateSelectAllCheckbox() {
        const selectAll = document.getElementById('workflows-select-all');
        const checkboxes = document.querySelectorAll('.workflow-checkbox');
        if (!selectAll || checkboxes.length === 0) return;

        const allChecked = Array.from(checkboxes).every(cb => cb.checked);
        const someChecked = Array.from(checkboxes).some(cb => cb.checked);

        selectAll.checked = allChecked;
        selectAll.indeterminate = someChecked && !allChecked;
    },

    updateBulkActionBar() {
        const bar = document.getElementById('workflows-bulk-actions');
        const countSpan = document.getElementById('workflows-selection-count');
        const count = this.selectedWorkflowIds.size;

        if (bar) {
            bar.style.display = count > 0 ? 'flex' : 'none';
        }
        if (countSpan) {
            countSpan.textContent = count;
        }
    },

    async bulkDeleteWorkflows() {
        const count = this.selectedWorkflowIds.size;
        if (count === 0) return;

        const plural = count === 1 ? 'workflow' : 'workflows';
        if (!confirm(`Delete ${count} ${plural}? This action cannot be undone.`)) {
            return;
        }

        const idsToDelete = Array.from(this.selectedWorkflowIds);
        let successCount = 0;
        let errorCount = 0;

        this.showToast(`Deleting ${count} ${plural}...`, 'info');

        // Delete in parallel with a reasonable concurrency limit
        const results = await Promise.allSettled(
            idsToDelete.map(id => api.cliDeleteWorkflow(id))
        );

        results.forEach((result, index) => {
            if (result.status === 'fulfilled' && result.value.success) {
                successCount++;
                this.selectedWorkflowIds.delete(idsToDelete[index]);
            } else {
                errorCount++;
                console.error(`Failed to delete workflow ${idsToDelete[index]}:`, result);
            }
        });

        if (successCount > 0) {
            this.showToast(`Deleted ${successCount} ${successCount === 1 ? 'workflow' : 'workflows'}`, 'success');
        }
        if (errorCount > 0) {
            this.showToast(`Failed to delete ${errorCount} ${errorCount === 1 ? 'workflow' : 'workflows'}`, 'error');
        }

        await this.loadWorkflows();
    },
});
