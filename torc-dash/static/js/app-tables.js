/**
 * Torc Dashboard - Table Rendering
 * All table rendering functions for different data types
 */

Object.assign(TorcDashboard.prototype, {
    /**
     * Generate a human-readable message from event data.
     * Tries various fields that might contain meaningful info.
     */
    getEventMessage(event) {
        const data = event.data;
        if (!data) return '-';

        // If there's an explicit message field, use it
        if (data.message) return data.message;

        // For job events, show job name
        if (data.job_name) {
            return `Job: ${data.job_name}`;
        }

        // For action events
        if (data.action) {
            const parts = [data.action];
            if (data.user) parts.push(`by ${data.user}`);
            return parts.join(' ');
        }

        // For category-based events
        if (data.category && data.type) {
            return `${data.category}: ${data.type}`;
        }

        // Fallback: show truncated JSON
        const jsonStr = JSON.stringify(data);
        return this.truncate(jsonStr, 60);
    },

    renderJobsTable(jobs) {
        const controls = this.renderTableControls('jobs');
        const count = `<span class="table-count">${jobs.length} job${jobs.length !== 1 ? 's' : ''}</span>`;

        if (!jobs || jobs.length === 0) {
            return `${controls}<div class="placeholder-message">No jobs in this workflow</div>`;
        }

        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('ID', 'id')}
                        ${this.renderSortableHeader('Name', 'name')}
                        ${this.renderSortableHeader('Status', 'status')}
                        ${this.renderSortableHeader('Command', 'command')}
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    ${jobs.map(job => `
                        <tr>
                            <td><code>${job.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(job.name || '-')}</td>
                            <td><span class="status-badge status-${statusNames[job.status]?.toLowerCase() || 'unknown'}">${statusNames[job.status] || job.status}</span></td>
                            <td><code>${this.escapeHtml(this.truncate(job.command || '-', 80))}</code></td>
                            <td><button class="btn-job-details" data-job-id="${job.id}" data-job-name="${this.escapeHtml(job.name || '')}">Details</button></td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    renderFilesTable(files) {
        const controls = this.renderTableControls('files');
        const count = `<span class="table-count">${files.length} file${files.length !== 1 ? 's' : ''}</span>`;

        if (!files || files.length === 0) {
            return `${controls}<div class="placeholder-message">No files in this workflow</div>`;
        }

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('ID', 'id')}
                        ${this.renderSortableHeader('Name', 'name')}
                        ${this.renderSortableHeader('Path', 'path')}
                        ${this.renderSortableHeader('Modified Time', 'st_mtime')}
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    ${files.map(file => `
                        <tr>
                            <td><code>${file.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(file.name || '-')}</td>
                            <td><code>${this.escapeHtml(file.path || '-')}</code></td>
                            <td>${this.formatUnixTimestamp(file.st_mtime)}</td>
                            <td>
                                ${file.path ? `<button class="btn-view-file" data-path="${this.escapeHtml(file.path)}" data-name="${this.escapeHtml(file.name || 'File')}">View</button>` : '-'}
                            </td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    renderUserDataTable(userData) {
        const controls = this.renderTableControls('user-data');
        const count = `<span class="table-count">${userData.length} record${userData.length !== 1 ? 's' : ''}</span>`;

        if (!userData || userData.length === 0) {
            return `${controls}<div class="placeholder-message">No user data in this workflow</div>`;
        }

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('ID', 'id')}
                        ${this.renderSortableHeader('Name', 'name')}
                        <th>Data</th>
                    </tr>
                </thead>
                <tbody>
                    ${userData.map(ud => `
                        <tr>
                            <td><code>${ud.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(ud.name || '-')}</td>
                            <td><code>${this.escapeHtml(this.truncate(JSON.stringify(ud.data) || '-', 100))}</code></td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    renderResultsTable(results, jobs, jobNameMapOverride) {
        const controls = this.renderTableControls('results');
        const count = `<span class="table-count">${results.length} result${results.length !== 1 ? 's' : ''}</span>`;

        if (!results || results.length === 0) {
            return `${controls}<div class="placeholder-message">No results in this workflow</div>`;
        }

        const jobNameMap = jobNameMapOverride || {};
        if (!jobNameMapOverride && jobs) {
            jobs.forEach(job => {
                jobNameMap[job.id] = job.name;
            });
        }

        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('Job ID', 'job_id')}
                        ${this.renderSortableHeader('Job Name', 'job_name')}
                        ${this.renderSortableHeader('Run ID', 'run_id')}
                        ${this.renderSortableHeader('Attempt', 'attempt_id')}
                        ${this.renderSortableHeader('Return Code', 'return_code')}
                        ${this.renderSortableHeader('Status', 'status')}
                        ${this.renderSortableHeader('Exec Time (min)', 'exec_time_minutes')}
                        ${this.renderSortableHeader('Peak Mem', 'peak_memory_bytes')}
                        ${this.renderSortableHeader('Avg CPU %', 'avg_cpu_percent')}
                    </tr>
                </thead>
                <tbody>
                    ${results.map(result => `
                        <tr>
                            <td><code>${result.job_id ?? '-'}</code></td>
                            <td>${this.escapeHtml(jobNameMap[result.job_id] || '-')}</td>
                            <td>${result.run_id ?? '-'}</td>
                            <td>${result.attempt_id ?? 1}</td>
                            <td class="${result.return_code === 0 ? 'return-code-0' : 'return-code-error'}">${result.return_code ?? '-'}</td>
                            <td><span class="status-badge status-${statusNames[result.status]?.toLowerCase() || 'unknown'}">${statusNames[result.status] || result.status}</span></td>
                            <td>${result.exec_time_minutes != null ? result.exec_time_minutes.toFixed(2) : '-'}</td>
                            <td>${this.formatBytes(result.peak_memory_bytes)}</td>
                            <td>${result.avg_cpu_percent != null ? result.avg_cpu_percent.toFixed(1) : '-'}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    renderWorkflowEventsTable(events) {
        const items = Array.isArray(events) ? events : api.extractItems(events);
        const controls = this.renderTableControls('events');
        const count = `<span class="table-count">${items.length} event${items.length !== 1 ? 's' : ''}</span>`;

        if (!items || items.length === 0) {
            return `${controls}<div class="placeholder-message">No events in this workflow</div>`;
        }

        return `
            ${controls}
            ${count}
            <div class="events-split-view">
                <div class="events-table-container">
                    <table class="data-table" id="events-detail-table">
                        <thead>
                            <tr>
                                ${this.renderSortableHeader('ID', 'id')}
                                ${this.renderSortableHeader('Timestamp', 'timestamp')}
                                <th>Message</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${items.map((event, index) => `
                                <tr class="event-row clickable" data-event-index="${index}">
                                    <td><code>${event.id ?? '-'}</code></td>
                                    <td>${this.formatTimestamp(event.timestamp)}</td>
                                    <td>${this.escapeHtml(this.getEventMessage(event))}</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
                <div id="event-data-preview" class="event-data-preview-side">
                    <div class="event-data-header">
                        <span>Event Data</span>
                    </div>
                    <pre id="event-data-content" class="event-data-content">Click on an event to view details</pre>
                </div>
            </div>
        `;
    },

    // Store events for preview lookup
    setEventsForPreview(events) {
        this._eventsForPreview = Array.isArray(events) ? events : api.extractItems(events);
    },

    setupEventRowClickHandlers() {
        const table = document.getElementById('events-detail-table');
        const previewContent = document.getElementById('event-data-content');

        if (!table || !previewContent) {
            return;
        }

        // Add click handler to each row
        table.querySelectorAll('.event-row').forEach(row => {
            row.onclick = () => {
                const index = parseInt(row.dataset.eventIndex);
                const event = this._eventsForPreview?.[index];
                if (!event) return;

                // Highlight selected row
                table.querySelectorAll('.event-row').forEach(r => r.classList.remove('selected'));
                row.classList.add('selected');

                // Show preview with pretty-printed JSON
                previewContent.textContent = JSON.stringify(event.data, null, 2);
            };
        });
    },

    renderSchedulersTable(schedulers) {
        const controls = this.renderTableControls('schedulers');
        const count = `<span class="table-count">${schedulers.length} scheduler${schedulers.length !== 1 ? 's' : ''}</span>`;

        if (!schedulers || schedulers.length === 0) {
            return `${controls}<div class="placeholder-message">No Slurm schedulers configured for this workflow</div>`;
        }

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('ID', 'id')}
                        ${this.renderSortableHeader('Name', 'name')}
                        ${this.renderSortableHeader('Account', 'account')}
                        ${this.renderSortableHeader('Partition', 'partition')}
                        ${this.renderSortableHeader('Walltime', 'walltime')}
                        ${this.renderSortableHeader('Nodes', 'nodes')}
                        ${this.renderSortableHeader('Mem', 'mem')}
                    </tr>
                </thead>
                <tbody>
                    ${schedulers.map(s => `
                        <tr>
                            <td><code>${s.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(s.name || '-')}</td>
                            <td>${this.escapeHtml(s.account || '-')}</td>
                            <td>${this.escapeHtml(s.partition || '-')}</td>
                            <td>${this.escapeHtml(s.walltime || '-')}</td>
                            <td>${s.nodes ?? '-'}</td>
                            <td>${this.escapeHtml(s.mem || '-')}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    renderComputeNodesTable(nodes) {
        const controls = this.renderTableControls('compute-nodes');
        const count = `<span class="table-count">${nodes.length} node${nodes.length !== 1 ? 's' : ''}</span>`;

        if (!nodes || nodes.length === 0) {
            return `${controls}<div class="placeholder-message">No compute nodes in this workflow</div>`;
        }

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('ID', 'id')}
                        ${this.renderSortableHeader('Hostname', 'hostname')}
                        ${this.renderSortableHeader('CPUs', 'num_cpus')}
                        ${this.renderSortableHeader('Memory (GB)', 'memory_gb')}
                        ${this.renderSortableHeader('GPUs', 'num_gpus')}
                        ${this.renderSortableHeader('Active', 'is_active')}
                        ${this.renderSortableHeader('CPU peak/avg', 'peak_cpu_percent')}
                        ${this.renderSortableHeader('Mem peak/avg', 'peak_memory_bytes')}
                    </tr>
                </thead>
                <tbody>
                    ${nodes.map(n => {
                        const sysCpu = n.peak_cpu_percent != null && n.avg_cpu_percent != null
                            ? `${n.peak_cpu_percent.toFixed(1)}% / ${n.avg_cpu_percent.toFixed(1)}%`
                            : '-';
                        const sysMem = n.peak_memory_bytes != null && n.avg_memory_bytes != null
                            ? `${this.formatBytes(n.peak_memory_bytes)} / ${this.formatBytes(n.avg_memory_bytes)}`
                            : '-';
                        return `
                        <tr>
                            <td><code>${n.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(n.hostname || '-')}</td>
                            <td>${n.num_cpus ?? '-'}</td>
                            <td>${n.memory_gb ?? '-'}</td>
                            <td>${n.num_gpus ?? '-'}</td>
                            <td>${n.is_active != null ? (n.is_active ? 'Yes' : 'No') : '-'}</td>
                            <td>${sysCpu}</td>
                            <td>${sysMem}</td>
                        </tr>
                    `}).join('')}
                </tbody>
            </table>
        `;
    },

    renderScheduledNodesTable(nodes) {
        const controls = this.renderTableControls('scheduled-nodes');
        const count = `<span class="table-count">${nodes.length} scheduled node${nodes.length !== 1 ? 's' : ''}</span>`;

        if (!nodes || nodes.length === 0) {
            return `${controls}<div class="placeholder-message">No scheduled compute nodes in this workflow</div>`;
        }

        const getStatusClass = (status) => {
            const s = (status || '').toLowerCase();
            if (s === 'running' || s === 'active') return 'status-running';
            if (s === 'pending' || s === 'scheduled') return 'status-pending';
            if (s === 'completed' || s === 'done' || s === 'complete') return 'status-completed';
            if (s === 'failed' || s === 'error') return 'status-failed';
            return '';
        };

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('ID', 'id')}
                        ${this.renderSortableHeader('Scheduler ID', 'scheduler_id')}
                        ${this.renderSortableHeader('Config ID', 'scheduler_config_id')}
                        ${this.renderSortableHeader('Type', 'scheduler_type')}
                        ${this.renderSortableHeader('Status', 'status')}
                        <th>Logs</th>
                    </tr>
                </thead>
                <tbody>
                    ${nodes.map(n => {
                        const isSlurm = (n.scheduler_type || '').toLowerCase() === 'slurm';
                        const logsButton = isSlurm && n.scheduler_id
                            ? `<button class="btn-slurm-logs" data-scheduler-id="${n.scheduler_id}">View Logs</button>`
                            : '-';
                        return `
                        <tr>
                            <td><code>${n.id ?? '-'}</code></td>
                            <td><code>${n.scheduler_id ?? '-'}</code></td>
                            <td><code>${n.scheduler_config_id ?? '-'}</code></td>
                            <td>${this.escapeHtml(n.scheduler_type || '-')}</td>
                            <td><span class="status-badge ${getStatusClass(n.status)}">${this.escapeHtml(n.status || '-')}</span></td>
                            <td>${logsButton}</td>
                        </tr>
                    `}).join('')}
                </tbody>
            </table>
        `;
    },

    renderSlurmStatsTable(stats) {
        const controls = this.renderTableControls('slurm-stats');

        if (!stats || stats.length === 0) {
            return `${controls}<div class="placeholder-message">No Slurm stats in this workflow</div>`;
        }

        const count = `<span class="table-count">${stats.length} stat${stats.length !== 1 ? 's' : ''}</span>`;

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('Job ID', 'job_id')}
                        ${this.renderSortableHeader('Run', 'run_id')}
                        ${this.renderSortableHeader('Attempt', 'attempt_id')}
                        ${this.renderSortableHeader('Slurm Job', 'slurm_job_id')}
                        ${this.renderSortableHeader('Max RSS', 'max_rss_bytes')}
                        ${this.renderSortableHeader('Max VM', 'max_vm_size_bytes')}
                        ${this.renderSortableHeader('Ave CPU (s)', 'ave_cpu_seconds')}
                        <th>CPU %</th>
                        <th>Nodes</th>
                    </tr>
                </thead>
                <tbody>
                    ${stats.map(stat => `
                        <tr>
                            <td><code>${stat.job_id ?? '-'}</code></td>
                            <td>${stat.run_id ?? '-'}</td>
                            <td>${stat.attempt_id ?? '-'}</td>
                            <td><code>${this.escapeHtml(stat.slurm_job_id || '-')}</code></td>
                            <td>${stat.max_rss_bytes != null && stat.max_rss_bytes > 0 ? this.formatBytes(stat.max_rss_bytes) : '-'}</td>
                            <td>${stat.max_vm_size_bytes != null && stat.max_vm_size_bytes > 0 ? this.formatBytes(stat.max_vm_size_bytes) : '-'}</td>
                            <td>${stat.ave_cpu_seconds != null && stat.ave_cpu_seconds > 0 ? stat.ave_cpu_seconds.toFixed(1) : '-'}</td>
                            <td>${stat.cpu_percent != null ? stat.cpu_percent.toFixed(1) + '%' : '-'}</td>
                            <td>${this.escapeHtml(stat.node_list || '-')}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    renderResourcesTable(resources) {
        const controls = this.renderTableControls('resources');
        const count = `<span class="table-count">${resources.length} requirement${resources.length !== 1 ? 's' : ''}</span>`;

        if (!resources || resources.length === 0) {
            return `${controls}<div class="placeholder-message">No resource requirements in this workflow</div>`;
        }

        return `
            ${controls}
            ${count}
            <table class="data-table">
                <thead>
                    <tr>
                        ${this.renderSortableHeader('ID', 'id')}
                        ${this.renderSortableHeader('Name', 'name')}
                        ${this.renderSortableHeader('CPUs', 'num_cpus')}
                        ${this.renderSortableHeader('Memory', 'memory')}
                        ${this.renderSortableHeader('GPUs', 'num_gpus')}
                        ${this.renderSortableHeader('Runtime', 'runtime')}
                    </tr>
                </thead>
                <tbody>
                    ${resources.map(r => `
                        <tr>
                            <td><code>${r.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(r.name || '-')}</td>
                            <td>${r.num_cpus ?? '-'}</td>
                            <td>${this.escapeHtml(r.memory || '-')}</td>
                            <td>${r.num_gpus ?? '-'}</td>
                            <td>${this.escapeHtml(r.runtime || '-')}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },
});
