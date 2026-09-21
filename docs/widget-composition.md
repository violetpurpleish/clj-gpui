# Widget composition

The same production renderer handles root children, overlay bodies, collection cells, rich slots, dock panels, navigation pages and virtualized rows. Stateful controls retain their native entities while their logical identity remains mounted. Use unique explicit `:id` values within the window for application-owned controls; anonymous slots receive structural identities. Scrolling a virtual row out of view retains an initialized control until its node leaves the tree. Reordering a collection with stable row/column ids preserves its cell identities.

Native typed child contracts still apply. A Form contains Fields, a ButtonGroup contains Buttons, an AvatarGroup contains Avatars, and an InputGroup contains an Input or Textarea plus Addons. Slots accepting arbitrary native elements accept ordinary clj-gpui widgets.

```clojure
(ui/form {:columns 2 :label-layout :vertical}
  (ui/field {:title "File" :required true :col-span 2
             :description (ui/label "Choose a name for this source file.")}
    (ui/input-group {:id "filename-group"}
      (ui/input @!filename {:id "filename" :on-change #(reset! !filename %)})
      (ui/input-group-addon {:align :inline-end}
        (ui/input-group-text ".clj")))))

(ui/select selected
  {:options [{:id :local :label "Local"
              :content (ui/hstack (ui/icon :folder) (ui/label "Local files"))
              :display (ui/tag "Local")}]
   :on-change set-selected!
   :empty (ui/empty
            (ui/empty-header
              (ui/empty-title "No locations")))})
```

InputGroup addon alignments are `:inline-start`, `:inline-end`, `:block-start`, and `:block-end`. The group forwards disabled, readonly, size and invalid state to native controls. Other new composition families are `collapsible`, `carousel-*`, `empty-*`, `dialog-*`, `sidebar-*`, standalone `radio`, `tab`, `stepper-item`, `list-item`, `list-separator-item`, and `searchable-list-item`. Their docstrings list their options.

## Rich slots

| Widget | Additional content/options |
|---|---|
| Input / NumberInput | string or widget `:prefix`, `:suffix` |
| Select | option `:content`, string/widget `:display`, grouped option `:header`, widget `:empty` |
| Combobox | option `:content`, group `:header`, widget `:empty`, `:trigger`, `:footer` |
| List | item `:content`, group `:header` / `:footer`, widget `:empty`, `:initial-content`, `:loading-content`; `:query`, `:on-query`, `:filterable false` for externally supplied results |
| DataTable | arbitrary cell widgets; column `:content`, header-group `:content`, `:last-column-content`, `:loading-content`, widget `:empty`, row style/header style, `:context-menu` |
| DataTable scrolling | `:scroll-to-row`, `:scroll-to-column` accept id or index; change `:scroll-generation` to repeat; `:on-visible-rows` / `:on-visible-columns` receive `{:start :end}` with exclusive end |
| Command | item `:content` / `:children`, widget `:header`, `:footer`, `:empty`, `:on-matched-count` |
| DescriptionList | widget labels/values, separators, column spans |
| Tooltip | `:tooltip-content`, optional `:tooltip-key` keystroke |
| Notification | widget `:content`, `:action`; `:delivery :system` or in-app default |
| Menus | item `:content`, `:variant :label`, `:href`; `:menu-width`, `:menu-min-w`, `:menu-max-w`, `:menu-max-h`, `:scrollable`, `:check-side`, `:external-link-icon` |
| Ordinary widgets | `:context-menu` attaches a native Kit popup menu; text controls use native menu recipes |

List query callbacks return the string. External search can set `:loading` while updating `:items`; `:filterable false` avoids applying a second substring filter to the supplied results. Kit displays `:initial-content` while its search field is empty. Custom loading content remains inside the native List, preserving its search field.

## Dock and Settings

Dock item bodies accept stateful widgets. `:selected` selects a panel by id. Item `:children` supplies toolbar Buttons and `:items` supplies its popup menu. Panel options include `:header`, `:suffix`, `:title-style`, `:title-bar`, `:inner-padding`, `:closable`, `:zoomable`, `:visible`, `:tab-name`, and `:zoom-control`.

`:on-layout-change` receives Kit's JSON layout dump; store it and send it back as `:dock-layout` to restore the layout. Panel ids in a saved layout must exist in the current item catalog. Restoration reuses those panel entities. An echoed layout does not trigger another load. `:dock-options` accepts `:locked` and per-region `:left`, `:right`, `:bottom` maps with `:size`, `:open`, and `:collapsible`. These requests apply when their values change.

`:on-panel-event` receives `{:id wire-panel-id :event :value}` for `"active"`, `"zoomed"`, `"added"`, and `"removed"`; activation/zoom values are booleans. Panel ids are wire strings in this event.

Settings adds `:header-style`, `:sidebar-style`, `:default-selected-index`; page `:icon`, `:description`, `:header`, `:suffix`, `:default-open`, `:resettable`; and custom field `:content`. Field `:default-value` controls native reset behavior. Custom fields can provide `:dirty` and zero-argument `:on-reset`. Field `:disabled` is preserved when live content is refreshed.

## Markdown block and inline widgets

`:extensions` is a vector of replacement recipes, or a function receiving the current Markdown string and returning that vector. Each recipe targets **one complete native Markdown AST node**. Use `:source` for its first exact occurrence, `:source-pattern` for the first regex match, or explicit UTF-8 byte `:source-range [start end]` to distinguish repeated occurrences. An unmatched or partial-node range leaves ordinary Markdown intact.

```clojure
(ui/markdown "```widget\nsettings\n```\n\nOpen [status](widget)."
  {:id "rich-document"
   :extensions
   [{:id :settings :source "```widget\nsettings\n```"
     :text "Settings"
     :content (ui/input @!name {:id "embedded-name"
                               :on-change #(reset! !name %)})}
    {:id :status :variant :inline :source "[status](widget)"
     :text "Ready" :baseline 14
     :content (ui/tag "Ready")}]
   :selection-format :plain})
```

Block is the default; `:variant :inline` uses Kit's native inline element, with optional baseline in pixels. `:text` supplies plain-text copy/accessibility fallback; the original source remains available for source-format selection. Inline recipes without content use Kit's atomic text fallback. Give repeated widgets distinct ids/ranges. Renderer-only content changes do not reparse the Markdown document.

Markdown/HTML also accept `:text-style`, `:text-motion`, `:stream-fade`, `:scrollable`, `:max-lines`, `:selection-format`, `:code-block-actions`, `:table-actions`, and `:on-link-click`. Markdown accepts `:mdx` and `:frontmatter`.

Text style keys: `:paragraph-gap` (rem), `:heading-base-font-size` (px), `:heading-font-sizes`, `:code-block`, `:table`, `:table-head`, `:table-cell`, `:inline-code`, and `:is-dark`. Text motion keys: `:duration`, `:stagger` (seconds), and `:easing` (`:linear`, `:ease`, `:ease-in`, `:ease-out`, `:ease-in-out`).
