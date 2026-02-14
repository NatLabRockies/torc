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

    async listWorkflows(offset = 0, limit = 100) {
        const response = await this.request(`/workflows?offset=${offset}&limit=${limit}`);
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

    async listJobs(workflowId, offset = 0, limit = 10000) {
        const response = await this.request(`/workflows/${workflowId}/jobs?offset=${offset}&limit=${limit}`);
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
        const response = await this.request(`/workflows/${workflowId}/jobs/dependencies`);
        return this.extractItems(response);
    }

    // ==================== Files ====================

    async listFiles(workflowId, offset = 0, limit = 10000) {
        const response = await this.request(`/workflows/${workflowId}/files?offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    async getFile(fileId) {
        return this.request(`/files/${fileId}`);
    }

    async getJobFileRelationships(workflowId) {
        const response = await this.request(`/workflows/${workflowId}/jobs/file_relationships`);
        return this.extractItems(response);
    }

    // ==================== User Data ====================

    async listUserData(workflowId, offset = 0, limit = 10000) {
        const response = await this.request(`/workflows/${workflowId}/user_data?offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    async getUserData(userDataId) {
        return this.request(`/user_data/${userDataId}`);
    }

    async getJobUserDataRelationships(workflowId) {
        const response = await this.request(`/workflows/${workflowId}/jobs/user_data_relationships`);
        return this.extractItems(response);
    }

    // ==================== Results ====================

    async listResults(workflowId, offset = 0, limit = 10000) {
        const response = await this.request(`/workflows/${workflowId}/results?offset=${offset}&limit=${limit}`);
        return this.extractItems(response);
    }

    async getResult(resultId) {
        return this.request(`/results/${resultId}`);
    }

    // ==================== Events ====================

    async listEvents(workflowId = null, offset = 0, limit = 100, afterId = null) {
        let url = '/events';
        const params = new URLSearchParams();

        if (workflowId) {
            url = `/workflows/${workflowId}/events`;
        }

        params.set('offset', offset);
        params.set('limit', limit);

        if (afterId !== null) {
            params.set('after_id', afterId);
        }

        const response = await this.request(`${url}?${params.toString()}`);
        return this.extractItems(response);
    }

    // ==================== Compute Nodes ====================

    async listComputeNodes(workflowId) {
        return this.request(`/workflows/${workflowId}/compute_nodes`);
    }

    // ==================== Resource Requirements ====================

    async listResourceRequirements(workflowId) {
        return this.request(`/workflows/${workflowId}/resource_requirements`);
    }

    // ==================== Schedulers ====================

    async listSchedulers(workflowId) {
        return this.request(`/workflows/${workflowId}/schedulers`);
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
}

// Export singleton instance
const api = new TorcAPI();
