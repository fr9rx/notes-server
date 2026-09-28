import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { compression } from "vite-plugin-compression2";
import { defineConfig } from "vitest/config";

// `npm run dev` proxies the API and images to a local notes-server
// (cargo run → https://localhost:3443, self-signed, hence secure: false).
const backend = process.env.NOTES_BACKEND ?? "https://localhost:3443";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    // The Rust server serves these .br/.gz files as-is (src/web.rs), so the
    // board never compresses anything at request time.
    compression({ algorithms: ["brotliCompress", "gzip"], threshold: 1024 }),
  ],
  server: {
    proxy: {
      "/api": { target: backend, secure: false, changeOrigin: true },
      "/files": { target: backend, secure: false, changeOrigin: true },
    },
  },
  build: {
    target: "es2022",
    cssCodeSplit: true,
    sourcemap: false,
    chunkSizeWarningLimit: 400,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});
