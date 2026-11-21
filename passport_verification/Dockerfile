FROM node:22-bookworm-slim

WORKDIR /app

COPY passport_verification/package.json passport_verification/package-lock.json ./
RUN npm ci --include=dev

COPY passport_verification/src ./src

EXPOSE 3100
CMD ["npm", "run", "start"]
