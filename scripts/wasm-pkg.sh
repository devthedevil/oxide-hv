#!/usr/bin/env bash
# Rebuilds (or verifies) web/public/pkg, the committed wasm-pack output that
# Vercel deploys as-is without ever running wasm-pack itself.
#
# The compiled .wasm binary is NOT bit-for-bit reproducible across builds
# even with an identical toolchain and unchanged source (LTO/codegen
# ordering, wasm-opt internals), so drift detection can't diff the binary
# directly — it would be permanently "stale" even when nothing changed.
# Instead we hash the source inputs that determine the wasm's behavior and
# record that hash alongside the artifact in `.source-hash`.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

SOURCE_PATHS=(
  crates/engine/src
  crates/engine/Cargo.toml
  crates/wasm/src
  crates/wasm/Cargo.toml
  Cargo.toml
  Cargo.lock
)

compute_hash() {
  find "${SOURCE_PATHS[@]}" -type f | LC_ALL=C sort | xargs shasum -a 256 | shasum -a 256 | awk '{print $1}'
}

case "${1:-}" in
  build)
    (cd crates/wasm && wasm-pack build --target web --out-dir ../../web/public/pkg)
    rm -f web/public/pkg/.gitignore
    compute_hash > web/public/pkg/.source-hash
    echo "Rebuilt web/public/pkg. Review the diff and commit it (including .source-hash)."
    ;;
  verify)
    expected="$(compute_hash)"
    actual="$(cat web/public/pkg/.source-hash 2>/dev/null || true)"
    if [[ "$expected" != "$actual" ]]; then
      echo "::error::web/public/pkg is out of date with crates/engine + crates/wasm."
      echo "::error::Rebuild it and commit the result: ./scripts/wasm-pkg.sh build"
      exit 1
    fi
    echo "web/public/pkg is up to date with its source."
    ;;
  *)
    echo "usage: $0 {build|verify}" >&2
    exit 2
    ;;
esac
