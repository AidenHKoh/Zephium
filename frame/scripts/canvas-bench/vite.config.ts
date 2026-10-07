import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { alias } from "../../aliases";

/** Pictures come from the seeded photos, not the native media route. */
const media: Plugin = {
  name: "canvas-bench-media",
  enforce: "pre",
  load(id) {
    if (!id.endsWith("/src/domain/resources/media.ts")) return null;
    return `
      export function pageFrameUrl() { return null; }
      export function mediaUrl(_profile, digest) { return globalThis.__benchPictures.get(digest) ?? null; }
      export function mediaSize(bytes) { return bytes + " B"; }
    `;
  },
};

const PHOTOS = fileURLToPath(new URL("../../node_modules/.canvas-bench/photos", import.meta.url));
/** Serves the photos where they were made instead of copying them into every build. */
const photos: Plugin = {
  name: "canvas-bench-photos",
  configurePreviewServer(server) {
    server.middlewares.use((request, response, next) => {
      const name = /^\/photos\/(photo-\d+\.jpg)$/u.exec(request.url ?? "")?.[1];
      const file = name ? join(PHOTOS, name) : "";
      if (!file || !existsSync(file)) return next();
      response.setHeader("Content-Type", "image/jpeg");
      response.end(readFileSync(file));
    });
  },
};

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  publicDir: false,
  resolve: { alias },
  plugins: [media, photos, svelte(), tailwindcss()],
  logLevel: "warn",
  build: {
    target: "es2022",
    outDir: fileURLToPath(new URL("../../node_modules/.canvas-bench-dist", import.meta.url)),
    emptyOutDir: true,
  },
  preview: { port: Number(process.env.CANVAS_BENCH_PORT ?? 4721), strictPort: true },
});
