# Maintenance and eventual return to official Convex

Own a small patch, not a permanently divergent Convex product. Keep upstream license/history and build guidance intact.

For each intentional ENGINE upgrade:

1. Review upstream releases, relevant fixes, compatibility/migration notes and patch changes.
2. Select a source commit and immutable baseline image; do not automatically track latest.
3. Bring the minimal patch forward, resolving conflicts and checking lifecycle semantics even if Git merges cleanly.
4. Rebuild with controlled toolchains/dependencies and rerun focused tests plus representative preview validation.
5. Record the new image digest, application revision, test results and rollback compatibility before promotion.

Application function pushes and npm SDK updates are separate from engine releases. They do not automatically require rebuilding this image; verify compatibility when upgrading either component.

Aim to contribute the native fix upstream. When an appropriate official fix is released, validate the official pinned image under the same workload, prove data/rollback compatibility, and retire the custom patch/image after review. An upstream PR being open or merged alone is not proof that the running image contains the fix.

Keep allocator/native Rust memory investigation separate from Node recycling. The observed large Rust footprint remains unresolved; do not promise a 50% reduction in the whole production service or bill from the synthetic Node result.

This bootstrap intentionally adds no automatic deployment workflow, registry secrets, custom runtime patch or executable release configuration. Those require a concrete candidate and reviewed destinations.
