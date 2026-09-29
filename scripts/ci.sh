#!/usr/bin/env bash
# Headless checks used by GitHub Actions. Assumes cargo, java, Clojure CLI, Node.js, and installed npm dependencies.
# Builds a debug host so protocol-test does not pay for `cargo build --release`.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

echo "==> cargo test"
cargo test --locked --manifest-path host/Cargo.toml

echo "==> cargo clippy"
cargo clippy --locked --manifest-path host/Cargo.toml --all-targets --test rendering -- -D warnings

echo "==> cargo build (debug host)"
cargo build --locked --manifest-path host/Cargo.toml

target_dir="${CARGO_TARGET_DIR:-}"
if [[ -z "$target_dir" ]]; then
  metadata="$(cargo metadata --locked --manifest-path host/Cargo.toml --format-version 1 --no-deps)"
  target_dir="$(printf '%s\n' "$metadata" | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
fi
bin="${CLJ_GPUI_BIN:-$target_dir/debug/clj-gpui}"
if [[ ! -x "$bin" ]]; then
  echo "missing executable host binary: $bin" >&2
  exit 1
fi
export CLJ_GPUI_BIN="$bin"

echo "==> clojure -M:test"
clojure -M:test

echo "==> clojure -M:cljfmt check"
clojure -M:cljfmt check

echo "==> clojure -M:protocol-test"
clojure -M:protocol-test

echo "==> ClojureScript compile"
node node_modules/shadow-cljs/cli/runner.js compile test protocol

echo "==> ClojureScript tests"
node target/cljs-tests.js

echo "==> ClojureScript protocol test"
node target/cljs-protocol.js

echo "==> Optimized ClojureScript protocol test"
node node_modules/shadow-cljs/cli/runner.js release protocol
node target/cljs-protocol.js
