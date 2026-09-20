(ns title-bar.app
  "A unified GPUI Kit toolbar with native window controls."
  (:require [gpui.ratom :as r]
            [gpui.ui :as ui]))

(defonce !refreshes (r/atom 0))

(defn app []
  (ui/window
   {:title (str "Unified toolbar — " @!refreshes " refreshes")
    :chrome :app
    :width 1000
    :height 700}
   ;; The direct child selects Kit's native window options at startup.
   ;; Omit the style map entirely to use Kit's default height and position.
   (ui/title-bar
    {:height 56 :traffic-light-position [10 19]}
    (ui/hstack
     {:flex 1 :justify :between :padding 12}
     (ui/label "Unified toolbar" {:font-weight :semibold})
     (ui/button "Refresh" #(swap! !refreshes inc) {:variant :ghost})))
   (ui/vstack
    {:flex 1 :gap 12 :padding 24}
    (ui/label "Application content" {:font-size 24 :font-weight :bold})
    (ui/label (str "Refreshed " @!refreshes " times."))
    (ui/label "Drag the toolbar to move the window. Double-click its empty space for the platform window action."))))
