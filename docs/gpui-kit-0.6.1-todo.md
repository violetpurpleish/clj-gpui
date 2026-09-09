# GPUI Kit 0.6.1 support checklist

Status: **implemented and validated on macOS** on 2026-09-09. The compatibility/API work, production-renderer interaction fixture, and explicit Metal target are complete. Remaining unchecked items below are deliberately retained as deeper follow-up coverage, not known defects.

## Current baseline

- `host/Cargo.toml` now requires `gpui-kit` and `gpui-fps` 0.6.1; `host/Cargo.lock` resolves the Kit/Base/Component/assets/FPS family to 0.6.1, `gpui-pre` to 0.3.3, and Tree-sitter to 0.26.13.
- The host already uses the Kit facade, edition 2024, bundled Tree-sitter languages, and a separately registered Clojure grammar in `host/src/syntax.rs`.
- The wire protocol remains 11 because the new fields are additive. Rust now includes the Kit `test-support` dev feature and production-`RootView` interaction tests in addition to pure renderer/state and bridge/RPC regressions.
- CI runs on Linux and macOS through `.github/workflows/ci.yml` and `scripts/ci.sh`; macOS also runs the explicit offscreen-Metal target.

## 1. Required dependency and compatibility work

- [x] Raise the declared Kit and FPS minimums to 0.6.1 in `host/Cargo.toml` and refresh `host/Cargo.lock` deliberately. `cargo tree -d` shows one GPUI family (`gpui-pre` 0.3.3); repeated helper crates do not introduce incompatible GPUI types.
- [x] Keep the host's HTTP client wiring working in `host/src/main.rs`. The direct `gpui-pre-reqwest-client` alias remains on the same 0.3.3 GPUI family and the normal production build passes.
- [x] Compile the production host and resolve actual API changes across the renderer modules. The significant API change was syntax initialization; other paths needed additive wiring rather than compatibility rewrites.
- [x] Verify the custom Clojure grammar against the updated highlighter dependencies. The highlighter test and language-rule tests pass; retained editor state still handles live language updates.
- [x] Audit Select synchronization in `renderer.rs`. Existing grouped/search/controlled-value and callback-coalescing regressions pass unchanged on 0.6.1.
- [x] Check the existing `gpui-fps` dev chrome with the updated FPS behavior. Fresh captures from Counter, Widgets, and Catppuccin Violet showed the 60 Hz maximum, live frame/P95/drop/GPU/CPU/memory metrics, and interval changes after idle and input without destabilizing the examples.
- [x] Run the existing Rust, Clojure and protocol suites via the repository CI script, plus a normal build without test instrumentation. Stable Rust remains sufficient. `scripts/ci.sh` now respects Cargo's configured target directory.

## 2. Add headless tests of the real host renderer

