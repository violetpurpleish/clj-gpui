# GPUI Kit parity audit

Audit target: the repository's locked **gpui-kit 0.6.4**, **gpui-component 0.6.4**, **gpui-base 0.6.4**, and **gpui-pre 0.3.5**. This is a source audit, not an assertion that having a constructor is equivalent to exposing every native API. `gpui-shell` and `gpui-wry` remain the two deliberate subsystem exclusions.

The reproducible source indexes record [Component declarations and trait contracts](inventory/gpui-component-0.6.4.tsv) and [Base declarations and trait contracts](inventory/gpui-base-0.6.4.tsv). These indexes include implementation modules; public reachability and macro-generated APIs require the family audit below. Regenerate it with:

```sh
python3 scripts/inventory-kit.py /path/to/gpui-component-0.6.4/src > docs/inventory/gpui-component-0.6.4.tsv
python3 scripts/inventory-kit.py /path/to/gpui-base-0.6.4/src > docs/inventory/gpui-base-0.6.4.tsv
```

The checks below include inherent builders, `ParentElement`/typed child contracts, state and delegate rendering hooks, callback normalization in `gpui.ui`, wire deserialization, and the production renderer. Native implementation details such as entities, focus handles, scroll handles and action types are represented by retained host state and declarative properties, rather than passed across the JVM boundary.

## Child composition

The principal defect was architectural: RootView, dialog content, table cells, dock panels, navigation pages, and virtual rows did not share one widget renderer. Several surfaces accepted a widget map but painted a restricted substitute or omitted it.

Native deferred builders now use a per-window weak reference to RootView. The actual widget renders after the enclosing entity's render borrow has ended. Slots have structural addresses, read fresh content from the current tree, and retain the ordinary native control state. Initialized controls remain alive while their nodes are in the tree, including when virtualization skips their paint; retained callback references refresh on every incoming tree; removing the nodes releases them. Offscreen controls are not eagerly initialized. This covers inputs, editors, lists, nested tables, date pickers, menus, and the other ordinary widgets in arbitrary `IntoElement` slots. Typed native contracts remain typed: for example Form takes Fields, InputGroup takes an Input or Textarea plus Addons, and ButtonGroup takes Buttons.

| Surface | Previous gap | Binding now |
|---|---|---|
| Dialog / AlertDialog / Sheet | Static child subset; only specially prepared dialog inputs | Production child renderer; dialog input focus/submit scoping retained |
| Popover / HoverCard | Static content; Popover required a button trigger | Production content and arbitrary selectable trigger |
| DataTable cells | Restricted RenderOnce subset | Production widgets; stable row/column identities retained |
| Dock panel bodies | Static content subset | Production child renderer |
| NavStack pages | Static content subset | Production child renderer |
| VirtualScroll / MessageScroller | Separate static renderer omitted stateful widgets | Production widgets alongside the existing transcript and follow behavior |
| VirtualList rows | Labels only | Item `:content` |
| List / Tree rows | Labels only | Item `:content` |
| Select / Combobox options | Strings only | Item `:content`, Select `:display` widget, group `:header` |
| Input / NumberInput affixes | Strings only | String or widget `:prefix` / `:suffix` |
| Empty states | Strings only | String or widget `:empty` |
| Command rows/header/footer | Restricted item presentation | Item `:content` / `:children`, widget `:header` / `:footer` |
| DescriptionList values/labels | Strings only | Widget label/value and separators |
| Notification / Tooltip | Text only | `:content`, `:action`; `:tooltip-content` |

## Widget inventory

“Existing” means the family already had a binding before this audit. The final column records the native presentation contract checked and changes made here. The outstanding API list below is part of this inventory; it must not be hidden behind a green checkmark.

