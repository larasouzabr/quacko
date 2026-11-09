import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Porta fixa para o Tauri achar o servidor de desenvolvimento.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "safari15", outDir: "dist" },
  test: { environment: "node" },
});
