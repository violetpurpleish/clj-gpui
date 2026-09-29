(ns my.host
  "Compile-time host discovery for local checkouts and pinned Git dependencies."
  (:require [gpui.host :as host]))

(defmacro library-root []
  (some-> (host/library-root) .getCanonicalPath))
