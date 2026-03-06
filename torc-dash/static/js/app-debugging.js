/**
 * Torc Dashboard - Debugging Tab
 * Debug reports and log viewing
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Debugging Tab ====================

    setupDebuggingTab() {
        document.getElementById('debug-workflow-selector')?.addEventListener('change', (e) => {
            this.selectedWorkflowId = e.target.value;
        });

        document.getElementById('btn-generate-report')?.addEventListener('click', () => {
            this.generateDebugReport();
        });

        // Log tab navigation
        document.querySelectorAll('.sub-tab[data-logtab]').forEach(tab => {
            tab.addEventListener('click', () => {
                this.switchLogTab(tab.dataset.logtab);
            });
        });

        // Slurm log analysis
        document.getElementById('btn-slurm-parse-logs')?.addEventListener('click', () => {
            this.analyzeSlurmLogs();
        });

        // Slurm sacct collection
        document.getElementById('btn-slurm-sacct')?.addEventListener('click', () => {
            this.collectSlurmSacct();
        });
    },

    async generateDebugReport() {
        const workflowId = document.getElementById('debug-workflow-selector')?.value;
        if (!workflowId) {
            this.showToast('Please select a workflow first', 'warning');
            return;
        }

        // Get output directory from the input field
        this.debugOutputDir = document.getElementById('debug-output-dir')?.value || 'torc_output';

        try {
            // Get jobs and results for the workflow
            const [jobs, results] = await Promise.all([
                api.listJobs(workflowId),
                api.listResults(workflowId),
            ]);

            const failedOnly = document.getElementById('debug-failed-only')?.checked;

            // Build a map of job results, adding stdout/stderr paths
            const resultMap = {};
            results.forEach(r => {
                if (!resultMap[r.job_id]) resultMap[r.job_id] = [];
                // Construct stdout/stderr file paths based on naming convention:
                // {output_dir}/job_stdio/job_wf{workflow_id}_j{job_id}_r{run_id}_a{attempt_id}.o (stdout)
                // {output_dir}/job_stdio/job_wf{workflow_id}_j{job_id}_r{run_id}_a{attempt_id}.e (stderr)
                const attemptId = r.attempt_id ?? 1;
                const stdioBase = `${this.debugOutputDir}/job_stdio/job_wf${r.workflow_id}_j${r.job_id}_r${r.run_id}_a${attemptId}`;
                resultMap[r.job_id].push({
                    ...r,
                    stdoutPath: `${stdioBase}.o`,
                    stderrPath: `${stdioBase}.e`,
                });
            });

            // Filter and enrich jobs with result data
            this.debugJobs = jobs.map(job => ({
                ...job,
                results: resultMap[job.id] || [],
                latestResult: resultMap[job.id]?.[resultMap[job.id].length - 1],
            }));

            if (failedOnly) {
                this.debugJobs = this.debugJobs.filter(j =>
                    j.latestResult && j.latestResult.return_code !== 0
                );
            }

            this.renderDebugJobsTable();
            document.getElementById('debug-job-count').textContent = `(${this.debugJobs.length})`;
        } catch (error) {
            this.showToast('Error generating report: ' + error.message, 'error');
        }
    },

    renderDebugJobsTable() {
        const container = document.getElementById('debug-jobs-table-container');
        if (!container) return;

        if (this.debugJobs.length === 0) {
            const failedOnly = document.getElementById('debug-failed-only')?.checked;
            const message = failedOnly
                ? 'No failed jobs found. Uncheck "Show only failed jobs" to see all jobs with results.'
                : 'No jobs match the criteria';
            container.innerHTML = `<div class="placeholder-message">${message}</div>`;
            return;
        }

        const statusNames = ['Uninitialized', 'Blocked', 'Ready', 'Pending', 'Running', 'Completed', 'Failed', 'Canceled', 'Terminated', 'Disabled'];

        container.innerHTML = `
            <table class="data-table">
                <thead>
                    <tr>
                        <th>Job Name</th>
                        <th>Status</th>
                        <th>Return Code</th>
                        <th>Stdout</th>
                        <th>Stderr</th>
                    </tr>
                </thead>
                <tbody>
                    ${this.debugJobs.map((job, idx) => {
                        const result = job.latestResult;
                        return `
                            <tr class="debug-table-row" onclick="app.selectDebugJob(${idx})">
                                <td>${this.escapeHtml(job.name || '-')}</td>
                                <td><span class="status-badge status-${statusNames[job.status]?.toLowerCase() || 'unknown'}">${statusNames[job.status] || '-'}</span></td>
                                <td class="${result?.return_code === 0 ? 'return-code-0' : 'return-code-error'}">${result?.return_code ?? '-'}</td>
                                <td><code>${result?.stdoutPath ? this.escapeHtml(this.truncate(result.stdoutPath, 40)) : '-'}</code></td>
                                <td><code>${result?.stderrPath ? this.escapeHtml(this.truncate(result.stderrPath, 40)) : '-'}</code></td>
                            </tr>
                        `;
                    }).join('')}
                </tbody>
            </table>
        `;
    },

    selectDebugJob(index) {
        this.selectedDebugJob = this.debugJobs[index];

        // Update selection styling
        document.querySelectorAll('.debug-table-row').forEach((row, i) => {
            row.classList.toggle('selected', i === index);
        });

        // Show job info
        const infoEl = document.getElementById('debug-selected-job-info');
        if (infoEl && this.selectedDebugJob) {
            infoEl.innerHTML = `<strong>${this.escapeHtml(this.selectedDebugJob.name)}</strong> (ID: ${this.truncateId(this.selectedDebugJob.id)})`;
            infoEl.classList.remove('placeholder-message');
        }

        // Show log tabs and viewer
        document.getElementById('log-tabs').style.display = 'flex';
        document.getElementById('log-viewer').style.display = 'block';

        // Load current log tab
        this.loadLogContent();
    },

    switchLogTab(logtab) {
        this.currentLogTab = logtab;

        document.querySelectorAll('.sub-tab[data-logtab]').forEach(tab => {
            tab.classList.toggle('active', tab.dataset.logtab === logtab);
        });

        this.loadLogContent();
    },

    async loadLogContent() {
        const logPath = document.getElementById('log-path');
        const logContent = document.getElementById('log-content');

        if (!this.selectedDebugJob?.latestResult) {
            logContent.textContent = 'No result data available';
            logPath.textContent = '';
            return;
        }

        const result = this.selectedDebugJob.latestResult;
        const isStdout = this.currentLogTab === 'stdout';
        const filePath = isStdout ? result.stdoutPath : result.stderrPath;

        logPath.textContent = filePath || '';
        logContent.classList.toggle('stderr', !isStdout);

        if (!filePath) {
            logContent.textContent = 'No file path available';
            return;
        }

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

    // ==================== Slurm Debugging ====================

    async analyzeSlurmLogs() {
        const workflowId = document.getElementById('debug-workflow-selector')?.value;
        if (!workflowId) {
            this.showToast('Please select a workflow first', 'warning');
            return;
        }

        const outputDir = document.getElementById('debug-output-dir')?.value || 'torc_output';
        const errorsOnly = document.getElementById('slurm-errors-only')?.checked || false;
        const resultsContainer = document.getElementById('slurm-logs-results');

        resultsContainer.innerHTML = '<div class="loading-indicator">Analyzing Slurm logs...</div>';

        try {
            const response = await fetch('/api/cli/slurm-parse-logs', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    workflow_id: parseInt(workflowId),
                    output_dir: outputDir,
                    errors_only: errorsOnly,
                }),
            });

            const result = await response.json();

            if (!result.success) {
                resultsContainer.innerHTML = `<div class="error-message">Error: ${this.escapeHtml(result.error || 'Unknown error')}</div>`;
                return;
            }

            this.renderSlurmLogsResults(result.data, resultsContainer);
        } catch (error) {
            resultsContainer.innerHTML = `<div class="error-message">Error: ${this.escapeHtml(error.message)}</div>`;
        }
    },

    renderSlurmLogsResults(data, container) {
        if (!data) {
            container.innerHTML = '<div class="placeholder-message">No data returned</div>';
            return;
        }

        const totalIssues = data.total_issues || 0;
        const errorCount = data.errors || 0;
        const warningCount = data.warnings || 0;
        const filesScanned = data.files_scanned || 0;
        const issues = data.issues || [];

        if (totalIssues === 0) {
            container.innerHTML = `
                <div class="success-message">
                    No issues found in Slurm logs (scanned ${filesScanned} file(s))
                </div>
            `;
            return;
        }

        // Group issues by Slurm job ID
        const issuesByJob = {};
        issues.forEach(issue => {
            const jobId = issue.slurm_job_id || 'unknown';
            if (!issuesByJob[jobId]) {
                issuesByJob[jobId] = [];
            }
            issuesByJob[jobId].push(issue);
        });

        let html = `
            <div class="slurm-summary">
                <strong>Summary:</strong> ${errorCount} error(s), ${warningCount} warning(s) in ${filesScanned} file(s)
            </div>
            <div class="slurm-issues-list">
        `;

        for (const [jobId, jobIssues] of Object.entries(issuesByJob)) {
            const firstIssue = jobIssues[0];
            const affectedJobs = firstIssue.affected_jobs || [];
            const affectedJobsHtml = affectedJobs.length > 0
                ? `<div class="affected-jobs">Affected Torc jobs: ${affectedJobs.map(j => `<span class="job-tag">${this.escapeHtml(j.job_name)} (ID: ${j.job_id})</span>`).join(', ')}</div>`
                : '';

            html += `
                <div class="slurm-job-group">
                    <div class="slurm-job-header">
                        <strong>Slurm Job ${this.escapeHtml(jobId)}</strong>
                        <span class="issue-count">${jobIssues.length} issue(s)</span>
                    </div>
                    ${affectedJobsHtml}
                    <div class="slurm-job-issues">
            `;

            jobIssues.forEach(issue => {
                const severityClass = issue.severity === 'error' ? 'severity-error' : 'severity-warning';
                const nodeInfo = issue.node ? ` (node: ${this.escapeHtml(issue.node)})` : '';
                html += `
                    <div class="slurm-issue ${severityClass}">
                        <div class="issue-header">
                            <span class="severity-badge ${severityClass}">${issue.severity.toUpperCase()}</span>
                            <span class="issue-description">${this.escapeHtml(issue.pattern_description)}${nodeInfo}</span>
                        </div>
                        <div class="issue-details">
                            <code class="issue-line">${this.escapeHtml(issue.line)}</code>
                            <div class="issue-location">${this.escapeHtml(issue.file)}:${issue.line_number}</div>
                        </div>
                    </div>
                `;
            });

            html += `
                    </div>
                </div>
            `;
        }

        html += '</div>';
        container.innerHTML = html;
    },

    async collectSlurmSacct() {
        const workflowId = document.getElementById('debug-workflow-selector')?.value;
        if (!workflowId) {
            this.showToast('Please select a workflow first', 'warning');
            return;
        }

        const outputDir = document.getElementById('debug-output-dir')?.value || 'torc_output';
        const resultsContainer = document.getElementById('slurm-sacct-results');

        resultsContainer.innerHTML = '<div class="loading-indicator">Collecting sacct data...</div>';

        try {
            const response = await fetch('/api/cli/slurm-sacct', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    workflow_id: parseInt(workflowId),
                    output_dir: outputDir,
                }),
            });

            const result = await response.json();

            if (!result.success) {
                resultsContainer.innerHTML = `<div class="error-message">Error: ${this.escapeHtml(result.error || 'Unknown error')}</div>`;
                return;
            }

            this.renderSlurmSacctResults(result.data, resultsContainer);
        } catch (error) {
            resultsContainer.innerHTML = `<div class="error-message">Error: ${this.escapeHtml(error.message)}</div>`;
        }
    },

    renderSlurmSacctResults(data, container) {
        if (!data) {
            container.innerHTML = '<div class="placeholder-message">No data returned</div>';
            return;
        }

        const totalSlurmJobs = data.total_slurm_jobs || 0;
        const summary = data.summary || [];
        const errors = data.errors || [];

        if (totalSlurmJobs === 0) {
            container.innerHTML = '<div class="placeholder-message">No Slurm scheduled compute nodes found for this workflow</div>';
            return;
        }

        let html = `
            <div class="sacct-summary">
                <strong>Slurm Job Accounting Summary:</strong> ${totalSlurmJobs} Slurm job(s), ${summary.length} job step(s)
            </div>
        `;

        if (summary.length > 0) {
            html += `
                <table class="data-table sacct-results-table">
                    <thead>
                        <tr>
                            <th>Slurm Job</th>
                            <th>Job Step</th>
                            <th>State</th>
                            <th>Exit Code</th>
                            <th>Elapsed</th>
                            <th>Max RSS</th>
                            <th>CPU Time</th>
                            <th>Nodes</th>
                        </tr>
                    </thead>
                    <tbody>
            `;

            summary.forEach(row => {
                const state = row.state || '-';
                const stateClass = this.getSacctStateClass(state);

                html += `
                    <tr>
                        <td>${this.escapeHtml(row.slurm_job_id || '-')}</td>
                        <td>${this.escapeHtml(row.job_step || '-')}</td>
                        <td><span class="status-badge ${stateClass}">${this.escapeHtml(state)}</span></td>
                        <td>${this.escapeHtml(row.exit_code || '-')}</td>
                        <td>${this.escapeHtml(row.elapsed || '-')}</td>
                        <td>${this.escapeHtml(row.max_rss || '-')}</td>
                        <td>${this.escapeHtml(row.cpu_time || '-')}</td>
                        <td>${this.escapeHtml(row.nodes || '-')}</td>
                    </tr>
                `;
            });

            html += `
                    </tbody>
                </table>
            `;
        }

        if (errors.length > 0) {
            html += `<div class="sacct-errors"><strong>Errors:</strong><ul>`;
            errors.forEach(err => {
                html += `<li class="error-text">${this.escapeHtml(err)}</li>`;
            });
            html += `</ul></div>`;
        }

        container.innerHTML = html;
    },

    getSacctStateClass(state) {
        const s = (state || '').toUpperCase();
        if (s === 'COMPLETED') return 'status-completed';
        if (s === 'RUNNING') return 'status-running';
        if (s === 'PENDING') return 'status-pending';
        if (s === 'FAILED' || s === 'NODE_FAIL' || s === 'OUT_OF_MEMORY') return 'status-failed';
        if (s === 'CANCELLED' || s === 'TIMEOUT' || s === 'PREEMPTED') return 'status-canceled';
        return 'status-unknown';
    },
});
