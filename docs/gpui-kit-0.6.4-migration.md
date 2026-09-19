# GPUI Kit 0.6.4 migration

The desktop host moves from GPUI Kit 0.6.1 to 0.6.4. This includes the 0.6.2
release and 0.6.4's popup-placement and pie-chart fixes. The Clojure API is
unchanged and the wire protocol remains version 11.

## Dependencies and compatibility

- Normal and test `gpui-kit` dependencies, `gpui-fps`, Base, Component,
  component macros, and assets resolve to 0.6.4.
- The GPUI snapshot family resolves to 0.3.5, including the direct
  `gpui-pre-reqwest-client` dependency used for remote images. The lockfile
  update is limited to the Kit/GPUI upgrade and its dependency edges.
- Existing renderer, overlay, editor, Clojure grammar, collection, and dock
  integrations compile against the published crates. Stable Rust suffices.
- Kit removed its tiles canvas in 0.6.2. Our dock already uses `DockLayout`
  tabs and `CljPanel`, so no application layout conversion is needed. Removed
  obsolete `tiles.background` entries from the bundled and example themes.
- Searchable Select clears its query on dismissal or confirmation. A new
  production-renderer test filters grouped options, cancels or commits,
  applies the Clojure tree response, then reopens and selects from the full
  list. This covers retained selection IDs and callback delivery.

## Pie radius behavior

The host previously always supplied a concrete outer radius to work around
Kit's invisible default pie slices. Kit 0.6.4 fixes both painting and hit
testing, so ordinary pie charts now leave the radius unset unless Clojure
provides `:outer-radius`. Kit derives 40% of the actual laid-out height,
including a `:flex 1` viewport.

Explicit radii and per-slice overrides still apply. When only some slices
set a radius, the other slices retain the node radius or 40% of the declared
or default viewport height: Kit's per-slice callback requires a concrete
number and does not receive the layout bounds. The macOS Metal regression
checks a flex-filled donut, an explicit radius, and mixed per-slice radii
through the production renderer.

## Scope of upstream additions

Existing widgets inherit the upstream editor, text, menu, dialog, chart,
Select, FPS, and reduced-motion changes. This migration does not introduce
new Clojure controls or callbacks. Carousel, Empty, InputGroup, custom editor
search/replace controls, clipboard paste hooks, Markdown plugins, streaming
motion, parser factories, motion sequences, and additional dock APIs are
listed as future bindings in [the coverage inventory](gpui-component.md).

Mobile application support in upstream Kit does not turn this desktop JVM
host into an iOS or Android application. The TodoMVC font workaround remains;
its original native glyph-clipping case has not been rechecked here.

## Validation

- `./scripts/ci.sh`: passed locally on macOS, including 315 Rust tests, the
  isolated missing-monospace fixture, strict Clippy, the normal host build,
  124 Clojure tests / 2,122 assertions, cljfmt, and the protocol-v11 socket
  round trip.
- `cargo tree --locked --manifest-path host/Cargo.toml -d`: no incompatible
  duplicate Kit or GPUI versions. Cargo may show the same collections crate
  in distinct build/target contexts.
- `cargo test --locked --manifest-path host/Cargo.toml --test rendering`:
  five checks passed with the actual macOS Metal renderer, including the
  pie-radius cases.
- Pull-request CI runs the same host/Clojure checks on Linux and macOS;
  macOS also runs the explicit Metal target.

Interactive native-window smoke, IME, Windows, mobile, and a real Linux
tiling compositor are not validated by these headless checks.

## Upstream references

- [0.6.2 release](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.2)
- [0.6.4 release](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.4)
- [0.6.4 workspace dependencies](https://github.com/longbridge/gpui-kit/blob/v0.6.4/Cargo.toml)
- [0.6.4 pie radius implementation](https://github.com/longbridge/gpui-kit/blob/v0.6.4/crates/component/src/chart/pie_chart.rs)
