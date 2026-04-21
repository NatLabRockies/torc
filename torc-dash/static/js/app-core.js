/**
 * Torc Dashboard - Core Methods
 * Settings, Navigation, Connection, and Auto-Refresh
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Settings ====================

    loadSettings() {
        const darkMode = localStorage.getItem('torc-dark-mode') === 'true';
        const theme = localStorage.getItem('torc-theme') || 'neutral';

        if (darkMode) {
            document.body.classList.add('dark-mode');
            this.applyTheme(theme);
            const checkbox = document.getElementById('dark-mode');
            if (checkbox) checkbox.checked = true;
        }

        // Set theme selector value and visibility
        const themeSelector = document.getElementById('theme-selector');
        const themeSelectorGroup = document.getElementById('theme-selector-group');
        if (themeSelector) themeSelector.value = theme;
        if (themeSelectorGroup) themeSelectorGroup.style.display = darkMode ? 'block' : 'none';

        // Sync sidebar dark mode toggle
        this.updateSidebarDarkToggle(darkMode);

        const refreshInterval = localStorage.getItem('torc-refresh-interval') || '30';
        const intervalInput = document.getElementById('refresh-interval');
        if (intervalInput) intervalInput.value = refreshInterval;

        const apiUrl = api.getBaseUrl();
        const apiInput = document.getElementById('api-url');
        if (apiInput) apiInput.value = apiUrl;

        // Setup theme change listeners
        this.setupThemeListeners();
    },

    setupThemeListeners() {
        const darkModeCheckbox = document.getElementById('dark-mode');
        const themeSelector = document.getElementById('theme-selector');
        const themeSelectorGroup = document.getElementById('theme-selector-group');

        // Toggle theme selector visibility when dark mode changes
        darkModeCheckbox?.addEventListener('change', (e) => {
            if (themeSelectorGroup) {
                themeSelectorGroup.style.display = e.target.checked ? 'block' : 'none';
            }
            if (e.target.checked) {
                const theme = themeSelector?.value || 'neutral';
                this.applyTheme(theme);
            } else {
                this.removeAllThemes();
            }
            this.updateSidebarDarkToggle(e.target.checked);
        });

        // Apply theme immediately when selector changes
        themeSelector?.addEventListener('change', (e) => {
            this.applyTheme(e.target.value);
            localStorage.setItem('torc-theme', e.target.value);
        });

        // Sidebar dark mode toggle
        document.getElementById('sidebar-dark-toggle')?.addEventListener('click', () => {
            this.toggleDarkMode();
        });
    },

    updateSidebarDarkToggle(isDark) {
        const icon = document.getElementById('dark-toggle-icon');
        const label = document.getElementById('dark-toggle-label');
        if (icon) icon.innerHTML = isDark ? '&#9788;' : '&#9790;';
        if (label) label.textContent = isDark ? 'Light Mode' : 'Dark Mode';
    },

    applyTheme(theme) {
        this.removeAllThemes();
        if (theme && theme !== 'midnight') {
            document.body.classList.add(`theme-${theme}`);
        }
    },

    removeAllThemes() {
        const themes = [
            'theme-midnight', 'theme-neutral', 'theme-warm', 'theme-nord',
            'theme-dracula', 'theme-monokai', 'theme-dimmed', 'theme-high-contrast'
        ];
        themes.forEach(t => document.body.classList.remove(t));
    },

    // All available themes in rotation order (light first, then dark themes)
    getAllThemes() {
        return [
            { id: 'light', name: 'Light', dark: false },
            { id: 'warm', name: 'Warm Gray', dark: true },
            { id: 'neutral', name: 'Neutral', dark: true },
            { id: 'dimmed', name: 'Dimmed', dark: true },
            { id: 'nord', name: 'Nord', dark: true },
            { id: 'dracula', name: 'Dracula', dark: true },
            { id: 'midnight', name: 'Midnight Blue', dark: true },
            { id: 'high-contrast', name: 'High Contrast', dark: true },
            { id: 'monokai', name: 'Monokai', dark: true }
        ];
    },

    getCurrentThemeIndex() {
        const themes = this.getAllThemes();
        const isDark = document.body.classList.contains('dark-mode');
        if (!isDark) return 0; // light mode

        const currentTheme = localStorage.getItem('torc-theme') || 'warm';
        const index = themes.findIndex(t => t.id === currentTheme);
        return index >= 0 ? index : 1;
    },

    cycleTheme() {
        const themes = this.getAllThemes();
        const currentIndex = this.getCurrentThemeIndex();
        const nextIndex = (currentIndex + 1) % themes.length;
        const nextTheme = themes[nextIndex];

        if (nextTheme.dark) {
            document.body.classList.add('dark-mode');
            this.applyTheme(nextTheme.id);
            localStorage.setItem('torc-dark-mode', 'true');
            localStorage.setItem('torc-theme', nextTheme.id);

            // Update UI
            const checkbox = document.getElementById('dark-mode');
            if (checkbox) checkbox.checked = true;
            const selector = document.getElementById('theme-selector');
            if (selector) selector.value = nextTheme.id;
            const selectorGroup = document.getElementById('theme-selector-group');
            if (selectorGroup) selectorGroup.style.display = 'block';
            this.updateSidebarDarkToggle(true);
        } else {
            document.body.classList.remove('dark-mode');
            this.removeAllThemes();
            localStorage.setItem('torc-dark-mode', 'false');

            // Update UI
            const checkbox = document.getElementById('dark-mode');
            if (checkbox) checkbox.checked = false;
            const selectorGroup = document.getElementById('theme-selector-group');
            if (selectorGroup) selectorGroup.style.display = 'none';
            this.updateSidebarDarkToggle(false);
        }

        this.showToast(`Theme: ${nextTheme.name}`, 'info');
    },

    saveSettings() {
        const darkMode = document.getElementById('dark-mode')?.checked || false;
        const theme = document.getElementById('theme-selector')?.value || 'neutral';
        const refreshInterval = document.getElementById('refresh-interval')?.value || '30';
        const apiUrl = document.getElementById('api-url')?.value || '/torc-service/v1';

        localStorage.setItem('torc-dark-mode', darkMode);
        localStorage.setItem('torc-theme', theme);
        localStorage.setItem('torc-refresh-interval', refreshInterval);
        api.setBaseUrl(apiUrl);

        if (darkMode) {
            document.body.classList.add('dark-mode');
            this.applyTheme(theme);
        } else {
            document.body.classList.remove('dark-mode');
            this.removeAllThemes();
        }
        this.updateSidebarDarkToggle(darkMode);

        this.showToast('Settings saved', 'success');

        // Restart auto-refresh with new interval
        this.stopAutoRefresh();
        this.startAutoRefresh();
    },

    // ==================== Navigation ====================

    setupNavigation() {
        const navItems = document.querySelectorAll('.nav-item');
        navItems.forEach(item => {
            item.addEventListener('click', () => {
                const tab = item.dataset.tab;
                this.switchTab(tab);
            });
        });
    },

    switchTab(tabName, skipHistory = false) {
        // Update nav items
        document.querySelectorAll('.nav-item').forEach(item => {
            item.classList.toggle('active', item.dataset.tab === tabName);
        });

        // Update tab content
        document.querySelectorAll('.tab-content').forEach(content => {
            content.classList.toggle('active', content.id === `tab-${tabName}`);
        });

        // Track previous tab for back navigation (unless we're going back)
        if (!skipHistory && this.currentTab !== tabName) {
            this.previousTab = this.currentTab;
        }
        this.currentTab = tabName;

        // Update back button visibility in DAG tab
        this.updateDAGBackButton();

        // Tab-specific initialization
        if (tabName === 'dag' && dagVisualizer && this.selectedWorkflowId) {
            dagVisualizer.initialize();
            dagVisualizer.loadJobDependencies(this.selectedWorkflowId);
        }

        // Sync events workflow selector with selected workflow and start SSE stream
        if (tabName === 'events') {
            const badge = document.getElementById('event-badge');
            if (badge) badge.style.display = 'none';
            if (this.selectedWorkflowId) {
                const eventsSelector = document.getElementById('events-workflow-selector');
                if (eventsSelector && eventsSelector.value !== this.selectedWorkflowId) {
                    eventsSelector.value = this.selectedWorkflowId;
                }
                // Start SSE stream if not already connected for this workflow
                if (this._lastEventsWorkflowId !== this.selectedWorkflowId) {
                    this.startEventStream(this.selectedWorkflowId);
                }
            }
        }

        // Sync debug workflow selector with selected workflow
        if (tabName === 'debugging' && this.selectedWorkflowId) {
            const debugSelector = document.getElementById('debug-workflow-selector');
            if (debugSelector) {
                debugSelector.value = this.selectedWorkflowId;
            }
        }

        // Sync resource plots workflow selector with selected workflow
        if (tabName === 'resource-plots' && this.selectedWorkflowId) {
            const resourceSelector = document.getElementById('resource-workflow-selector');
            if (resourceSelector && resourceSelector.value !== this.selectedWorkflowId) {
                resourceSelector.value = this.selectedWorkflowId;
                this.clearResourcePlotsState?.();
                this.renderResourceDatabaseList?.();
            }
        }
    },

    // ==================== Connection ====================

    async testConnection() {
        const result = await api.testConnection();
        this.isConnected = result.success;
        this.updateConnectionStatus(result.success);

        const serverInfo = document.getElementById('server-info');
        if (serverInfo) {
            if (result.success) {
                serverInfo.innerHTML = `<p style="color: var(--success-color)">Connected to ${api.getBaseUrl()}</p>`;
            } else {
                serverInfo.innerHTML = `<p style="color: var(--danger-color)">Connection failed: ${result.error}</p>`;
            }
        }

        return result;
    },

    updateConnectionStatus(connected) {
        const statusEl = document.getElementById('connection-status');
        if (statusEl) {
            const dot = statusEl.querySelector('.status-dot');
            const text = statusEl.querySelector('.status-text');
            if (connected) {
                dot.classList.remove('disconnected');
                dot.classList.add('connected');
                text.textContent = 'Connected';
            } else {
                dot.classList.remove('connected');
                dot.classList.add('disconnected');
                text.textContent = 'Disconnected';
            }
        }
    },

    async loadVersion() {
        try {
            const response = await fetch('/api/version');
            if (response.ok) {
                const data = await response.json();
                const versionEl = document.getElementById('version-display');
                if (versionEl) {
                    const text = versionEl.querySelector('.version-text');
                    if (text) {
                        // Show dashboard version and server version if available
                        let versionStr = `v${data.version}`;
                        if (data.server_version) {
                            versionStr += ` / server v${data.server_version}`;
                        }
                        text.textContent = versionStr;
                    }

                    // Handle version mismatch warnings
                    if (data.version_mismatch) {
                        versionEl.title = data.version_mismatch;
                        // Apply styling based on severity
                        versionEl.classList.remove('version-warning', 'version-error');
                        if (data.mismatch_severity === 'major') {
                            versionEl.classList.add('version-error');
                            this.showVersionWarning(data.version_mismatch, 'error');
                        } else if (data.mismatch_severity === 'minor') {
                            versionEl.classList.add('version-warning');
                            this.showVersionWarning(data.version_mismatch, 'warning');
                        } else if (data.mismatch_severity === 'patch') {
                            versionEl.classList.add('version-warning');
                            // Don't show a popup for patch differences, just visual indicator
                        }
                    } else {
                        versionEl.title = 'Torc version';
                        versionEl.classList.remove('version-warning', 'version-error');
                    }
                }
            }
        } catch (e) {
            // Silently ignore version fetch errors
            console.debug('Failed to fetch version:', e);
        }
    },

    async loadUser() {
        try {
            const response = await fetch('/api/user');
            if (response.ok) {
                const data = await response.json();
                if (data.user) {
                    this.currentUser = data.user;
                }
                const userEl = document.getElementById('user-display');
                if (userEl) {
                    const text = userEl.querySelector('.user-text');
                    if (text && data.user) {
                        text.textContent = data.user;
                        userEl.title = `Current user: ${data.user}`;
                    }
                }
            }
        } catch (e) {
            // Silently ignore user fetch errors
            console.debug('Failed to fetch user:', e);
        }
    },

    showVersionWarning(message, severity) {
        // Show a brief warning notification
        const notification = document.createElement('div');
        notification.className = `version-notification version-notification-${severity}`;
        notification.innerHTML = `
            <span class="notification-icon">${severity === 'error' ? '⛔' : '⚠️'}</span>
            <span class="notification-message">${message}</span>
            <button class="notification-close" onclick="this.parentElement.remove()">×</button>
        `;
        document.body.appendChild(notification);

        // Auto-dismiss after 10 seconds for warnings, keep errors visible
        if (severity === 'warning') {
            setTimeout(() => notification.remove(), 10000);
        }
    },

    // ==================== Auto Refresh ====================

    startAutoRefresh() {
        const interval = parseInt(localStorage.getItem('torc-refresh-interval') || '30') * 1000;
        this.autoRefreshInterval = setInterval(() => {
            if (this.currentTab === 'workflows') {
                this.loadWorkflows();
            } else if (this.currentTab === 'details' && this.selectedWorkflowId) {
                this.loadWorkflowDetails(this.selectedWorkflowId);
            } else if (this.currentTab === 'dag' && this.selectedWorkflowId) {
                dagVisualizer.refresh();
            }
        }, interval);
    },

    stopAutoRefresh() {
        if (this.autoRefreshInterval) {
            clearInterval(this.autoRefreshInterval);
            this.autoRefreshInterval = null;
        }
    },
});
