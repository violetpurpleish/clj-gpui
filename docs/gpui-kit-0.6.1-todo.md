# GPUI Kit 0.6.1 support checklist

Status: **planned; no dependency or implementation changes made**. Reviewed against this checkout and upstream tag `v0.6.1` on 2026-09-09. Complete compatibility work first, then the headless harness and new API coverage. Unchecked items are work to do, not confirmed defects.

## Current baseline

- `host/Cargo.toml` requests `gpui-kit` and `gpui-fps` 0.6.0; `host/Cargo.lock` resolves both to 0.6.0, `gpui-pre` to 0.3.3, and Tree-sitter to 0.26.13. Cargo's `"0.6.0"` requirement is a compatible range, not an exact pin.
- The host already uses the Kit facade, edition 2024, bundled Tree-sitter languages, and a separately registered Clojure grammar in `host/src/syntax.rs`.
- The wire protocol is 11. Existing Rust tests include pure renderer/state tests and bridge/RPC regression tests; Clojure tests cover constructors, runtime and protocol behavior. There is no Kit `test-support` dependency yet.
- CI currently runs on Linux and macOS through `.github/workflows/ci.yml` and `scripts/ci.sh`.

## 1. Required dependency and compatibility work

- [ ] Raise the declared Kit and FPS minimums to 0.6.1 in `host/Cargo.toml` and refresh `host/Cargo.lock` deliberately. Review the resolved Kit/Base/Component/assets and GPUI platform versions together; check `cargo tree -d` for incompatible duplicate GPUI types.
- [ ] Keep the host's HTTP client wiring working in `host/src/main.rs`. Check that the direct `gpui-pre-reqwest-client` alias still matches the resolved GPUI family; upstream removing an unused dependency does not replace our remote-image client.
- [ ] Compile the production host and resolve actual API changes across `renderer.rs`, `overlay.rs`, `extra.rs`, `rows.rs`, `mapping.rs`, `catalog.rs` and `syntax.rs`. Do not assume every release-note item requires a wrapper rewrite.
- [ ] Verify the custom Clojure grammar against the updated highlighter dependencies. Run `clojure_highlighter_loads_without_plain_text_fallback`; test changing languages and editing after a rerender before removing any existing refresh/highlighter handling.
- [ ] Audit Select synchronization in `renderer.rs` (`select_slot`, `apply_select_controlled_value`, confirmation dispatch) against the new committed-value behavior. Preserve option IDs, clearing, grouped options, open search queries and exactly-once callbacks; distinguish a highlighted option from a committed selection.
- [ ] Check the existing `gpui-fps` dev chrome with the updated FPS behavior, including idle activity and display refresh rates.
- [ ] Run the existing Rust, Clojure and protocol suites via the repository CI script, plus a normal build without test instrumentation. Keep stable Rust unless a verified upstream requirement says otherwise.

## 2. Add headless tests of the real host renderer