| Native family / public parts | Clojure API | Audit findings and changes |
|---|---|---|
| Button, ButtonCustomVariant, ButtonIcon | `button`, icon options | Existing variants, sizing, accessibility, focus, loading, custom colors; child widgets and `:on-hover` |
| ButtonGroup | `button-group` | Added native grouped buttons, axis, multiple selection, compact/outline, disabled, index-vector callback |
| Toggle, ToggleGroup | `toggle`, `toggle-group` | Existing controlled checks/variants; added child content |
| DropdownButton | `dropdown-button` | Existing split button and native menu adapter |
| Checkbox | `checkbox` | Existing click contract; added boolean `:on-change` and children |
| Radio, RadioGroup | `radio`, `radio-group` | Added standalone Radio; group custom content and item disabled forwarding |
| Switch | `switch` | Added `:checked-color` for native checked-track color |
| Slider | `slider` | Existing range, scale, bounds, step, reverse, Change/Release and retained state |
| Rating | `rating` | Existing max, value, color, size, disabled and activation |
| Input, InputState | `input` | Widget affixes, native context menu, role/tab index and selection requests; existing masking/autofill/change/submit/blur |
| Textarea, TextareaState | `textarea` | Native context menu and role/tab index; wrapping, whitespace/tab options, auto-grow, search/replace, search snapshots and selection requests; retained text and submit behavior |
| Editor, EditorState | `editor` | Native context menu and role/tab index; native wrapping/folding/line-number/indent/whitespace/tab and scroll-margin controls, selection requests, diagnostics, tracked decorations, search/replace, search snapshots, clipboard interception and asynchronous completion/inline completion/hover/definition/color/semantic-token/code-action providers; existing highlighting, indentation and autoclose |
| NumberInput | `number-input` | Widget affixes; existing numeric stepping/bounds and text-state adapter |
| OtpInput, OtpState | `otp-input` | Existing cell count/grouping/masking/focus and complete-value event |
| InputGroup, InputGroupInput, InputGroupTextarea, InputGroupAddon, InputGroupButton, InputGroupText | `input-group`, `input`, `textarea`, `input-group-addon`, `input-group-button`, `input-group-text` | Added native group frame, four addon alignments, inherited disabled/readonly/size, invalid state, group accessibility |
| Form, Field | `form`, `field` | Added native grid/label layout, label width/text size, footer, label/description widgets, required/visible/indent/alignment and column placement |
| Collapsible | `collapsible` | Added native open state, persistent children, conditional content and motion identity |
| Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent | `empty` family | Added all six typed composition parts and media variant |
| Carousel, CarouselState, CarouselContent, CarouselItem, CarouselPrevious, CarouselNext, CarouselPagination, CarouselPaginationItem | `carousel` family | Added retained native state, controlled index, axis/looping, Change, track style, children, labels and focus ring |
| Calendar, CalendarState | `calendar` | Added standalone calendar, controlled date/range, month count, first weekday, year range, disabled dates and Selected event |
| DatePicker, DatePickerState, DateRangePreset | `date-picker` | Added date format, year range, disabled dates and presets; existing range/clear/appearance |
| ColorPicker, ColorPickerState | `color-picker` | Existing controlled value, featured colors, trigger icon/label/anchor and accessibility |
| Label | `label` | Existing masked/secondary/highlighted text and clipping |
| Icon / IconName | `icon` | Existing full bundled catalog and inline SVG; path and declarative scale/translate/rotate transform |
| Link | `link` | Added arbitrary children; existing href/disabled/click |
| Tag | `tag` | Added arbitrary children; existing variants/colors/outline/rounding |
| Badge | `badge` | Existing arbitrary child, dot/count/icon/max/color |
| Alert | `alert` | Existing semantic variants, icon/title/banner/visible/close |
| Progress / ProgressCircle | `progress`, `progress-circle` | Existing determinate/indeterminate/color/size/accessibility; circle children |
| Skeleton | `skeleton` | Existing primary/secondary native presentation |
| Spinner | `spinner` | Existing icon/color/control size and layout wrapper |
| ShimmerText / ShimmerStyle | `shimmer` and chat shimmer options | Existing duration/highlight/spread/reverse/once and stable identities |
| Kbd | `kbd` | Existing keystroke/appearance/outline |
| Clipboard | `clipboard` | Existing controlled copy value/event; native tooltip forwarding |
| Separator | `separator` | Existing axis/label/color/dashed |
| GroupBox | `group-box` | Added title/content styles; existing variants and child composition |
| Accordion / AccordionItem | `accordion` | Added title/content/hover style slots; existing multiple/open/icon/children |
| DescriptionList / DescriptionItem | `description-list` | Added widget label/value, separator and span preservation |
| Breadcrumb / BreadcrumbItem | `breadcrumb` | Existing typed labels, disabled, activation and original ids |
| TabBar / Tab | `tabs`, `tab` | Added standalone Tab, rich item children/icons/disabled, bar prefix/suffix and Tab slots |
| Stepper / StepperItem | `stepper`, `stepper-item` | Added standalone item and custom item children; existing selected id, axis, icon/disabled |
| Pagination | `pagination` | Existing current/total/visible pages, compact, size, disabled and activation |
| Select / SelectState / SelectGroup | `select` | Added row/display/empty widgets and custom section headers; existing grouped filtering and value identity |
| Combobox / ComboboxState | `combobox` | Added row/empty widgets, custom group headers, trigger/footer; existing multiple/query/selection callbacks |
| List / ListState / ListItem / ListSeparatorItem | `list`, `list-item`, `list-separator-item` | Added custom rows, grouped sections/header/footer, empty/initial/loading widgets, controlled query/query event and external filtering; standalone item parts |
| SearchableListItemElement | `searchable-list-item` | Added public presentation primitive; selection/search state remains native inside Select/Combobox |
| DataTable / TableState / Column / ColumnGroup | `data-table` | Added arbitrary cells, row/header/group/last-column presentation, empty/loading widgets, context menus, visible-range events and scroll requests; existing sorting/selection/reorder/resize/export/loading |
| Table / Header / Body / Footer / Row / Head / Cell / Caption | `table` family | Existing native typed sections and arbitrary cell children; uses shared production child renderer |
| Tree / TreeState / TreeItem | `tree` | Added custom row content; existing nested expansion and controlled visible selection |
| VirtualList | `virtual-list` | Added arbitrary row content; existing per-row extent, axis and retained scroll handle |
| Dialog / DialogButtonProps / AlertDialog | `dialog`, `alert-dialog` | Added custom footer, margin/overlay/show-cancel; existing native focus, keyboard and controlled overlay lifecycle |
| DialogContent / Header / Title / Description / Footer / Close / Action | `dialog-*` | Added native composition parts and close/confirm dispatch |
| Sheet | `sheet` | Shared child renderer; existing native placement/size/resizable/overlay/footer/close |
| Popover | `popover` | Added general trigger, trigger style, mouse button, appearance, anchor and overlay dismissal |
| HoverCard | `hover-card` | General native trigger/content; existing anchor/delays/appearance/open event |
| PopupMenu / PopupMenuItem / ContextMenu / DropdownMenu | menu constructors and `:context-menu` | Added custom item content and context-menu attachment on ordinary nodes; existing nested actions/checks/icons/disabled |
| AppMenuBar | `app-menu-bar` | Added retained in-window menu bar, nested semantic actions and menu reload on presentation changes |
| NativeMenu | `native-menu`, input `:context-menu` | Existing native Action bridge; added native input menu recipes |
| Command / CommandGroup / CommandItem / CommandState | `command` | Added rich items, header/footer/empty slots; existing native search/query/highlight/confirm/cancel/loading |
| Tooltip | `:tooltip`, `:tooltip-content` | Added arbitrary widget content and keybinding text |
| Notification / NotificationList | `notification` | Added widget content/action and system/in-app delivery; retained live click callback and dismissal lifecycle |
| Avatar / AvatarGroup | `avatar`, `avatar-group` | Existing typed avatars, HTTP/assets/local media, placeholder, limit/ellipsis |
| Sidebar / SidebarGroup / SidebarMenu / SidebarMenuItem / SidebarHeader / SidebarFooter / SidebarToggleButton | `sidebar` family | Added typed composition, headers/footers, nested menus, suffixes, initial expansion and click-to-open/toggle |
| Settings / SettingPage / SettingGroup / SettingItem / SettingField | `settings` | Added root/header/sidebar styles, default selection, custom content, per-page icons/descriptions/header/suffix/reset/expansion and field reset/default/dirty controls; existing typed callbacks |
| DockArea / DockLayout / Panel / DockSkin | `dock` | Shared child renderer, layout save/restore, panel lifecycle events, dock lock/open/size/collapse requests, controlled panel selection, title/suffix/style, toolbar/menu and per-panel title/zoom/visibility controls |
| ResizablePanelGroup / ResizablePanel / ResizableState | `resizable` | Existing typed panels, controlled sizes and resize events |
| TitleBar | `title-bar` | Existing child widgets, native titlebar geometry and window title handling |
| StatusBar | `status-bar` | Existing left/right/ordinary children and native styling |
| Root / WindowBorder | host window | Host-owned window root/overlay layers and frame; not child constructors |
| Scrollable / scroll handles | `scroll`, `virtual-scroll` | Existing viewport/axis/scroll state; shared child rendering |
| Message / MessageGroup / Avatar / Header / Content / Footer | `message-*` | Existing typed slots; arbitrary child widgets now share RootView |
| Bubble / BubbleContent / BubbleGroup / BubbleReactions | `bubble-*` | Existing alignment/variant/action contracts and children |
| Attachment / Media / Content / Title / Description / Actions / Group | `attachment-*` | Existing status/media/shimmer/click/typed slot contracts |
| Marker / MarkerIcon / MarkerContent | `marker-*` | Existing role/variant/loading/shimmer/separator and children |
| MessageScroller | `message-scroller` | Shared native row renderer; existing append/prepend/remeasure/follow/style/jump-button hooks |
| NavStack / NavPage | `nav-stack`, `nav-page` | Shared native page renderer; existing host-evaluated transition recipes and retained back/forward pages |
| LineChart / AreaChart / BarChart / PieChart / RadarChart / CandlestickChart / SankeyChart | `chart` and named helpers | Existing data/series/color/axes/labels/gradient/radius/interpolation and tooltip content adapters |
| TextView (Markdown / HTML), TextViewStyle, FrontmatterPlugin | `markdown`, `html` | Added stream fade/motion/style, scrollable, selection format, max lines, code/table action slots, link events, MDX and source-targeted native block/inline plugins |
| Theme / ThemeSet / ThemeRegistry / semantic tokens | `gpui.theme`, `:theme` | Existing named/custom themes and semantic configuration |

