(ns my.app
  (:require [gpui.node :as node]
            [gpui.ratom :as r]
            [gpui.ui :as ui]
            ["dayjs" :as dayjs])
  (:require-macros [my.host :refer [library-root]]))

(defonce !state (r/atom {:n 0 :updated nil}))

(defn app []
  (let [{:keys [n updated]} @!state]
    (ui/window
     {:title "My App" :chrome :app :theme :tokyo-night
      :width 480 :height 280 :padding 24 :gap 16}
     (ui/label "Hello from ClojureScript" {:font-size 24 :font-weight :semibold})
     (ui/hstack
      {:gap 12}
      (ui/label (str "Clicks: " n) {:font-size 18})
      (ui/button "Click"
                 #(swap! !state (fn [state]
                                  (-> state
                                      (update :n inc)
                                      (assoc :updated (.format (dayjs) "HH:mm:ss")))))
                 {:primary true}))
     (ui/label (if updated (str "Updated at " updated " with dayjs") "Native GPUI · shadow-cljs · npm")))))

(defn reload! [] (node/reload!))

(defn main []
  (try
    (-> (node/start! {:app #(app)
                      :app-id "my.app/app"
                      :root (when ^boolean goog.DEBUG (library-root))})
        (.catch (fn [error]
                  (js/console.error error)
                  (set! (.-exitCode js/process) 1))))
    (catch :default error
      (js/console.error error)
      (set! (.-exitCode js/process) 1))))
