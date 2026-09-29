(ns gpui.cljs-test-guard
  "An unresolved Promise must not let Node exit with a successful test status."
  (:require [cljs.test :as test]))

(.on js/process "beforeExit"
     (fn []
       (when (seq (:testing-vars (test/get-current-env)))
         (js/console.error "ClojureScript async tests ended without calling done")
         (set! (.-exitCode js/process) 1))))
