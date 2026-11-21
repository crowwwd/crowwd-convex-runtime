# Native candidate — review before building

Engine base: `72ce13c379fe6dbdfc305bc8ad699c5663c68c2a`, recovered from the recorded production image linux/amd64 OCI config label. The unchanged baseline branch `codex/production-baseline-72ce13c` points at this upstream commit. Candidate branch: `codex/native-executor-recycling`. No allocator, database, storage, or application code changes.

## Implementation

Changes are limited to `crates/node_executor/src/local.rs` and two disabled-by-default settings in `crates/common/src/knobs.rs`: `NODE_EXECUTOR_IDLE_TIMEOUT_SECONDS` and `NODE_EXECUTOR_MAX_LIFETIME_SECONDS`.

Age retirement removes the current admission slot. Existing owned invocations retain their worker and files through response completion or the existing transport timeout; fresh calls lazily create a replacement. Idle retirement requires no active invocations and measures time from the last completion. A Weak reaper avoids keeping the executor alive. Late failure/exiting responses only remove their own worker's slot.

Invocation tasks continue draining after caller cancellation or log receiver closure. Actions are not retried. Existing shutdown semantics are retained. Tests use real Node child processes and fragmented HTTP responses for disabled settings, idle/age policy, identity-safe replacement, cancellation draining and concurrent calls.

## Build and review destination

Proposed dedicated Linux CI runs on the runtime fork and publishes only to `ghcr.io/crowwwd/crowwd-convex-runtime`, with commit-specific tags and recorded immutable digests. It must not import Crowwd application source, customer data, env files, keys or business credentials. Application source stays local or on dedicated private test services. The tested image is built once and reused by digest.

Inherited release workflows use upstream runner/cache infrastructure and upstream registry paths. Do not invoke them. Use a dedicated build workflow without production credentials and without Railway deployment permissions. Keep the original production runtime image as the runtime filesystem/Node/key-generator base; replace only the source-built engine binary identically in the unchanged and candidate arms. A rebuilt baseline is not claimed bit-identical to the historical production artifact.

## Status and remaining gates

Source draft only. Native compilation, formatting and all runtime tests are pending; no new image digest exists. Local machine has no Rust, Docker or WSL. Automatic approval review rejected a combined command that would prepare/push a GitHub workflow and publish GHCR images because export to those external destinations had not been explicitly authorized. That command did not execute.

Required before acceptance: native tests; unchanged and candidate builds on the same controlled Linux toolchain; separate test services, data and credentials; blocked/mocked external effects; same exact intended application commit; active calls, cancellations, long actions, streaming, worker failure and shutdown checks; repeated-deployment memory comparison; sustained representative traffic and latency; rollback/forward restore with fingerprints and pinned images. No native conclusions may be inferred from supervisor prototype measurements.

Production and existing preview have not been mutated. No production rollout is proposed.