Use the [tagged testing guide](https://github.com/longbridge/gpui-kit/blob/v0.6.1/website/docs/test.md) and [Kit test source](https://github.com/longbridge/gpui-kit/blob/v0.6.1/crates/kit/src/test.rs) as the API reference.

- [ ] Add a matching `gpui-kit` **dev-dependency** with `test-support` in `host/Cargo.toml`. Keep normal application builds free of observation. Use explicit imports so the GPUI test macro does not accidentally shadow ordinary Rust `#[test]`.
- [ ] Add a small test fixture around the production renderer, Kit initialization, custom syntax setup and real `Root` overlay layers. The host is currently a binary: start with an internal test module; extract a library seam only if a separate integration target needs it. Do not duplicate the widget renderer in fixtures.
- [ ] Provide a controlled bridge peer and collect emitted callback payloads. Reuse the registry-replacing peer approach in `host/src/overlay_regression_tests.rs`, with bounded synchronization for the real socket worker. GPUI's test clock does not simulate socket/network progress.
- [ ] Map existing explicit Clojure IDs and host widget keys to queryable GPUI identity scopes. Use `TestWindowExt::within` for repeated controls; register custom containers with `TestSupportExt` only where observation is needed. Preserve native layout and focus bindings.
- [ ] Use `render_frame` before initial queries and after external tree changes. Re-query snapshots after actions; await deferred work outside the window update using `TestAppContextExt::wait_for`. Avoid redrawing while the typed root entity is already borrowed.
- [ ] First end-to-end fixture: render a Clojure-shaped input/button tree, type Unicode text, click the button, inspect callback arguments, apply the returned tree and verify the updated native control state and bounds.
- [ ] Cover controlled values and retained identity: checkbox/switch/slider, Select/Combobox, focus and selection across unrelated rerenders, reordered explicit IDs, unmount/remount, and disabled controls rejecting input.
- [ ] Cover overlay callback lifetimes through actual clicks/keys: change the callback registry while open, submit/cancel, close and reopen, nested menu paths, and focus restoration. Assert both native UI state and callback count/order.
- [ ] Add collection/layout cases for List, DataTable, Tree and resizable/dock surfaces: empty or shrinking collections, scrolling to virtualized rows, selection and bounds. Only query rows after they have been painted.
- [ ] Add editor interactions for language switching, search, CRLF navigation and new editing behavior. Inspect retained editor state where the snapshot API cannot expose the needed property.
- [ ] Document snapshot limits: accessible labels/values are not rendered text or pixels; absent optional state is not false; slider numeric state needs an application assertion. Test enabled/disabled behavior by attempting interaction.

## 3. Pixel checks and CI

- [ ] Run the new interaction/layout tests in existing Linux/macOS CI alongside current tests. Keep platform build dependencies installed even for headless execution.
- [ ] Add an explicitly selected macOS rendering target modeled on [upstream rendering.rs](https://github.com/longbridge/gpui-kit/blob/v0.6.1/crates/kit/tests/rendering.rs): `test = false`, `harness = false`, and `test-support`. AppKit needs main-thread initialization; `--test-threads=1` is not a substitute.
- [ ] Use the real offscreen Metal renderer for a small set of reviewed image checks: label glyph edges, editor highlighting, selected ghost buttons and Markdown inline code/line breaks. Fix fonts, dimensions, theme and animation state; save failure images for review.
- [ ] Make the explicitly enabled macOS pixel job fail when its renderer is unavailable. Report pixel checks as unsupported/skipped on other platforms; do not substitute semantic snapshots for images.
- [ ] Retain native-window smoke checks for titlebars, menus, capture, real focus/IME and packaged-app behavior. Reassess old comments claiming all headless UI checks require a real window/GPU, without weakening platform-specific verification.
- [ ] Document exact local commands once the targets exist. Windows headless CI is a follow-up platform expansion, not something the current CI already covers.

## 4. Expose new Kit capabilities through Clojure

These are additive API tasks, separate from getting existing applications onto 0.6.1. For each accepted option, update `src/gpui/ui.clj`, the serialized `Node`/item schema in `host/src/protocol.rs`, the relevant painter, constructor/protocol tests, documentation and a focused example. Names below are proposed Clojure spellings until checked against the tagged API.

- [ ] **Editor preferences:** expose independent `:auto-close` and `:smart-indent` options, preserving upstream defaults when omitted and explicit `false`. Apply live changes without unnecessarily replacing editor state. Inspect the language-rule API and add suitable Clojure bracket/indent rules to our custom registration; test strings/comments and closing delimiters.
- [ ] **Multiple cursors:** verify native gestures, column selection over short lines, edits and undo through `ui/editor`, including controlled-value echoes. Document supported gestures. Add a serialized selection API only if needed; native multi-cursor editing alone should not require one.
- [ ] **Button tooltip placement:** add `:tooltip-placement` to the native Button path, including applicable overlay/button recipes. Verify fallback placement near window edges and compatibility with the existing `:tooltip` handling.
- [ ] **Sidebar item styling:** expose per-item styling and `label_style` through the sidebar item schema and `SidebarMenuItem` painter. Keep item styling distinct from sidebar-container styling and preserve selected/disabled behavior.
- [ ] **SVG icons:** audit `mapping::parse_icon`, the icon catalog and component icon slots against shared Lucide names. Define one serializable representation for SVG bytes/content, then wire supported slots without breaking existing keyword names or default icons. Cover non-ASCII SVG content and invalid input handling.
- [ ] **Markdown frontmatter:** add an opt-in option using `MarkdownExtensions::frontmatter()` in `extra::paint_markdown`. Verify structured mappings and fallback YAML blocks; leave HTML and the omitted-option behavior unchanged.
- [ ] **Select dismissal:** decide whether to expose a distinct `:on-dismiss` callback using the new consistent close behavior. Keep it separate from selection confirmation; test Escape, outside click, selection and programmatic closure for ordering and duplicates.
- [ ] Review whether any added fields require a protocol-version change under the existing host/runtime compatibility contract. Update both ends and package compatibility tests together if so; a dependency-only bump does not by itself require new wire semantics.

## 5. Regressions inherited from upstream fixes

Verify these through existing wrappers; do not reimplement the upstream fixes locally. Scope the tests to our integration rather than duplicating the entire Kit suite.

- [ ] Editor search reopening, active-match preservation and reveal after scrolling; pending highlighting after language changes.
- [ ] Markdown hard/soft breaks, replacing documents with equal block counts, and text-selection autoscroll stopping on release.
- [ ] Selected ghost-button appearance, accessible Select activation, dialog close-button and calendar item accessibility.
- [ ] Resizable-panel behavior, list measurement with a missing configured row, and missing-default-monospace-font fallback.
- [ ] Confirm any existing font workaround still has a reproducible reason before removing it; the release does not establish that our glyph-clipping issue is fixed.

## 6. Documentation and completion

- [ ] Update the version and API inventory in `docs/gpui-component.md` after implementation; preserve explicit gaps rather than marking all 0.6.1 APIs supported.
- [ ] Update README testing guidance (currently directing headless checks to protocol tests) to distinguish protocol, rendered interaction/layout, Metal pixels and native smoke tests.
- [ ] Add new options to `docs/protocol.md` and examples in `examples/widgets/src/widgets/app.clj` or its page modules. Keep the older `docs/gpui-kit-migration.md` as migration history and link this follow-up.
- [ ] Record actual validation results and remaining platform limitations here before declaring 0.6.1 support complete.

## Sources and scope

- [0.6.1 release and linked changes](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.1)
- [Tagged Kit feature/dependency manifest](https://github.com/longbridge/gpui-kit/blob/v0.6.1/crates/kit/Cargo.toml)
- [Tagged workspace dependency versions](https://github.com/longbridge/gpui-kit/blob/v0.6.1/Cargo.toml)
- [Tagged test suites](https://github.com/longbridge/gpui-kit/tree/v0.6.1/crates/kit/tests)

GPUI Shell's scripting changes and the upstream documentation-site migration do not require adopting a JavaScript runtime or changing clj-gpui's architecture. This checklist does not include pre-existing unrelated widget parity work. Exact new builder signatures and resolved crates must be verified during implementation; no upgrade build has been run for this planning document.
