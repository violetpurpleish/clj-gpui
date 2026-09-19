# Rendering boundaries

These findings apply to GPUI Kit / `gpui-component` 0.6.1 and `gpui-pre` 0.3.3,
as pinned by `host/Cargo.lock`. They distinguish host mapping errors from
upstream behavior; no dependency fork or application-specific renderer is used.

## Image sources: clj-gpui

Both Kit `Avatar::src` (`src/avatar/avatar.rs`) and `AttachmentMedia::src`
(`src/attachment.rs`) accept `impl Into<ImageSource>`. GPUI's
`src/elements/img.rs` implements string conversion as URI-or-embedded-asset,
while `Path` / `PathBuf` conversion produces `Resource::Path`.
`ImageAssetLoader::load` reads `Resource::Path` from disk, sends `Resource::Uri`
through the HTTP client, and resolves `Resource::Embedded` through the asset
provider. Consequently a filesystem path passed as a string does not read
the file, and a `file:` string is not a substitute for the path conversion.

The host now uses `mapping::image_source` for both public image consumers:

| Source | GPUI resource |
|---|---|
| Platform-absolute path | Filesystem path |
| `./image.png`, `../image.png` | Filesystem path relative to the host working directory |
| Local `file:` URL | Decoded filesystem path, including spaces and Unicode |
| Bare name such as `images/image.png` | Embedded asset, preserving GPUI string semantics |
| Other URL | Existing GPUI URI behavior; HTTP(S) uses the host HTTP client |

Windows native absolute paths and explicit `.\` / `..\` relative paths use the
same filesystem rule. Source classification does not depend on whether a file
currently exists and does not expand `~`; use an absolute path for a home directory.
The rules also apply to avatar-group children and static renderer contexts.
There is no general public `ui/image` API; that is a separate API addition.

## Explicit-newline line clamping: GPUI

The host's `mapping::apply_text_overflow` forwards `:line-clamp` to GPUI
`Styled::line_clamp`. Kit `Label` preserves the text and forwards its style to
GPUI text. The explicit-newline limitation is already present in GPUI's own
`WindowTextSystem::shape_text` (`src/text_system.rs`, lines 525–653): it splits
on `\n`, processes every resulting logical line, and counts wrap boundaries
without counting each logical line itself. Even after the wrap budget is
exhausted, later logical lines are still appended. `src/elements/text.rs`
then sums all returned line heights during layout.

A minimal upstream reproduction is to shape `"one\ntwo\nthree"` with a width
large enough to avoid wrapping and `line_clamp: Some(2)`. All three logical
lines are returned. Blank paragraph separators also consume layout height.
This explains why a wrapped paragraph can respect the limit while multiple
paragraphs exceed it. The host preserves the original text and GPUI semantics;
an upstream fix should account for explicit line breaks as part of the global
line budget and stop appending logical lines once it is exhausted.

An independent bare-GPUI test was run with
`cargo test --locked --manifest-path host/Cargo.toml --test upstream_line_clamp_repro -- --nocapture`
using a temporary test target (removed after the investigation). It created an
empty GPUI view, without Kit `Label` or the clj-gpui renderer, and called:

```rust
let runs = [window.text_style().to_run(text.len())];
let lines = window.text_system().shape_text(
    text.to_string().into(), px(16.), &runs, Some(px(320.)), Some(2),
).unwrap();
let count: usize = lines.iter().map(|line| line.wrap_boundaries.len() + 1).sum();
```

Observed output:

```text
bare GPUI: text="one\ntwo\nthree", clamp=2, logical_lines=3, displayed_lines=3
bare GPUI: text="one\n\ntwo\n\nthree", clamp=2, logical_lines=5, displayed_lines=5
```

This reproduction is evidence of the pinned upstream limitation, not a
regression assertion requiring future GPUI releases to retain the bug.

## Boolean preparation: clj-gpui API, with a strict JSON protocol

Widget constructors still produce ordinary Clojure maps. The common
`gpui.runtime/export-tree` path prepares typed boolean fields before callback
registration and JSON serialization, including maps assembled or updated by
application code after construction:

| Clojure field | Omitted | Present nil | Present true/false | Any other value |
|---|---|---|---|---|
| Ordinary Node/Item flag, such as disabled/loading/selected | Stays omitted | False | Unchanged | Type error |
| Nullable override, such as focus-ring/bordered/auto-close/smart-indent | Stays omitted | Stays nil | Unchanged | Type error |

The categories follow the Rust schema's `bool` versus `Option<bool>` fields.
Tests audit those field sets against `host/src/protocol.rs`. Existing constructor
defaults remain intact, including `:searchable true` on Combobox and Command.
Type errors include the exact field path and value type, for example:

```text
invalid UI boolean at children[0].disabled: expected true, false, or nil, got keyword
```

This preparation visits only declared Node/Item locations: child/slot nodes,
collection items, table object cells and header groups, and style maps even
without `:type`. It does not walk arbitrary `:value`, chart data, callback
payloads, or unknown metadata. NavStack recipes and custom variants validate
their own nullable style overrides; their nil values are not converted to false.

Constructor audit:

- Explicit checked state arguments to Checkbox/Switch/Toggle, explicit open
  state arguments to overlays, and `:open?` retain their existing nil-to-false
  shorthand. They no longer coerce other truthy values to true. A raw nullable
  `:checked` or `:open` map override keeps nil; `:open?` takes precedence over
  `:open` when both are supplied.
- Item disabled/expanded/separator flags preserve false and invalid values
  instead of silently dropping them. Item checked, column
  selectable/resizable/movable, and custom-variant shadow preserve nullable
  values rather than applying Clojure truthiness.
- Collection `:selected` remains an identity alias for `:value`, rewritten
  before preparation. Non-boolean shorthand APIs keep their existing rules:
  column `:sort`/`:fixed` accept named modes as well as booleans; literal-true
  aliases such as column `:sortable`/`:fixed-left` and Skeleton `:secondary`
  choose named modes and are not protocol boolean fields.

Use `(boolean expression)` or `(contains? selected-ids id)` when state can be a
non-boolean truthy value. In particular, invoking a set returns the matching
member, so `:disabled (selected-ids id)` is still invalid when that member is a
keyword. The nil rule does not make non-boolean members valid.

The native JSON protocol is unchanged. Sending raw `"disabled": null` still
fails with `invalid UI tree from Clojure at children[0].disabled: invalid type:
null, expected a boolean`. `bridge::parse_tree` retains its field-path and cause
diagnostics; no Rust default, upstream behavior, or protocol version changed.

## Custom button color: intentional Kit styling

Kit `src/button/button.rs`, `ButtonVariant::outline_background` / `bg_color`, applies
`colors.color.mix_oklab(cx.theme().transparent, 0.2)` for a custom variant's
normal background. The host forwards the supplied custom variant. This
intentional Kit color treatment remains unchanged.
