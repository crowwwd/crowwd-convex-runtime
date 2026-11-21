#!/usr/bin/env bash
set -euo pipefail
base=72ce13c379fe6dbdfc305bc8ad699c5663c68c2a
candidate=$(git rev-parse HEAD)
registry=ghcr.io/crowwwd/crowwd-convex-runtime
out=${RUNNER_TEMP:?}/native-evidence
mkdir -p "$out"
cp crowwd/build/Dockerfile "$RUNNER_TEMP/runtime.Dockerfile"
trap 'git diff --binary > "$out/source-diff.patch"' EXIT
rustc --version --verbose > "$out/rustc.txt"
node --version > "$out/node.txt"
dpkg-query -W > "$out/os-packages.txt"
for arm in baseline candidate; do
  revision=$candidate
  if [ "$arm" = baseline ]; then revision=$base; fi
  git checkout --detach "$revision"
  test -z "$(git status --porcelain --untracked-files=no)"
  npm ci --prefix scripts
  just install-js
  sha256sum Cargo.lock npm-packages/pnpm-lock.yaml scripts/package-lock.json rust-toolchain .nvmrc mise.lock > "$out/$arm-locks.txt"
  export VERGEN_GIT_SHA="$revision"
  export VERGEN_GIT_COMMIT_TIMESTAMP=$(git show -s --format=%cI HEAD)
  if [ "$arm" = candidate ]; then
    cargo test --locked --release -p node_executor --lib local::tests -- --nocapture 2>&1 | tee "$out/native-tests.txt"
  fi
  cargo build --locked --release -p local_backend --bin convex-local-backend
  mkdir -p "$RUNNER_TEMP/$arm"
  cp target/release/convex-local-backend "$RUNNER_TEMP/$arm/"
  sha256sum "$RUNNER_TEMP/$arm/convex-local-backend" > "$out/$arm-binary.txt"
  ldd "$RUNNER_TEMP/$arm/convex-local-backend" > "$out/$arm-ldd.txt"
  ref="$registry:$arm-$candidate"
  docker build --build-arg ENGINE_COMMIT="$revision" -f "$RUNNER_TEMP/runtime.Dockerfile" -t "$ref" "$RUNNER_TEMP/$arm"
  docker run -d --name "smoke-$arm" -e DISABLE_BEACON=true -p 3210:3210 "$ref"
  ready=false
  for attempt in $(seq 1 90); do
    if curl -fsS http://localhost:3210/version > "$out/$arm-version.txt"; then ready=true; break; fi
    sleep 2
  done
  docker logs "smoke-$arm" > "$out/$arm-smoke.txt" 2>&1
  docker rm -f "smoke-$arm"
  test "$ready" = true
  docker push "$ref"
  docker inspect --format '{{json .RepoDigests}}' "$ref" > "$out/$arm-digests.json"
  printf '%s\n' "$revision" > "$out/$arm-commit.txt"
done
