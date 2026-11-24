/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** sync server URL baked into the build (e.g. the Tailscale Funnel address); the connect form starts with it */
  readonly VITE_SYNC_SERVER?: string;
}
