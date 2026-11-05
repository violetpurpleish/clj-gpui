# GPUI Kit 0.9.0

The host now uses Kit, Component, Base, assets, macros and FPS 0.9.0, with the matching GPUI snapshot family 0.3.11 (including the direct HTTP-client alias). Rebuild and restart the native host. The changes are additive on protocol v11 and apply to both Clojure and ClojureScript; an older host cannot render the new node kinds.

Sources: [0.9.0 release](https://github.com/longbridge/gpui-kit/releases/tag/v0.9.0), [0.8.1](https://github.com/longbridge/gpui-kit/releases/tag/v0.8.1), [0.8.0](https://github.com/longbridge/gpui-kit/releases/tag/v0.8.0), [0.7.3](https://github.com/longbridge/gpui-kit/releases/tag/v0.7.3), [0.7.1](https://github.com/longbridge/gpui-kit/releases/tag/v0.7.1). The [Component](inventory/gpui-component-0.9.0.tsv) and [Base](inventory/gpui-base-0.9.0.tsv) source inventories include the releases between this checkout's 0.7.0 baseline and 0.9.0.

## Theme compatibility

Upstream theme/config/token structs are now `#[non_exhaustive]`. Rust callers must start from constructors or `Default` and then assign fields. clj-gpui already deserializes ThemeSets and mutates existing themes, so registered palettes keep their existing Clojure API. Palette activation and restoration still synchronize Component colors/tokens into Base without calling the global refresh APIs during subtree rendering.

Every UI node can optionally set `:cursor-blink` within its theme scope:

```clojure
(ui/window {:theme :dark
            :cursor-blink {:interval-ms 700 :idle-timeout-ms 5000}}
  (ui/input "Normal blinking")
  (ui/input "Steady caret" {:cursor-blink :steady}))
```

A nonnegative integer is also accepted as the blink interval in milliseconds; zero means steady. Omitted values inherit the active scope. The default remains 500 ms without an idle timeout. This is a UI scope option, not a ThemeConfig JSON field. ThemeSet/highlight data continues to pass through native deserialization, including the new `created`, `modified`, and `deleted` highlight colors.

A root window's `:icon-paths` map implements the new application-level icon resolver, including icons inside native controls:

```clojure
(ui/window {:icon-paths {"icons/check.svg" "icons/star.svg"}} ...)
```

Keys and values are asset paths; the existing asset source must be able to load the replacement. Unmapped paths keep the original icon. Removing a mapping restores the original. This is application-wide, not a per-subtree theme property; embedded SVG data is unaffected.

## New widgets

### ColorSelect

`ui/color-select` is the framed, form-field version of `ui/color-picker`. It uses the same retained ColorPickerState, hex/nil value and `:on-change` payload. It accepts `:featured-colors`, `:size`, `:placeholder`, `:accessibility-label` and ordinary styles.

```clojure
(ui/color-select (:accent @state)
  {:id "accent" :placeholder "Choose a color"
   :accessibility-label "Accent" :on-change #(swap! state assoc :accent %)})
```

### Diff

`ui/diff` renders supplied unified or Git patches. It does not invoke Git or compute a patch. Give it a stable `:id` and a constrained height. It retains native selection, folding and scrolling across unrelated renders and mode changes. Changing the patch updates the retained state; malformed patches display the parser error instead of stale content.

```clojure
(ui/diff patch
  {:id "review" :height 400 :diff-mode :split :context-lines 3
   :language "clojure"
   :diff-options {:inline-unit :word :hover-highlight :both}
   :on-change #(swap! state assoc :review-event %)})
```

- `:diff-mode`: `:unified` (default) or `:split`.
- `:line-number` and `:soft-wrap`: native presentation flags, default true.
- `:context-lines`: visible unchanged context, default 3.
- `:diff-options`: `:source :patch|:text|:conflicts` (default patch); `:path` for text/conflict sources; `:truncated-hunks` (default false); `:expand-all`; `:expansion-lines` (20); `:min-collapsed-lines` (4); `:inline-unit :word|:character|:none`; `:header-visible`, `:syntax-highlight`, `:change-background` (all true); `:hover-highlight :none|:line|:line-number|:both`; `:hunk-separator :metadata|:line-info|:simple`; `:change-indicator :signs|:bars|:none`.
- `:diff-action` is an edge-triggered request map. Change the request or its `:generation` to repeat it. `:action` is `:next-change`, `:previous-change`, `:expand-all`, `:collapse-all`, `:scroll-to-file` with `:path`, `:scroll-to-line` with a position, or `:select` with a range. A select request without a range clears selection.
- Positions are `{:path "a.clj" :side :original|:modified :line 1}`. Line numbers are **one-based**. Ranges use `:range [first last]`, inclusive, with optional `:end-side` for a cross-side selection.
- `:on-change` receives maps with `:event` strings: `selection-started`, `selection-changed`, `selection-ended`, `file-expanded`, `file-collapsed`, or `conflict-resolved`. Selection events carry `:selection`; changed/ended also include `:text`. File events carry `:path`. Conflict resolution carries `:path`, zero-based `:index` and resulting `:text`.
- `:on-click` receives a line position; `:on-hover` receives a position or nil. `:on-add-annotation` receives a selected line range.

Annotations and gutter markers accept ordinary widgets, including working callbacks and retained inputs. Each needs a stable string `:id` and a `:value` position; omit `:line` for a whole-file annotation. Give a gutter marker a compact focusable control with an accessible name.

```clojure
(ui/diff patch
  {:id "review" :height 400
   :annotations [{:id "note-1"
                  :value {:path "a.clj" :side :modified :line 3}
                  :content (ui/button "Resolve" {:on-click resolve!})}]
   :gutter-markers [{:id "comment-3" :label "Comment on line 3"
                    :value {:path "a.clj" :side :modified :line 3}
                    :content (ui/button "+" {:size :small
                                              :accessibility-label "Comment on line 3"
                                              :on-click comment!})}]
   :diff-decorations [{:path "a.clj" :side :modified :range [3 4]
                       :color "#335588"}]})
```

The constructor maps annotations to wire `:items` and gutter markers to `:options`, preserving the existing nested-widget traversal and callback rules.

### Speech

`ui/speech-button` toggles native dictation. `ui/speech-waveform` displays audio levels (`:variant :liquid` selects the new liquid appearance); `ui/speech-indicator` displays the compact voice indicator. Share one session with the same string `:speech-id`, while keeping each node's own `:id` distinct. Without `:speech-id`, each node owns its own state. No microphone capture begins just by rendering.

```clojure
(ui/hstack
  (ui/speech-button {:id "dictate" :speech-id "draft"
                     :show-when-unsupported true
                     :on-speech #(swap! state assoc :speech %)})
  (ui/speech-waveform {:id "levels" :speech-id "draft"
                       :width 220 :height 32 :variant :liquid}))
```

`:on-speech` receives `{:event "started"}`, `{:event "partial"|"final" :text string}`, `{:event "cancelled"}`, or `{:event "error" :error string}`. Put the callback on one member unless each member intentionally needs the event. `:disabled` and control `:size` are available on the button; indicator also accepts `:size`. Removing every member releases the session.

The host enables upstream's `speech` feature. System recognition is available on supported macOS/Windows configurations; Linux has no system recognizer, so the button is hidden or disabled with `:show-when-unsupported true`. Custom Rust recognizers/audio sources are not Clojure provider protocols. Linux builds now need `libasound2-dev`; Debian packages declare the ALSA runtime dependency.

Packaged macOS apps include microphone and speech-recognition usage descriptions. Set package options `:microphone-usage-description` and `:speech-recognition-usage-description` to customize them. Native authorization and an installed/available system recognizer still determine availability. Unbundled development hosts can be unavailable; package an app to exercise its own OS authorization identity.

### ResizeHandle

`ui/resize-handle` supplies the native divider affordance when the application owns the resized dimension. `:orientation :horizontal|:vertical` is the resize axis; `:placement :leading|:trailing` optionally hugs a container edge. `:on-change` receives window-space `{:x :y}` during dragging. The application computes and stores the new dimension. `ui/resizable` remains the simpler choice for adjacent panels; its children now accept `:fixed-size true` to preserve their size when the container resizes.

## Text and existing-widget additions

- Input, Textarea and Editor: `:on-edits` receives ordered edit maps containing `:start-byte`, `:old-end-byte`, `:new-end-byte`, `:start-position`, `:old-end-position`, `:new-end-position`, and `:text` (the document after that edit). Positions are zero-based `[row byte-column]`; offsets are UTF-8 bytes. The `:on-change` payload remains unchanged; when both handlers are present, they run in the same callback batch. Controlled value synchronization is silent.
- Input/textarea atomic tokens: `:on-token-hover` receives `:id`, `:text`, `:label`, UTF-8 `:range`, `:hovered`, and window-space `:bounds {:x :y :width :height}`.
- Editor: `:gutter-decorations [{:range [start end] :color "#..." :folded-color "#..."}]` and `:overview-ruler-decorations [{:range [start end] :lane :left|:center|:right|:full :color "#..."}]`. Ranges are half-open UTF-8 byte ranges and track native edits. Empty ranges mark boundaries. Change the data or `:decoration-generation` to replace them; empty vectors clear them. The native gutter reserves its strip while its collection exists. Editor now forwards control `:size`.
- Markdown/HTML: `:selection-format :source-across-styles` uses the new native mixed plain/source copy policy.
- Tabs: `:variant :folder`.
- Dialog/AlertDialog: `:entrance :slide-down|:fade|:fade-slide|:none` and `:centered true`. AlertDialog also forwards `:margin-top`.
- All seven charts: `:appear false` disables native entrance motion; changing string `:appear-key` replays it. Native reduced-motion policy is respected.

Upstream text selection, IME, accessibility, menus, table resizing, combobox filtering, virtual-list geometry and scrolling fixes arrive through the dependencies. Markdown typography and macOS scroll bounce changed upstream; applications should review their own dense text screens after upgrading.

## Boundaries and validation

The widgets gallery includes a **New in 0.9** page. Native Rust closure interfaces (custom recognizers, syntax-highlighter/scrollbar renderers, external editor hosts, custom Plot types) remain outside the wire API. `gpui-shell`, the optional GPUI Fast backend and `gpui-webview` are not added to this JVM/Bun host. This is not a claim that every Rust builder is a Clojure option.

Final validation ran locally on macOS through the configured mbx wrapper:

- `./scripts/ci.sh`: 366 Rust tests, the isolated missing-monospace fixture, strict Clippy, debug host build, 146 Clojure tests / 2,569 assertions, formatting, JVM protocol v11, 58 ClojureScript tests / 1,090 assertions, and both development and optimized ClojureScript protocol checks passed.
- Optimized release host rebuilt successfully; its protocol-v11 socket/reload check passed. Restart running apps to load the new executable.
- Nine production RootView Metal checks passed; the new Diff, ColorSelect, speech and editor-decoration fixture was visually inspected. Geometry fixtures disable chart entrance motion to sample the final layout.
- Production regressions cover retained Diff/speech state, current callbacks and nested annotation buttons, standalone resize dragging, paired UTF-8 edit/value callbacks, edit-tracked decorations, and restoration of both theme layers.
- ClojureScript template packaging, plist validation, and relocated launch/callback/npm checks passed (using the existing PNG icon fallback).
- All 21 gallery pages exported successfully. The lockfile and `cargo tree -d` audit confirmed the matching Kit 0.9.0 / GPUI 0.3.11 dependency families. `cargo fmt --check` and `git diff --check` passed.

Speech event tests use emitted fixtures and do not record audio or prove OS recognition. Linux/Windows native builds and rendering, actual microphone authorization, and live IME interaction were not exercised locally.
