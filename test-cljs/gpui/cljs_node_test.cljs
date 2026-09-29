(ns gpui.cljs-node-test
  (:require [cljs.test :refer-macros [deftest is async]]
            [gpui.cljs-test-guard]
            [gpui.node :as node]
            [gpui.ratom :as r]
            [gpui.ui :as ui]
            ["node:path" :as path]))

(defn- scenario! [mode]
  (js/Promise.
   (fn [resolve reject]
     (let [state (r/atom "old")
           previous-mode (.. js/process -env -CLJ_GPUI_TEST_MODE)
           listeners (.listenerCount js/process "SIGTERM")
           options {:app #(ui/input @state {:on-change (fn [value] (reset! state value))})
                    :host (path/resolve "test-cljs/fixtures/host.cjs")
                    :connect-timeout-ms (if (= mode "timeout") 100 5000)
                    :on-exit (fn [code]
                               (if previous-mode
                                 (set! (.. js/process -env -CLJ_GPUI_TEST_MODE) previous-mode)
                                 (js-delete (.-env js/process) "CLJ_GPUI_TEST_MODE"))
                               (is (not (node/running?)))
                               (is (= listeners (.listenerCount js/process "SIGTERM")))
                               (resolve code))}]
       (set! (.. js/process -env -CLJ_GPUI_TEST_MODE) mode)
       (try
         (let [ready (node/start! options)]
           ;; Repeated start updates the app, and never spawns a second host.
           (reset! state "fresh")
           (is (identical? ready (node/start! options)))
           (.catch ready (fn [_] nil)))
         (catch :default error (reject error)))))))

(deftest native-child-lifecycle-and-fragmented-utf8
  (async done
         ((fn []
            (-> (scenario! "rpc")
                (.then (fn [code] (is (= 0 code)) (scenario! "exit")))
                (.then (fn [code] (is (= 7 code)) (scenario! "timeout")))
                (.then (fn [code] (is (= 1 code)) (scenario! "disconnect")))
                (.then (fn [code] (is (= 1 code)) (scenario! "invalid")))
                (.then (fn [code] (is (= 1 code)) (done)))
                (.catch (fn [error] (is false (str error)) (node/stop!) (done))))))))
