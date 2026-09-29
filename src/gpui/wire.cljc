(ns gpui.wire
  "Shared Clojure/ClojureScript UI serialization and callback generations.
  Internal protocol implementation; applications use gpui.ui."
  (:require [gpui.ui :as ui]))

(defonce ^:private callbacks (atom {}))
(defonce ^:private providers (atom {}))
(def ^:dynamic *provider-export* nil)
;; Never reset: an old native event must not name a new callback.
(defonce ^:private callback-counter (atom 0))
(defonce ^:private callback-depth (atom 0))
(defonce ^:private callback-hold (atom 0))

(defn callback-active? []
  (or (pos? @callback-depth) (pos? @callback-hold)))

(defn finish-callbacks! []
  (reset! callback-hold 0))

(defn- register-callback!
  [f]
  (let [id (str "cb-" (swap! callback-counter inc))]
    (swap! callbacks assoc id f)
    id))

(def ^:private callback-keys
  [:on-current-change :on-complete :on-remove :on-retry :on-dismiss :on-token-click :on-content-change :on-reveal :on-text-state :on-missing-glyphs :on-layout-change :on-panel-event :on-click :on-change :on-release :on-submit :on-double-click :on-blur
   :on-escape :on-close :on-copied :on-ok :on-cancel :on-confirm
   :on-open-change :on-forward-change :on-search :on-query :on-select :on-export
   :on-reset :on-sort :on-load-more :on-link-click :on-hover :on-paste :on-visible-rows :on-visible-columns :on-matched-count])

(declare sanitize)

;; These are the non-nullable bool fields on protocol::Node / Item. Nullable
;; overrides (Option<bool>) deliberately do not appear here. Keep the lists
;; aligned with the schema; the boolean tests audit them against protocol.rs.
(def ^:private node-boolean-flags
  #{:primary :compact :strikethrough :truncate :shadow :focus :disabled
    :dot :dashed :outline :selected :searchable :cleanable :multiple
    :frontmatter :masked :readonly :mask-toggle :collapsed :range :loading
    :overflow-hidden :reverse :once :ellipsis :has-more :dropdown-caret
    :caret :banner :segmented})

(def ^:private item-boolean-flags
  #{:disabled :separator :expanded})

(def ^:private node-boolean-overrides
  #{:arrow :y-axis :grid-dashed :soft-wrap :folding :line-number :indent-guides :show-whitespaces :hard-tabs :external-link-icon :mdx :checked :interactive :filterable :bordered :focus-ring :open :overlay-closable
    :autohide :auto-close :smart-indent :label-axis :value-axis :grid :labels :x-axis
    :node-label :value-label :reuse-forward :appearance :content-inset :scrollbar
    :jump-button :scroll-to-end :cell-selectable :row-header :stripe :sortable
    :col-movable :col-resizable :col-fixed :loop-selection :row-selectable
    :col-selectable :selectable :toggled :tab-stop :visible :close-button :keyboard
    :default-open :click-to-open :click-to-toggle :overlay :resizable :menu :looping :invalid :required :label-indent :show-cancel :stream-fade :scrollable})

