# Editor configuration, providers and search

`ui/editor` keeps its native `EditorState` while Clojure supplies functions in `:lsp`. A stable editor `:id` is required. Provider functions receive one parameter map and return a JSON-compatible value or a Clojure future. They execute away from the bridge reader, so edits, renders and ordinary callbacks continue while a provider waits.

```clojure
(ui/editor source
  {:id "source-editor"
   :language "clojure"
   :on-change #(reset! !source %)
   :lsp {:completions
         (fn [{:keys [text offset trigger]}]
           [{:label "println" :kind 3 :insertText "println"}])
         :hover
         (fn [{:keys [text offset]}]
           {:contents {:kind "markdown" :value "Clojure source"}})
         :trigger-characters ["." "/"]
         :inline-debounce-ms 300
         :completion-menu-width 480}})
```

Result objects follow the native `lsp-types 0.97` schema, including **camelCase** LSP field names. Request `:offset` and `:range` are UTF-8 byte positions in the supplied `:text`; LSP result `Position.character` values use UTF-16 code units. Do not use JVM string indexes as UTF-8 offsets.

| Function | Request fields | Result |
|---|---|---|
| `:completions` | `:text`, `:offset`, LSP `:trigger` | CompletionItem vector or CompletionList |
| `:inline-completions` | `:text`, `:offset`, LSP `:trigger` | InlineCompletionItem vector or InlineCompletionList |
| `:hover` | `:text`, `:offset` | Hover or nil |
| `:definitions` | `:text`, `:offset` | LocationLink vector |
| `:document-colors` | `:text` | ColorInformation vector |
| `:semantic-tokens` | `:text`, byte `:range {:start :end}` | SemanticTokens; supply `:legend {:tokenTypes [...] :tokenModifiers [...]}` alongside the function |
| `:code-actions` | `:text`, byte `:range` | CodeAction vector |
| `:perform-code-action` | `:text`, `:action`, `:push-to-history` | nil or `{:edits [TextEdit ...]}`; edits address the supplied document and are rejected if it changed while awaiting the result |
| `:show-document` | native ShowDocumentParams | JSON-compatible result, ignored; `:show-document-policy :consume` suppresses native URL fallback |

The provider registry uses the editor id and method, independently of short-lived UI callback ids. Re-rendering replaces the Clojure function behind that stable id. Add a changed `:revision` inside `:lsp` to force a native provider refresh without replacing the editor. Removing `:lsp` removes providers. Requests time out after 30 seconds; errors reach the native provider task. Cancelling a native request does not interrupt an already-running Clojure function. This adapter supplies native providers; it does not start or configure a language-server process.

## Search and replacement

Editor and Textarea accept `:search` to control the native search session. Omit `:open` to drive highlights from a custom search UI; `:open true` shows Kit's own panel. `:open false` or `:action :close` closes the search session. Supported keys are `:query`, `:case-insensitive` (default true), `:enabled`, `:replace-mode`, `:replacement`, `:action` and `:generation`.

Actions are `:next`, `:previous`, `:replace`, `:replace-all` and `:close`. Change `:generation` to repeat an unchanged action. Unrelated renders do not repeat an action or refocus the search panel. Replacements emit the ordinary editor `:on-change` callback.

`:on-search` receives `{:query :replacement :open :active :replace-mode :case-insensitive :count :current}` when the native session changes. `:current` is a zero-based match index or nil. This includes changes made in Kit's search panel, so an application can display an accurate match count.

## Clipboard interception

Input, Textarea and Editor accept `:on-paste` and `:paste-policy`. The event is `{:text ... :entries [...]}`; entries are text with metadata, an image with `:mime-type` and base64 `:data`, or files with `:paths`.

Default policy lets Kit paste normally. `:consume` suppresses native insertion; `:consume-non-text` suppresses it when any entry is an image or file. The policy is evaluated locally at the paste event; the callback's return value does not decide whether a paste is consumed.

## Native editor options and annotations

Editor supports `:soft-wrap`, `:folding`, `:line-number`, `:indent-guides` (default true), `:show-whitespaces`, `:hard-tabs` (default false), `:tab-size` (default 2), and `:wrapping-indent` (`:same` or `:none`). `:scroll-beyond-last-line` and `:cursor-surrounding-lines` take row counts; omission restores native automatic margins. Removing the boolean properties restores their defaults without replacing the editor. Placeholder changes and `:focus` also apply to the retained editor.

Textarea supports wrapping, whitespace and tab options, plus `:auto-grow [min-rows max-rows]`. Omit auto-grow to use the requested `:rows` height.

Input, Textarea and Editor accept `:selected-range [start end]` using UTF-8 bytes. It is applied when the range or `:selection-generation` changes, allowing subsequent native cursor movement. An unchanged export does not select the text again.

```clojure
(ui/editor source
  {:id "annotated-source"
   :diagnostics [{:range {:start {:line 0 :character 1}
                         :end {:line 0 :character 3}}
                  :severity 2 :message "Example warning"}]
   :decorations [{:range [1 4] :background "#553311"
                  :underline :wavy :underline-color "#ffaa00"}]})
```

Diagnostics use the complete native LSP Diagnostic schema, with UTF-16 positions. They are refreshed for a changed diagnostics vector or document; `[]` clears them. Decorations use UTF-8 byte ranges and native tracked ranges: edits move them until `:decorations` or `:decoration-generation` changes. Supported highlight keys are `:color`, `:background`, `:font-weight`, `:italic`, `:underline` (true or `:wavy`), `:underline-color`, `:strikethrough`, `:strikethrough-color`, and `:fade-out` (0–1). An empty vector clears the collection. Native character-boundary clipping applies to ranges.
