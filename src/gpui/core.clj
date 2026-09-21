(ns gpui.core
  "Compatibility namespace. Prefer `[gpui.ui :as ui]` in new code."
  (:require [gpui.ui :as ui]))

(doseq [[sym v] (ns-publics 'gpui.ui)]
  ;; Keep Clojure collection functions intact in the compatibility namespace.
  (when-not (#{'list 'empty} sym)
    (intern *ns* (with-meta sym (merge (meta v) {:doc (or (:doc (meta v)) "")})) @v)))

(def ui-list
  "See `gpui.ui/list`. Not interned as `list` so `clojure.core/list` stays intact."
  ui/list)

(def ui-empty
  "See `gpui.ui/empty`; preserves `clojure.core/empty` in this namespace."
  ui/empty)
