# Build from the repo root:
#   docker build -f apps/server/Dockerfile -t outliner .
# Runs the sync server and serves the web build (apps/web/dist) if it exists.

FROM node:24-slim AS build
WORKDIR /app
COPY . .
RUN npm ci --no-audit --no-fund \
 && npm run build --workspaces --if-present \
 && npm prune --omit=dev --no-audit --no-fund \
 && mkdir -p apps/web/dist

FROM node:24-slim
ENV NODE_ENV=production \
    PORT=8080 \
    OUTLINER_DB=/data/outliner.db \
    OUTLINER_STATIC=/app/apps/web/dist
WORKDIR /app
COPY --from=build /app/package.json ./
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/packages/core ./packages/core
COPY --from=build /app/apps/server ./apps/server
COPY --from=build /app/apps/web/dist ./apps/web/dist
RUN mkdir -p /data && chown node:node /data
USER node
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s CMD node -e "fetch('http://127.0.0.1:'+process.env.PORT+'/api/health').then(r=>process.exit(r.ok?0:1),()=>process.exit(1))"
CMD ["node", "apps/server/src/main.ts"]