Use the [tagged testing guide](https://github.com/longbridge/gpui-kit/blob/v0.6.1/website/docs/test.md) and [Kit test source](https://github.com/longbridge/gpui-kit/blob/v0.6.1/crates/kit/src/test.rs) as the API reference.

- [x] Add a matching `gpui-kit` **dev-dependency** with `test-support` in `host/Cargo.toml`. Normal builds do not enable observation.
- [x] Add a fixture around the production `RootView`, Kit/syntax initialization, and real `Root` overlay owner. It reuses the renderer; it does not duplicate widget painting.
- [x] Provide a controlled bridge peer and collect emitted callback payloads. The headless fixture drives the real `HostEvent`/`Cmd` channel contract; the existing bounded socket protocol test separately covers the network worker and registry replacement.
- [x] Map explicit input/button/checkbox/switch IDs to queryable GPUI identities. Adding the missing native Input id made the production identity observable without a test-only wrapper.
- [x] Use `render_frame`, fresh snapshots, and bounded `wait_for` around external tree changes.
- [x] First end-to-end fixture: a Clojure-shaped input/button tree types `Ada λ🦀`, checks the change and click callbacks, acknowledges the callback sequence with a returned tree, and verifies updated native label/value state and bounds.
- [x] Cover controlled values and retained identity. Production fixtures now cover checkbox/switch/slider and Select, focused editor preservation, explicit-ID reordering, slider unmount/remount, and disabled rejection; the existing Select/Combobox registry and coalescing regressions cover their retained controlled state.
- [x] Cover overlay callback lifetimes through actual clicks/keys. The production `Root` fixture changes callbacks while an alert is open, checks OK/Escape/close-button callback count and order, closes and reopens, restores focus, verifies a deferred sheet open/close batch, and clicks a nested menu leaf before its parent path callback.
- [x] Add collection/layout cases for List, DataTable, Tree and resizable/dock surfaces. The production fixture checks shrinking rows and selection, hidden-tree selection, retained layout entities, and nonzero resizable sizes; a separate 200-row DataTable case queries only painted rows and verifies keyboard scrolling plus viewport bounds.
- [x] Add editor interactions for live language switching, search reopening/active-match retention, exact CRLF text, auto-closing pairs, and focused state preservation. The fixture inspects the retained `EditorState` where semantic snapshots cannot expose those properties.
- [x] Document snapshot limits in README. The second production fixture attempts a disabled button click and proves no callback is emitted, while controlled checkbox/switch state and explicit-ID reordering are checked after a returned tree.

## 3. Pixel checks and CI

- [x] Run the new interaction/layout tests in existing Linux/macOS CI alongside current tests.
- [x] Add an explicitly selected macOS `rendering` target with `test = false`, `harness = false`, and test-support.
- [x] Use the real offscreen Metal renderer on a fixed production `RootView` fixture containing label glyph edges, highlighted Clojure editor text, a selected ghost button, Markdown inline code/hard break, and frontmatter. The fixture checks determinism and a visible state change; failures save PNGs under `/private/tmp` for CI upload.
- [x] Make the macOS target fail when Metal is unavailable. Other platforms print an explicit unsupported/skipped message and do not substitute semantic snapshots.
- [x] Retain native-window smoke checks for titlebars, menus, capture, and real focus. Command & Capture now also verifies the Widgets native menu opens beside its trigger, accepts keyboard traversal, dispatches Word wrap, closes, and updates its checkmark.
- [x] Add a dedicated composed-text IME smoke run; semantic and Metal headless tests do not stand in for it. In the focused Widgets Search field, a real German `^` dead-key composition committed `â`; the shared Clojure atom rerendered the same value in all three text controls, ordinary `x` input produced `âx`, and Backspace returned it to `â`.
- [x] Package the Widgets example with `clj -X:build package` and smoke-test the resulting `.app`. `gpui.prod` forces app chrome so the bundle cannot ship the development footer or FPS HUD even when the source tree requests `:chrome :dev` locally.
- [x] Document exact local commands in README. Windows CI remains a follow-up.

## 4. Expose new Kit capabilities through Clojure

These are additive API tasks, separate from getting existing applications onto 0.6.1. For each accepted option, update `src/gpui/ui.clj`, the serialized `Node`/item schema in `host/src/protocol.rs`, the relevant painter, constructor/protocol tests, documentation and a focused example. Names below are proposed Clojure spellings until checked against the tagged API.

- [x] **Editor preferences:** `:auto-close` and `:smart-indent` are independent optional booleans, default true, and update retained `EditorState` live. Clojure bracket/quote rules exclude string/comment contexts and smart indent handles unmatched openers plus leading closing delimiters.
- [x] **Multiple cursors:** `ui/editor` inherits Kit's native Option-click, Option-drag column selection, and vertical cursor keybindings (Cmd-Option on macOS; Ctrl/Shift-Option aliases). The gestures are documented; no redundant serialized selection model was added.
- [x] **Button tooltip placement:** `:tooltip-placement` forwards the four public Kit placements. Omitted/invalid values preserve Kit's automatic/default behavior and existing `:tooltip` remains unchanged, including overlay button recipes using the shared chrome function.
- [x] **Sidebar item styling:** item `:style`, `:label-style`, `:icon-svg`, and real native disabled state are wired separately from sidebar-container styling.
- [x] **SVG icons:** `:icon-svg` is UTF-8 inline content and takes precedence; malformed content falls back safely. Existing names remain supported and `AllAssets` plus `IconName::ALL` opens the complete shared Lucide catalog across supported icon slots.
- [x] **Markdown frontmatter:** opt-in Markdown enables both `MarkdownExtensions::frontmatter()` and `FrontmatterPlugin`; HTML and omitted/false behavior are unchanged.
- [x] **Select dismissal:** deliberately not exposed. In 0.6.1 the styled `Select` declares a dismiss event but does not forward BaseSelect dismissal, so a Clojure callback would be misleading and untestable. Selection confirmation remains separate.
- [x] **Protocol version:** remains 11. All fields are additive and unknown fields are already ignored; both ends' constructor/protocol tests cover the additions.

## 5. Regressions inherited from upstream fixes

Verify these through existing wrappers; do not reimplement the upstream fixes locally. Scope the tests to our integration rather than duplicating the entire Kit suite.

- [x] Editor search reopening and active-match preservation use retained production state; search navigation exercises Kit's reveal path, and a live Clojure-to-Rust language change refreshes the same editor before the Metal syntax fixture is captured.
- [x] Markdown hard/soft breaks and replacement with equal block counts are covered by an isolated third Metal capture.
- [x] Add a production-wrapper drag/release case for Markdown text-selection autoscroll stopping on release. The fixed-height Clojure `Node` fixture advances GPUI test time while the pointer is held at the viewport edge, proves the selection expands, releases the mouse, then proves it stays fixed.
- [x] Selected ghost-button appearance, accessible Select activation, dialog close-button behavior, and calendar-day accessibility/selection are covered by Metal and production headless fixtures.
- [x] Resizable panels retain their state and report two nonzero laid-out sizes; shrinking List/DataTable fixtures clear a configured selection whose row disappeared.
- [x] Add a deterministic production-wrapper fixture that removes the configured default monospace font and proves fallback rendering. A separate test executable avoids Kit's process-global font-resolution cache, uses GPUI's deterministic no-op text system without the platform monospace default, asserts `.SystemUIFont`, and lays out a production editor plus Markdown code block.
- [x] Keep the TodoMVC title font workaround. The resolved renderer remains `gpui-pre` 0.3.3, and the fresh native TodoMVC capture still exercises the workaround; this Kit release does not change the underlying macOS glyph-edge-clipping cause.

## 6. Documentation and completion

- [x] Update the 0.6.1 version/API inventory in `docs/gpui-component.md`, including the explicit Select-dismiss gap.
- [x] Update README testing guidance to distinguish protocol, rendered interaction/layout, Metal pixels, and native smoke tests.
- [x] Add the options to `docs/protocol.md`, `gpui.ui` docstrings/tests, and the widgets gallery.
- [x] Record actual validation results and remaining platform limitations below.

## Validation result

- `./scripts/ci.sh`: passed — 308 Rust tests plus the isolated missing-default-monospace executable; 116 Clojure tests / 1,330 assertions; cljfmt; normal debug host build; socket protocol test. Production headless coverage now contains eleven renderer interaction/layout tests plus the deferred-sheet regression.
- `cargo test --locked --manifest-path host/Cargo.toml --test rendering`: passed on macOS with the real offscreen Metal renderer. It now reports three checks, including isolated equal-block Markdown replacement. One earlier launch exited during a transient `com.apple.hiservices-xpcservice` connection failure; CI intentionally does not convert that into a skip.
- `cargo tree --locked --manifest-path host/Cargo.toml -d`: no incompatible duplicate GPUI family.
- Native Command & Capture smoke passed after launching all four approved examples from their project directories. Fresh post-action captures verified Counter increment/reset; TodoMVC focused Unicode entry, submit, toggle, and delete; Widgets navigation, Select keyboard commit, Clojure editor auto-pairing, frontmatter rendering, and a pointer-anchored native menu with keyboard traversal/selection of Word wrap; and Catppuccin theme switching, checkbox input, and focused Unicode typing. A physical German dead-key composition in Widgets committed `â`, propagated through the Clojure owner, accepted following text, and remained editable; a fresh capture and accessibility snapshot verified the final `â`. Window titlebars and capture also remained healthy.
- `cd examples/widgets && clj -X:build package`: produced a 142 MB ARM64 `target/package/widgets.app` with a valid plist, bundled release host, jlink runtime, application config, and uberjar. LaunchServices and fresh Command & Capture captures verified the packaged window has no FPS/nREPL development chrome and that a switch click still rerenders through the bundled production bridge.
- Linux keeps semantic headless coverage but no pixel renderer. Windows remains outside the current CI matrix.

## Sources and scope

- [0.6.1 release and linked changes](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.1)
- [Tagged Kit feature/dependency manifest](https://github.com/longbridge/gpui-kit/blob/v0.6.1/crates/kit/Cargo.toml)
- [Tagged workspace dependency versions](https://github.com/longbridge/gpui-kit/blob/v0.6.1/Cargo.toml)
- [Tagged test suites](https://github.com/longbridge/gpui-kit/tree/v0.6.1/crates/kit/tests)

GPUI Shell's scripting changes and the upstream documentation-site migration do not require adopting a JavaScript runtime or changing clj-gpui's architecture. This checklist does not include pre-existing unrelated widget parity work. Builder signatures, resolved crates, and the production host were verified during this implementation.
