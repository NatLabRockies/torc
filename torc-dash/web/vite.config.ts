import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// `npm run dev` proxies API calls to a running torc-dash (default http://127.0.0.1:8090).
const dash = process.env.TORC_DASH_URL ?? "http://127.0.0.1:8090";

export default defineConfig({
  plugins: [svelte()],
  // torc-dash embeds torc-dash/static/ and serves it under /static/.
  base: "/static/app/",
  build: {
    outDir: "../static/app",
    emptyOutDir: true,
    chunkSizeWarningLimit: 5000,
  },
  server: {
    port: 5173,
    proxy: {
      "/torc-service": dash,
      "/api": dash,
      "/static/images": dash,
    },
  },
});
