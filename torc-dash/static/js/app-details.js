/**
 * Torc Dashboard - Details Tab
 * Workflow details, job listing, and sub-tab navigation
 */

Object.assign(TorcDashboard.prototype, {
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

        document.getElementById('btn-refresh-details')?.addEventListener('click', async () => {
            if (this.selectedWorkflowId) {
                await this.loadWorkflowDetails(this.selectedWorkflowId);
            }
        });

        // Sub-tab navigation
        document.querySelectorAll('.sub-tab[data-subtab]').forEach(tab => {
            tab.addEventListener('click', () => {
                this.switchSubTab(tab.dataset.subtab);
            });
        });

        // Workflow action buttons
        document.getElementById('btn-init-workflow')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.initializeWorkflow(this.selectedWorkflowId);
        });

        document.getElementById('btn-reinit-workflow')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.reinitializeWorkflow(this.selectedWorkflowId);
        });

        document.getElementById('btn-reset-workflow')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.resetWorkflow(this.selectedWorkflowId);
        });

        document.getElementById('btn-run-workflow-detail')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.runWorkflow(this.selectedWorkflowId);
        });

        document.getElementById('btn-submit-workflow-detail')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.submitWorkflow(this.selectedWorkflowId);
        });

        document.getElementById('btn-show-dag')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.viewDAG(this.selectedWorkflowId);
        });

        document.getElementById('btn-show-plan')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.showExecutionPlan(this.selectedWorkflowId);
        });

        document.getElementById('btn-recover-workflow')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.recoverWorkflow(this.selectedWorkflowId);
        });

        document.getElementById('btn-cancel-workflow')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) this.cancelWorkflow(this.selectedWorkflowId);
        });

        document.getElementById('btn-export-workflow')?.addEventListener('click', () => {
            if (this.selectedWorkflowId) {
                document.getElementById('export-status').innerHTML = '';
                // Pre-populate output path with workflow_name_id.json
                const pathInput = document.getElementById('export-output-path');
                if (pathInput) {
                    const workflow = this.workflows.find(w => String(w.id) === String(this.selectedWorkflowId));
                    const safeName = (workflow?.name || 'workflow').replace(/\s+/g, '_').replace(/[^a-zA-Z0-9_-]/g, '_');
                    pathInput.value = `${safeName}_${this.selectedWorkflowId}.json`;
                }
                this.showModal('export-workflow-modal');
            }
        });
    },

    async cancelWorkflow(workflowId) {
        if (!confirm('Cancel this workflow? This will terminate all running Slurm jobs.')) {
            return;
        }

        try {
            const response = await fetch('/api/cli/cancel', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ workflow_id: workflowId.toString() })
            });

            const result = await response.json();

            if (result.success) {
                this.showToast('Workflow canceled', 'success');
                // Refresh workflow data
                await this.loadWorkflows();
                await this.loadWorkflowDetails(workflowId);
            } else {
                this.showToast('Error: ' + (result.stderr || result.stdout || 'Cancel failed'), 'error');
            }
        } catch (error) {
            this.showToast('Error canceling workflow: ' + error.message, 'error');
        }
    },

    async reinitializeWorkflow(workflowId) {
        try {
            // Existing output files just generate warnings - no confirmation needed
            const result = await api.cliReinitializeWorkflow(workflowId, false);
            if (result.success) {
                this.showToast('Workflow reinitialized', 'success');
                await this.loadWorkflows();
                await this.loadWorkflowDetails(workflowId);
            } else {
                this.showToast('Error: ' + (result.stderr || result.stdout), 'error');
            }
        } catch (error) {
            this.showToast('Error reinitializing workflow: ' + error.message, 'error');
        }
    },

    showReinitializeConfirmModal(workflowId, fileCount, files) {
        this.pendingReinitializeWorkflowId = workflowId;
        const content = document.getElementById('reinit-confirm-content');
        if (content) {
            const fileList = files.slice(0, 10).map(f => `<li><code>${this.escapeHtml(f)}</code></li>`).join('');
            const moreFiles = files.length > 10 ? `<li>... and ${files.length - 10} more</li>` : '';
            content.innerHTML = `
                <p>This workflow has <strong>${fileCount}</strong> existing output file(s) that will be deleted:</p>
                <ul class="file-list">${fileList}${moreFiles}</ul>
                <p>Do you want to proceed and delete these files?</p>
            `;
        }
        this.showModal('reinit-confirm-modal');
    },

    async resetWorkflow(workflowId) {
        if (!confirm('Reset workflow status? This will set all jobs back to uninitialized state.')) {
            return;
        }
        try {
            const result = await api.cliResetStatus(workflowId);
            if (result.success) {
                this.showToast('Workflow status reset', 'success');
                await this.loadWorkflows();
                await this.loadWorkflowDetails(workflowId);
            } else {
                this.showToast('Error: ' + (result.stderr || result.stdout), 'error');
            }
        } catch (error) {
            this.showToast('Error resetting: ' + error.message, 'error');
        }
    },

    async loadWorkflowDetails(workflowId) {
        try {
            const workflow = await api.getWorkflow(workflowId);
            const container = document.getElementById('details-container');
            container.innerHTML = `
                <div class="workflow-summary">
                    <div class="summary-card">
                        <div class="value">${workflow.id ?? '-'}</div>
                        <div class="label">ID</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">${this.escapeHtml(workflow.name || 'Unnamed')}</div>
                        <div class="label">Name</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">${this.escapeHtml(workflow.user || '-')}</div>
                        <div class="label">User</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">${this.formatTimestamp(workflow.timestamp)}</div>
                        <div class="label">Timestamp</div>
                    </div>
                </div>
            `;
            document.getElementById('workflow-actions-panel').style.display = 'flex';
            document.getElementById('details-sub-tabs').style.display = 'flex';
            await this.loadSubTabContent(workflowId, this.selectedSubTab);
        } catch (error) {
            console.error('Error loading workflow details:', error);
            this.showToast('Error loading workflow details: ' + error.message, 'error');
        }
    },

    clearWorkflowDetails() {
        document.getElementById('details-container').innerHTML = `
            <div class="placeholder-message">Select a workflow to view details</div>
        `;
        document.getElementById('workflow-actions-panel').style.display = 'none';
        document.getElementById('details-sub-tabs').style.display = 'none';
        document.getElementById('details-content').innerHTML = '';
    },

    switchSubTab(subtab) {
        this.selectedSubTab = subtab;
        document.querySelectorAll('.sub-tab[data-subtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.subtab === subtab);
        });
        if (this.selectedWorkflowId) {
            this.loadSubTabContent(this.selectedWorkflowId, subtab);
        }
    },

    async loadSubTabContent(workflowId, subtab) {
        const content = document.getElementById('details-content');
        this.tableState = {
            data: [],
            filteredData: [],
            sortColumn: null,
            sortDirection: 'asc',
            filterText: '',
            tabType: subtab,
            jobNameMap: {}
        };

        try {
            switch (subtab) {
                case 'jobs':
                    this.tableState.data = await api.listJobs(workflowId);
                    break;
                case 'results':
                    const [results, resultJobs] = await Promise.all([
                        api.listResults(workflowId),
                        api.listJobs(workflowId),
                    ]);
                    this.tableState.data = results;
                    if (resultJobs) {
                        resultJobs.forEach(job => {
                            this.tableState.jobNameMap[job.id] = job.name;
                        });
                    }
                    break;
                case 'events':
                    const events = await api.listWorkflowEvents(workflowId);
                    this.tableState.data = api.extractItems(events);
                    break;
                case 'files':
                    this.tableState.data = await api.listFiles(workflowId);
                    break;
                case 'user-data':
                    this.tableState.data = await api.listUserData(workflowId);
                    break;
                case 'resources':
                    this.tableState.data = await api.listResourceRequirements(workflowId);
                    break;
                case 'schedulers':
                    this.tableState.data = await api.listSlurmSchedulers(workflowId);
                    break;
                case 'compute-nodes':
                    this.tableState.data = await api.listComputeNodes(workflowId);
                    break;
                case 'scheduled-nodes':
                    this.tableState.data = await api.listScheduledComputeNodes(workflowId);
                    break;
            }
            this.tableState.filteredData = [...this.tableState.data];
            this.renderCurrentTable();
        } catch (error) {
            content.innerHTML = `<div class="placeholder-message">Error loading ${subtab}: ${error.message}</div>`;
        }
    },

    renderCurrentTable() {
        const content = document.getElementById('details-content');
        const { filteredData, tabType, jobNameMap } = this.tableState;

        switch (tabType) {
            case 'jobs':
                content.innerHTML = this.renderJobsTable(filteredData);
                break;
            case 'results':
                content.innerHTML = this.renderResultsTable(filteredData, null, jobNameMap);
                break;
            case 'events':
                this.setEventsForPreview(filteredData);
                content.innerHTML = this.renderWorkflowEventsTable(filteredData);
                this.setupEventRowClickHandlers();
                break;
            case 'files':
                content.innerHTML = this.renderFilesTable(filteredData);
                break;
            case 'user-data':
                content.innerHTML = this.renderUserDataTable(filteredData);
                break;
            case 'resources':
                content.innerHTML = this.renderResourcesTable(filteredData);
                break;
            case 'schedulers':
                content.innerHTML = this.renderSchedulersTable(filteredData);
                break;
            case 'compute-nodes':
                content.innerHTML = this.renderComputeNodesTable(filteredData);
                break;
            case 'scheduled-nodes':
                content.innerHTML = this.renderScheduledNodesTable(filteredData);
                break;
        }
        this.setupTableInteractions();
    },

    setupTableInteractions() {
        document.querySelectorAll('#details-content th[data-sort]').forEach(th => {
            th.addEventListener('click', () => this.handleSort(th.dataset.sort));
        });
        const filterInput = document.getElementById('table-filter-input');
        if (filterInput) {
            filterInput.value = this.tableState.filterText;
            filterInput.addEventListener('input', (e) => this.handleFilter(e.target.value));
        }
        document.querySelectorAll('.quick-filter-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const filterInput = document.getElementById('table-filter-input');
                if (filterInput) {
                    filterInput.value = btn.dataset.filter;
                    this.handleFilter(btn.dataset.filter);
                }
            });
        });
    },

    handleSort(column) {
        const { sortColumn, sortDirection } = this.tableState;
        if (sortColumn === column) {
            this.tableState.sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
        } else {
            this.tableState.sortColumn = column;
            this.tableState.sortDirection = 'asc';
        }
        this.applySortAndFilter();
        this.renderCurrentTable();
    },

    handleFilter(filterText) {
        this.tableState.filterText = filterText;
        this.applySortAndFilter();
        this.renderCurrentTableBody();
    },

    renderCurrentTableBody() {
        const { filteredData, tabType, jobNameMap } = this.tableState;
        const countEl = document.querySelector('#details-content .table-count');
        if (countEl) {
            const itemName = this.getItemNameForTab(tabType);
            countEl.textContent = `${filteredData.length} ${itemName}${filteredData.length !== 1 ? 's' : ''}`;
        }
        const tbody = document.querySelector('#details-content .data-table tbody');
        if (tbody) {
            tbody.innerHTML = this.renderTableBodyRows(filteredData, tabType, jobNameMap);
        }
    },

    getItemNameForTab(tabType) {
        const names = {
            'jobs': 'job',
            'results': 'result',
            'events': 'event',
            'files': 'file',
            'user-data': 'record',
            'resources': 'requirement',
            'schedulers': 'scheduler',
            'compute-nodes': 'node',
        };
        return names[tabType] || 'item';
    },

    renderTableBodyRows(items, tabType, jobNameMap) {
        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];

        switch (tabType) {
            case 'jobs':
                return items.map(job => `
                    <tr>
                        <td><code>${job.id ?? '-'}</code></td>
                        <td>${this.escapeHtml(job.name || '-')}</td>
                        <td><span class="status-badge status-${statusNames[job.status]?.toLowerCase() || 'unknown'}">${statusNames[job.status] || job.status}</span></td>
                        <td><code>${this.escapeHtml(this.truncate(job.command || '-', 80))}</code></td>
                        <td><button class="btn-job-details" data-job-id="${job.id}" data-job-name="${this.escapeHtml(job.name || '')}">Details</button></td>
                    </tr>
                `).join('');

            case 'results':
                return items.map(result => `
                    <tr>
                        <td><code>${result.job_id ?? '-'}</code></td>
                        <td>${this.escapeHtml(jobNameMap[result.job_id] || '-')}</td>
                        <td>${result.run_id ?? '-'}</td>
                        <td>${result.attempt_id ?? 1}</td>
                        <td class="${result.return_code === 0 ? 'return-code-0' : 'return-code-error'}">${result.return_code ?? '-'}</td>
                        <td><span class="status-badge status-${statusNames[result.status]?.toLowerCase() || 'unknown'}">${statusNames[result.status] || result.status}</span></td>
                        <td>${result.exec_time_minutes != null ? result.exec_time_minutes.toFixed(2) : '-'}</td>
                        <td>${this.formatBytes(result.peak_memory_bytes)}</td>
                        <td>${result.peak_cpu_percent != null ? result.peak_cpu_percent.toFixed(1) : '-'}</td>
                    </tr>
                `).join('');

            case 'events':
                return items.map(event => `
                    <tr>
                        <td><code>${event.id ?? '-'}</code></td>
                        <td>${this.formatTimestamp(event.timestamp)}</td>
                        <td><code>${this.escapeHtml(this.truncate(JSON.stringify(event.data) || '-', 100))}</code></td>
                    </tr>
                `).join('');

            case 'files':
                return items.map(file => `
                    <tr>
                        <td><code>${file.id ?? '-'}</code></td>
                        <td>${this.escapeHtml(file.name || '-')}</td>
                        <td><code>${this.escapeHtml(file.path || '-')}</code></td>
                        <td>${this.formatUnixTimestamp(file.st_mtime)}</td>
                        <td>${file.path ? `<button class="btn-view-file" data-path="${this.escapeHtml(file.path)}" data-name="${this.escapeHtml(file.name || 'File')}">View</button>` : '-'}</td>
                    </tr>
                `).join('');

            case 'user-data':
                return items.map(ud => `
                    <tr>
                        <td><code>${ud.id ?? '-'}</code></td>
                        <td>${this.escapeHtml(ud.name || '-')}</td>
                        <td><code>${this.escapeHtml(this.truncate(JSON.stringify(ud.data) || '-', 100))}</code></td>
                    </tr>
                `).join('');

            case 'resources':
                return items.map(r => `
                    <tr>
                        <td><code>${r.id ?? '-'}</code></td>
                        <td>${this.escapeHtml(r.name || '-')}</td>
                        <td>${r.num_cpus ?? '-'}</td>
                        <td>${this.escapeHtml(r.memory || '-')}</td>
                        <td>${r.num_gpus ?? '-'}</td>
                        <td>${this.escapeHtml(r.runtime || '-')}</td>
                    </tr>
                `).join('');

            case 'schedulers':
                return items.map(s => `
                    <tr>
                        <td><code>${s.id ?? '-'}</code></td>
                        <td>${this.escapeHtml(s.name || '-')}</td>
                        <td>${this.escapeHtml(s.account || '-')}</td>
                        <td>${this.escapeHtml(s.partition || '-')}</td>
                        <td>${this.escapeHtml(s.walltime || '-')}</td>
                        <td>${s.nodes ?? '-'}</td>
                        <td>${this.escapeHtml(s.mem || '-')}</td>
                    </tr>
                `).join('');

            case 'compute-nodes':
                return items.map(n => `
                    <tr>
                        <td><code>${n.id ?? '-'}</code></td>
                        <td>${this.escapeHtml(n.hostname || '-')}</td>
                        <td>${n.num_cpus ?? '-'}</td>
                        <td>${n.memory_gb ?? '-'}</td>
                        <td>${n.num_gpus ?? '-'}</td>
                        <td>${n.is_active != null ? (n.is_active ? 'Yes' : 'No') : '-'}</td>
                    </tr>
                `).join('');

            default:
                return '';
        }
    },

    applySortAndFilter() {
        const { data, sortColumn, sortDirection, filterText, tabType, jobNameMap } = this.tableState;
        let filtered = [...data];

        if (filterText.trim()) {
            const lowerFilter = filterText.toLowerCase().trim();
            const operatorMatch = lowerFilter.match(/^(\w+)\s*(!=|>=|<=|>|<|=|~|:)\s*(.+)$/);

            if (operatorMatch) {
                const [, field, operator, value] = operatorMatch;
                filtered = this.applyOperatorFilter(filtered, field, operator, value, tabType, jobNameMap);
            } else {
                filtered = filtered.filter(item => {
                    return this.getSearchableText(item, tabType, jobNameMap).toLowerCase().includes(lowerFilter);
                });
            }
        }

        if (sortColumn) {
            filtered.sort((a, b) => {
                let aVal = this.getSortValue(a, sortColumn, tabType, jobNameMap);
                let bVal = this.getSortValue(b, sortColumn, tabType, jobNameMap);
                if (aVal == null && bVal == null) return 0;
                if (aVal == null) return 1;
                if (bVal == null) return -1;
                let result;
                if (typeof aVal === 'number' && typeof bVal === 'number') {
                    result = aVal - bVal;
                } else {
                    result = String(aVal).localeCompare(String(bVal));
                }
                return sortDirection === 'desc' ? -result : result;
            });
        }

        this.tableState.filteredData = filtered;
    },

    applyOperatorFilter(data, field, operator, value, tabType, jobNameMap) {
        const numValue = parseFloat(value);
        const isNumeric = !isNaN(numValue);

        return data.filter(item => {
            let itemValue = this.getFieldValue(item, field, tabType, jobNameMap);

            if (field === 'status') {
                const statusNames = ['uninitialized', 'blocked', 'ready', 'pending', 'running', 'completed', 'failed', 'canceled', 'terminated', 'disabled'];
                const filterStatusName = value.toLowerCase();
                let itemStatusName;
                if (typeof item.status === 'number') {
                    itemStatusName = statusNames[item.status] || '';
                } else {
                    itemStatusName = String(item.status).toLowerCase();
                }
                switch (operator) {
                    case '=': return itemStatusName === filterStatusName;
                    case '!=': return itemStatusName !== filterStatusName;
                    default: return true;
                }
            }

            if (isNumeric) {
                const itemNumValue = typeof itemValue === 'number' ? itemValue : parseFloat(itemValue);
                if (!isNaN(itemNumValue)) {
                    switch (operator) {
                        case '=': return itemNumValue === numValue;
                        case '!=': return itemNumValue !== numValue;
                        case '>': return itemNumValue > numValue;
                        case '<': return itemNumValue < numValue;
                        case '>=': return itemNumValue >= numValue;
                        case '<=': return itemNumValue <= numValue;
                    }
                }
            }

            const strValue = String(itemValue ?? '').toLowerCase();
            const compareValue = value.toLowerCase();
            switch (operator) {
                case '=': return strValue === compareValue;
                case '!=': return strValue !== compareValue;
                case '~':
                case ':': return strValue.includes(compareValue);
                default: return strValue.includes(compareValue);
            }
        });
    },

    getFieldValue(item, field, tabType, jobNameMap) {
        const fieldMap = {
            'job_name': () => jobNameMap[item.job_id] || '',
            'return_code': () => item.return_code,
            'exec_time': () => item.exec_time_minutes,
            'peak_mem': () => item.peak_memory_bytes,
            'peak_cpu': () => item.peak_cpu_percent,
            'modified': () => item.st_mtime,
        };
        if (fieldMap[field]) {
            return fieldMap[field]();
        }
        return item[field];
    },

    getSortValue(item, column, tabType, jobNameMap) {
        return this.getFieldValue(item, column, tabType, jobNameMap);
    },

    getSearchableText(item, tabType, jobNameMap) {
        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];
        const parts = [];
        if (item.id != null) parts.push(String(item.id));
        if (item.name) parts.push(item.name);
        if (item.status != null) parts.push(statusNames[item.status] || '');

        switch (tabType) {
            case 'jobs':
                if (item.command) parts.push(item.command);
                break;
            case 'results':
                if (item.job_id != null) parts.push(String(item.job_id));
                if (jobNameMap[item.job_id]) parts.push(jobNameMap[item.job_id]);
                if (item.return_code != null) parts.push(String(item.return_code));
                break;
            case 'files':
                if (item.path) parts.push(item.path);
                break;
            case 'events':
                if (item.timestamp) parts.push(item.timestamp);
                if (item.data) parts.push(JSON.stringify(item.data));
                break;
        }
        return parts.join(' ');
    },

    renderTableControls(tabType) {
        const quickFilters = this.getQuickFilters(tabType);
        const quickFilterHtml = quickFilters.map(f =>
            `<button class="quick-filter-btn btn btn-sm btn-secondary" data-filter="${this.escapeHtml(f.filter)}" title="${this.escapeHtml(f.title)}">${this.escapeHtml(f.label)}</button>`
        ).join('');

        return `
            <div class="table-controls">
                <div class="filter-group">
                    <input type="text" id="table-filter-input" class="text-input" placeholder="Filter... (e.g., name:work, status=ready, id>5)" style="width: 300px;">
                    <button class="btn btn-sm btn-secondary" onclick="app.clearTableFilter()">Clear</button>
                </div>
                ${quickFilterHtml ? `<div class="quick-filters">${quickFilterHtml}</div>` : ''}
            </div>
        `;
    },

    getQuickFilters(tabType) {
        switch (tabType) {
            case 'jobs':
                return [
                    { label: 'Failed', filter: 'status=failed', title: 'Show only failed jobs' },
                    { label: 'Running', filter: 'status=running', title: 'Show only running jobs' },
                    { label: 'Ready', filter: 'status=ready', title: 'Show only ready jobs' },
                    { label: 'Blocked', filter: 'status=blocked', title: 'Show only blocked jobs' },
                ];
            case 'results':
                return [
                    { label: 'Errors', filter: 'return_code!=0', title: 'Show results with non-zero return code' },
                    { label: 'Success', filter: 'return_code=0', title: 'Show results with return code 0' },
                    { label: 'Failed', filter: 'status=failed', title: 'Show failed results' },
                ];
            case 'events':
                return [
                    { label: 'Errors', filter: 'error', title: 'Show error events' },
                ];
            default:
                return [];
        }
    },

    clearTableFilter() {
        const filterInput = document.getElementById('table-filter-input');
        if (filterInput) {
            filterInput.value = '';
        }
        this.handleFilter('');
    },

    renderSortableHeader(label, column) {
        const { sortColumn, sortDirection } = this.tableState;
        const isActive = sortColumn === column;
        const arrow = isActive ? (sortDirection === 'asc' ? ' ▲' : ' ▼') : '';
        return `<th data-sort="${column}" class="sortable${isActive ? ' sorted' : ''}">${label}${arrow}</th>`;
    },

    // ==================== Slurm Logs Viewer ====================

    setupSlurmLogsModal() {
        // Modal close handlers
        document.getElementById('slurm-logs-modal-close')?.addEventListener('click', () => {
            this.hideModal('slurm-logs-modal');
        });

        document.getElementById('btn-close-slurm-logs')?.addEventListener('click', () => {
            this.hideModal('slurm-logs-modal');
        });

        // Tab navigation for Slurm logs
        document.querySelectorAll('.sub-tab[data-slurm-logtab]').forEach(tab => {
            tab.addEventListener('click', () => {
                this.switchSlurmLogTab(tab.dataset.slurmLogtab);
            });
        });

        // Event delegation for Slurm logs buttons
        document.addEventListener('click', (e) => {
            if (e.target.classList.contains('btn-slurm-logs')) {
                const schedulerId = e.target.dataset.schedulerId;
                if (schedulerId) {
                    this.showSlurmLogs(schedulerId);
                }
            }
        });
    },

    showSlurmLogs(schedulerId) {
        this.currentSlurmJobId = schedulerId;
        this.currentSlurmLogTab = 'stdout';

        // Get output directory from the debugging tab or use default
        const outputDir = document.getElementById('debug-output-dir')?.value || 'output';
        this.slurmLogsOutputDir = outputDir;

        // Update modal title and info
        document.getElementById('slurm-logs-title').textContent = `Slurm Job ${schedulerId} Logs`;
        document.getElementById('slurm-logs-info').innerHTML = `
            <div class="slurm-logs-summary">
                <strong>Slurm Job ID:</strong> ${this.escapeHtml(schedulerId)}
                <span style="margin-left: 20px;"><strong>Output Directory:</strong> <code>${this.escapeHtml(outputDir)}</code></span>
            </div>
        `;

        // Reset tab state
        document.querySelectorAll('.sub-tab[data-slurm-logtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.slurmLogtab === 'stdout');
        });

        // Load initial log content
        this.loadSlurmLogContent();

        // Show modal
        this.showModal('slurm-logs-modal');
    },

    switchSlurmLogTab(logtab) {
        this.currentSlurmLogTab = logtab;

        document.querySelectorAll('.sub-tab[data-slurm-logtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.slurmLogtab === logtab);
        });

        this.loadSlurmLogContent();
    },

    async loadSlurmLogContent() {
        const logPath = document.getElementById('slurm-log-path');
        const logContent = document.getElementById('slurm-log-content');

        if (!this.currentSlurmJobId) {
            logContent.textContent = 'No Slurm job selected';
            logPath.textContent = '';
            return;
        }

        // Construct the log file path based on the naming convention
        // stdout: {output_dir}/slurm_output_wf{workflow_id}_sl{slurm_job_id}.o
        // stderr: {output_dir}/slurm_output_wf{workflow_id}_sl{slurm_job_id}.e
        const outputDir = this.slurmLogsOutputDir || 'output';
        const extension = this.currentSlurmLogTab === 'stdout' ? 'o' : 'e';
        const workflowId = this.selectedWorkflowId || 0;
        const filePath = `${outputDir}/slurm_output_wf${workflowId}_sl${this.currentSlurmJobId}.${extension}`;

        logPath.textContent = filePath;
        logContent.classList.toggle('stderr', this.currentSlurmLogTab !== 'stdout');
        logContent.textContent = 'Loading...';

        try {
            const response = await fetch('/api/cli/read-file', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: filePath }),
            });

            const data = await response.json();

            if (!data.exists) {
                logContent.textContent = '(file does not exist)';
            } else if (!data.success) {
                logContent.textContent = `Error: ${data.error || 'Unknown error'}`;
            } else if (!data.content || data.content.trim() === '') {
                logContent.textContent = '(empty)';
            } else {
                logContent.textContent = data.content;
            }
        } catch (error) {
            logContent.textContent = `Error loading file: ${error.message}`;
        }
    },
});
