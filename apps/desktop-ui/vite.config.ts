import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const apiBase = env.VITE_WORMHOLE_API_BASE;
  return {
    plugins: [react()],
    server: apiBase
      ? {
          proxy: {
            "/local": {
              target: apiBase,
              changeOrigin: true,
            },
          },
        }
      : undefined,
  };
});
