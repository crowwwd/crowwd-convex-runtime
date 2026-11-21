# AGENTS.md — Crowwd runtime fork

Read CROWWD.md and crowwd/docs/CONTEXT.md before making runtime changes. Follow more-specific upstream instructions where present.

Scope: native self-hosted Convex runtime maintenance. Crowwd's Expo application and Convex business functions live in the sibling crowwd repository. Do not modify that project as part of runtime work without an active request.

- Keep changes narrowly scoped to executor lifecycle. Treat allocator work independently.
- Never treat the upstream main checkout, an open PR, or a mutable latest image tag as a validated release.
- Resolve the production image's upstream revision before preparing a release branch. Preserve pinned rollback artifacts and existing database/storage compatibility.
- Build a candidate once; promote the exact tested image digest. Record the application commit separately from the engine commit/image.
- Preview environment membership alone is not isolation: new test services need separate data and test credentials. Disable/mock external business side effects in the isolated test setup; do not casually change the existing preview's behavior.
- Cover active-request draining, concurrency, cancellations, long actions, streaming, worker failures and memory growth across deployments. Native patch results must not be inferred from supervisor prototype results.
- Keep production read-only during investigation/preview tests. A setup task does not authorize a production rollout; show the concrete tested candidate for user review before proceeding to a production release.
- Do not import env files, admin keys, raw authentication/config snapshots or private application/customer data from the sibling project. No secrets in commits, images, logs or reports.
- Review inherited GitHub workflows before enabling builds, publishing or deployment. Do not copy production credentials into generic upstream workflows.
- Preserve upstream license/history. Keep the patch documented so it can be removed when a compatible official fix is verified.
- Do not spawn sub-agents unless the active user explicitly requests them.
