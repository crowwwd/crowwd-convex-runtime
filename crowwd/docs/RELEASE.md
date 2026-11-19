# Preview-to-production release contract

No build or deployment is enabled by this bootstrap. Keep existing production/preview services unchanged until a concrete plan and candidate are reviewed.

## Release identity

Record all of the following for each candidate:

- Original production engine image digest and resolved upstream source commit.
- Upstream source commit, Crowwd patch commit and build-toolchain/dependency identities.
- Custom image registry reference with immutable sha256 digest.
- Intended Crowwd APPLICATION commit; engine and application commits are different things.
- Configuration/schema/migration assumptions, test report and pinned rollback image.

The current source checkout is upstream main for project setup only. Recover the production OCI revision and establish its source/build baseline before implementing a release. Do not silently upgrade the engine because current main is convenient.

## Implement and build

1. Review executor ownership, active invocation tracking and streaming completion semantics in the native source. Keep the patch minimal. Open upstream proposals are reference material, not proven fixes.
2. Build the unchanged source baseline and the native candidate with a controlled Linux builder. Follow upstream BUILD.md and self-hosted/docker-build/README.md; the backend Dockerfile is self-hosted/docker-build/Dockerfile.backend.
3. Validate the native patch with appropriate focused tests, including active calls, concurrency, disconnect/cancellation, long actions, worker failures and shutdown.
4. Build the candidate image once in CI or a controlled builder. Publish to an explicitly chosen registry destination (for example GHCR under the organization), then record its immutable digest. Publishing permissions/credentials are not configured yet.

## Isolated preview validation

Use independent services, data and credentials. Do not clone live payment/email/notification/AI secrets into the test environment. Mock/block external business side effects. Run the same known application commit and representative sanitized/seeded data on control and candidate.

Reproduce growth over repeated function deployments. Compare process RSS, live heaps/caches, cgroup memory, OOM events, errors, latency including tails/cold starts, and active-call completion. Run sustained representative traffic for hours, not only the short synthetic fixture. Agree explicit acceptance thresholds before calling the candidate accepted.

Separately test rollback and forward restoration using the same persistent test data. Verify module/application revision, image digests and data fingerprints. Record availability interruptions. An engine rollback cannot be assumed to reverse an incompatible database migration.

## Promotion

Show the candidate and results to the user before proposing production release. Once the concrete candidate is accepted and production deployment is authorized, promote the EXACT tested engine image digest and intended application commit. Do not rebuild for production and do not use latest.

Preview-specific credentials/endpoints remain different from production, but each difference must be deliberate and documented. If code, patch, image or material configuration changes after validation, retest it.

Railway can build a Dockerfile from GitHub, but independent preview/production builds do not prove artifact equality. For this release path, prefer build once, publish once, then configure both services to the same registry digest. No automatic production deployment on every Git push.

Maintain the original image digest and a data-compatible recovery procedure. Stop test compute afterward; verify actual deployment/replica state rather than trusting a scale command alone.

Railway source options: https://docs.railway.com/services
