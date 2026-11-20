import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

// While developing, the UI runs on Vite's port and API calls are forwarded to
// the Rust server. In a release build the server serves both itself.
const server = `http://127.0.0.1:${process.env.BINGE_PORT ?? "8686"}`;

/**
 * The API token to show the server with every forwarded call, so the app can
 * be looked at without signing in. Only when asked for with
 * `BINGE_DEV_AS_TOKEN=1`, and only while developing: the token is the one
 * kept as `BINGE_LOCAL_API_TOKEN` in the `.env` at the root, which git
 * ignores. It stays in this process and never reaches the page.
 *
 * Anything that can reach Vite's port is then that token's person, so this
 * is for a machine of one's own, where Vite listens on localhost only.
 */
function devToken(mode: string): string | undefined {
  if (!process.env.BINGE_DEV_AS_TOKEN) {
    return undefined;
  }
  const token = loadEnv(mode, "..", "BINGE_").BINGE_LOCAL_API_TOKEN;
  if (!token) {
    throw new Error(
      "BINGE_DEV_AS_TOKEN is set, but the .env at the root has no BINGE_LOCAL_API_TOKEN",
    );
  }
  return token;
}

export default defineConfig(({ mode }) => {
  const token = devToken(mode);
  return {
    plugins: [tanstackRouter({ target: "react", autoCodeSplitting: true }), react(), tailwindcss()],
    // The folder above `dist` is committed, so the server's build script always
    // has something to watch for a new build.
    build: { outDir: "out/dist" },
    server: {
      proxy: {
        "/api": { target: server, headers: token ? { Authorization: `Bearer ${token}` } : {} },
      },
    },
  };
});
