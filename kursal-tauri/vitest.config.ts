import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";
import { emojiIndexPlugin } from "./emoji-index-plugin.js";

const resolvePath = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  plugins: [emojiIndexPlugin(), svelte()],
  define: {
    __TERMS_UPDATED__: JSON.stringify("2026-07-27"), // just a testing value
  },
  server: {
    fs: {
      allow: [resolvePath("..")],
    },
  },
  resolve: {
    conditions: ["browser"],
    alias: [
      {
        find: "$app/environment",
        replacement: resolvePath("./src/test/stubs/app-environment.ts"),
      },
      { find: "$lib", replacement: resolvePath("./src/lib") },
    ],
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
