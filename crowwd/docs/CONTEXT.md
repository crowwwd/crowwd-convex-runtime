# Investigation context — 19 November 2026

## Goal and architecture

Crowwd self-hosts Convex on Railway, with separate production and preview environments, PostgreSQL and persistent/object storage. The application uses Expo React Native, TypeScript, Expo Router, Tamagui and Convex. The app's backend functions live in the sibling application's convex/ directory; ordinary function deployments do not rebuild the Convex engine container.

This project is intended to maintain a small native engine patch until Convex provides an appropriate upstream fix. The user wants: implement here, validate in isolated preview with the exact intended application code and engine artifact, review the candidate, then promote those same artifacts to production. Production is not a test environment.

## Evidence already gathered

- Production logs showed Node's JavaScript heap limit being reached during function analysis on 15 November 2026 at 17:41:58 IST. Convex replaced its Node executor immediately afterward.
- The backend Rust process and the Node executor were separate large memory owners. A production sample showed roughly 6.7 GiB in Rust and 3 GiB in Node. The large Rust footprint is not explained by the Node fix.
- The actual deployed executor source used package-specific ESM import paths during analysis and envHash-qualified imports during actions. File/package cache cleanup does not itself unload Node's imported module graphs.
- Restarting preview and repeatedly calling one unchanged module plateaued; it did not reproduce cross-deployment growth.
- Subsequent isolated tests on the pinned preview engine did reproduce growth across twelve function deployments. A synthetic module retained 40,000 bounded objects per generation. Baseline Node heap rose approximately 26.3 MiB per deployment in the same PID.

## Completed isolated comparison

| Arm | Node RSS including supervisor | Rust backend RSS |
| --- | ---: | ---: |
| Baseline | 439.7 MiB | 144.3 MiB |
| Experimental recycling supervisor v2 | 218.1 MiB | 143.4 MiB |
| Jemalloc preload only | 458.3 MiB | 262.5 MiB |

Recycling reduced aggregate Node RSS by 48–50% across the synthetic observations. Nine long actions per arm completed during boundary checks. The primary workloads plus two paired latency checks recorded zero errors across 2,374 measured calls. Remote tail latency was variable; this is not production SLO certification. Both candidate rollback/restoration round trips preserved synthetic data and module hashes, with deployments ready in about 20–25 seconds per transition. Brief non-200 polls occurred only on test endpoints.

The supervisor was a proof of concept, not a native Convex patch. Its final version drained worker responses after caller disconnect and passed streaming/concurrency/disconnect/shutdown regression tests. Do not present those results as validation of a future Rust patch. The jemalloc experiment used LD_PRELOAD, not the compiled upstream allocator proposal, and demonstrated no savings here.

Test services were stopped after verification. A scale-to-zero attempt fell back to a default region; explicit deployment removal was needed to stop compute. Historical test artifact IDs are not assumed to remain available forever.

## Application source recovery limitation

Preview's last recorded application push was 23 October 2026 at 10:21:16 IST, with no Git SHA. Candidate c5e78abd64bc223e98fdce0203d5e51c7378c2f3 was committed 46 seconds earlier. It matched 349 named modules after generated-path normalization; two still differed. It is an inferred candidate, not an exact verified restoration baseline. Current app HEAD must not silently replace the existing preview deployment.

The prior comparison therefore used new isolated services, fresh SQLite data and synthetic functions. It did not validate the complete Crowwd PostgreSQL/S3 workload. For the next phase choose and record a known intended Crowwd release commit, and deploy that same commit to both isolated control and candidate services.

## Upstream leads (status must be rechecked before use)

- https://github.com/convex/convex-backend/issues/597 — source-package module retention.
- https://github.com/convex/convex-backend/issues/563 — separate backend/Node retention concerns.
- https://github.com/convex/convex-backend/pull/574 — proposed native executor retirement; open at the time of investigation. Review/adapt; do not blindly apply.
- https://github.com/convex/convex-backend/pull/598 — proposed allocator change; independently unvalidated here.

Pinned engine identities are in ../pins.json. Production and preview used different digests. A candidate built on the preview base is not automatically a production-compatible engine release.

## Existing local evidence

The original investigation remains in ../crowwd/reports/convex-memory/ and its fix-test/ directory. Start with FINDINGS.md and fix-test/CANDIDATE-REVIEW.md. Raw directories also contain private/local credentials and metadata: do not copy or publish them wholesale. This context is deliberately curated without keys, raw account snapshots or customer data.
