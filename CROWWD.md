# Crowwd Convex runtime

Start here for Crowwd-specific work. This repository is a local checkout of the upstream Convex backend, prepared for an organization fork. Upstream README.md, source, build tooling and license are retained.

## Status

- Setup only: no native memory fix has been applied, no custom release has been built, and nothing has been deployed from this project.
- GitHub organization fork and origin remote are not created yet. See crowwd/docs/GITHUB.md.
- The bootstrap branch is crowwd-runtime-bootstrap. Its upstream snapshot is d77e88d30d5d6ce4b64120a847c9805fc345fc46, not a validated production release base.
- Existing Crowwd application code remains in ../crowwd. This repository owns the self-hosted Convex ENGINE, not Expo screens or application functions.

## Read in this order

1. crowwd/docs/CONTEXT.md — investigation, measured results and open questions.
2. crowwd/docs/GITHUB.md — create the organization fork and publish this branch.
3. crowwd/docs/RELEASE.md — build once, validate in isolated preview, promote exact artifacts.
4. crowwd/docs/MAINTENANCE.md — keep the patch small and eventually return to upstream.
5. crowwd/pins.json — historical image identities, clearly separated from unbuilt native candidates.

## Intended direction

Maintain a minimal native executor-lifecycle patch close to upstream until an appropriate official fix is available and tested. The supervisor prototype demonstrated the mechanism but is not the production implementation chosen for this repository. The jemalloc preload experiment did not demonstrate a memory improvement.

The next engineering task is to identify the source revision of the recorded PRODUCTION image, create an unpatched baseline build, then implement/review a native recycling candidate and test both with the same known Crowwd application revision. Do not use this checkout's latest main as an implicit engine upgrade.

Upstream: https://github.com/convex/convex-backend
