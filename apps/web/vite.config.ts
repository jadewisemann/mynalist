import { defineConfig } from 'vite';

export default defineConfig({
  base: './',
  server: {
    host: true,
    port: 5173,
    proxy: { '/api': { target: 'http://localhost:8787', ws: true } },
  },
  build: { target: 'es2022', outDir: 'dist' },
});
