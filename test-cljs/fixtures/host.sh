#!/bin/sh
set -eu
exec "$CLJ_GPUI_TEST_JS_RUNTIME" "$(dirname -- "$0")/host.cjs" "$@"
