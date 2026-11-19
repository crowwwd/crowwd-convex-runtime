# GitHub organization setup

The existing app's remote uses the organization slug crowwwd (three w characters). Confirm that this is the intended owner in GitHub.

## Recommended: a real organization fork

1. Open https://github.com/convex/convex-backend/fork while signed in.
2. Select crowwwd as Owner. If it is unavailable, your account may lack organization repository/fork permissions.
3. Name the fork crowwd-convex-runtime and create it. Review visibility before publishing these context files; a public upstream fork is normally public. Do not include secrets or private raw investigation reports.
4. In PowerShell, run the commands below. This local repository is already cloned, so do not clone another copy into it.

```powershell
cd C:\Users\Mann\Documents\Apps\crowwd-new\crowwd-convex-runtime
git remote -v
git remote add origin git@github.com:crowwwd/crowwd-convex-runtime.git
git add AGENTS.md CROWWD.md crowwd
git commit -m "Document Crowwd runtime investigation and release process"
git push -u origin crowwd-runtime-bootstrap
```

Open a pull request in YOUR organization's fork from crowwd-runtime-bootstrap to its main branch. Review the context commit; do not accidentally open it against the upstream Convex repository. No force push is needed. The local upstream remote already points to the official repository; origin should point to the organization fork.

GitHub fork instructions: https://docs.github.com/en/pull-requests/how-tos/work-with-forks/fork-a-repo

## What happens later

Keep the org fork's main close to upstream. Develop the native fix on a branch based on the explicitly selected production source revision, with a documented release/patch branch strategy. The bootstrap context branch is based on current upstream main and is not the production patch base. Update context on the release branch as needed.

Review inherited .github/workflows before enabling them. Set up a reviewed build/test pipeline and explicit GHCR publishing destination/permissions. Do not enable an inherited release workflow with production credentials.

Railway deployment comes AFTER a native candidate exists and passes preview validation. See RELEASE.md. Keep Crowwd application code in its existing repository.

## If a private company repository is required

GitHub's public-fork visibility rules may not fit that requirement. A separate non-fork mirror with an upstream remote is another design, subject to upstream license requirements and organization policy. Decide this explicitly before creating a different remote/history arrangement; the instructions above create a genuine organization fork.
