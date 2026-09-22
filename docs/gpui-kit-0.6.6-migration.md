# GPUI Kit 0.6.6 migration

The desktop host moves from GPUI Kit 0.6.4 to 0.6.6 and from GPUI 0.3.5
to 0.3.6. The Clojure API and wire protocol version 11 stay unchanged.

## Dependency alignment

Production and test `gpui-kit`, `gpui-fps`, Base, Component, macros and assets
resolve to 0.6.6. The GPUI snapshot family resolves to 0.3.6; the direct
`gpui-pre-reqwest-client` alias now requires exactly `=0.3.6`, matching Kit's
new snapshot pin policy. Future Kit upgrades must check this direct pin too.

GPUI snapshots can change Rust APIs within a patch version. This release
adapts Kit's inspector registration to GPUI's per-window factory and prevents
fresh dependency resolution from selecting an untested snapshot. clj-gpui
uses Kit's inspector integration and needs no separate registration change.
The lockfile changes only the Kit/GPUI family; other package versions are
unchanged. The obsolete transitive `gpui-pre-media` dependency disappears.

## Masked labels

Kit now suppresses highlight measurement while a Label is masked, fixing
invalid UTF-8 offsets when secondary text or search highlights accompany
ASCII, accented, emoji or CJK text.

Removed clj-gpui's workaround that flattened secondary text and discarded
highlight options before constructing a masked Label. The host now forwards
those properties directly. Existing behavior remains: both text parts become
bullets while masked, and unmasking restores secondary styling and highlights.
The separate relative line-height adjustment remains necessary for headings.

Replaced tests that copied the old Kit highlight algorithm with a production
RootView regression covering masked/unmasked/remasked tree updates. A Metal
pixel check compares masked output with literal bullets, verifies restored
styles on reveal, and checks remasking.

## API audit and possible follow-ups

Regenerated the [Component](inventory/gpui-component-0.6.6.tsv) and
[Base](inventory/gpui-base-0.6.6.tsv) source indexes. Their 4,050 and 3,239
declaration entries are unchanged apart from source line locations. Reviewing
the tagged source diff also found no new Kit widgets, builders, events or
features to bind. Existing parity work and the remaining extension gaps are
recorded in [the current audit](gpui-kit-parity.md).

The underlying GPUI snapshot adds `App::on_missing_glyphs`, an opt-in callback
for grapheme clusters that exhaust font fallback. A subsequent binding adds
root-window `:on-missing-glyphs` using the existing protocol-v11 callback
transport. See [font diagnostics](font-diagnostics.md) for payloads and
limitations: the published backend emits reports on Linux, but not macOS or
Windows DirectWrite. The new shaped-line, underline and atlas APIs are
low-level native rendering facilities, not missing Kit widget properties.

Select dismissal forwarding and Accordion per-item disabled handling are
still upstream limitations in the published 0.6.6 source. This release does
not include fixes for them. The TodoMVC font workaround also remains; its
original interactive glyph-clipping case has not been revalidated here.

## Validation

Passed locally on macOS:

- `./scripts/ci.sh`: 354 Rust tests, the separate missing-monospace fixture,
  strict Clippy (including the standalone rendering target), the normal host
  build, 131 Clojure tests / 2,420 assertions, cljfmt, and the protocol-v11
  socket test including callbacks, asynchronous providers and reload.
- `cargo test --locked --manifest-path host/Cargo.toml --test rendering`:
  seven production RootView checks with the actual Metal renderer, including
  masked labels, nested composition, editor/Markdown updates, local media
  and pie radii.
- `cargo fmt --manifest-path host/Cargo.toml --check` and `git diff --check`.
- `cargo tree --locked --manifest-path host/Cargo.toml -d`: one compatible
  Kit/GPUI family. Cargo lists collections twice for separate build contexts,
  both at 0.3.6.

The Rust count decreases by one because two tests of the copied highlight
algorithm were replaced by one test of the production renderer. The expected
third-party `block 0.1.6` future-incompatibility notice remains.

Interactive native-window/IME, Windows, Linux rendering and hosted CI were
not exercised in this local migration.

## Upstream references

- [0.6.6 release](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.6)
- [Complete 0.6.4 to 0.6.6 diff](https://github.com/longbridge/gpui-kit/compare/v0.6.4...v0.6.6)
- [Exact GPUI snapshot pins](https://github.com/longbridge/gpui-kit/pull/3163)
- [GPUI 0.3.6 and inspector adaptation](https://github.com/longbridge/gpui-kit/pull/3147)
- [Masked-label fix](https://github.com/longbridge/gpui-kit/pull/3142)
