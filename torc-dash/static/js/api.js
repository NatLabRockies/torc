/**
 * Torc API Client
 * Handles all communication with the Torc REST API
 */

class TorcAPI {
    constructor() {
        // Default to same origin (dashboard served by torc-server)
        this.baseUrl = '/torc-service/v1';
        this.loadSettings();
    }

    loadSettings() {
        const savedUrl = localStorage.getItem('torc-api-url');
        if (savedUrl) {
            this.baseUrl = savedUrl;
        }
    }

    setBaseUrl(url) {
        this.baseUrl = url;
        localStorage.setItem('torc-api-url', url);
    }

    getBaseUrl() {
        return this.baseUrl;
    }

    async request(endpoint, options = {}) {
        const url = `${this.baseUrl}${endpoint}`;
        const defaultOptions = {
            headers: {
                'Content-Type': 'application/json',
            },
        };

        const finalOptions = {
            ...defaultOptions,
            ...options,
            headers: {
                ...defaultOptions.headers,
                ...options.headers,
            },
        };

        try {
            const response = await fetch(url, finalOptions);

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(`HTTP ${response.status}: ${errorText || response.statusText}`);
            }

            // Handle empty responses
            const text = await response.text();
            if (!text) {
                return null;
            }

            return JSON.parse(text);
        } catch (error) {
            console.error(`API Error (${endpoint}):`, error);
            throw error;
        }
    }

    // ==================== Helper for paginated responses ====================

    /**
     * Extract items array from paginated API response
     * API returns: {items: [...], offset, count, total_count, has_more}
     */
    extractItems(response) {
        if (!response) return [];
        if (Array.isArray(response)) return response;
        if (response.items && Array.isArray(response.items)) return response.items;
        return [];
    }

    // ==================== Workflows ====================

    async listWorkflows(offset = 0, limit = 100, user = null) {
        let url = `/workflows?offset=${offset}&limit=${limit}`;
        if (user) {
            url += `&user=${encodeURIComponent(user)}`;
        }
        const response = await this.request(url);
        return this.extractItems(response);
    }

    async getWorkflow(workflowId) {
        return this.request(`/workflows/${workflowId}`);
    }

    async createWorkflow(workflow) {
        return this.request('/workflows', {
            method: 'POST',
            body: JSON.stringify(workflow),
        });
    }

    async deleteWorkflow(workflowId) {
        return this.request(`/workflows/${workflowId}`, {
            method: 'DELETE',
        });
    }

    async getWorkflowStatus(workflowId) {
        return this.request(`/workflows/${workflowId}/status`);
    }

    async initializeWorkflow(workflowId) {
        return this.request(`/workflows/${workflowId}/initialize`, {
            method: 'POST',
        });
    }

    // ==================== Jobs ====================

    async listJobs(workflowId, offset = 0, limit = 1000) {
        const response = await this.request(`/jobs?workflow_id=${workflowId}&offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    async getJob(jobId) {
        return this.request(`/jobs/${jobId}`);
    }

    async updateJobStatus(jobId, status) {
        return this.request(`/jobs/${jobId}`, {
            method: 'PATCH',
            body: JSON.stringify({ status }),
        });
    }

    async getJobDependencies(jobId) {
        return this.request(`/jobs/${jobId}/dependencies`);
    }

    async getJobsDependencies(workflowId) {
        // Get all jobs with their dependencies
        const response = await this.request(`/workflows/${workflowId}/job_dependencies`);
        return this.extractItems(response);
    }

    // ==================== Files ====================

    async listFiles(workflowId, offset = 0, limit = 1000) {
        const response = await this.request(`/files?workflow_id=${workflowId}&offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    async getFile(fileId) {
        return this.request(`/files/${fileId}`);
    }

    async getJobFileRelationships(workflowId) {
        const response = await this.request(`/workflows/${workflowId}/job_file_relationships`);
        return this.extractItems(response);
    }

    // ==================== User Data ====================

    async listUserData(workflowId, offset = 0, limit = 1000) {
        const response = await this.request(`/user_data?workflow_id=${workflowId}&offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    async getUserData(userDataId) {
        return this.request(`/user_data/${userDataId}`);
    }

    async getJobUserDataRelationships(workflowId) {
        const response = await this.request(`/workflows/${workflowId}/job_user_data_relationships`);
        return this.extractItems(response);
    }

    // ==================== Results ====================

    async listResults(workflowId, offset = 0, limit = 1000) {
        const response = await this.request(`/results?workflow_id=${workflowId}&offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    async getResult(resultId) {
        return this.request(`/results/${resultId}`);
    }

    // ==================== Events ====================

    async listEvents(workflowId = null, offset = 0, limit = 100, afterTimestamp = null) {
        const params = new URLSearchParams();

        if (workflowId) {
            params.set('workflow_id', workflowId);
        }

        params.set('offset', offset);
        params.set('limit', limit);

        if (afterTimestamp !== null) {
            params.set('after_timestamp', afterTimestamp);
        }

        const response = await this.request(`/events?${params.toString()}`);
        return this.extractItems(response);
    }

    // ==================== Compute Nodes ====================

    async listComputeNodes(workflowId) {
        const response = await this.request(`/compute_nodes?workflow_id=${workflowId}`);
        return this.extractItems(response);
    }

    // ==================== Resource Requirements ====================

    async listResourceRequirements(workflowId) {
        const response = await this.request(`/resource_requirements?workflow_id=${workflowId}`);
        return this.extractItems(response);
    }

    // ==================== Schedulers ====================

    async listSlurmSchedulers(workflowId) {
        const response = await this.request(`/slurm_schedulers?workflow_id=${workflowId}`);
        return this.extractItems(response);
    }

    // ==================== Scheduled Compute Nodes ====================

    async listScheduledComputeNodes(workflowId, offset = 0, limit = 1000) {
        const response = await this.request(`/scheduled_compute_nodes?workflow_id=${workflowId}&offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    // ==================== Workflow Events ====================

    async listWorkflowEvents(workflowId, offset = 0, limit = 1000) {
        return this.request(`/events?workflow_id=${workflowId}&offset=${offset}&limit=${limit}`);
    }

    // ==================== Health Check ====================

    async testConnection() {
        try {
            // Try to list workflows as a connection test
            await this.listWorkflows(0, 1);
            return { success: true };
        } catch (error) {
            return { success: false, error: error.message };
        }
    }

    // ==================== CLI Commands ====================
    // These endpoints execute torc CLI commands on the server

    /**
     * Create a workflow from a spec file or inline spec
     * @param {string} spec - File path or inline JSON/YAML content
     * @param {boolean} isFile - True if spec is a file path
     * @param {string} [fileExtension] - Original file extension (e.g., '.yaml', '.kdl') for uploaded content
     */
    async cliCreateWorkflow(spec, isFile = false, fileExtension = null) {
        const body = { spec, is_file: isFile };
        if (fileExtension) {
            body.file_extension = fileExtension;
        }
        return this.cliRequest('/api/cli/create', body);
    }

    /**
     * Create a workflow with auto-generated Slurm schedulers
     * @param {string} spec - File path or inline JSON/YAML content
     * @param {boolean} isFile - True if spec is a file path
     * @param {string} [fileExtension] - Original file extension (e.g., '.yaml', '.kdl')
     * @param {string} account - Slurm account name (required)
     * @param {string} [profile] - HPC profile name (optional, auto-detected if not provided)
     */
    async cliCreateSlurmWorkflow(spec, isFile, fileExtension, account, profile = null) {
        const body = { spec, is_file: isFile, account };
        if (fileExtension) {
            body.file_extension = fileExtension;
        }
        if (profile) {
            body.profile = profile;
        }
        return this.cliRequest('/api/cli/create-slurm', body);
    }

    /**
     * Run a workflow locally using the CLI
     * @param {string} workflowId - Workflow ID
     */
    async cliRunWorkflow(workflowId) {
        return this.cliRequest('/api/cli/run', { workflow_id: workflowId });
    }

    /**
     * Submit a workflow to the scheduler (e.g., Slurm)
     * @param {string} workflowId - Workflow ID
     */
    async cliSubmitWorkflow(workflowId) {
        return this.cliRequest('/api/cli/submit', { workflow_id: workflowId });
    }

    /**
     * Check initialization status (dry-run) to see if there are existing output files
     * @param {string} workflowId - Workflow ID
     * @returns {object} CLI response with JSON in stdout containing existing_output_file_count
     */
    async cliCheckInitialize(workflowId) {
        return this.cliRequest('/api/cli/check-initialize', { workflow_id: workflowId });
    }

    /**
     * Initialize a workflow
     * @param {string} workflowId - Workflow ID
     * @param {boolean} force - Force initialization (delete existing output files)
     */
    async cliInitializeWorkflow(workflowId, force = false) {
        return this.cliRequest('/api/cli/initialize', { workflow_id: workflowId, force });
    }

    /**
     * Delete a workflow using CLI
     * @param {string} workflowId - Workflow ID
     */
    async cliDeleteWorkflow(workflowId) {
        return this.cliRequest('/api/cli/delete', { workflow_id: workflowId });
    }

    /**
     * Reinitialize a workflow using CLI
     * @param {string} workflowId - Workflow ID
     * @param {boolean} force - Force reinitialization (when true: ignore missing input files)
     */
    async cliReinitializeWorkflow(workflowId, force = false) {
        return this.cliRequest('/api/cli/reinitialize', { workflow_id: workflowId, force });
    }

    /**
     * Reset workflow status using CLI
     * @param {string} workflowId - Workflow ID
     */
    async cliResetStatus(workflowId) {
        return this.cliRequest('/api/cli/reset-status', { workflow_id: workflowId });
    }

    /**
     * Get execution plan for a workflow
     * @param {string} workflowId - Workflow ID
     * @returns {object} Response with execution plan data
     */
    async getExecutionPlan(workflowId) {
        return this.cliRequest('/api/cli/execution-plan', { workflow_id: workflowId });
    }

    /**
     * Make a CLI command request
     */
    async cliRequest(endpoint, body) {
        try {
            const response = await fetch(endpoint, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(body),
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            return await response.json();
        } catch (error) {
            console.error(`CLI Error (${endpoint}):`, error);
            throw error;
        }
    }

    // ==================== Import / Export ====================

    /**
     * Export a workflow to a JSON file on the server
     * @param {string} workflowId - Workflow ID
     * @param {string} [output] - Output file path on server (default: workflow_<id>.json)
     * @param {boolean} includeResults - Include job results in export
     * @param {boolean} includeEvents - Include events in export
     * @returns {object} CLI response with export result
     */
    async cliExportWorkflow(workflowId, output = null, includeResults = false, includeEvents = false) {
        const body = {
            workflow_id: workflowId,
            include_results: includeResults,
            include_events: includeEvents,
        };
        if (output) {
            body.output = output;
        }
        return this.cliRequest('/api/cli/export', body);
    }

    /**
     * Import a workflow from a server-side file or uploaded content
     * @param {object} options - Import options
     * @param {string} [options.filePath] - Server-side file path to import from
     * @param {string} [options.content] - Uploaded JSON content (alternative to filePath)
     * @param {string} [options.name] - Optional name override
     * @param {boolean} [options.skipResults] - Skip importing results
     * @param {boolean} [options.skipEvents] - Skip importing events
     * @returns {object} CLI response with import result in stdout
     */
    async cliImportWorkflow({ filePath = null, content = null, name = null, skipResults = false, skipEvents = false } = {}) {
        const body = { skip_results: skipResults, skip_events: skipEvents };
        if (filePath) {
            body.file_path = filePath;
        } else if (content) {
            body.content = content;
        }
        if (name) {
            body.name = name;
        }
        return this.cliRequest('/api/cli/import', body);
    }

    // ==================== Resource Plots ====================

    /**
     * List resource database files in a directory
     * @param {string} baseDir - Directory to search for .db files
     * @returns {object} Response with databases array
     */
    async listResourceDatabases(baseDir) {
        return this.cliRequest('/api/cli/list-resource-dbs', { base_dir: baseDir });
    }

    /**
     * Generate resource plots from database files
     * @param {string[]} dbPaths - Paths to resource database files
     * @param {string} [prefix] - Prefix for output filenames
     * @returns {object} Response with plots array containing Plotly JSON data
     */
    async generateResourcePlots(dbPaths, prefix = 'resource_plot') {
        return this.cliRequest('/api/cli/plot-resources', {
            db_paths: dbPaths,
            prefix: prefix,
        });
    }

    // ==================== HPC Profiles & Slurm Generation ====================

    /**
     * Get available HPC profiles and detect current system
     * @returns {object} Response with profiles array and detected_profile
     */
    async getHpcProfiles() {
        try {
            const response = await fetch('/api/cli/hpc-profiles');
            return await response.json();
        } catch (error) {
            console.error('HPC profiles error:', error);
            return { success: false, profiles: [], error: error.message };
        }
    }

    // ==================== Server Management ====================

    /**
     * Start the torc-server process
     * @param {object} options - Server configuration options
     * @param {number} [options.port=8080] - Port to listen on
     * @param {string} [options.database] - Database path
     * @param {number} [options.completion_check_interval_secs=5] - Completion check interval
     * @param {string} [options.log_level='info'] - Log level
     * @returns {object} Response with success, message, pid, port
     */
    async startServer(options = {}) {
        try {
            const response = await fetch('/api/server/start', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(options),
            });
            return await response.json();
        } catch (error) {
            console.error('Server start error:', error);
            return { success: false, message: error.message };
        }
    }

    /**
     * Stop the managed torc-server process
     * @returns {object} Response with success, message
     */
    async stopServer() {
        try {
            const response = await fetch('/api/server/stop', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
            });
            return await response.json();
        } catch (error) {
            console.error('Server stop error:', error);
            return { success: false, message: error.message };
        }
    }

    /**
     * Get the status of the managed server
     * @returns {object} Response with running, managed, pid, port, output_lines
     */
    async getServerStatus() {
        try {
            const response = await fetch('/api/server/status');
            return await response.json();
        } catch (error) {
            console.error('Server status error:', error);
            return { running: false, managed: false };
        }
    }
}

// Export singleton instance
const api = new TorcAPI();