(def ^:private item-boolean-overrides
  #{:resettable :dirty :scrollable :title-bar :inner-padding :closable :zoomable :visible :selectable :resizable :movable :checked :default-open :click-to-open :click-to-toggle})

(def ^:private styled-boolean-overrides
  #{:strikethrough :shadow :truncate :overflow-hidden})

(defn- field-path [path k]
  (str path (when (seq path) ".") (name k)))

(defn- prepare-boolean-fields
  [m flags overrides path]
  (reduce-kv
   (fn [m k value]
     (if (or (contains? flags k) (contains? overrides k))
       (do
         (when-not (or (nil? value) (boolean? value))
           (let [location (field-path path k)
                 value-type (cond
                              (fn? value) "function"
                              (keyword? value) "keyword"
                              (string? value) "string"
                              (number? value) "number"
                              (map? value) "map"
                              :else #?(:clj (.getSimpleName (class value))
                                       :cljs (goog/typeOf value)))]
             (throw (ex-info (str "invalid UI boolean at " location
                                  ": expected true, false, or nil, got " value-type)
                             {:path location :value-type value-type}))))
         (if (and (nil? value) (contains? flags k)) (assoc m k false) m))
       m))
   m m))

(defn- map-typed-sequence [f xs path]
  (if (sequential? xs)
    (mapv (fn [i x] (f x (str path "[" i "]"))) (range) xs)
    xs))

(defn- update-typed [m k f path]
  (if (contains? m k) (update m k f (field-path path k)) m))

(defn- update-typed-sequence [m k f path]
  (update-typed m k (partial map-typed-sequence f) path))

(declare prepare-node-booleans)

(defn- prepare-item-booleans [item path]
  (if (map? item)
    (let [item (-> (prepare-boolean-fields item item-boolean-flags item-boolean-overrides path)
                   (update-typed-sequence :children prepare-node-booleans path)
                   (update-typed-sequence :items prepare-item-booleans path)
                   ;; A TableCell object is a Node even without a :type key.
                   (update-typed-sequence :cells prepare-node-booleans path))]
      (reduce (fn [m k] (update-typed m k prepare-node-booleans path))
              item [:tooltip-content :content :style :label-style :display-content :suffix :title-style :content-style :hover-style :header :footer]))
    item))

(defn- prepare-nav-case-booleans [recipe path]
  (if (map? recipe)
    (prepare-boolean-fields recipe #{} styled-boolean-overrides path)
    recipe))

(defn- prepare-nav-booleans [recipe path]
  (cond
    (map? recipe)
    (-> (prepare-nav-case-booleans recipe path)
        (update-typed-sequence :match prepare-nav-case-booleans path))
    (sequential? recipe) (map-typed-sequence prepare-nav-case-booleans recipe path)
    :else recipe))

(defn- prepare-custom-variant-booleans [variant path]
  (if (map? variant)
    (prepare-boolean-fields variant #{} #{:shadow} path)
    variant))

(defn- prepare-node-booleans
  "Prepare typed UI locations only. Opaque values, callback data and chart
  payloads are never traversed. NavStack recipes/custom variants validate their
  own nullable overrides, without applying ordinary Node flag defaults."
  [node path]
  (if (map? node)
    (let [node (prepare-boolean-fields node node-boolean-flags node-boolean-overrides path)
          node (reduce (fn [m k]
                         (update-typed-sequence m k prepare-node-booleans path))
                       node [:children :left :right])
          node (reduce (fn [m k]
                         (update-typed-sequence m k prepare-item-booleans path))
                       node [:items :options :links :series :presets :context-menu])
          node (-> node
                   (update-typed :header-groups (partial map-typed-sequence
                                                         (partial map-typed-sequence prepare-item-booleans)) path)
                   (update-typed :item prepare-nav-booleans path)
                   (update-typed :custom-variant prepare-custom-variant-booleans path))]
      (reduce (fn [m k] (update-typed m k prepare-node-booleans path))
              node [:indicator-style :shortcut-style :token-style :last-column-content :tooltip-content :action :code-block-actions :table-actions :content :header :description :label-content :prefix :suffix :empty :loading-content :initial-content :title-style :header-style :sidebar-style :hover-style :track-style :trigger-style :trigger :footer :stack-style :shimmer-style :separator-style
                    :content-style :list-style :row-style :jump-button-style
                    :jump-button-renderer]))
    node))

(defn- sanitize-item
  [item]
  (if (map? item)
    (cond-> (reduce (fn [m k]
                      (if (fn? (get m k))
                        (assoc m k (register-callback! (get m k)))
                        m))
                    item
                    callback-keys)
      (some? (:tooltip-content item)) (update :tooltip-content sanitize)
      (some? (:content item)) (update :content sanitize)
      (some? (:display-content item)) (update :display-content sanitize)
      (some? (:suffix item)) (update :suffix sanitize)
      (some? (:header item)) (update :header sanitize)
      (some? (:footer item)) (update :footer sanitize)
      (seq (:children item)) (update :children #(mapv sanitize %))
      (seq (:items item)) (update :items #(mapv sanitize-item %))
      (seq (:cells item)) (update :cells #(mapv (fn [cell]
                                                  (cond
                                                    (ui/ui-node? cell) (sanitize cell)
                                                    (map? cell) (sanitize-item cell)
                                                    :else cell))
                                                %)))
    item))

(def ^:private nested-node-keys
  [:indicator-style :shortcut-style :token-style :last-column-content :tooltip-content :action :code-block-actions :table-actions :content :header :description :label-content :prefix :suffix :empty :loading-content :initial-content :title-style :header-style :sidebar-style :hover-style :track-style :trigger-style :trigger :footer :stack-style :shimmer-style :separator-style
   :content-style :list-style :row-style :jump-button-style
   :jump-button-renderer :left :right])

(defn- sanitize-providers [node]
  (if (map? (:lsp node))
    (let [id (:id node)]
      (when-not (and (some? id) (seq (str id)))
        (throw (ex-info "Editor :lsp requires a stable :id" {})))
      (update node :lsp
              (fn [lsp]
                (into {} (for [[method value] lsp]
                           [method (if (fn? value)
                                     (let [key (str "lsp/" (pr-str [(str id) (name method)]))]
                                       (swap! *provider-export* assoc key value)
                                       key)
                                     value)])))))
    node))

(defn- sanitize
  "Replace Clojure functions in the UI tree with callback ids before JSON."
  [node]
  (cond
    (ui/ui-node? node)
    (let [node (reduce (fn [m k]
                         (if (fn? (get m k))
                           (assoc m k (register-callback! (get m k)))
                           m))
                       (sanitize-providers node)
                       callback-keys)]
      (reduce (fn [n k]
                (cond-> n (some? (get n k)) (update k sanitize)))
              (-> node
                  (update :children #(mapv sanitize (or % [])))
                  (cond-> (seq (:items node)) (update :items #(mapv sanitize-item %)))
                  (cond-> (seq (:context-menu node)) (update :context-menu #(mapv sanitize-item %)))
                  (cond-> (seq (:options node)) (update :options #(mapv sanitize-item %)))
                  (cond-> (seq (:links node)) (update :links #(mapv sanitize-item %)))
                  (cond-> (seq (:series node)) (update :series #(mapv sanitize-item %))))
              nested-node-keys))

    (sequential? node)
    (mapv sanitize node)

    :else node))

(defn- json-tree
  [node]
  (cond
    (map? node)
    (into {}
          (for [[k v] node]
            [k (json-tree v)]))

    (sequential? node)
    (mapv json-tree node)

    (keyword? node)
    (name node)

    (fn? node)
    (register-callback! node)

    :else node))

(defn reset-callbacks!
  "Drop the current callback map. The id counter is process-lifetime
  monotonic so a stale `cb-N` cannot name a newer function."
  []
  (reset! callbacks {})
  (reset! callback-hold 0)
  (reset! callback-depth 0))

(defn lookup-callback
  [id]
  (get @callbacks id))

(defn export-node [tree]
  (binding [*provider-export* (atom {})]
    (let [exported (json-tree
                    (sanitize
                     (prepare-node-booleans
                      tree "")))]
      ;; Publish atomically: retained native providers resolve the current function,
      ;; including when an unrelated render regenerated ordinary callback ids.
      (reset! providers @*provider-export*)
      exported)))

(defn invoke-provider! [provider-id params]
  (if-let [f (get @providers provider-id)]
    (try
      (let [result (f params)]
        #?(:clj
           {:ok true :value (json-tree (if (instance? java.util.concurrent.Future result)
                                         @result result))}
           :cljs
           (-> (js/Promise.resolve result)
               (.then (fn [value] {:ok true :value (json-tree value)}))
               (.catch (fn [error] {:ok false :error (str error)})))))
      (catch #?(:clj Exception :cljs :default) e
        {:ok false :error #?(:clj (str (.getMessage e)) :cljs (str e))}))
    {:ok false :error (str "unknown provider " provider-id)}))

(defn invoke-callback!
  "Invoke a previously registered Clojure function from a GPUI event.

  Buttons and checkboxes are 0-arg. When the host includes `value`
  (string, boolean, number, JSON collection, or JSON `null`), Clojure
  calls `(f value)`. `:on-escape` is 0-arg. `:on-double-click` is 0-arg."
  ([callback-id]
   (invoke-callback! callback-id nil false))
  ([callback-id value]
   (invoke-callback! callback-id value (some? value)))
  ([callback-id value present?]
   (if-let [f (get @callbacks callback-id)]
     (do
       (swap! callback-depth inc)
       (try
         (let [result (if present? (f value) (f))
               response {:ok true :id callback-id}]
           #?(:clj response
              :cljs (if (and result (fn? (.-then ^js result)))
                      (.then (js/Promise.resolve result) (fn [_] response))
                      response)))
         (finally
           (swap! callback-depth dec))))
     {:ok false :error (str "unknown callback " callback-id)})))

(defn invoke-callback-batch!
  "Invoke several callbacks against the current registry without exporting.

  Same-generation contract the host uses for one native action: sequential
  invoke, no `export-tree` between items. Stops on the first failure so a
  failed prerequisite does not run later actions (unknown id or `ok:false`).
  Thrown handlers propagate. Each item is `{:id ...}` with optional `:value`
  (including JSON `nil`, which is `(f nil)` not a 0-arg call)."
  [calls]
  #?(:clj
     (loop [calls (vec calls)
            results []]
       (if (empty? calls)
         {:ok true :results results}
         (let [call (first calls)
               id (:id call)
               present? (contains? call :value)
               result (invoke-callback! id (:value call) present?)]
           (if (:ok result)
             (recur (subvec calls 1) (conj results result))
             {:ok false :results (conj results result)}))))
     :cljs
     (letfn [(step [calls results]
               (if (empty? calls)
                 {:ok true :results results}
                 (let [call (first calls)
                       result (invoke-callback! (:id call) (:value call) (contains? call :value))
                       advance (fn [result]
                                 (if (:ok result)
                                   (step (rest calls) (conj results result))
                                   {:ok false :results (conj results result)}))]
                   (if (instance? js/Promise result)
                     (.then result advance)
                     (advance result)))))]
       (step calls []))))

(defn apply-callback-msg
  [msg]
  (let [defer? (true? (:defer-render msg))]
    (when defer?
      (swap! callback-hold inc))
    (try
      (invoke-callback! (:callback-id msg)
                        (:value msg)
                        (contains? msg :value))
      (finally
        (when-not defer?
          (reset! callback-hold 0))))))
