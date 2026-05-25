/**
 * DAG Visualization Module
 * Uses Cytoscape.js for rendering workflow dependency graphs
 */

class DAGVisualizer {
    constructor(containerId) {
        this.containerId = containerId;
        this.cy = null;
        this.currentWorkflowId = null;
        this.currentType = 'jobs';
    }

    // Status to color mapping
    // Keyed by the snake_case JobStatus string the server sends, per the
    // OpenAPI enum. (Previously keyed by the legacy integer status, which
    // made every node fall back to the gray default once the API switched
    // to string status.)
    static statusColors = {
        uninitialized: '#6c757d',
        blocked: '#ffc107',
        ready: '#17a2b8',
        pending: '#fd7e14',
        running: '#007bff',
        completed: '#28a745',
        failed: '#dc3545',
        canceled: '#6f42c1',
        terminated: '#adb5bd',
        disabled: '#e9ecef',
        pending_failed: '#dc3545',
    };

    static statusNames = {
        uninitialized: 'Uninitialized',
        blocked: 'Blocked',
        ready: 'Ready',
        pending: 'Pending',
        running: 'Running',
        completed: 'Completed',
        failed: 'Failed',
        canceled: 'Canceled',
        terminated: 'Terminated',
        disabled: 'Disabled',
        pending_failed: 'Pending Failed',
    };

    initialize() {
        const container = document.getElementById(this.containerId);
        if (!container) {
            console.error('DAG container not found:', this.containerId);
            return;
        }

        // Clear placeholder
        container.innerHTML = '';

        this.cy = cytoscape({
            container: container,
            style: [
                // Node styles
                {
                    selector: 'node',
                    style: {
                        'label': 'data(label)',
                        'text-valign': 'center',
                        'text-halign': 'center',
                        'background-color': 'data(color)',
                        'color': '#fff',
                        'text-outline-color': 'data(color)',
                        'text-outline-width': 2,
                        'font-size': '11px',
                        'width': 'label',
                        'height': 30,
                        'padding': '10px',
                        'shape': 'roundrectangle',
                    }
                },
                // Job nodes
                {
                    selector: 'node.job',
                    style: {
                        'shape': 'roundrectangle',
                    }
                },
                // File nodes
                {
                    selector: 'node.file',
                    style: {
                        'shape': 'diamond',
                        'background-color': '#20c997',
                    }
                },
                // User data nodes
                {
                    selector: 'node.userdata',
                    style: {
                        'shape': 'ellipse',
                        'background-color': '#e83e8c',
                    }
                },
                // Edge styles
                {
                    selector: 'edge',
                    style: {
                        'width': 2,
                        'line-color': '#adb5bd',
                        'target-arrow-color': '#adb5bd',
                        'target-arrow-shape': 'triangle',
                        'curve-style': 'bezier',
                        'arrow-scale': 1.2,
                    }
                },
                // Input edge (file/data -> job)
                {
                    selector: 'edge.input',
                    style: {
                        'line-color': '#28a745',
                        'target-arrow-color': '#28a745',
                        'line-style': 'dashed',
                    }
                },
                // Output edge (job -> file/data)
                {
                    selector: 'edge.output',
                    style: {
                        'line-color': '#007bff',
                        'target-arrow-color': '#007bff',
                    }
                },
                // Highlighted node
                {
                    selector: 'node:selected',
                    style: {
                        'border-width': 3,
                        'border-color': '#000',
                    }
                },
            ],
            layout: { name: 'preset' },
            wheelSensitivity: 0.3,
            minZoom: 0.1,
            maxZoom: 3,
        });

        // Add click handlers
        this.cy.on('tap', 'node', (evt) => {
            const node = evt.target;
            console.log('Node clicked:', node.data());
            this.showNodeInfo(node.data());
        });
    }

    async loadJobDependencies(workflowId) {
        this.currentWorkflowId = workflowId;
        this.currentType = 'jobs';

        try {
            // Get jobs and their dependencies
            const [jobs, dependencies] = await Promise.all([
                api.listJobs(workflowId),
                api.getJobsDependencies(workflowId),
            ]);

            if (!jobs || jobs.length === 0) {
                this.showEmpty('No jobs in this workflow');
                return;
            }

            // Create node elements
            const nodes = jobs.map(job => ({
                data: {
                    id: `job-${job.id}`,
                    label: job.name || `Job ${job.id}`,
                    color: DAGVisualizer.statusColors[job.status] || '#6c757d',
                    type: 'job',
                    jobId: job.id,
                    status: job.status,
                    statusName: DAGVisualizer.statusNames[job.status],
                },
                classes: 'job',
            }));

            // Create edge elements from dependencies
            const edges = [];
            if (dependencies) {
                for (const dep of dependencies) {
                    edges.push({
                        data: {
                            id: `edge-${dep.depends_on_job_id}-${dep.job_id}`,
                            source: `job-${dep.depends_on_job_id}`,
                            target: `job-${dep.job_id}`,
                        }
                    });
                }
            }

            this.renderGraph(nodes, edges);
        } catch (error) {
            console.error('Error loading job dependencies:', error);
            this.showEmpty('Error loading dependencies: ' + error.message);
        }
    }

