/**
 * Torc Dashboard - Main Application
 *
 * This file contains the core TorcDashboard class with constructor and init.
 * All other methods are defined in separate module files that extend the prototype.
 *
 * Module files (loaded in order):
 *   - app-utils.js       - Utility functions (formatters, helpers)
 *   - app-core.js        - Settings, navigation, connection, auto-refresh
 *   - app-workflows.js   - Workflows tab and workflow operations
 *   - app-details.js     - Details tab, sub-tabs, table state management
 *   - app-tables.js      - Table rendering functions
 *   - app-events.js      - Events tab
 *   - app-dag-tab.js     - DAG tab
 *   - app-debugging.js   - Debugging tab
 *   - app-resources.js   - Resource plots tab
 *   - app-config.js      - Configuration/settings tab
 *   - app-modals.js      - Modal handling (create workflow, execution plan, file viewer)
 *   - app-job-details.js - Job details modal
 *   - app-wizard.js      - Workflow wizard (includes schedulers and actions)
 */

class TorcDashboard {
    constructor() {
        // State
        this.currentUser = null;
        this.workflows = [];
        this.selectedWorkflowId = null;
        this.selectedSubTab = 'jobs';
        this.currentTab = 'workflows';
        this.previousTab = null;
        this.isConnected = false;
        this.autoRefreshInterval = null;
        this.currentCreateTab = 'upload';
        this.uploadedSpecContent = null;
        this.uploadedSpecExtension = null;
        this.importFileContent = null;

        // Show all users toggle
        this.showAllUsers = false;

        // Events tab state
        this.events = [];
        this.eventPollInterval = null;

        // Debugging tab state
        this.debugJobs = [];
        this.selectedDebugJob = null;
        this.currentLogTab = 'stdout';
        this.debugOutputDir = 'torc_output';

        // Resource plots tab state
        this.resourceDatabases = [];
        this.selectedDatabases = [];
        this.resourcePlots = [];
        this.currentPlotIndex = 0;

        // Job details modal state
        this.jobDetailsData = null;
        this.currentJobDetailTab = 'results';

        // Initialize/Reinitialize confirmation state
        this.pendingInitializeWorkflowId = null;
        this.pendingReinitializeWorkflowId = null;

        // Execution streaming
        this.currentEventSource = null;

        // Multi-select state for bulk operations
        this.selectedWorkflowIds = new Set();

        // Table state for sorting/filtering
        this.tableState = {
            data: [],
            filteredData: [],
            sortColumn: null,
            sortDirection: 'asc',
            filterText: '',
            tabType: '',
            jobNameMap: {}
        };
    }

    async init() {
        // Load settings
        this.loadSettings();

        // Setup all UI components
        this.setupNavigation();
        this.setupWorkflowsTab();
        this.setupDetailsTab();
        this.setupDAGTab();
        this.setupEventsTab();
        this.setupDebuggingTab();
        this.setupResourcePlotsTab();
        this.setupSettingsTab();
        this.setupChatTab();
        this.setupModal();
        this.setupExecutionPlanModal();
        this.setupInitConfirmModal();
        this.setupReinitConfirmModal();
        this.setupRecoverModal();
        this.setupSyncStatusModal();
        this.setupExportModal();
        this.setupImportModal();
        this.setupFileViewerModal();
        this.setupJobDetailsModal();
        this.setupSlurmLogsModal();
        this.setupWizard();
        this.setupExecutionPanel();
        this.setupKeyboardShortcuts();

        // Load version info (fire and forget)
        this.loadVersion();

        // Load user info (needed for filtering workflows)
        await this.loadUser();

        // Test connection and load data
        await this.testConnection();
        await this.loadWorkflows();

        // Start auto-refresh
        this.startAutoRefresh();
    }
}

// Initialize application
const app = new TorcDashboard();
document.addEventListener('DOMContentLoaded', () => app.init());
