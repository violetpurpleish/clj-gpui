# GPUI Kit 0.7.0

The host uses `gpui-kit`, `gpui-fps`, `gpui-component`, and `gpui-base` 0.7.0 with the exact GPUI snapshot family 0.3.7. The direct `gpui-pre-reqwest-client` alias matches that snapshot. The independent `gpui-pre-reqwest` package remains 0.12.15. These changes are additive on the Clojure wire protocol (v11); rebuild the host to use them.

Source audit: [upstream release](https://github.com/longbridge/gpui-kit/releases/tag/v0.7.0), [Component inventory](inventory/gpui-component-0.7.0.tsv), [Base inventory](inventory/gpui-base-0.7.0.tsv). Inventory declarations are a review aid; macro-generated types and native closure APIs need manual inspection.

## New components

`ui/toolbar` and `ui/toolbar-group` retain native roving keyboard focus. Left/Right wrap between enabled controls. Controls inherit `:size` (default `:small`; `:large` uses `:medium`), and buttons use compact ghost styling. Groups accept `:label` / `:accessibility-label`. Content remains in source order. Disabling the toolbar's navigation does not disable its child controls.

```clojure
(ui/toolbar {:id "commands" :size :small}
  (ui/toolbar-group {:label "Document"}
    (ui/button "Save" {:on-click save!})
    (ui/button "Publish" {:on-click publish!}))
  (ui/label "Ready"))
```

`ui/time-field` uses native segmented time editing. `:time-precision` is `:minute` (default) or `:second`; `:hour-cycle` is `:h23` (default) or `:h12`. Values and `:on-change` payloads always use 24-hour `HH:mm` or `HH:mm:ss`. `:disabled`, `:invalid`, `:focus` and control sizing are supported. Changing the controlled value is silent and retains the entity.

```clojure
(ui/time-field "14:35:22"
  {:id "meeting-time" :time-precision :second :hour-cycle :h12
   :on-change #(swap! !state assoc :time %)})
```

`ui/questionnaire` owns native choice/freeform state, required-answer validation, optional skipping, navigation, keyboard shortcuts and submission. Questions have stable string/keyword `:id`, `:label`, optional `:description`, `:required`, `:multiple`, `:disabled`, `:choices`, and `:input`. Choice maps use `:id`, `:label`, `:description`, `:disabled`, and initial `:selected`. Input maps use `:label`, `:placeholder`, and `:disabled`. `:pattern` and `:validation-message` provide synchronous native regex validation of freeform answers; `:questionnaire-errors` supplies application validation errors by question id.

```clojure
(ui/questionnaire
  {:id "setup" :shortcuts :numbers
   :questions [{:id :language :label "Language" :required true
                :choices [{:id :clj :label "Clojure"}
                          {:id :rust :label "Rust"}]}
               {:id :notes :label "Notes" :input {:placeholder "Optional notes"}}]
   :on-submit #(reset! !submission %)})
```

Omitting children supplies the standard presentation. Compose your own with `questionnaire-progress`, `questionnaire-item`, `questionnaire-title`, `questionnaire-description`, `questionnaire-choices`, `questionnaire-choice`, `questionnaire-choice-description`, `questionnaire-input`, `questionnaire-error`, `questionnaire-actions`, `questionnaire-previous`, `questionnaire-next`, `questionnaire-skip`, and `questionnaire-submit`. Parts take ordinary styles and children; `:question` selects a question id and `questionnaire-choice` takes `:value`. Item children inherit their enclosing question. Choice parts also accept `:indicator-style`, `:content-style`, and `:shortcut-style` maps.

`:answers` maps question ids to `{:choices [ids] :freeform text}`; `:current-item` requests navigation. These are applied when changed, preserving native answers/navigation across unrelated rerenders. Schema or shortcut-mode changes rebuild the questionnaire. `:questionnaire-action {:action :next|:previous|:skip|:submit|:reset :generation n}` requests an action once per changed request.

Events use wire string ids: `:on-change` receives `{:item "id" :answer {:choices ["id"] :freeform text-or-nil} :status "answered"|"unanswered"|"skipped"}`; `:on-current-change` receives `{:previous id-or-nil :current id-or-nil}`; `:on-complete` and `:on-submit` receive ordered vectors of `{:item :answer :status}`.

## DatePicker compatibility

Existing date-only values and callbacks remain `YYYY-MM-DD`, `nil`, or `[start end]`. With `:time-precision :minute|:second`, single-date values and callbacks use `YYYY-MM-DDTHH:mm[:ss]`, without timezone conversion. `:hour-cycle` controls presentation, and `:default-time` supplies the time for date-only values. Time-enabled preset values use the same ISO date/time string. Range mode remains date-only. Set `:date-format` to include time when desired, for example `%Y-%m-%d %H:%M`.

## Text editing and Markdown

Input and Textarea accept atomic `:tokens [{:id "ada" :text "@Ada" :label "Ada" :range [3 7]}]`. Ranges are UTF-8 byte offsets into the controlled text and must match the token text exactly, respect character boundaries, and not overlap. Native editing treats tokens atomically; clipboard text remains plain text. `:token-insert {:id ... :text ... :label ... :range [start end] :generation n}` replaces a range, or the native selection when omitted. Change the request/generation to repeat insertion. An insertion survives the old controlled text until a new text/token configuration arrives.

`:on-content-change` receives `{:text string :tokens [...]}` for saving/restoring drafts; existing `:on-change` still receives a string. `:on-token-click` receives `{:id :text :label :range}`. `:token-style` styles the native InputToken. Tokens are unavailable in Editor, NumberInput, masked/password and formatted-mask modes. Textarea now forwards standard `:size` to the native control.

Editor accepts `:range-decorations [{:range [start end] :style :frame|:fill :color "#..."}]`. These geometric annotations complement existing `:decorations`; ranges track edits until the vector or `:decoration-generation` changes. Clearing the vector clears the collection. Native entities and annotation collections remain host-owned.

Markdown accepts `:range-highlights [{:range [start end] :color "#..."}]` in **rendered copy text**, not source Markdown. `:search {:query "literal text" :color "#..."}` computes case-sensitive literal matches across formatting. `:reveal-range [start end]` scrolls its starting line into view; change `:reveal-generation` to repeat. `:on-reveal` reports window-space `{:x :y :width :height}` for an outer scroll container. `:on-text-state` reports changed `{:rendered-text :selected-text :source-range}` snapshots; `:source-range` is the original Markdown byte range or nil. Highlights are Markdown-only and native reveal is best effort. Retained TextView state preserves selection and append-only streaming behavior.

## Charts

All seven charts explicitly forward `:interactive` and receive stable host ids. clj-gpui preserves its existing default of no hover interaction; pass `:interactive true` for tooltips.

Line/Area add `:y-domain [min max]`, `:point-count`, `:y-axis`, `:y-axis-label-placement :inside|:outside`, `:y-tick-count`, `:x-tick-count`, `:grid-columns`, `:grid-dashed`, `:reference-lines [values]`, and `:y-padding [top bottom]`. `:y-tick-format` accepts `{:precision 1 :prefix "$" :suffix "k" :scale 0.001}`.

Bar adds `:band-count`, `:band-tick-count`, `:value-axis-label-placement`, `:value-tick-format` (same recipe), `:grid-dashed`, `:padding-inner`, `:padding-outer`, `:min-length`, and `:max-band-width`. Candlestick also adds `:max-band-width`. Bar point `:label-color` overrides the chart label color. **`:value-tick-count` now counts ticks, not intervals**; add one to an old explicit count to retain its appearance. Default bar layout is preserved.

Points accept `:tooltip-title`, `:tooltip-value`, and `:tooltip-value-color`; value/color may be a vector for multivalue Area/Radar/Candlestick rows. Pie/Sankey title maps to native `tooltip_name`, independently of visible labels. Line/Area/Bar/Radar/Candlestick also accept point `:tooltip-content` containing arbitrary widgets. Chart options with these names (and `:label-color`) can be Clojure functions of a point; they run while building the tree and their results are transmitted as presentation data. Native paint callbacks never synchronously call the JVM. `chart.grid` is accepted in theme color maps.

## Other additions and inherited fixes

- Attachment: `:on-remove`, `:on-retry`, `:progress` (0–100, native clamping), and `:tooltip`; give interactive attachments an `:id`. AttachmentGroup adds `:edge-fade` color and `:scroll-offset [x y]` (nonnegative distance from the start); change `:scroll-generation` to repeat a scroll request.
- GroupBox adds widget `:footer`. Settings group maps add `:footer` and per-group `:variant`; `:normal` has no card surface.
- Popover adds `:offset` and `:arrow`; native trigger styling and all eight anchors now work as upstream specifies.
- Select adds zero-argument `:on-dismiss`; confirmation emits the value change before dismissal. An already-closed menu does not dismiss again.
- Switch forwards `:focus-ring`, `:tab-stop`, and `:tab-index`. Sidebar menu items/toggle buttons and standalone ListItem forward `:accessibility-label`.
- Message forwards `:id` / `:role`; Marker forwards `:alignment :start|:center|:end`.
- Dock adds `:close-button true` for closable tabs. Upstream handles nested docks and continuous bottom-dock resize.
- AlertDialog uses the existing `:ok-text`, `:cancel-text`, `:ok-variant`, and `:cancel-variant` bindings. The new upstream convenience methods do not require a second API.
- Native Table selection now reports only its active type; the existing Clojure row/column/cell selection adapter remains in place.
- Individually disabled Accordion items now remain disabled. Closed native accordion content unmounts after its animation; clj-gpui's mounted-tree retention preserves its host-owned text controls while their nodes remain in the tree.
- Form visibility/styling, menu lifecycle, input focus/Unicode/IME, Markdown rendering/streaming, chart painting and accessibility fixes arrive through the upgraded native dependencies.

## Host integration and boundaries

Production startup uses `gpui_kit::open_window`; Base Root owns all overlay hosting. The old manual dialog/sheet/notification layers are removed. Window and subtree theme scopes explicitly synchronize/restore Base tokens without refreshing every window during render. This avoids repaint loops when windows or nested scopes use different themes.

`gpui-shell` and `gpui-wry` remain outside this JVM host. The existing [native extension boundaries](gpui-kit-parity.md#remaining-native-extension-apis) still apply: custom Rust Plot implementations/layers, Root plugins and arbitrary synchronous native render/delegate/validator closures are not Clojure protocols. Declarative options and computed presentation data expose the application-facing features described above; this is not a claim of literal Rust API equivalence.

The widgets gallery has a **New in 0.7** page with runnable examples.

## Validation

Validated locally on macOS:

- Repository CI script: 360 Rust tests, the missing-monospace fixture, strict Clippy, debug host build, Clojure tests, formatting, and the protocol-v11 socket/reload test passed.
- Optimized release host rebuilt successfully; its protocol-v11 socket/reload test also passed. Restart running apps to load the new host.
- Final Clojure run: 134 tests / 2,465 assertions, including callback export and nested chart tooltip content.
- Eight production RootView Metal checks passed. The new fixture covers Toolbar/Group, TimeField, time-enabled DatePicker, Questionnaire, InputGroup tokens, fixed-domain chart options and Markdown search highlights; its image was visually inspected.
- Native interaction regressions cover toolbar focus wrapping around disabled controls, Select Confirm/Dismiss ordering through the callback acknowledgement barrier, retained time/questionnaire state and current handlers, token insertion, edit-tracked range decorations, and retained Markdown state.
- Lockfile audit: all 24 resolved GPUI snapshot packages use 0.3.7; the six Kit/Base/Component/FPS/assets packages use 0.7.0. Gallery tree export and `git diff --check` passed.

Linux and Windows native builds/rendering were not run locally. The existing third-party `block 0.1.6` future-incompatibility warning remains.
