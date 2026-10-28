// Build of the render test page (not part of the app): `node apps/web/e2e/render.mjs` builds and drives it.
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  base: './',
  build: { target: 'es2022', outDir: '/tmp/outliner-rendertest', emptyOutDir: true, minify: false },
  preview: { port: 5175, strictPort: true },
});
