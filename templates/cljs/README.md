# ClojureScript app template

Copy this directory next to a clj-gpui checkout, then customize `src/my/app.cljs`,
`gpui.edn`, `package.json`, and the placeholder `resources/icon.png`.

Requires Node.js 22+, npm, Java 21+, the Clojure CLI, and the
[native host build dependencies](https://github.com/violetpurpleish/clj-gpui#quick-start).
Java/Clojure are needed for compilation and packaging only.

## Run and reload

```sh
npm ci
npm run compile
npm start
```

For live development, run `npm run watch` in one terminal and `npm start` in
another after the first compile. `defonce` state survives reloads. Connect with
`npx shadow-cljs cljs-repl app` or your editor's shadow-cljs integration.

The starter uses `dayjs` from npm. Add Node-compatible application packages with
`npm install <package>`; keep them in `dependencies`, not `devDependencies`.
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
npm run package
# equivalent: clj -X:build
```

Run on the OS and CPU architecture you intend to ship. The packager compiles a
shadow-cljs release, builds the Rust host, downloads a checksum-verified official
Node runtime, and installs the locked production npm dependencies in a clean
staging directory. Your development `node_modules` stays intact.

| Build machine | Output |
|---|---|
| macOS | `target/package/my-app.app` |
| Linux | `target/package/my-app-0.1.0-<arch>.AppImage` and `my-app_0.1.0_<arch>.deb` |

The packages include Node, compiled ClojureScript, npm dependencies, resources,
and the native GPUI host. End users need no Node, Java, Clojure, or Rust install.
Builds support x64 and arm64. Linux needs the normal GPUI graphics/system
libraries at runtime; these are not bundled. Build on the oldest Linux system
you intend to support ([official Node 22 requires glibc 2.28 or newer](https://github.com/nodejs/node/blob/v22.23.3/BUILDING.md#platform-list)).

Linux packaging also needs `curl`, `tar`, `fakeroot`, and `dpkg-deb`. The packager
uses an installed `appimagetool` or downloads its pinned, verified release.
The bundled runtime is pinned to Node 22.23.3. Dependency engine requirements
are checked during installation. Build tools must be available for any npm native addons; they are installed
using the bundled Node version and its headers. Dependency install scripts run;
the application's own npm lifecycle scripts do not. Run any custom asset
generation before packaging. Put registry/git/tarball dependencies in the
lockfile; local `file:` dependencies and npm workspaces need to be published or
packed and referenced by a remote tarball first. `:npm-args` in `gpui.edn` can
pass install options such as `["--legacy-peer-deps"]` when the lockfile needs them.

`resources/` is copied into the application directory, which is also its working
directory at launch. Treat it as read-only; store user data in a user-writable
location. `CLJ_GPUI_APP_HOME` names this application directory. App launchers
forward command-line arguments and put bundled Node on `PATH` for subprocesses.

`LICENSE` and `NOTICE` are copied automatically when present; `:license-files`
adds more files. The Node license and npm package license files are retained.
macOS signing/notarization remains a separate distribution step, as for the JVM
template. Preserve Node's JIT entitlements when signing the runtime, and sign
native npm addons before the outer app bundle. Builds replace their own generated
output and can be rerun directly.

`:cljs-build` must name a shadow-cljs `:node-script` build, and `:cljs-output`
must match its `:output-to`. `npm run release` only compiles JavaScript; use
`npm run package` for the self-contained native application.
