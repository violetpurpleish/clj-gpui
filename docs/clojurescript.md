# ClojureScript and shadow-cljs

clj-gpui can run application code in either JVM Clojure or **ClojureScript on Bun**. Both use the same native Rust host, GPUI Kit widgets, reactive atoms, theme registration and version 11 JSON protocol. No WebView is involved.

## Why this uses Bun and the native host rather than GPUI Shell

Investigated against [GPUI Shell's documentation](https://gpui-kit.com/shell/) on 2026-09-29:

- Shell embeds QuickJS, not Node.js. It provides neither Node built-ins nor a DOM. Compiling ClojureScript into JavaScript alone would not supply the environment npm libraries expect.
- Its [dependency loader](https://gpui-kit.com/shell/dependencies/) resolves Shell packages from Git; it does not implement the npm ecosystem. Selected bundled pure JavaScript libraries could work, but arbitrary Node packages would require a separate compatibility layer.
- Shell exposes unstyled `gpui-base` elements. clj-gpui's existing styled `gpui-component` widgets are a different rendering layer. Adopting Shell would require another widget adapter and would not automatically preserve this API.
- Shell's published documentation describes the interface as an unpublished, unstable M0 baseline. Its sandbox disables dynamic code evaluation, another constraint for development tooling.

Running a shadow-cljs `:node-script` in a Node-compatible runtime against the existing native protocol meets the intended requirement with substantially less duplication. Shell remains a possible separate integration for sandboxed in-process plugins; it is not a dependency of this implementation. A working ClojureScript-on-QuickJS Shell build has **not** been established here.

**The default workflow now uses Bun for both package management and execution.**
Bun is independent of Node.js. shadow-cljs's `:node-script` target and the
`gpui.node` namespace describe the Node-compatible API surface; they do not
require the Node binary. The JVM application path is unchanged.

**npm compatibility is bounded by Bun's compatibility.** Pure JavaScript packages,
Node built-ins and supported Node-API addons work subject to their normal platform
requirements. Bun is not fully Node-compatible, particularly for some native
addons and APIs; consult its [compatibility table](https://bun.sh/docs/runtime/nodejs-compat).
Browser-only packages expecting a DOM cannot render through GPUI. Applications
needing a Node-specific feature can still run the compiled script manually under
Node, but the template, CI workflow and packager use Bun.

## Run the example

Requirements: Bun 1.4.2+, Java 21+ for shadow-cljs compilation, and the usual clj-gpui host build/display requirements. Java and shadow-cljs are build/development tools; a released application runs with bundled Bun, its runtime npm dependencies and the native host, without a JVM.

From the repository root:

```sh
bun install --frozen-lockfile
bun run cljs:compile
bun run cljs:start
```

The compiler CLI and app are explicitly run with Bun. Node.js and the npm CLI are not required. The repository and template commit `bun.lock` for frozen installs.

The example imports the npm package `dayjs` and the Node built-in `node:os`. It has a counter, editable input and a timer that updates the native window. To use an already-built host:

```sh
CLJ_GPUI_BIN="$PWD/host/target/debug/clj-gpui" bun run cljs:start
```

Without `CLJ_GPUI_BIN`, a development build runs `cargo build --locked --release` in the clj-gpui checkout. Cargo's JSON output locates the actual executable, including custom target directories. An external app supplies `:root` or `CLJ_GPUI_ROOT`; its own working directory remains the application's directory when launching the host.

## Your project

Start from [`templates/cljs/`](../templates/cljs/) for a complete app with
shadow-cljs, Bun, hot reload and macOS/Linux packaging. The JVM starter lives in
[`templates/clj/`](../templates/clj/).

For a sibling checkout, add the library's source path to `shadow-cljs.edn`:

```clojure
{:source-paths ["src" "../clj-gpui/src"]
 :builds
 {:app {:target :node-script
        :output-to "target/app.js"
        :main my.app/main
        :devtools {:after-load my.app/reload!}}}}
```

Alternatively, use shadow-cljs's `:deps true` integration and an ordinary local/git dependency in `deps.edn`. The library's shared sources are `.cljc`; no npm publication of clj-gpui is required. Install `shadow-cljs` and application npm dependencies in your own `package.json`.

```clojure
(ns my.app
  (:require [gpui.node :as node]
            [gpui.ratom :as r]
            [gpui.ui :as ui]
            ["dayjs" :as dayjs]))

(defonce !count (r/atom 0))

(defn app []
  (ui/window {:title "ClojureScript" :chrome :app}
    (ui/label (str @!count " · " (.format (dayjs) "YYYY-MM-DD")))
    (ui/button "+" #(swap! !count inc))))

(defn reload! [] (node/reload!))

(defn main []
  (node/start! {:app #(app)
               :app-id "my.app/app"
               :root "../clj-gpui"}))
```

Pass a thunk, `#(app)`, so the runtime reads the current definition after reloading. `start!` returns a Promise that resolves when the host connects. Calling it again updates the root without opening another window. `node/stop!` closes the host. Closing the native host normally exits the JavaScript runtime with the host's status; embedders can supply `:on-exit (fn [code] ...)` to retain their JavaScript process. Startup exceptions and connection failures should be handled by the application's entrypoint.

## Hot reload and REPL

In one terminal:

```sh
bun run cljs:watch
```

After the first compile, in another:

```sh
bun run cljs:start
```

shadow-cljs reloads successful compilations into the running Bun process, then calls `counter.app/reload!` to request a native render. `defonce` atoms and the native window survive. Runtime render errors appear in the native window; compiler errors appear in shadow-cljs's terminal and the last successful UI remains visible. The host's reload command only rerenders the current compiled code; shadow-cljs owns compilation and code loading.

Connect to the live application with `bun --no-install node_modules/shadow-cljs/cli/runner.js cljs-repl counter` (or use your editor's shadow-cljs integration). `ui/request-render!` refreshes a redefined view, and reactive atom changes request it automatically. The host does not advertise a JVM nREPL port for ClojureScript apps; set `:chrome :app` to hide its JVM-oriented development footer.

## API and asynchronous work

- `gpui.ui`, `gpui.ratom`, `gpui.theme` and `gpui.platform` share their implementations across both languages. All current widget constructors are available. The old dynamically interned `gpui.core` compatibility namespace is JVM-only; use `gpui.ui`.
- A callback receives the same Clojure data shapes as on the JVM, including `nil` for explicit JSON null. Callback IDs remain monotonic and stale events cannot invoke a later handler. Native batches share a callback generation.
- Event handlers can return Promises. The host waits for completion before its next render; a rejected Promise becomes an error response. Mutations after asynchronous work also request rendering through `r/atom`. Keep CPU-heavy work off the JavaScript event loop.
- Editor `:lsp` providers can return values or Promises. Pending providers do not block incoming UI callbacks, and retained provider IDs resolve the latest function after a render.
- `platform/pick-directory`, `reveal-path!` and `open-path!` use the same native host actions. `runtime/preview-png` returns a **Promise** for a PNG or nil, rather than blocking as on the JVM.
- `theme/write-json` writes through Node-compatible filesystem APIs and returns the absolute path string. JVM callers still receive a `java.io.File`.
- Share portable application code in `.cljc`. Java interop, JVM libraries and `future` need ClojureScript equivalents. The JVM watcher also recognizes `.cljc` files.

## Release

```sh
bun run cljs:release
CLJ_GPUI_BIN=/absolute/path/to/clj-gpui bun --no-install target/cljs-counter.js
```

A release build hides development chrome and requires a prebuilt host via `:host` / `CLJ_GPUI_BIN` by default.

For a self-contained application, copy [`templates/cljs/`](../templates/cljs/),
customize `gpui.edn`, and run `bun install --frozen-lockfile` then `bun run package`.
`gpui.package` supports `:backend :cljs`, `:cljs-build` and `:cljs-output` alongside
its JVM backend. It builds `.app` on macOS or AppImage and `.deb` on Linux, with the
released JavaScript, pinned Bun 1.4.2, GPUI host and production npm dependencies.
Bun archives are checked against the [official release checksums](https://github.com/oven-sh/bun/releases/tag/bun-v1.4.2).
The packager requires `bun.lock` and installs with `--frozen-lockfile --production`.
Existing projects using `package-lock.json` should migrate with `bun install`,
commit `bun.lock`, and use the updated template's scripts.

The default is [Bun's built-in trusted-dependency list](https://bun.sh/docs/pm/lifecycle).
No empty list or custom allowlist is injected. App-specific `trustedDependencies`
are preserved if present. The application's root lifecycle scripts are omitted
from the staged manifest; dependency scripts follow Bun's trust policy. The
runtime launcher uses `--no-install` to disable automatic dependency downloads.
These safeguards reduce installation risk; they do not guarantee safe package code.

Build on each destination OS/architecture. Bun bundles require macOS 13+ or a
supported glibc Linux, on x64/arm64. Native addons need Bun compatibility and may
need additional build/system libraries. Resources are copied into the bundle;
signing/notarization remains separate. See the template README for details.

## Checks

```sh
bun run test                         # shared widget suite + Bun runtime/lifecycle tests
CLJ_GPUI_BIN="$PWD/host/target/debug/clj-gpui" bun run test:protocol
clojure -M:test                       # existing JVM suite
```

`./scripts/ci.sh` runs both languages, including both protocol tests. The JavaScript lifecycle tests exercise fragmented UTF-8, early host failure, connection timeout, unexpected disconnect, invalid JSON, duplicate starts and listener cleanup. The real Rust protocol test checks callback mutation, asynchronous provider concurrency, Unicode and state retention across a render request. Actual shadow-cljs hot reload is a separate development smoke test.

### Initial Node backend verification (2026-09-29)

- Full local `./scripts/ci.sh`: 360 Rust tests, strict Clippy, 135 JVM tests / 2,468 assertions, 56 ClojureScript tests / 1,081 assertions, formatting, and JVM plus development/optimized ClojureScript socket protocol tests passed.
- The same 56 ClojureScript tests / 1,081 assertions also passed with advanced release optimization. The optimized example compiled without warnings.
- Native example: clicked the counter twice, edited the input, and ran the asynchronous dayjs callback. A source edit hot-reloaded into the same window with count, text and timestamp retained. Closing the native window exited Node successfully.
- Automatic Cargo build and executable discovery passed the real protocol test without `CLJ_GPUI_BIN`.
- Linux CI is configured but was not run locally.
- Template packaging: the macOS `.app` built and launched with a working native counter and bundled `dayjs`. Relocation without system Node and repeated packaging passed. Unit tests exercise macOS, AppImage and Debian layouts; CI additionally builds/extracts actual Linux artifacts.

### Bun verification (2026-09-29)

- Development and optimized builds passed all 56 ClojureScript tests / 1,081 assertions and the real native host protocol test on Bun 1.4.2, without Node on `PATH`.
- Native counter, text input and asynchronous `dayjs` callback worked. A source edit hot-reloaded into the same native window with count, text and timestamp retained.
- The Bun `.app` built, launched natively, and passed the relocated package test with only bundled Bun on `PATH`. The package contains no Node executable and preserves the default built-in dependency trust policy.
- The JVM suite passed 143 tests / 2,526 assertions, including macOS/Linux package layout checks. Actual Linux artifact builds remain a CI check and were not run locally.
