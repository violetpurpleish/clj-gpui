(ns gpui.node
  "Launch native GPUI applications from shadow-cljs :node-script builds."
  (:require [clojure.string :as str]
            [gpui.platform :as platform]
            [gpui.runtime :as runtime]
            [gpui.ui :as ui]
            ["node:child_process" :as process]
            ["node:fs" :as fs]
            ["node:net" :as net]
            ["node:path" :as path]
            ["node:readline" :as readline]))

(defonce ^:private session* (atom nil))

(defn- executable! [file]
  (let [file (path/resolve file)]
    (fs/accessSync file (.-X_OK fs/constants))
    file))

(defn- host-binary! [{:keys [host root build?] :or {build? goog.DEBUG}}]
  (if-let [binary (or host (.. js/process -env -CLJ_GPUI_BIN))]
    (executable! binary)
    (if-not build?
      (throw (ex-info "Set CLJ_GPUI_BIN or :host to the native host binary for a release app" {}))
      (let [root (path/resolve (or root (.. js/process -env -CLJ_GPUI_ROOT) "."))
            manifest (path/join root "host" "Cargo.toml")]
        (when-not (fs/existsSync manifest)
          (throw (ex-info "Set :root or CLJ_GPUI_ROOT to the clj-gpui checkout, or CLJ_GPUI_BIN to a built host" {:root root})))
        (println "[clj-gpui] building native host")
        (let [result (process/spawnSync
                      "cargo" #js ["build" "--locked" "--release" "--manifest-path" manifest "--message-format=json"]
                      #js {:cwd (path/dirname manifest) :encoding "utf8" :stdio #js ["inherit" "pipe" "inherit"]
                           :maxBuffer (* 64 1024 1024)})]
          (when (or (.-error result) (not= 0 (.-status result)))
            (throw (ex-info "Native host build failed" {:error (str (.-error result)) :status (.-status result)})))
          (or (some (fn [line]
                      (when-not (str/blank? line)
                        (let [artifact (js/JSON.parse line)]
                          (when (and (= "compiler-artifact" (.-reason artifact))
                                     (= "clj-gpui" (.. artifact -target -name))
                                     (.-executable artifact))
                            (executable! (.-executable artifact))))))
                    (str/split-lines (.-stdout result)))
              (throw (ex-info "Cargo did not report a clj-gpui executable" {}))))))))

(defn running? [] (some? @session*))

(defn stop!
  "Close the native host and socket. Safe to call more than once."
  []
  (when-let [stop (:stop @session*)] (stop 0)))

(defn reload!
  "shadow-cljs :after-load hook. Pass a thunk to start! so redefined app vars are read."
  []
  (ui/request-render!))

(defn start!
  "Start one native window. Returns a Promise resolving when the host connects.

  Required :app is a thunk, e.g. #(app). Optional :host / CLJ_GPUI_BIN uses
  a prebuilt host; development can build from :root / CLJ_GPUI_ROOT.
  :on-exit receives the host's exit code (defaults to exiting the JS runtime).
  Calling again updates the root function without creating a second window."
  [{:keys [app app-id protocol-test? on-exit connect-timeout-ms]
    :or {app-id "ClojureScript" connect-timeout-ms 60000}
    :as options}]
  (runtime/set-app! app)
  (runtime/set-production-mode! (not goog.DEBUG))
  (if-let [session @session*]
    (do (reload!) (:ready session))
    (let [binary (host-binary! options)
          state (atom {})
          finished? (atom false)
          requested-code (atom nil)
          resolve* (atom nil)
          reject* (atom nil)
          ready (js/Promise. (fn [resolve reject] (reset! resolve* resolve) (reset! reject* reject)))
          on-exit (or on-exit #(.exit js/process %))]
      (letfn [(finish! [code]
                (when (compare-and-set! finished? false true)
                  (let [{:keys [^js server ^js socket ^js lines timer kill-timer disconnect-timer signal-int signal-term exit-handler]} @state]
                    (js/clearTimeout timer)
                    (js/clearTimeout kill-timer)
                    (js/clearTimeout disconnect-timer)
                    (when lines (.close lines))
                    (when socket (.destroy socket))
                    (when server (.close server))
                    (.removeListener js/process "SIGINT" signal-int)
                    (.removeListener js/process "SIGTERM" signal-term)
                    (.removeListener js/process "exit" exit-handler)
                    (runtime/bind-connection! nil)
                    (platform/clear-pending!)
                    (reset! session* nil)
                    (@reject* (ex-info "Native host exited before connecting" {:code code}))
                    (on-exit code))))
              (stop [code]
                (when (nil? @requested-code) (reset! requested-code code))
                (if-let [^js child (:child @state)]
                  (when-not @finished?
                    (.kill child "SIGTERM")
                    (when-not (:kill-timer @state)
                      (swap! state assoc :kill-timer (js/setTimeout #(.kill child "SIGKILL") 2000))))
                  (finish! code)))
              (fail [error]
                (js/console.error "[clj-gpui]" error)
                (@reject* error)
                (stop 1))
              (accept [^js socket]
                (if (or (:socket @state) @finished?)
                  (.destroy socket)
                  (let [lines (readline/createInterface #js {:input socket :crlfDelay js/Infinity})]
                    (swap! state assoc :socket socket :lines lines)
                    (js/clearTimeout (:timer @state))
                    (.setNoDelay socket true)
                    (.on socket "error" fail)
                    ;; Give a normally exiting child time to report its exit status.
                    ;; A host that drops the socket but remains alive must be reaped.
                    (.on socket "close"
                         (fn []
                           (when-not @finished?
                             (swap! state assoc :disconnect-timer (js/setTimeout #(stop 1) 250)))))
                    (.on lines "line"
                         (fn [line]
                           (when-not (str/blank? line)
                             (try
                               (runtime/handle (js->clj (js/JSON.parse line) :keywordize-keys true))
                               (catch :default error (fail error))))))
                    (runtime/bind-connection!
                     {:send (fn [message]
                              (.write socket (str (js/JSON.stringify (clj->js message)) "\n") "utf8"))})
                    (runtime/send-ready! app-id)
                    (@resolve* true))))]
        (let [server (net/createServer accept)
              signal-int #(stop 130)
              signal-term #(stop 143)
              exit-handler #(when-let [^js child (:child @state)] (.kill child "SIGTERM"))]
          (reset! state {:server server :signal-int signal-int :signal-term signal-term :exit-handler exit-handler})
          (reset! session* {:ready ready :stop stop})
          (runtime/install-render-hook!)
          (.on js/process "SIGINT" signal-int)
          (.on js/process "SIGTERM" signal-term)
          (.on js/process "exit" exit-handler)
          (.on server "error" fail)
          (.listen server 0 "127.0.0.1"
                   (fn []
                     (when-not @finished?
                       (let [port (.-port (.address server))
                             env (js/Object.assign #js {} (.-env js/process)
                                                   #js {:CLJ_GPUI_HOST "127.0.0.1" :CLJ_GPUI_PORT (str port)})
                             child (process/spawn binary (if protocol-test? #js ["--protocol-test"] #js [])
                                                  #js {:env env :stdio "inherit"})]
                         (swap! state assoc :child child
                                :timer (js/setTimeout #(fail (js/Error. "Timed out waiting for native host")) connect-timeout-ms))
                         (.on child "error" fail)
                         (.on child "close" (fn [code _signal]
                                              (finish! (or @requested-code code 1))))))))
          ready)))))
