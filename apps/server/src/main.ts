import { loadEnv } from './config.ts';
import { startServer } from './server.ts';

let env;
try {
  env = loadEnv();
} catch (err) {
  console.error(`[outliner] ${(err as Error).message}`);
  process.exit(1);
}

const srv = await startServer(env.config, env.port, env.host);
console.log(
  `[outliner] listening on http://${env.host}:${srv.port} (db: ${env.config.store.path}` +
    (env.config.staticDir ? `, static: ${env.config.staticDir}` : '') + ')',
);

let closing = false;
const shutdown = (sig: string) => {
  if (closing) return;
  closing = true;
  console.log(`[outliner] ${sig}, shutting down`);
  srv.close().then(
    () => process.exit(0),
    (err: unknown) => {
      console.error(err);
      process.exit(1);
    },
  );
};
process.on('SIGINT', () => shutdown('SIGINT'));
process.on('SIGTERM', () => shutdown('SIGTERM'));
