#!/usr/bin/env bash
# Build the Cloudflare Worker with worker-build, optionally enabling the
# tul_cv feature (browser-side converter tools).
#
# Usage: TUL_CV_FEATURES=tul_cv scripts/build-worker.sh [worker-build args...]
#
# worker-build invokes `cargo build` without --features, so the feature is
# passed through RUSTFLAGS as --cfg feature="tul_cv" (equivalent to cargo
# features at the cfg level). The base target flags from .cargo/config.toml
# are preserved because RUSTFLAGS overrides target rustflags.
set -euo pipefail

BASE_RUSTFLAGS='--cfg getrandom_backend="wasm_js" -C target-feature=+bulk-memory,+mutable-globals,+nontrapping-fptoint,+sign-ext,+reference-types'

if [[ "${TUL_CV_FEATURES:-}" == *"tul_cv"* ]]; then
  RUSTFLAGS="$BASE_RUSTFLAGS --cfg feature=\"tul_cv\"" worker-build "$@"
else
  RUSTFLAGS="$BASE_RUSTFLAGS" worker-build "$@"
fi
