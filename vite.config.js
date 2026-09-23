import { defineConfig } from "vite";
export default defineConfig({
  base: "./",
  server: {
    port: 1431,
    strictPort: true,
    watch: {
      ignored: ["**/.build/**", "**/android/**", "**/src-tauri/**", "**/artifacts/**", "**/dist/**"],
      awaitWriteFinish: { stabilityThreshold: 150, pollInterval: 50 },
    },
  },
  build: { target: "es2021", sourcemap: false },
});
