/**
 * Torc Dashboard - Job Details Modal
 * Full job details view with tabs for results, files, dependencies
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Job Details Modal ====================

    setupJobDetailsModal() {
        document.getElementById('job-details-modal-close')?.addEventListener('click', () => {
            this.hideModal('job-details-modal');
        });

        document.getElementById('btn-close-job-details')?.addEventListener('click', () => {
            this.hideModal('job-details-modal');
        });

        document.getElementById('job-details-modal')?.addEventListener('click', (e) => {
            if (e.target.classList.contains('modal')) {
                this.hideModal('job-details-modal');
            }
        });

        document.querySelectorAll('.sub-tab[data-jobdetailtab]').forEach(tab => {
            tab.addEventListener('click', () => {
                this.switchJobDetailTab(tab.dataset.jobdetailtab);
            });
        });

        document.addEventListener('click', async (e) => {
            if (e.target.classList.contains('btn-job-details')) {
                const jobId = e.target.dataset.jobId;
                const jobName = e.target.dataset.jobName;
                if (jobId) {
                    await this.showJobDetails(jobId, jobName);
                }
            }
        });
    },

    async showJobDetails(jobId, jobName) {
        this.showModal('job-details-modal');
        this.currentJobDetailTab = 'results';
        this.jobDetailsData = null;

        const jobIdNum = parseInt(jobId, 10);

        const titleEl = document.getElementById('job-details-title');
        titleEl.textContent = jobName ? `Job: ${jobName}` : `Job Details`;

        document.querySelectorAll('.sub-tab[data-jobdetailtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.jobdetailtab === 'results');
        });

        const summaryEl = document.getElementById('job-details-summary');
        const contentEl = document.getElementById('job-details-content');
        summaryEl.innerHTML = '<div class="placeholder-message">Loading job details...</div>';
        contentEl.innerHTML = '';

        try {
            const workflowId = this.selectedWorkflowId;
            const [
                job,
                results,
                allFiles,
                fileRelationships,
                allUserData,
                userDataRelationships,
                resourceRequirements,
                allJobs,
                jobDependencies,
            ] = await Promise.all([
                api.getJob(jobId),
                api.listResults(workflowId),
                api.listFiles(workflowId),
                api.getJobFileRelationships(workflowId),
                api.listUserData(workflowId),
                api.getJobUserDataRelationships(workflowId),
                api.listResourceRequirements(workflowId),
                api.listJobs(workflowId),
                api.getJobsDependencies(workflowId),
            ]);

            const jobResults = results.filter(r => r.job_id === jobIdNum);

            const inputFileIds = new Set(
                fileRelationships
                    .filter(r => r.consumer_job_id === jobIdNum)
                    .map(r => r.file_id)
            );
            const outputFileIds = new Set(
                fileRelationships
                    .filter(r => r.producer_job_id === jobIdNum)
                    .map(r => r.file_id)
            );
            const inputFiles = allFiles.filter(f => inputFileIds.has(f.id));
            const outputFiles = allFiles.filter(f => outputFileIds.has(f.id));

            const inputUserDataIds = new Set(
                userDataRelationships
                    .filter(r => r.consumer_job_id === jobIdNum)
                    .map(r => r.user_data_id)
            );
            const outputUserDataIds = new Set(
                userDataRelationships
                    .filter(r => r.producer_job_id === jobIdNum)
                    .map(r => r.user_data_id)
            );
            const inputUserData = allUserData.filter(ud => inputUserDataIds.has(ud.id));
            const outputUserData = allUserData.filter(ud => outputUserDataIds.has(ud.id));

            const jobResourceReq = job.resource_requirements_id
                ? resourceRequirements.find(r => r.id === job.resource_requirements_id)
                : null;

            const blockedByJobIds = jobDependencies
                .filter(d => d.job_id === jobIdNum)
                .map(d => d.depends_on_job_id);
            const blockedByJobs = allJobs.filter(j => blockedByJobIds.includes(j.id));

            const blocksJobIds = jobDependencies
                .filter(d => d.depends_on_job_id === jobIdNum)
                .map(d => d.job_id);
            const blocksJobs = allJobs.filter(j => blocksJobIds.includes(j.id));

            this.jobDetailsData = {
                job,
                results: jobResults,
                inputFiles,
                outputFiles,
                inputUserData,
                outputUserData,
                resourceReq: jobResourceReq,
                blockedByJobs,
                blocksJobs,
            };

            this.renderJobDetailsSummary(job);
            this.renderJobDetailTabContent('results');

        } catch (error) {
            summaryEl.innerHTML = `<div class="placeholder-message">Error loading job details: ${this.escapeHtml(error.message)}</div>`;
        }
    },

    renderJobDetailsSummary(job) {
        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];
        const summaryEl = document.getElementById('job-details-summary');

        summaryEl.innerHTML = `
            <div class="job-details-summary-grid">
                <div class="job-details-summary-item">
                    <span class="label">ID</span>
                    <span class="value"><code>${job.id ?? '-'}</code></span>
                </div>
                <div class="job-details-summary-item">
                    <span class="label">Name</span>
                    <span class="value">${this.escapeHtml(job.name || '-')}</span>
                </div>
                <div class="job-details-summary-item">
                    <span class="label">Status</span>
                    <span class="value"><span class="status-badge status-${statusNames[job.status]?.toLowerCase() || 'unknown'}">${statusNames[job.status] || job.status}</span></span>
                </div>
                <div class="job-details-summary-item">
                    <span class="label">Command</span>
                    <span class="value"><code>${this.escapeHtml(this.truncate(job.command || '-', 50))}</code></span>
                </div>
            </div>
        `;
    },

    switchJobDetailTab(tabName) {
        this.currentJobDetailTab = tabName;

        document.querySelectorAll('.sub-tab[data-jobdetailtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.jobdetailtab === tabName);
        });

        this.renderJobDetailTabContent(tabName);
    },

    renderJobDetailTabContent(tabName) {
        const contentEl = document.getElementById('job-details-content');

        if (!this.jobDetailsData) {
            contentEl.innerHTML = '<div class="job-details-empty">No data available</div>';
            return;
        }

        const data = this.jobDetailsData;
        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];

        switch (tabName) {
            case 'results':
                if (data.results.length === 0) {
                    contentEl.innerHTML = '<div class="job-details-empty">No results for this job</div>';
                } else {
                    contentEl.innerHTML = `
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th>Run ID</th>
                                    <th>Attempt</th>
                                    <th>Return Code</th>
                                    <th>Status</th>
                                    <th>Exec Time (min)</th>
                                    <th>Peak Mem</th>
                                    <th>Avg CPU %</th>
                                </tr>
                            </thead>
                            <tbody>
                                ${data.results.map(r => `
                                    <tr>
                                        <td>${r.run_id ?? '-'}</td>
                                        <td>${r.attempt_id ?? 1}</td>
                                        <td class="${r.return_code === 0 ? 'return-code-0' : 'return-code-error'}">${r.return_code ?? '-'}</td>
                                        <td><span class="status-badge status-${statusNames[r.status]?.toLowerCase() || 'unknown'}">${statusNames[r.status] || r.status}</span></td>
                                        <td>${r.exec_time_minutes != null ? r.exec_time_minutes.toFixed(2) : '-'}</td>
                                        <td>${this.formatBytes(r.peak_memory_bytes)}</td>
                                        <td>${r.avg_cpu_percent != null ? r.avg_cpu_percent.toFixed(1) : '-'}</td>
                                    </tr>
                                `).join('')}
                            </tbody>
                        </table>
                    `;
                }
                break;

            case 'input-files':
                if (data.inputFiles.length === 0) {
                    contentEl.innerHTML = '<div class="job-details-empty">No input files for this job</div>';
                } else {
                    contentEl.innerHTML = this.renderJobDetailFilesTable(data.inputFiles);
                }
                break;

            case 'output-files':
                if (data.outputFiles.length === 0) {
                    contentEl.innerHTML = '<div class="job-details-empty">No output files for this job</div>';
                } else {
                    contentEl.innerHTML = this.renderJobDetailFilesTable(data.outputFiles);
                }
                break;

            case 'input-user-data':
                if (data.inputUserData.length === 0) {
                    contentEl.innerHTML = '<div class="job-details-empty">No input user data for this job</div>';
                } else {
                    contentEl.innerHTML = this.renderJobDetailUserDataTable(data.inputUserData);
                }
                break;

            case 'output-user-data':
                if (data.outputUserData.length === 0) {
                    contentEl.innerHTML = '<div class="job-details-empty">No output user data for this job</div>';
                } else {
                    contentEl.innerHTML = this.renderJobDetailUserDataTable(data.outputUserData);
                }
                break;

            case 'resource-req':
                if (!data.resourceReq) {
                    contentEl.innerHTML = '<div class="job-details-empty">No resource requirement assigned to this job</div>';
                } else {
                    const r = data.resourceReq;
                    contentEl.innerHTML = `
                        <table class="data-table">
                            <tbody>
                                <tr><th>ID</th><td><code>${r.id ?? '-'}</code></td></tr>
                                <tr><th>Name</th><td>${this.escapeHtml(r.name || '-')}</td></tr>
                                <tr><th>CPUs</th><td>${r.num_cpus ?? '-'}</td></tr>
                                <tr><th>Memory</th><td>${this.escapeHtml(r.memory || '-')}</td></tr>
                                <tr><th>GPUs</th><td>${r.num_gpus ?? '-'}</td></tr>
                                <tr><th>Runtime</th><td>${this.escapeHtml(r.runtime || '-')}</td></tr>
                            </tbody>
                        </table>
                    `;
                }
                break;

            case 'logs':
                this.renderJobLogsTab(contentEl);
                break;

            case 'dependencies':
                let depsHtml = '';

                if (data.blockedByJobs.length > 0) {
                    depsHtml += `
                        <div class="job-details-section">
                            <h4>Blocked By (${data.blockedByJobs.length})</h4>
                            <table class="data-table">
                                <thead>
                                    <tr><th>ID</th><th>Name</th><th>Status</th></tr>
                                </thead>
                                <tbody>
                                    ${data.blockedByJobs.map(j => `
                                        <tr>
                                            <td><code>${j.id ?? '-'}</code></td>
                                            <td>${this.escapeHtml(j.name || '-')}</td>
                                            <td><span class="status-badge status-${statusNames[j.status]?.toLowerCase() || 'unknown'}">${statusNames[j.status] || j.status}</span></td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    `;
                } else {
                    depsHtml += '<div class="job-details-section"><h4>Blocked By</h4><div class="job-details-empty">This job has no dependencies</div></div>';
                }

                if (data.blocksJobs.length > 0) {
                    depsHtml += `
                        <div class="job-details-section">
                            <h4>Blocks (${data.blocksJobs.length})</h4>
                            <table class="data-table">
                                <thead>
                                    <tr><th>ID</th><th>Name</th><th>Status</th></tr>
                                </thead>
                                <tbody>
                                    ${data.blocksJobs.map(j => `
                                        <tr>
                                            <td><code>${j.id ?? '-'}</code></td>
                                            <td>${this.escapeHtml(j.name || '-')}</td>
                                            <td><span class="status-badge status-${statusNames[j.status]?.toLowerCase() || 'unknown'}">${statusNames[j.status] || j.status}</span></td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    `;
                } else {
                    depsHtml += '<div class="job-details-section"><h4>Blocks</h4><div class="job-details-empty">No jobs depend on this job</div></div>';
                }

                contentEl.innerHTML = depsHtml;
                break;
        }
    },

    renderJobDetailFilesTable(files) {
        return `
            <table class="data-table">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                        <th>Path</th>
                        <th>Modified Time</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    ${files.map(f => `
                        <tr>
                            <td><code>${f.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(f.name || '-')}</td>
                            <td><code>${this.escapeHtml(f.path || '-')}</code></td>
                            <td>${this.formatUnixTimestamp(f.st_mtime)}</td>
                            <td>${f.path ? `<button class="btn-view-file" data-path="${this.escapeHtml(f.path)}" data-name="${this.escapeHtml(f.name || 'File')}">View</button>` : '-'}</td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    renderJobDetailUserDataTable(userData) {
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
                            <td><code>${ud.id ?? '-'}</code></td>
                            <td>${this.escapeHtml(ud.name || '-')}</td>
                            <td><code>${this.escapeHtml(this.truncate(JSON.stringify(ud.data) || '-', 100))}</code></td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;
    },

    // ==================== Job Logs Tab ====================

    renderJobLogsTab(contentEl) {
        const data = this.jobDetailsData;

        if (data.results.length === 0) {
            contentEl.innerHTML = '<div class="job-details-empty">No results yet - logs will be available after the job runs</div>';
            return;
        }

        // Get stored output directory or default
        const outputDir = localStorage.getItem('torc-job-logs-output-dir') || 'torc_output';
        const currentLogTab = this._jobLogTab || 'stdout';

        // Build run selector if multiple runs
        const runOptions = data.results.map((r, idx) =>
            `<option value="${idx}" ${idx === data.results.length - 1 ? 'selected' : ''}>Run ${r.run_id} Attempt ${r.attempt_id ?? 1} (Return: ${r.return_code})</option>`
        ).join('');

        contentEl.innerHTML = `
            <div class="job-logs-container">
                <div class="job-logs-controls">
                    <div class="job-logs-control-group">
                        <label for="job-logs-output-dir">Output Directory:</label>
                        <input type="text" id="job-logs-output-dir" value="${this.escapeHtml(outputDir)}" placeholder="torc_output">
                    </div>
                    ${data.results.length > 1 ? `
                        <div class="job-logs-control-group">
                            <label for="job-logs-run-selector">Run:</label>
                            <select id="job-logs-run-selector">${runOptions}</select>
                        </div>
                    ` : ''}
                    <button class="btn btn-primary btn-sm" id="btn-load-job-logs">Load Logs</button>
                </div>
                <div class="job-logs-tabs">
                    <button class="sub-tab ${currentLogTab === 'stdout' ? 'active' : ''}" data-joblogtab="stdout">stdout</button>
                    <button class="sub-tab ${currentLogTab === 'stderr' ? 'active' : ''}" data-joblogtab="stderr">stderr</button>
                </div>
                <div id="job-log-path" class="job-log-path"></div>
                <div id="job-log-content-wrapper" class="job-log-content-wrapper">
                    <pre id="job-log-content" class="job-log-content">Click "Load Logs" to view log content</pre>
                </div>
            </div>
        `;

        // Setup event handlers
        this.setupJobLogsHandlers();
    },

    setupJobLogsHandlers() {
        // Output dir change - save to localStorage
        document.getElementById('job-logs-output-dir')?.addEventListener('change', (e) => {
            localStorage.setItem('torc-job-logs-output-dir', e.target.value);
        });

        // Run selector change
        document.getElementById('job-logs-run-selector')?.addEventListener('change', () => {
            this.loadJobLogContent();
        });

        // Load logs button
        document.getElementById('btn-load-job-logs')?.addEventListener('click', () => {
            this.loadJobLogContent();
        });

        // Log tab switching
        document.querySelectorAll('[data-joblogtab]').forEach(tab => {
            tab.addEventListener('click', () => {
                this._jobLogTab = tab.dataset.joblogtab;
                document.querySelectorAll('[data-joblogtab]').forEach(t => {
                    t.classList.toggle('active', t.dataset.joblogtab === this._jobLogTab);
                });
                this.loadJobLogContent();
            });
        });
    },

    async loadJobLogContent() {
        const data = this.jobDetailsData;
        const logPathEl = document.getElementById('job-log-path');
        const logContentEl = document.getElementById('job-log-content');

        if (!logPathEl || !logContentEl) return;

        // Get selected run (default to latest)
        const runSelector = document.getElementById('job-logs-run-selector');
        const runIndex = runSelector ? parseInt(runSelector.value) : data.results.length - 1;
        const result = data.results[runIndex];

        if (!result) {
            logContentEl.textContent = 'No result selected';
            logPathEl.textContent = '';
            return;
        }

        // Get output directory
        const outputDir = document.getElementById('job-logs-output-dir')?.value || 'torc_output';
        const isStdout = (this._jobLogTab || 'stdout') === 'stdout';

        // Construct log file path based on naming convention
        // Include attempt_id in the path (defaults to 1 if not present)
        const attemptId = result.attempt_id ?? 1;
        const stdioBase = `${outputDir}/job_stdio/job_wf${result.workflow_id}_j${result.job_id}_r${result.run_id}_a${attemptId}`;
        // Try the separate file (.o/.e) first, then fall back to combined (.log)
        const primaryPath = isStdout ? `${stdioBase}.o` : `${stdioBase}.e`;
        const combinedPath = `${stdioBase}.log`;

        logContentEl.classList.toggle('stderr', !isStdout);
        logContentEl.textContent = 'Loading...';

        try {
            let response = await fetch('/api/cli/read-file', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: primaryPath }),
            });

            let responseData = await response.json();

            // Fall back to combined .log file if separate file doesn't exist
            let filePath = primaryPath;
            if (!responseData.exists) {
                response = await fetch('/api/cli/read-file', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ path: combinedPath }),
                });
                responseData = await response.json();
                if (responseData.exists) {
                    filePath = combinedPath;
                }
            }

            logPathEl.textContent = filePath;

            if (!responseData.exists) {
                logContentEl.textContent = '(file does not exist)';
            } else if (!responseData.success) {
                logContentEl.textContent = `Error: ${responseData.error || 'Unknown error'}`;
            } else if (!responseData.content || responseData.content.trim() === '') {
                logContentEl.textContent = '(empty)';
            } else {
                logContentEl.textContent = responseData.content;
            }
        } catch (error) {
            logContentEl.textContent = `Error loading file: ${error.message}`;
        }
    },
});
