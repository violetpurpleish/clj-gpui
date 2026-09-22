# Missing-glyph diagnostics

Set `:on-missing-glyphs` on the root `ui/window` to receive GPUI's reports
about Unicode grapheme clusters that could not be rendered after font
fallback. The callback is optional and receives a vector of maps:

```clojure
(ui/window
  {:title "Font diagnostics"
   :on-missing-glyphs
   (fn [reports]
     (doseq [{:keys [grapheme font-class]} reports]
       (prn :missing-glyph grapheme :font-class font-class)))}
  (ui/label "Text to inspect"))
```

For example, a batch can contain
`[{:grapheme "👩‍💻" :font-class "proportional"}]`. This is an illustrative
payload, not a claim that this emoji is missing from your installed fonts.
`:grapheme` preserves the entire Unicode cluster, including combining marks
and joined emoji. `:font-class` is the string `"proportional"` or
`"monospace"`; it describes the required spacing, not a font family name.

## Platform support

The binding uses the published GPUI 0.3.6 `App::on_missing_glyphs` API.
Its platform support is narrower than the callback's availability:

| Text backend in GPUI 0.3.6 | Emits reports |
|---|---|
| Linux, Cosmic Text | Yes |
| macOS, Core Text | No |
| Windows, DirectWrite | No |

The macOS and Windows implementations inherit the default no-op
`PlatformTextSystem::set_missing_glyph_sink`. Installing the callback there
does not produce reports. Supporting those platforms requires upstream text
backend work. No reports therefore does **not** prove that every glyph was
rendered successfully.

This support matrix was checked in the published crate sources:
[Cosmic Text](https://docs.rs/crate/gpui-pre-wgpu/0.3.6/source/src/cosmic_text_system.rs),
[macOS](https://docs.rs/crate/gpui-pre-macos/0.3.6/source/src/text_system.rs), and
[Windows](https://docs.rs/crate/gpui-pre-windows/0.3.6/source/src/direct_write.rs).
It is not a claim of native Linux or Windows testing.

## Delivery and lifetime

- Reporting is application-wide. GPUI provides no widget ID, window ID,
  original font family, or source text location in these events. Only the
  root `ui/window` callback enables it; a callback on a nested node is ignored.
- GPUI delivers asynchronously after shaping releases its locks. Reports
  describe newly shaped text; enabling diagnostics is not a scan of already
  cached text or an inventory of installed fonts.
- GPUI deduplicates recent `(grapheme, font-class)` pairs. The subscription
  survives ordinary rerenders and handler changes, so rendering after a
  callback does not restart reporting. Its deduplication cache is bounded;
  previously reported clusters can eventually be reported again.
- Delivery is best effort. GPUI bounds its reporting channel; clj-gpui also
  merges pending reports into one batch of at most 1,024 distinct pairs while
  waiting for Clojure. Additional reports can be dropped. Diagnostics do not
  replace queued UI actions.
- Queued reports resolve the current Clojure handler after any pending
  rerender or callback acknowledgement, including after hot reload.
- Omit the property or set it to `nil` to drop the subscription and discard
  pending reports. Re-enabling starts a new reporting session. Closing the
  window or entering a host error view also drops the subscription; a valid
  tree with the callback re-enables it.

The optional wire field and existing callback payload format remain compatible
with protocol v11. An older host ignores the optional field.

## Verification

The Rust regression uses a deterministic platform text system to emit through
GPUI's real sink, asynchronous receiver and subscription, then checks the
production RootView callback queue. It covers Unicode clusters, both font
classes, deduplication across rerenders, current-handler resolution, queued
report removal, error recovery, and window-close cleanup. A separate queue
test checks the batch limit and preservation of UI actions.

Clojure tests check callback export, Unicode payloads and handler replacement.
The JVM/host protocol test sends both font classes and joined/combining Unicode
clusters over the real socket. These tests verify the binding; they do not
claim native font detection on an unsupported backend.
