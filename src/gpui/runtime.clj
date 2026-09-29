(ns gpui.runtime
  "Internal Clojure↔host protocol. Application code should not require this.

  The runtime:
    * loads the application var
    * sanitizes UI trees (functions become callback ids)
    * invokes Clojure functions when the host reports a click
    * reloads app namespaces on request"
  (:require [clojure.data.json :as json]
            [clojure.java.io :as io]
            [clojure.string :as str]
            [gpui.theme :as theme]
            [gpui.ui :as ui]
            [gpui.wire :as wire])
  (:import [java.io BufferedReader PushbackReader]
           [java.nio.charset StandardCharsets]))

(set! *warn-on-reflection* true)

(def protocol-version ui/protocol-version)

(defonce ^:private conn (atom nil))
(defonce ^:private render-scheduled? (atom false))
(defonce ^:private render-epoch (atom 0))
(defonce ^:private app-var* (atom nil))
(defonce ^:private app-sym* (atom nil))
(defonce ^:private production-mode* (atom false))
(defonce ^:private nrepl-port* (atom nil))
(defonce ^:private file-mtimes (atom {}))
(defonce ^:private load-error* (atom nil))
(defonce ^:private file-ns (atom {}))

(defn- env
  ([k]
   (env k nil))
  ([k default]
   (or (System/getenv k) default)))

(defn app-symbol
  []
  (or @app-sym*
      (when-let [s (env "CLJ_GPUI_APP")]
        (symbol s))))

(defn set-app-symbol!
  [sym]
  (reset! app-sym* (symbol (str sym)))
  @app-sym*)

(defn set-production-mode!
  "Set whether exported root trees must hide development-only host chrome.

  `gpui.prod` enables this before loading the application. Keeping the policy
  in the runtime lets applications use `:chrome :dev` during development
  without accidentally shipping the nREPL footer or FPS HUD."
  [enabled?]
  (reset! production-mode* (boolean enabled?)))

(defn nrepl-port
  []
  @nrepl-port*)

(defn- src-root
  []
  (io/file (env "CLJ_GPUI_SRC" "src")))

