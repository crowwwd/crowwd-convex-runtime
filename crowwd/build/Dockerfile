FROM ghcr.io/convex/convex-backend@sha256:294944b33dc55e8f68b2feda11c5e7b195f7778b9ad2951f1bea2744ad392e5a
ARG ENGINE_COMMIT
LABEL org.opencontainers.image.source="https://github.com/crowwwd/crowwd-convex-runtime"
LABEL org.opencontainers.image.revision="${ENGINE_COMMIT}"
COPY --chmod=744 convex-local-backend /convex/convex-local-backend
