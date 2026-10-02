import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Порт фиксированный: на него смотрит devUrl в tauri.conf.json.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
});
