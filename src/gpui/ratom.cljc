(ns gpui.ratom
  "Reagent-style reactive atoms for clj-gpui.

  Require this namespace as `r` and write `(r/atom ...)`. The result is
  a real Clojure or ClojureScript atom: `swap!`, `reset!`, `deref`, and `@` are
  Clojure's. The only extra behavior is a watch that asks GPUI to
  rerender the window."
  (:refer-clojure :exclude [atom])
  (:require [gpui.ui :as ui]))

(def atom
  "Like `clojure.core/atom`, but GPUI rerenders when the value changes.

  (require '[gpui.ratom :as r])
  (defonce count (r/atom 0))
  (swap! count inc)
  (deref count)"
  ui/ratom)

(defn atom?
  "True when `x` is a Clojure atom watched for GPUI rerenders."
  [x]
  (boolean
   (and #?(:clj (instance? clojure.lang.IAtom x) :cljs (instance? cljs.core/Atom x))
        (contains? #?(:clj (.getWatches ^clojure.lang.IRef x) :cljs (.-watches ^cljs.core/Atom x)) ui/ratom-watch-key))))
