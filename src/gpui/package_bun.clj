;; Loaded into gpui.package, alongside the JVM build and native assemblers.
(def bun-version "1.4.2")

(def bun-sha256
  "SHA-256 of the official bun-v1.4.2 release ZIPs. Use baseline builds on x64."
  {"darwin-aarch64" "90987a3a16d7db556d886ac3d551e7b6d3edf0a1cf43acaed622e8676be1d12f"
   "darwin-x64-baseline" "bad5bbd6cf14d0980d115f5954c9ff904df619d5e994d2da1ffccd3f316300b0"
   "linux-aarch64" "54328bbc2d9c8e0c9f892c544d66c57a83b84139e34909e5ee81758f1ac8fda7"
   "linux-x64-baseline" "c678040f14fe0440eb839d37cbd0ce4c051a32da72806ac97de6a6aab6bf728f"})

(def bun-license-sha256
  "b9caf52728691b4057e371232c221a132883198be2f3d2ddf92c90404c984b1a")

(defn bun-platform
  "Official Bun distribution for this build machine (no cross compilation)."
  []
  (let [os ({:macos "darwin" :linux "linux"} (os-key))
        arch ({"amd64" "x64-baseline" "x86_64" "x64-baseline"
               "aarch64" "aarch64" "arm64" "aarch64"}
              (System/getProperty "os.arch"))]
    (when-not (and os arch)
      (throw (ex-info "Bun packaging supports macOS/Linux on x64 or arm64."
                      {:os (os-key) :arch (System/getProperty "os.arch")})))
    (str os "-" arch)))

(defn bun-download-url [platform]
  (str "https://github.com/oven-sh/bun/releases/download/bun-v" bun-version
       "/bun-" platform ".zip"))

(defn- verified-download!
  [^java.io.File file url expected]
  (when-not (and (.isFile file) (= expected (sha256-hex file)))
    (sh! ["curl" "--fail" "--location" "--silent" "--show-error"
          "--output" (.getPath file) url] {})
    (when-not (= expected (sha256-hex file))
      (.delete file)
      (throw (ex-info "Bun download checksum mismatch" {:url url}))))
  file)

(defn- bun-distribution
  ^java.io.File [cfg]
  (let [platform (bun-platform)
        ^java.io.File cache (mkdirp (io/file (:target cfg) "bun-download" bun-version))
        base (str "bun-" platform)
        archive (io/file cache (str base ".zip"))
        dest (io/file cache base)]
    (println "[clj-gpui] preparing Bun" bun-version platform)
    (verified-download! archive (bun-download-url platform) (get bun-sha256 platform))
    ;; Re-extract the verified archive to repair interrupted or modified caches.
    (when (.exists dest) (sh! ["rm" "-rf" (.getPath dest)] {}))
    (sh! ["unzip" "-q" (.getPath archive) "-d" (.getPath cache)] {})
    (verified-download! (io/file cache "LICENSE.md")
                        (str "https://raw.githubusercontent.com/oven-sh/bun/bun-v" bun-version "/LICENSE.md")
                        bun-license-sha256)
    (copy-file (io/file cache "LICENSE.md") (io/file dest "LICENSE.md"))
    (.getCanonicalFile dest)))

(defn- production-manifest
  [manifest]
  (when (or (seq (get manifest "workspaces"))
            (some (fn [[_ version]] (re-find #"^(file:|link:|workspace:|\./|\.\./)" version))
                  (concat (get manifest "dependencies") (get manifest "optionalDependencies"))))
    (throw (ex-info "Packaging needs independently installable npm dependencies. Publish or pack local/workspace dependencies first."
                    {})))
  ;; Root lifecycle scripts commonly invoke dev-only compilers. Preserve Bun
  ;; trust policy: omitted trustedDependencies uses its built-in list; explicit
  ;; application overrides remain untouched.
  (dissoc manifest "scripts"))

(defn bun-app
  "Release a shadow-cljs :node-script and stage its npm dependencies and Bun runtime.

  Requires :cljs-build, :cljs-output, an installed shadow-cljs CLI, and
  bun.lock. resources/ is copied to app/resources/. Installs frozen production
  dependencies using the same official Bun binary shipped in the application."
  [opts]
  (let [cfg (load-config opts)
        _ (when-not (= :cljs (:backend cfg))
            (throw (ex-info "bun-app requires :backend :cljs" {})))
        manifest (production-manifest (json/read-str (slurp "package.json")))
        lockfile (io/file "bun.lock")
        cli (io/file "node_modules/shadow-cljs/cli/runner.js")
        output (io/file (:cljs-output cfg))
        _ (when-not (.isFile lockfile)
            (throw (ex-info "ClojureScript packaging needs bun.lock. Run bun install and commit the lockfile." {})))
        _ (when-not (.isFile cli)
            (throw (ex-info "Install shadow-cljs before packaging: bun install --frozen-lockfile" {})))
        distribution (bun-distribution cfg)
        bun (io/file distribution "bun")
        env {"PATH" (str distribution java.io.File/pathSeparator (System/getenv "PATH"))}
        app (io/file (:target cfg) "cljs-app")
        runtime (io/file (:target cfg) "bun-runtime")]
    (println "[clj-gpui] shadow-cljs release" (:cljs-build cfg))
    (when (.exists output) (io/delete-file output))
    (sh! [(.getPath bun) "--no-install" (.getPath cli) "release" (as-str (:cljs-build cfg))] {:env env})
    (when-not (.isFile output)
      (throw (ex-info "shadow-cljs did not produce :cljs-output; use a :node-script build with matching :output-to."
                      {:output (.getPath output)})))
    (doseq [^java.io.File dir [app runtime]]
      (when (.exists dir) (sh! ["rm" "-rf" (.getPath dir)] {}))
      (mkdirp dir))
    (copy-file output (io/file app "main.cjs"))
    (when (.isDirectory (io/file "resources"))
      (sh! ["cp" "-R" "resources" (.getPath (io/file app "resources"))] {}))
    (spit (io/file app "package.json") (json/write-str manifest))
    (copy-file lockfile (io/file app "bun.lock"))
    (println "[clj-gpui] installing production npm dependencies")
    (sh! [(.getPath bun) "--bun" "install" "--frozen-lockfile" "--production" "--linker" "hoisted"]
         {:dir app :env env})
    (chmod-exec (copy-file bun (io/file runtime "bin/bun")))
    (copy-file (io/file distribution "LICENSE.md") (io/file runtime "LICENSE.md"))
    (assoc cfg :bun-app app :runtime runtime)))

(defn- prepare-package
  [opts]
  (let [cfg (load-config opts)]
    (if (= :cljs (:backend cfg))
      (-> cfg bun-app host)
      (-> cfg uberjar host jre))))

(defn- copy-payload
  "Copy the backend payload into macOS Resources or a Linux runtime root."
  [cfg ^java.io.File root macos?]
  (if (= :cljs (:backend cfg))
    (sh! ["cp" "-R" (.getPath ^java.io.File (:bun-app cfg)) (.getPath (io/file root "app"))] {})
    (copy-file (:jar cfg) (io/file (if macos? root (mkdirp (io/file root "lib")))
                                   (str (:name cfg) ".jar"))))
  (sh! ["cp" "-R" (.getPath ^java.io.File (:runtime cfg)) (.getPath (io/file root "runtime"))] {}))
