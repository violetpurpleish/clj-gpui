(ns gpui.cljs-protocol
  (:require [gpui.node :as node]
            [gpui.ratom :as r]
            [gpui.ui :as ui]
            ["dayjs" :as dayjs]))

(defonce !state (r/atom {:count 0 :missing-glyphs []}))

(defn app []
  (let [{:keys [count missing-glyphs]} @!state]
    (ui/window
     {:on-missing-glyphs #(swap! !state assoc :missing-glyphs %)}
     (ui/label "clj-gpui")
     (ui/label (.format (dayjs "2026-09-29") "YYYY-MM-DD"))
     (ui/label (str "Count: " count))
     (ui/label (pr-str missing-glyphs))
     (ui/button "+" #(swap! !state update :count inc)
                {:disabled nil :loading nil :selected nil :focus-ring nil})
     (ui/editor "provider test"
                {:id "provider-test"
                 :lsp {:hover
                       (fn [{:keys [text await-count]}]
                         (js/Promise.
                          (fn [resolve _reject]
                            (letfn [(check []
                                      (if (and await-count (< (:count @!state) await-count))
                                        (js/setTimeout check 5)
                                        (resolve {:contents {:kind "plaintext"
                                                             :value (str text ":" (if await-count (:count @!state) count))}})))]
                              (check)))))}}))))

(defn main []
  (-> (node/start! {:app #(app) :app-id "gpui.cljs-protocol/app" :protocol-test? true})
      (.catch (fn [error] (js/console.error error) (set! (.-exitCode js/process) 1)))))