    async loadFileRelationships(workflowId) {
        this.currentWorkflowId = workflowId;
        this.currentType = 'files';

        try {
            const [jobs, files, relationships] = await Promise.all([
                api.listJobs(workflowId),
                api.listFiles(workflowId),
                api.getJobFileRelationships(workflowId),
            ]);

            if (!jobs || jobs.length === 0) {
                this.showEmpty('No jobs in this workflow');
                return;
            }

            // Create job nodes
            const nodes = jobs.map(job => ({
                data: {
                    id: `job-${job.id}`,
                    label: job.name || `Job ${job.id}`,
                    color: DAGVisualizer.statusColors[job.status] || '#6c757d',
                    type: 'job',
                },
                classes: 'job',
            }));

            // Create file nodes
            if (files) {
                for (const file of files) {
                    nodes.push({
                        data: {
                            id: `file-${file.id}`,
                            label: this.truncateName(file.name || file.path || `File ${file.id}`),
                            type: 'file',
                        },
                        classes: 'file',
                    });
                }
            }

            // Create edges from relationships
            // API returns producer_job_id (job that outputs the file) and consumer_job_id (job that inputs the file)
            const edges = [];
            if (relationships) {
                for (const rel of relationships) {
                    // Producer job -> File (output edge)
                    if (rel.producer_job_id) {
                        edges.push({
                            data: {
                                id: `edge-job-${rel.producer_job_id}-file-${rel.file_id}`,
                                source: `job-${rel.producer_job_id}`,
                                target: `file-${rel.file_id}`,
                            },
                            classes: 'output',
                        });
                    }
                    // File -> Consumer job (input edge)
                    if (rel.consumer_job_id) {
                        edges.push({
                            data: {
                                id: `edge-file-${rel.file_id}-job-${rel.consumer_job_id}`,
                                source: `file-${rel.file_id}`,
                                target: `job-${rel.consumer_job_id}`,
                            },
                            classes: 'input',
                        });
                    }
                }
            }

            this.renderGraph(nodes, edges);
        } catch (error) {
            console.error('Error loading file relationships:', error);
            this.showEmpty('Error loading file relationships: ' + error.message);
        }
    }

    async loadUserDataRelationships(workflowId) {
        this.currentWorkflowId = workflowId;
        this.currentType = 'userdata';

        try {
            const [jobs, userData, relationships] = await Promise.all([
                api.listJobs(workflowId),
                api.listUserData(workflowId),
                api.getJobUserDataRelationships(workflowId),
            ]);

            if (!jobs || jobs.length === 0) {
                this.showEmpty('No jobs in this workflow');
                return;
            }

            // Create job nodes
            const nodes = jobs.map(job => ({
                data: {
                    id: `job-${job.id}`,
                    label: job.name || `Job ${job.id}`,
                    color: DAGVisualizer.statusColors[job.status] || '#6c757d',
                    type: 'job',
                },
                classes: 'job',
            }));

            // Create user data nodes
            if (userData) {
                for (const ud of userData) {
                    nodes.push({
                        data: {
                            id: `ud-${ud.id}`,
                            label: this.truncateName(ud.name || `UserData ${ud.id}`),
                            type: 'userdata',
                        },
                        classes: 'userdata',
                    });
                }
            }

            // Create edges from relationships
            // API returns producer_job_id (job that outputs user data) and consumer_job_id (job that inputs user data)
            const edges = [];
            if (relationships) {
                for (const rel of relationships) {
                    // Producer job -> UserData (output edge)
                    if (rel.producer_job_id) {
                        edges.push({
                            data: {
                                id: `edge-job-${rel.producer_job_id}-ud-${rel.user_data_id}`,
                                source: `job-${rel.producer_job_id}`,
                                target: `ud-${rel.user_data_id}`,
                            },
                            classes: 'output',
                        });
                    }
                    // UserData -> Consumer job (input edge)
                    if (rel.consumer_job_id) {
                        edges.push({
                            data: {
                                id: `edge-ud-${rel.user_data_id}-job-${rel.consumer_job_id}`,
                                source: `ud-${rel.user_data_id}`,
                                target: `job-${rel.consumer_job_id}`,
                            },
                            classes: 'input',
                        });
                    }
                }
            }

            this.renderGraph(nodes, edges);
        } catch (error) {
            console.error('Error loading user data relationships:', error);
            this.showEmpty('Error loading user data relationships: ' + error.message);
        }
    }

    renderGraph(nodes, edges) {
        if (!this.cy) {
            this.initialize();
        }

        // Clear existing elements
        this.cy.elements().remove();

        // Add new elements
        this.cy.add([...nodes, ...edges]);

        // Apply dagre layout
        this.cy.layout({
            name: 'dagre',
            rankDir: 'TB',  // Top to Bottom
            nodeSep: 50,
            rankSep: 80,
            padding: 30,
            animate: true,
            animationDuration: 300,
        }).run();

        // Show legend
        const legend = document.getElementById('dag-legend');
        if (legend) {
            legend.style.display = 'flex';
        }
    }

    showEmpty(message) {
        const container = document.getElementById(this.containerId);
        if (container) {
            container.innerHTML = `<div class="placeholder-message">${message}</div>`;
        }

        const legend = document.getElementById('dag-legend');
        if (legend) {
            legend.style.display = 'none';
        }
    }

    showNodeInfo(data) {
        // Could show a tooltip or sidebar with node details
        console.log('Node info:', data);
    }

    fitToView() {
        if (this.cy) {
            this.cy.fit(50);
        }
    }

    truncateName(name, maxLen = 20) {
        if (name.length > maxLen) {
            return name.substring(0, maxLen - 3) + '...';
        }
        return name;
    }

    async refresh() {
        if (!this.currentWorkflowId) return;

        switch (this.currentType) {
            case 'jobs':
                await this.loadJobDependencies(this.currentWorkflowId);
                break;
            case 'files':
                await this.loadFileRelationships(this.currentWorkflowId);
                break;
            case 'userdata':
                await this.loadUserDataRelationships(this.currentWorkflowId);
                break;
        }
    }

    destroy() {
        if (this.cy) {
            this.cy.destroy();
            this.cy = null;
        }
    }
}

// Export
const dagVisualizer = new DAGVisualizer('dag-container');
