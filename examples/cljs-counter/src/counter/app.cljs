(ns counter.app
  (:require [gpui.node :as node]
            [gpui.ratom :as r]
            [gpui.ui :as ui]
            ["dayjs" :as dayjs]
            ["node:os" :as os]))

(defonce !state (r/atom {:count 0 :name "ClojureScript" :updated nil}))

(defn app []
  (let [{:keys [count name updated]} @!state]
    (ui/window
     {:title "clj-gpui · ClojureScript" :chrome :app :width 540 :height 360 :theme :tokyo-night
      :padding 24 :gap 18}
     (ui/label (str "Hello, " name) {:font-size 26 :font-weight :bold})
     (ui/label (str "Native GPUI + shadow-cljs + Node.js on " (os/platform)))
     (ui/input name #(swap! !state assoc :name %) {:id "name" :placeholder "Your name"})
     (ui/hstack {:gap 12}
                (ui/button "−" #(swap! !state update :count dec))
                (ui/label (str count) {:font-size 24})
                (ui/button "+" #(swap! !state update :count inc) {:primary true}))
     (ui/button "Async npm example"
                (fn []
                  (js/setTimeout
                   #(swap! !state assoc :updated (.format (dayjs) "HH:mm:ss")) 100)))
     (ui/label (if updated (str "dayjs updated at " updated) "Edit this file: defonce state survives reload.")))))

(defn reload! [] (node/reload!))

(defn main []
  (-> (node/start! {:app #(app) :app-id "counter.app/app"})
      (.catch (fn [error]
                (js/console.error error)
                (set! (.-exitCode js/process) 1)))))