## Remaining native extension APIs

The widget changes above do **not** establish exhaustive Rust API equivalence. These are still unbridged native extension surfaces, not additional deliberately excluded subsystems:

- Custom `Plot` implementations and raw plot layer/shape/scale composition. The seven native chart families are bound; implementing a new Rust plot is not represented by a Clojure protocol.
- Runtime highlighter parser factories and general motion sequences. The installed grammars, declarative widget styles, navigation recipes and TextView motion options are supported.
- Arbitrary synchronous predicates/delegates that inspect native App/Window/state, including InputBaseState `on_will_change`, custom Select matching/delegate selection policy, and a NavStack `item` renderer that replaces the native page. Existing controlled values, validation patterns, list external filtering and navigation recipes are narrower contracts.
- Markdown recipes target existing AST occurrences and return arbitrary widgets, but do not pass the entire native AST or inline layout context to a Clojure callback. Code/table action slots are static widget recipes, not functions receiving each block's parsed data.
- Dynamically computed completion-trigger predicates. The new provider bridge supports the result-bearing methods listed in [editor-providers.md](editor-providers.md).

These require further protocol/API work to reach literal native-API parity. They must not be represented as completed by counting constructors. Native entities, focus/scroll handles, Root/WindowBorder, internal LSP popovers, debug inspectors and undo/geometry helper types remain host implementation objects, with their user-visible behavior exposed through the owning widgets.

