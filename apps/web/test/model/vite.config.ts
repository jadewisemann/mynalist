// Build of the model page (not part of the app): `node qa/model.mjs` builds and drives it.
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  base: './',
  build: { target: 'es2022', outDir: '/tmp/outliner-modeltest', emptyOutDir: true, minify: false },
  preview: { port: 5176, strictPort: true },
  server: { fs: { allow: [fileURLToPath(new URL('../../../../', import.meta.url))] } },
});
