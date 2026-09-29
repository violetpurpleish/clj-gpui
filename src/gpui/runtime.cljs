(ns gpui.runtime
  "Node.js side of the native host protocol. Applications start with gpui.node."
  (:require [gpui.theme :as theme]
            [gpui.ui :as ui]
            [gpui.wire :as wire]))

(def protocol-version ui/protocol-version)
(defonce ^:private connection* (atom nil))
(defonce ^:private app* (atom nil))
(defonce ^:private render-timer* (atom nil))
(defonce ^:private directory-handler* (atom nil))
(defonce ^:private previews* (atom {}))
(defonce ^:private preview-counter* (atom 0))
(defonce ^:private production-mode* (atom false))

(def reset-callbacks! wire/reset-callbacks!)
(def lookup-callback wire/lookup-callback)
(def invoke-callback! wire/invoke-callback!)
(def invoke-callback-batch! wire/invoke-callback-batch!)
(def invoke-provider! wire/invoke-provider!)

(defn set-production-mode! [enabled?]
  (reset! production-mode* (boolean enabled?)))

(defn set-app! [f]
  (when-not (fn? f)
    (throw (ex-info "The application must be a zero-argument function" {})))
  (reset! app* f))

(defn send! [message]
  (when-let [send-fn (:send @connection*)]
    (send-fn message)))

(defn bind-connection! [connection]
  (when-let [timer @render-timer*]
    (js/clearTimeout timer))
  (reset! render-timer* nil)
  (wire/finish-callbacks!)
  (doseq [[_ {:keys [resolve timer]}] @previews*]
    (js/clearTimeout timer)
    (resolve nil))
  (reset! previews* {})
  (reset! connection* connection))

(defn- schedule-render! []
  (when (and @connection* (not (wire/callback-active?)) (nil? @render-timer*))
    (reset! render-timer*
            (js/setTimeout
             (fn []
               (reset! render-timer* nil)
               (when-not (wire/callback-active?)
                 (send! {:op "request-render"})))
             16))))

(defn install-render-hook! []
  (ui/set-request-render! schedule-render!))

(defn- error-tree [error]
  (ui/vstack {:gap 8 :padding 12}
             (ui/label "ClojureScript error" {:font-size 18 :color "#f7768e"})
             (ui/label (str error))
             (ui/scroll {:height 280}
                        (ui/label (or (.-stack error) (str error))))))

(defn- export-node [tree]
  (wire/export-node (if (and @production-mode* (map? tree))
                      (assoc tree :chrome :app) tree)))

(defn export-tree
  ([] (export-tree @app*))
  ([tree]
   (wire/reset-callbacks!)
   (try
     (export-node (if (fn? tree) (tree) tree))
     (catch :default error
       (export-node (error-tree error))))))

(defn set-directory-handler! [f]
  (reset! directory-handler* f))

(defn preview-png
  "Resolve to a base64 PNG of the native window, or nil after timeout/disconnect."
  []
  (if-not @connection*
    (js/Promise.resolve nil)
    (js/Promise.
     (fn [resolve _reject]
       (let [id (str "cap-" (swap! preview-counter* inc))
             timer (js/setTimeout
                    (fn [] (swap! previews* dissoc id) (resolve nil)) 6000)]
         (swap! previews* assoc id {:resolve resolve :timer timer})
         (send! {:op "capture-preview" :request-id id}))))))

(defn- deliver-preview! [{:keys [request-id png]}]
  (when-let [{:keys [resolve timer]} (get @previews* request-id)]
    (swap! previews* dissoc request-id)
    (js/clearTimeout timer)
    (resolve (when (and (string? png) (>= (count png) 32)) png))))

(defn handle
  "Serve each RPC independently so Promise providers cannot block UI callbacks."
  [message]
  (let [connection @connection*
        respond (fn [result]
                  (when (identical? connection @connection*)
                    (send! (assoc result :op "response" :id (:id message)))))
        fail (fn [error] (respond {:ok false :error (str error)}))]
    (try
      (let [result
            (case (:op message)
              ("render" "reload")
              (do (wire/finish-callbacks!)
                  {:ok true :tree (export-tree) :themes (theme/wire-sets)})
              "callback" (wire/apply-callback-msg message)
              "provider" (wire/invoke-provider! (:provider-id message) (:params message))
              "directory-picked" (do (when-let [f @directory-handler*] (f message)) {:ok true})
              "preview-captured" (do (deliver-preview! message) {:ok true})
              {:ok false :error (str "unknown op: " (:op message))})]
        (-> (js/Promise.resolve result) (.then respond) (.catch fail)))
      (catch :default error (fail error)))))

(defn send-ready! [app-id]
  (send! {:op "ready" :protocol-version protocol-version :nrepl 0 :app app-id}))
