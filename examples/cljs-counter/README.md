# ClojureScript counter

Run from the **repository root**:

```sh
bun install --frozen-lockfile
bun run cljs:compile
bun run cljs:start
```

For live reload, replace the compile command with `bun run cljs:watch` in one terminal, then start the app in a second terminal. Bun runs the compiler CLI and application; no Node.js or npm CLI is required. The example uses the npm package `dayjs`, Node's `os` module, native input and buttons, and reactive state updated from a timer.

See [ClojureScript setup and compatibility](../../docs/clojurescript.md) for external projects, REPL access and release builds.
