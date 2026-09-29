# ClojureScript app template

Copy this directory next to a clj-gpui checkout, then customize `src/my/app.cljs`,
`gpui.edn`, `package.json`, and the placeholder `resources/icon.png`.

Requires Bun 1.4.2+, Java 21+, the Clojure CLI, and the
[native host build dependencies](https://github.com/violetpurpleish/clj-gpui#quick-start).
Java/Clojure are needed for compilation and packaging only. Node.js and the npm
CLI are not needed.

## Run and reload

```sh
bun install --frozen-lockfile
bun run compile
bun run start
```

For live development, run `bun run watch` in one terminal and `bun run start` in
another after the first compile. `defonce` state survives reloads. Connect with
`bun --no-install node_modules/shadow-cljs/cli/runner.js cljs-repl app` or your editor's shadow-cljs integration.

The starter uses `dayjs` from npm. Add Node-compatible application packages with
`bun add <package>`; keep them in `dependencies`, not `devDependencies`.
Browser/DOM UI packages cannot render through native GPUI.

`deps.edn` points at `../clj-gpui`. Change it for another checkout location, or
replace the local dependency with a pinned Git dependency:

```clojure
clj-gpui/clj-gpui {:git/url "https://github.com/violetpurpleish/clj-gpui.git"
                 :git/sha "REPLACE_WITH_SHA"}
```

shadow-cljs uses this same classpath. `my.host/library-root` discovers the native
host checkout at compile time, including Git dependencies. Development builds
automatically build the host there; `CLJ_GPUI_BIN` can select a prebuilt host.
Release builds remove that development path and use the bundled host.

## Package for macOS or Linux

```sh
bun run package
# equivalent: clj -X:build
```

Run on the OS and CPU architecture you intend to ship. The packager compiles a
shadow-cljs release, builds the Rust host, downloads a checksum-verified official
Bun runtime, and installs the locked production npm dependencies in a clean
staging directory. Your development `node_modules` stays intact.

| Build machine | Output |
|---|---|
| macOS | `target/package/my-app.app` |
| Linux | `target/package/my-app-0.1.0-<arch>.AppImage` and `my-app_0.1.0_<arch>.deb` |

The packages include Bun, compiled ClojureScript, npm dependencies, resources,
and the native GPUI host. End users need no Bun, Node, Java, Clojure, or Rust install.
Builds support x64 and arm64. Bun packages require macOS 13 or newer. Linux needs the normal GPUI graphics/system
libraries at runtime; these are not bundled. Build on the oldest Linux system
you intend to support. The bundled Linux runtime uses glibc; the x64 build is
Bun's baseline variant for broader CPU compatibility.

Packaging needs `curl` and `unzip`; Linux additionally needs `fakeroot` and
`dpkg-deb`. The packager uses an installed `appimagetool` or downloads its pinned,
verified release. Bun 1.4.2 is pinned with separate SHA-256 checksums for each
platform. `bun.lock` is required, and production installs use `--frozen-lockfile`.

The template uses [Bun's built-in trusted-dependency list](https://bun.sh/docs/pm/lifecycle).
It deliberately omits `trustedDependencies`. Dependency lifecycle scripts run
only as permitted by Bun's trust policy; the application's own lifecycle scripts
are omitted from packaging. Run custom asset generation before packaging.
If you choose to add `trustedDependencies`, Bun treats it as a replacement for
its built-in list, not an extension. Review blocked scripts with `bun pm untrusted`.
The packager preserves your chosen policy and never automatically trusts packages.
This limits install-script exposure; it does not establish that package runtime
code is safe.

Bun supports many Node-compatible packages, but not all Node APIs or native
addons. Check [Bun's compatibility documentation](https://bun.sh/docs/runtime/nodejs-compat)
and test native dependencies against the bundled Bun version. Packages that
require Node/V8 internals or invoke a separate `node` executable may need changes.
Put registry/git/remote-tarball dependencies in the lockfile; local `file:`
dependencies and npm workspaces need to be published or packed and referenced by
a remote tarball first.

`resources/` is copied into the application directory, which is also its working
directory at launch. Treat it as read-only; store user data in a user-writable
location. `CLJ_GPUI_APP_HOME` names this application directory. App launchers
forward command-line arguments and put bundled Bun on `PATH` for subprocesses. They use `--no-install`,
so a packaged app cannot silently download a missing dependency at startup.

`LICENSE` and `NOTICE` are copied automatically when present; `:license-files`
adds more files. Bun's upstream license notice and npm package license files are retained.
macOS signing/notarization remains a separate distribution step, as for the JVM
template. Preserve Bun's JIT entitlements when signing the runtime, and sign
native npm addons before the outer app bundle. Builds replace their own generated
output and can be rerun directly.

`:cljs-build` must name a shadow-cljs `:node-script` build, and `:cljs-output`
must match its `:output-to`. `bun run release` only compiles JavaScript; use
`bun run package` for the self-contained native application.
