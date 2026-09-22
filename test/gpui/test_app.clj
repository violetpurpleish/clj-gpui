(ns gpui.test-app
  "Minimal app used by `:protocol-test`. Keep labels stable: clj-gpui, Count, +."
  (:require [gpui.ratom :as r]
            [gpui.ui :as ui]))

(defonce !state (r/atom {:count 0 :missing-glyphs []}))

(defn app []
  (let [{:keys [count missing-glyphs]} @!state]
    (ui/window
     {:gap 12 :padding 16
      :on-missing-glyphs #(swap! !state assoc :missing-glyphs %)}
     (ui/label "clj-gpui" {:font-size 18 :font-weight :semibold})
     (ui/label (str "Missing glyphs: " (pr-str missing-glyphs)))
     (ui/hstack
      {:gap 12}
      (ui/label (str "Count: " count) {:font-size 16})
      (ui/button "+" #(swap! !state update :count inc)
                 {:disabled nil :loading nil :selected nil :focus-ring nil}))
     (ui/editor "provider test"
                {:id "provider-test"
                 :lsp {:hover
                       (fn [{:keys [text await-count]}]
                         (future
                           (when await-count
                             (loop []
                               (when (< (:count @!state) await-count)
                                 (Thread/sleep 5)
                                 (recur))))
                           {:contents {:kind "plaintext"
                                       :value (str text ":" (if await-count (:count @!state) count))}}))}}))))
