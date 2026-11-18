FROM node:22-bookworm-slim

WORKDIR /app

# Local SDK packages live outside /app, so let their CommonJS imports resolve
# dependencies installed by npm ci in /app/node_modules.
ENV NODE_PATH=/app/node_modules

# The verifier depends on local packages from the repository. Keep these paths
# aligned with the file: dependencies in package.json and package-lock.json.
COPY passport_verification/package.json passport_verification/package-lock.json ./
COPY --exclude=node_modules zkpassport-packages/packages/zkpassport-sdk /zkpassport-packages/packages/zkpassport-sdk
COPY --exclude=node_modules zkpassport-packages/packages/zkpassport-ui /zkpassport-packages/packages/zkpassport-ui
COPY --exclude=node_modules zkpassport-packages/packages/registry-sdk /zkpassport-packages/packages/registry-sdk
COPY --exclude=node_modules zkpassport-packages/packages/zkpassport-utils /zkpassport-packages/packages/zkpassport-utils
RUN npm install --omit=dev --prefix /zkpassport-packages/packages/zkpassport-utils
RUN npm ci --include=dev --legacy-peer-deps
RUN npm install --prefix /app --no-save tsup@8.5.0 rimraf@6.0.1 @zkpassport/poseidon2@0.6.2

# The local SDK packages resolve imports from their own directory. Share the
# verifier's installed modules with them and expose the local workspace packages.
RUN mkdir -p /app/node_modules/@zkpassport \
    && ln -sfn /zkpassport-packages/packages/zkpassport-utils /app/node_modules/@zkpassport/utils \
    && ln -sfn /zkpassport-packages/packages/registry-sdk /app/node_modules/@zkpassport/registry \
    && ln -sfn /app/node_modules /zkpassport-packages/packages/zkpassport-sdk/node_modules \
    && ln -sfn /app/node_modules /zkpassport-packages/packages/registry-sdk/node_modules

# Build CommonJS and ESM outputs required by package exports.
RUN cd /zkpassport-packages/packages/zkpassport-utils \
    && /app/node_modules/.bin/tsup --config tsup.config.ts \
    && cd /zkpassport-packages/packages/registry-sdk \
    && /app/node_modules/.bin/tsup --config tsup.config.ts \
    && cd /zkpassport-packages/packages/zkpassport-sdk \
    && /app/node_modules/.bin/tsup --config tsup.config.ts

COPY passport_verification/src ./src

EXPOSE 3100
CMD ["npm", "run", "start"]