(defn- clj-files
  [^java.io.File root]
  (if-not (.exists root)
    []
    (->> (file-seq root)
         (filter #(.isFile ^java.io.File %))
         (filter #(re-find #"\.cljc?$" (.getName ^java.io.File %)))
         (remove (fn [^java.io.File f]
                   (re-find #"(^|/)(runtime|dev)\.clj$" (.getPath f)))))))

(defn send!
  [message]
  (when-let [{:keys [out]} @conn]
    (let [^java.io.Writer out out]
      (locking out
        (.write out ^String (str (json/write-str message) "\n"))
        (.flush out)))))

(defn- schedule-render!
  []
  ;; The host always fetches a tree after a callback RPC (needed for
  ;; input submit sequencing and for handlers that do not touch an
  ;; atom). Skip the r/atom watch's request-render while that RPC is
  ;; running so one click is not two paints. A multi-callback native
  ;; action sets `defer-render` on every item so hold stays up across
  ;; sequential callback RPCs and the gap before the host's one
  ;; following `"render"` RPC, which clears it. nREPL / watcher /
  ;; explicit ui/request-render! still go through this path with depth
  ;; 0 and hold 0.
  (when (and (not (wire/callback-active?))
             (compare-and-set! render-scheduled? false true))
    (let [epoch @render-epoch]
      (future
        (try
          (Thread/sleep 16)
          (reset! render-scheduled? false)
          (when (= epoch @render-epoch)
            (send! {:op "request-render"}))
          (catch Exception e
            (reset! render-scheduled? false)
            (binding [*out* *err*]
              (println "[clj-gpui] request-render failed:" (.getMessage e)))))))))

(defn install-render-hook!
  []
  (ui/set-request-render! schedule-render!))

(defn bind-connection!
  [m]
  (swap! render-epoch inc)
  (reset! render-scheduled? false)
  (reset! conn m)
  m)

(defonce ^:private preview-pending* (atom {}))
(defonce ^:private preview-counter* (atom 0))

(def ^:private preview-timeout-ms
  "Host capture plus the JSON round-trip. Evalight waits 8s for the nREPL value."
  6000)

(defn deliver-preview!
  "Runtime hook: the host finished a `capture-preview` request."
  [{:keys [request-id png]}]
  (when-let [p (get @preview-pending* request-id)]
    (swap! preview-pending* dissoc request-id)
    (deliver p (when (and (string? png) (>= (count png) 32))
                 png)))
  true)

(defn- preview-png*
  [timeout-ms]
  (try
    (if-not @conn
      nil
      (let [id (str "cap-" (swap! preview-counter* inc))
            p (promise)]
        (swap! preview-pending* assoc id p)
        (send! {:op "capture-preview" :request-id id})
        (let [v (deref p timeout-ms ::timeout)]
          (swap! preview-pending* dissoc id)
          (when (string? v)
            v))))
    (catch Exception _
      nil)))

(defn preview-png
  "Return a base64 PNG of the current native window, or nil.

  Evalight (local mode) evals this over nREPL and shows the image in
  the Preview pane. The OS window is still the real program. Return
  nil when no window exists yet. Never throw."
  []
  (preview-png* preview-timeout-ms))

(defn- error-chain
  [^Throwable e]
  (take-while some? (iterate #(.getCause ^Throwable %) e)))

(defn- error-location
  "File:line:column from a CompilerException / ex-data chain, if present."
  [^Throwable e]
  (some (fn [^Throwable t]
          (let [data (ex-data t)
                src (or (get data :clojure.error/source)
                        (get data :clojure.error/file))
                line (get data :clojure.error/line)
                column (get data :clojure.error/column)]
            (when (or src line)
              (str (or src "unknown")
                   (when line (str ":" line))
                   (when (and line column) (str ":" column))))))
        (error-chain e)))

(defn- error-tree
  [^Throwable e]
  (let [sw (java.io.StringWriter.)
        pw (java.io.PrintWriter. sw)
        loc (error-location e)]
    (.printStackTrace e pw)
    (.flush pw)
    (ui/vstack
     {:gap 8 :padding 12}
     (ui/label "Clojure error" {:font-size 18 :font-weight :bold :color "#f7768e"})
     (ui/label (str (.getClass e) ": " (.getMessage e)) {:color "#c0caf5"})
     (when loc
       (ui/label loc {:font-size 13 :color "#9aa3b5"}))
     (ui/scroll
      {:height 280}
      (ui/label (str sw) {:color "#9aa3b5"})))))

(defn load-app!
  "Require the application namespace and resolve its root UI var."
  []
  (let [sym (or (app-symbol)
                (throw (ex-info "No application var. Pass my.app/app to gpui.dev." {})))
        nspace (symbol (namespace sym))
        nam (symbol (name sym))]
    (when-not (namespace sym)
      (throw (ex-info (str "Application var must be namespaced, got " sym) {:var sym})))
    (require nspace)
    (let [v (ns-resolve nspace nam)]
      (when-not v
        (throw (ex-info (str "Could not resolve " sym) {:var sym})))
      (reset! load-error* nil)
      (reset! app-var* v)
      v)))

(defn- app-ns
  []
  (when-let [sym (app-symbol)]
    (when (namespace sym)
      (symbol (namespace sym)))))

(defn- read-ns-name
  [^java.io.File f]
  (try
    (with-open [rdr (io/reader f)]
      (let [form (binding [*read-eval* false]
                   (read {:read-cond :allow :features #{:clj} :eof nil} (PushbackReader. rdr)))]
        (when (and (seq? form) (= 'ns (first form)))
          (let [n (second form)]
            (cond
              (symbol? n) n
              (string? n) (symbol n))))))
    (catch Exception _
      nil)))

(defn- ns-from-relative-path
  [^java.io.File src ^java.io.File f]
  (let [root (.getCanonicalPath src)
        path (.getCanonicalPath f)
        prefix (str root (System/getProperty "file.separator"))]
    (when (str/starts-with? path prefix)
      (let [rel (subs path (count prefix))
            rel (-> rel
                    (str/replace #"\.cljc?\z" "")
                    (str/replace #"/" ".")
                    (str/replace #"\\" "."))]
        (when (and (seq rel) (not (str/includes? rel " ")))
          (symbol rel))))))

(defn ns-from-file
  "Namespace symbol for a `.clj` or `.cljc` file: `ns` form, then path relative to `src`,
  then the last mapping we successfully used for this path (so a syntax error
  can still `(require ns :reload)`)."
  [^java.io.File src ^java.io.File f]
  (let [path (.getCanonicalPath f)
        n (or (read-ns-name f)
              (ns-from-relative-path src f)
              (get @file-ns path))]
    (when n
      (swap! file-ns assoc path n)
      n)))

(defn changed-clj-files
  "Files in `now` whose mtime is new or different from `prev`.
  `prev`/`now` are maps of canonical path → lastModified."
  [prev now]
  (->> now
       (keep (fn [[path mtime]]
               (when (not= mtime (get prev path))
                 (io/file path))))
       vec))

(defn- skip-reload-ns?
  [nspace]
  (let [s (str nspace)]
    (or (= nspace 'gpui.runtime)
        (= nspace 'gpui.dev)
        (= nspace 'gpui.host)
        (= nspace 'gpui.prod)
        ;; Fragments loaded into `gpui.package` (`package_build.clj`, …)
        ;; are not namespaces of their own; requiring them hangs/fails reload.
        (str/starts-with? s "gpui.package")
        (str/starts-with? s "clojure.")
        (str/starts-with? s "nrepl."))))

(defn- require-reload!
  [nspace]
  (println "[clj-gpui] reloading" nspace)
  (require nspace :reload))

(defn reload-app!
  "Reload application namespaces in place. `defonce` / ratom state is preserved.

  With no argument (host `reload` RPC), reloads every watched `.clj` / `.cljc` file then
  the root app namespace. With a seq of files (the watcher), reloads those
  namespaces first so a helper like `my.widgets` is picked up before the root
  `(require app :reload)`, which does not reload already-loaded deps.

  Does not `remove-ns` / tools.namespace refresh."
  ([]
   (reload-app! nil))
  ([changed-files]
   (try
     (let [src (src-root)
           app (or (app-ns)
                   (throw (ex-info "No application var. Pass my.app/app to gpui.dev." {})))
           files (mapv io/file (if (nil? changed-files)
                                 (clj-files src)
                                 changed-files))
           nses (->> files
                     (keep #(ns-from-file src %))
                     distinct
                     (remove skip-reload-ns?)
                     (remove #{app}))]
       (doseq [n nses]
         (require-reload! n))
       (require-reload! 'gpui.ui)
       (require-reload! 'gpui.core)
       (require-reload! 'gpui.ratom)
       (require-reload! 'gpui.theme)
       (require-reload! 'gpui.platform)
       (require-reload! app)
       (load-app!)
       {:ok true :ns (str app)})
     (catch Exception e
       (reset! load-error* e)
       (throw e)))))

(def reset-callbacks! wire/reset-callbacks!)
(def lookup-callback wire/lookup-callback)
(def invoke-provider! wire/invoke-provider!)
(def invoke-callback! wire/invoke-callback!)
(def invoke-callback-batch! wire/invoke-callback-batch!)

(defn- export-node [tree]
  (wire/export-node (if (and @production-mode* (map? tree))
                      (assoc tree :chrome :app) tree)))

(defn export-tree
  "Build a UI tree, preparing typed booleans and registering callbacks as ids.

  Present nil ordinary flags become false; nullable overrides retain nil.
  Invalid non-boolean values produce the existing Clojure error view with
  the field path and value type. Opaque application data is not normalized.

  Zero-arity calls the application var. One-arity sanitizes an already
  built tree (or invokes a 0-arg function that returns one). A failed
  `(require … :reload)` sticks until the next successful reload so the
  native window can show the compile error instead of the previous UI."
  ([]
   (reset-callbacks!)
   (if-let [e @load-error*]
     (export-node (error-tree e))
     (try
       (when-not @app-var*
         (load-app!))
       (let [app-fn (var-get @app-var*)
             tree (app-fn)]
         (export-node tree))
       (catch Exception e
         (.printStackTrace e)
         (export-node (error-tree e))))))
  ([tree]
   (reset-callbacks!)
   (try
     (export-node (if (fn? tree) (tree) tree))
     (catch Exception e
       (export-node (error-tree e))))))

(defn- handle-request
  [msg]
  (try
    (let [id (:id msg)
          op (:op msg)
          result (case op
                   "render" (do
                              (wire/finish-callbacks!)
                              {:ok true
                               :tree (export-tree)
                               :themes (theme/wire-sets)})
                   "callback" (wire/apply-callback-msg msg)
                   "provider" (invoke-provider! (:provider-id msg) (:params msg))
                   "directory-picked" (do
                                        (try
                                          (require 'gpui.platform)
                                          ((ns-resolve 'gpui.platform 'deliver-pick!) msg)
                                          (catch Exception e
                                            (binding [*out* *err*]
                                              (println "[clj-gpui] directory-picked failed:"
                                                       (.getMessage e)))))
                                        {:ok true})
                   "preview-captured" (do
                                        (deliver-preview! msg)
                                        {:ok true})
                   "reload" (try
                              (wire/finish-callbacks!)
                              (assoc (reload-app!)
                                     :ok true
                                     :tree (export-tree)
                                     :themes (theme/wire-sets))
                              (catch Exception _
                                (wire/finish-callbacks!)
                                {:ok true
                                 :tree (export-tree)
                                 :themes (theme/wire-sets)}))
                   {:ok false :error (str "unknown op: " op)})]
      (send! (cond-> result
               id (assoc :id id)
               true (assoc :op "response"))))
    (catch Exception e
      (.printStackTrace e)
      (send! {:op "response"
              :id (:id msg)
              :ok false
              :error (str (.getClass e) ": " (.getMessage e))}))))

(defn handle [msg]
  (if (= "provider" (:op msg))
    (future (handle-request msg))
    (handle-request msg)))

(defn- snapshot-mtimes
  []
  (into {}
        (for [^java.io.File f (clj-files (src-root))]
          [(.getCanonicalPath f) (.lastModified f)])))

(defn start-watcher!
  []
  (reset! file-mtimes (snapshot-mtimes))
  (future
    (loop []
      (try
        (Thread/sleep 400)
        (let [now (snapshot-mtimes)
              prev @file-mtimes]
          (when (and (seq prev) (not= prev now))
            (println "[clj-gpui] source change detected, reloading")
            (try
              (reload-app! (changed-clj-files prev now))
              (schedule-render!)
              (catch Exception e
                (binding [*out* *err*]
                  (println "[clj-gpui] reload failed:" (.getMessage e)))
                (schedule-render!))))
          (reset! file-mtimes now))
        (catch Exception e
          (binding [*out* *err*]
            (println "[clj-gpui] watcher error:" (.getMessage e)))))
      (recur))))

(defn start-nrepl!
  []
  (require 'nrepl.server)
  (let [start-server (ns-resolve 'nrepl.server 'start-server)
        preferred (Long/parseLong (env "CLJ_GPUI_NREPL_PORT" "7888"))]
    (try
      (let [server (start-server :port preferred :bind "127.0.0.1")]
        (reset! nrepl-port* preferred)
        (try (spit ".nrepl-port" (str preferred)) (catch Exception _))
        server)
      (catch Exception _
        (let [server (start-server :port 0 :bind "127.0.0.1")
              port (:port server)]
          (reset! nrepl-port* port)
          (try (spit ".nrepl-port" (str port)) (catch Exception _))
          server)))))

(defn send-ready!
  []
  (send! {:op "ready"
          :protocol-version protocol-version
          :nrepl (or @nrepl-port* 0)
          :app (str (app-symbol))}))

(defn read-loop
  [^BufferedReader in]
  (loop []
    (when-let [line (.readLine in)]
      (when-not (str/blank? line)
        (handle (json/read-str line :key-fn keyword)))
      (recur))))
