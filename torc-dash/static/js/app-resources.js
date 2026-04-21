/**
 * Torc Dashboard - Resource Plots Tab
 * Resource utilization plotting
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Resource Plots Tab ====================

    setupResourcePlotsTab() {
        document.getElementById('resource-workflow-selector')?.addEventListener('change', (e) => {
            this.selectedWorkflowId = e.target.value;
            this.clearResourcePlotsState();
            this.renderResourceDatabaseList();
        });

        document.getElementById('btn-scan-dbs')?.addEventListener('click', () => {
            this.scanResourceDatabases();
        });

        document.getElementById('btn-generate-plots')?.addEventListener('click', () => {
            this.generateResourcePlots();
        });
    },

    clearResourcePlotsState() {
        this.selectedDatabases = [];
        this.resourcePlots = [];
        this.currentPlotIndex = 0;

        const btn = document.getElementById('btn-generate-plots');
        if (btn) btn.disabled = true;

        const plotsSection = document.getElementById('plots-section');
        if (plotsSection) plotsSection.style.display = 'none';

        const plotTabs = document.getElementById('plot-tabs');
        if (plotTabs) plotTabs.innerHTML = '';

        const plotContainer = document.getElementById('plot-container');
        if (plotContainer) {
            plotContainer.innerHTML = '<div class="placeholder-message">Select a plot to view</div>';
        }
    },

    async scanResourceDatabases() {
        const baseDir = document.getElementById('resource-db-dir')?.value || 'torc_output/resource_utilization';
        const listContainer = document.getElementById('resource-db-list');

        if (!listContainer) return;

        listContainer.innerHTML = '<div class="placeholder-message">Scanning...</div>';

        try {
            const response = await api.listResourceDatabases(baseDir);

            if (!response.success) {
                listContainer.innerHTML = `<div class="placeholder-message" style="color: var(--danger-color)">Error: ${response.error}</div>`;
                return;
            }

            this.resourceDatabases = response.databases || [];
            this.clearResourcePlotsState();
            this.renderResourceDatabaseList();

            if (this.resourceDatabases.length === 0) {
                listContainer.innerHTML = '<div class="placeholder-message">No database files found in this directory</div>';
                document.getElementById('btn-generate-plots').disabled = true;
            }

        } catch (error) {
            listContainer.innerHTML = `<div class="placeholder-message" style="color: var(--danger-color)">Error: ${error.message}</div>`;
        }
    },

    getSelectedResourceWorkflowId() {
        return document.getElementById('resource-workflow-selector')?.value || '';
    },

    getFilteredResourceDatabases() {
        const workflowId = this.getSelectedResourceWorkflowId();
        if (!workflowId) return this.resourceDatabases;

        return this.resourceDatabases.filter(db => String(db.workflow_id || '') === String(workflowId));
    },

    renderResourceDatabaseList() {
        const listContainer = document.getElementById('resource-db-list');
        if (!listContainer) return;

        const workflowId = this.getSelectedResourceWorkflowId();
        const databases = this.getFilteredResourceDatabases();
        const btn = document.getElementById('btn-generate-plots');
        if (btn) btn.disabled = this.selectedDatabases.length === 0;

        if (this.resourceDatabases.length === 0) {
            listContainer.innerHTML = '<div class="placeholder-message">Click "Scan" to discover database files</div>';
            return;
        }

        if (databases.length === 0) {
            const message = workflowId
                ? `No database files found for workflow ${this.escapeHtml(workflowId)}`
                : 'No database files found in this directory';
            listContainer.innerHTML = `<div class="placeholder-message">${message}</div>`;
            return;
        }

        listContainer.innerHTML = databases.map((db, idx) => {
            const workflowLabel = db.workflow_id
                ? `<div class="db-workflow">Workflow ${this.escapeHtml(String(db.workflow_id))}</div>`
                : '<div class="db-workflow db-workflow-unknown">Workflow unknown</div>';
            const checked = this.selectedDatabases.includes(idx) ? 'checked' : '';

            return `
                <label class="resource-db-item">
                    <input type="checkbox" value="${idx}" ${checked} onchange="app.toggleDatabaseSelection(${idx}, this.checked)">
                    <div class="db-info">
                        <div class="db-name">${this.escapeHtml(db.name)}</div>
                        <div class="db-path">${this.escapeHtml(db.path)}</div>
                    </div>
                    <div class="db-meta">
                        ${workflowLabel}
                        <div>${this.formatBytes(db.size_bytes)}</div>
                        <div>${db.modified}</div>
                    </div>
                </label>
            `;
        }).join('');

        // If there's only one matching database, auto-select it
        if (databases.length === 1) {
            this.toggleDatabaseSelection(0, true);
            const checkbox = listContainer.querySelector('input[type="checkbox"]');
            if (checkbox) checkbox.checked = true;
        }
    },

    toggleDatabaseSelection(index, selected) {
        if (selected) {
            if (!this.selectedDatabases.includes(index)) {
                this.selectedDatabases.push(index);
            }
        } else {
            this.selectedDatabases = this.selectedDatabases.filter(i => i !== index);
        }

        // Enable/disable generate button
        const btn = document.getElementById('btn-generate-plots');
        if (btn) {
            btn.disabled = this.selectedDatabases.length === 0;
        }
    },

    async generateResourcePlots() {
        const btn = document.getElementById('btn-generate-plots');
        const plotsSection = document.getElementById('plots-section');
        const plotContainer = document.getElementById('plot-container');
        const plotTabs = document.getElementById('plot-tabs');

        if (this.selectedDatabases.length === 0) {
            this.showToast('Please select at least one database', 'warning');
            return;
        }

        // Get paths for selected databases
        const filteredDatabases = this.getFilteredResourceDatabases();
        const dbPaths = this.selectedDatabases.map(idx => filteredDatabases[idx].path);

        // Show loading state
        btn.disabled = true;
        btn.textContent = 'Generating...';
        plotsSection.style.display = 'block';
        plotContainer.innerHTML = '<div class="plot-loading">Generating plots</div>';
        plotTabs.innerHTML = '';

        try {
            const response = await api.generateResourcePlots(dbPaths);

            if (!response.success) {
                plotContainer.innerHTML = `<div class="placeholder-message" style="color: var(--danger-color)">Error: ${response.error}</div>`;
                return;
            }

            this.resourcePlots = response.plots || [];

            if (this.resourcePlots.length === 0) {
                plotContainer.innerHTML = '<div class="placeholder-message">No plots generated. The database may not contain any resource data.</div>';
                return;
            }

            // Create tabs for each plot
            plotTabs.innerHTML = this.resourcePlots.map((plot, idx) => {
                // Extract a friendly name from the filename
                const friendlyName = this.getPlotFriendlyName(plot.name);
                return `<button class="plot-tab ${idx === 0 ? 'active' : ''}" onclick="app.showPlot(${idx})">${friendlyName}</button>`;
            }).join('');

            // Show first plot
            this.currentPlotIndex = 0;
            this.showPlot(0);

        } catch (error) {
            plotContainer.innerHTML = `<div class="placeholder-message" style="color: var(--danger-color)">Error: ${error.message}</div>`;
        } finally {
            btn.disabled = false;
            btn.textContent = 'Generate Plots';
        }
    },

    getPlotFriendlyName(filename) {
        // Remove prefix and .json extension, then make human readable
        // e.g., "resource_plot_summary.json" -> "Summary"
        // e.g., "resource_plot_job_1.json" -> "Job 1"
        // e.g., "resource_plot_cpu_all_jobs.json" -> "CPU All Jobs"
        let name = filename.replace(/^resource_plot_?/, '').replace(/\.json$/, '');
        if (!name) return 'Summary';

        // Convert underscores to spaces and capitalize
        return name.split('_').map(word => {
            // Keep "cpu" and "gpu" uppercase
            if (['cpu', 'gpu'].includes(word.toLowerCase())) {
                return word.toUpperCase();
            }
            return word.charAt(0).toUpperCase() + word.slice(1);
        }).join(' ');
    },

    showPlot(index) {
        if (index < 0 || index >= this.resourcePlots.length) return;

        this.currentPlotIndex = index;

        // Update tab active state
        document.querySelectorAll('.plot-tab').forEach((tab, idx) => {
            tab.classList.toggle('active', idx === index);
        });

        // Get the plot data
        const plot = this.resourcePlots[index];
        const container = document.getElementById('plot-container');

        if (!plot || !plot.data) {
            container.innerHTML = '<div class="placeholder-message">No data available for this plot</div>';
            return;
        }

        // Clear container and create plot div
        container.innerHTML = '<div class="plot-wrapper"><div id="plotly-chart" style="width: 100%; height: 500px;"></div></div>';

        try {
            // Plotly expects data and layout from the JSON
            const plotData = plot.data.data || plot.data;
            const layout = plot.data.layout || {};

            // Adjust layout for better display
            layout.autosize = true;
            layout.margin = layout.margin || { l: 60, r: 60, t: 50, b: 60 };

            // Use responsive mode
            const config = {
                responsive: true,
                displayModeBar: true,
                modeBarButtonsToRemove: ['sendDataToCloud'],
            };

            Plotly.newPlot('plotly-chart', plotData, layout, config);
        } catch (error) {
            console.error('Error rendering plot:', error);
            container.innerHTML = `<div class="placeholder-message" style="color: var(--danger-color)">Error rendering plot: ${error.message}</div>`;
        }
    },
});