The pinned Base `CompletionProvider::resolve_completions` hook has no caller and takes the uninhabited `lsp_types::request::Completion` marker instead of completion items; it is not an operational feature that the binding can expose.

The native Select wrapper in 0.6.4 does not forward BaseSelect's dismissal event. Accordion 0.6.4 overwrites individual item disabled state with its parent flag. These are upstream limitations, not silently omitted binding properties.

## Verification

The production tests in `host/src/renderer_integration_tests.rs` run `RootView` inside the actual Kit `Root`. They check stateful children under dialogs, sheets, dock panels, navigation pages, data-table cells and both scrollers; native control entity retention; anonymous slot identities; new native families; live callbacks after tree replacement; controlled calendar updates without callback echo; Markdown block/inline controls; provider request/reply routing; dock layout echoes without reload loops; offscreen callback freshness; Unicode selection clipping; annotation updates; and native Textarea growth/search without entity replacement. Clojure tests exercise normalization, nullable booleans and callback export. Explicit platform rendering tests are separate from the headless interaction suite.

Validated on macOS against the locked dependencies:

- `cargo test --locked --manifest-path host/Cargo.toml`: **355 passed**, plus the separate missing-monospace process check.
- `clojure -M:test`: **131 tests / 2,420 assertions**, no failures or errors.
- `cargo clippy --locked --manifest-path host/Cargo.toml --all-targets --test rendering -- -D warnings`, Rust formatting and Clojure formatting: passed.
- Real JVM/host protocol test: callbacks and reload passed; an asynchronous provider can wait for a button callback without blocking it, and stable provider ids resolve refreshed Clojure functions.
- Explicit `--test rendering`: **6 Metal checks passed** using production RootView. The composition screenshot was inspected.

These results do not establish Windows/Linux rendering, physical IME behavior, or integration with an external language-server process.

Usage: [widget composition](widget-composition.md), [editor providers/search/clipboard](editor-providers.md).
