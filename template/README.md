# clj-gpui app template

Copy this directory to start a native GPUI app driven by JVM Clojure.

## Run

From this directory (clj-gpui needs to be in a sibling directory unless you use a git dependency):

```bash
clj -M:dev
```

Requires a Rust toolchain. The first run builds the native host (`cargo build --release` in the library's `host/`).

Edit `src/my/app.clj` and save. The window rerenders; `defonce` / `r/atom` state is kept. nREPL prints on startup (default port 7888, also written to `.nrepl-port`).

## Git dependency

When you do not have clj-gpui checked out in a sibling directory, replace the local root in `deps.edn`:

```clojure
{:deps {clj-gpui/clj-gpui {:git/url "https://github.com/violetpurpleish/clj-gpui.git"
                           :git/sha "REPLACE_WITH_SHA"}}}
```

You can also point at a built host binary with `CLJ_GPUI_BIN`, or at a library checkout with `CLJ_GPUI_ROOT`.

## Package

Customize the included `gpui.edn`, then run `clj -X:build package` on the OS you are shipping for. That uses `gpui.prod`: no nREPL, no source watcher, no Cargo or development chrome at runtime. Use `-X` so clj-gpui stays on the classpath.

On macOS this writes `target/package/my-app.app`. Linux writes an AppImage and a `.deb`. The bundle includes a reduced Java runtime and the native host, so end users do not need Clojure, Java, or Rust installed.
