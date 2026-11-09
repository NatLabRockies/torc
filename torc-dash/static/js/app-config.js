/**
 * Torc Dashboard - Configuration Tab
 * Settings and server management
 */

Object.assign(TorcDashboard.prototype, {
    // ==================== Configuration Tab ====================

    setupSettingsTab() {
        document.getElementById('btn-save-settings')?.addEventListener('click', () => {
            this.saveSettings();
        });

        document.getElementById('btn-test-connection')?.addEventListener('click', async () => {
            const apiUrl = document.getElementById('api-url')?.value;
            if (apiUrl) {
                api.setBaseUrl(apiUrl);
            }
            await this.testConnection();
        });

        document.getElementById('dark-mode')?.addEventListener('change', (e) => {
            if (e.target.checked) {
                document.body.classList.add('dark-mode');
            } else {
                document.body.classList.remove('dark-mode');
            }
        });

        // Server management
        document.getElementById('btn-start-server')?.addEventListener('click', () => {
            this.startServer();
        });

        document.getElementById('btn-stop-server')?.addEventListener('click', () => {
            this.stopServer();
        });

        // Check server status on init
        this.checkServerStatus();
    },

    async checkServerStatus() {
        const status = await api.getServerStatus();
        this.updateServerStatusUI(status);
    },

    updateServerStatusUI(status) {
        const panel = document.getElementById('server-status-panel');
        const dot = document.getElementById('server-status-dot');
        const text = document.getElementById('server-status-text');
        const startBtn = document.getElementById('btn-start-server');
        const stopBtn = document.getElementById('btn-stop-server');
        const outputDiv = document.getElementById('server-output');
        const outputContent = document.getElementById('server-output-content');

        if (status.running && status.managed) {
            panel?.classList.remove('stopped');
            panel?.classList.add('running');
            dot?.classList.remove('disconnected');
            dot?.classList.add('connected');
            if (text) text.textContent = `Server running (PID: ${status.pid}, Port: ${status.port})`;
            if (startBtn) startBtn.disabled = true;
            if (stopBtn) stopBtn.disabled = false;

            // Show output
            if (outputDiv && status.output_lines?.length > 0) {
                outputDiv.style.display = 'block';
                if (outputContent) {
                    outputContent.textContent = status.output_lines.join('\n');
                }
            }
        } else if (status.managed && !status.running) {
            // Server was managed but has stopped
            panel?.classList.remove('running');
            panel?.classList.add('stopped');
            dot?.classList.remove('connected');
            dot?.classList.add('disconnected');
            if (text) text.textContent = 'Server stopped (was managed)';
            if (startBtn) startBtn.disabled = false;
            if (stopBtn) stopBtn.disabled = true;
        } else {
            panel?.classList.remove('running');
            panel?.classList.remove('stopped');
            dot?.classList.remove('connected');
            dot?.classList.add('disconnected');
            if (text) text.textContent = 'No managed server';
            if (startBtn) startBtn.disabled = false;
            if (stopBtn) stopBtn.disabled = true;
            if (outputDiv) outputDiv.style.display = 'none';
        }
    },

    async startServer() {
        const port = parseInt(document.getElementById('server-port')?.value) || 8080;
        const database = document.getElementById('server-database')?.value || '';
        const logLevel = document.getElementById('server-log-level')?.value || 'info';

        const startBtn = document.getElementById('btn-start-server');
        if (startBtn) {
            startBtn.disabled = true;
            startBtn.textContent = 'Starting...';
        }

        const result = await api.startServer({
            port,
            database: database || null,
            log_level: logLevel,
        });

        if (result.success) {
            this.showToast(result.message, 'success');

            // Update API URL to point to the new server
            const newApiUrl = `http://localhost:${port}/torc-service/v1`;
            document.getElementById('api-url').value = newApiUrl;
            api.setBaseUrl(newApiUrl);

            // Wait a moment for server to start, then check status
            setTimeout(() => {
                this.checkServerStatus();
                this.testConnection();
            }, 1000);
        } else {
            this.showToast('Failed to start server: ' + result.message, 'error');
            if (startBtn) {
                startBtn.disabled = false;
            }
        }

        if (startBtn) {
            startBtn.textContent = 'Start Server';
        }
    },

    async stopServer() {
        const stopBtn = document.getElementById('btn-stop-server');
        if (stopBtn) {
            stopBtn.disabled = true;
            stopBtn.textContent = 'Stopping...';
        }

        const result = await api.stopServer();

        if (result.success) {
            this.showToast(result.message, 'success');
        } else {
            this.showToast('Failed to stop server: ' + result.message, 'error');
        }

        // Check status
        await this.checkServerStatus();
        await this.testConnection();

        if (stopBtn) {
            stopBtn.textContent = 'Stop Server';
        }
    },
});
