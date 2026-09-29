;; Loaded into gpui.package, alongside the JVM build and native assemblers.
(def node-version "22.23.3")

(def node-sha256
  "Official tar.gz checksums from https://nodejs.org/dist/v22.23.3/SHASUMS256.txt."
  {"darwin-arm64" "23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53"
   "darwin-x64" "8a677b0219178efd6eb0e475457c4afb452b521a92f6e67845a73bd85727f2a8"
   "linux-arm64" "5ced2d48d1d7198739b7f86804de0171aefb6823b684b12341d3321afc3cb0b2"
   "linux-x64" "1084aa36196bba4c3a5e69a1ee388a6e4ff729dad09445fbcd434b28fe3c24af"})

(defn node-platform
  "Official Node distribution for the current build machine (no cross compilation)."
  []
  (let [os ({:macos "darwin" :linux "linux"} (os-key))
        arch ({"amd64" "x64" "x86_64" "x64" "aarch64" "arm64" "arm64" "arm64"}
              (System/getProperty "os.arch"))]
    (when-not (and os arch)
      (throw (ex-info "Node packaging supports macOS/Linux on x64 or arm64."
                      {:os (os-key) :arch (System/getProperty "os.arch")})))
    (str os "-" arch)))

(defn node-download-url [platform]
  (str "https://nodejs.org/dist/v" node-version "/node-v" node-version "-" platform ".tar.gz"))

(defn- node-distribution
  ^java.io.File [cfg]
  (let [platform (node-platform)
        ^java.io.File cache (mkdirp (io/file (:target cfg) "node-download"))
        base (str "node-v" node-version "-" platform)
        archive (io/file cache (str base ".tar.gz"))
        expected (get node-sha256 platform)
        dest (io/file cache base)]
    (when-not (and (.isFile archive) (= expected (sha256-hex archive)))
      (println "[clj-gpui] downloading Node" node-version platform)
      (sh! ["curl" "--fail" "--location" "--silent" "--show-error"
            "--output" (.getPath archive) (node-download-url platform)] {})
      (when-not (= expected (sha256-hex archive))
        (.delete archive)
        (throw (ex-info "Node distribution checksum mismatch" {:platform platform}))))
    ;; Re-extract the verified archive, so interrupted or modified caches are repaired.
    (when (.exists dest) (sh! ["rm" "-rf" (.getPath dest)] {}))
    (sh! ["tar" "-xzf" (.getPath archive) "-C" (.getPath cache)] {})
    (.getCanonicalFile dest)))

(defn- production-manifest
  [manifest]
  (when (or (seq (get manifest "workspaces"))
            (some (fn [[_ version]] (re-find #"^(file:|link:|workspace:|\./|\.\./)" version))
                  (concat (get manifest "dependencies") (get manifest "optionalDependencies"))))
    (throw (ex-info "Packaging needs independently installable npm dependencies. Publish or pack local/workspace dependencies first."
                    {})))
  ;; Root lifecycle scripts commonly invoke dev-only compilers. Dependency install
  ;; scripts still run, including native addon builds with the bundled Node ABI.
  (dissoc manifest "scripts"))

(defn node-app
  "Release a shadow-cljs :node-script and stage its npm dependencies and Node runtime.

  Requires :cljs-build, :cljs-output, an installed shadow-cljs npm CLI, and
  package-lock.json. resources/ is copied to app/resources/. Runs npm ci with
  --omit=dev using the same official Node binary shipped in the application."
  [opts]
  (let [cfg (load-config opts)
        _ (when-not (= :cljs (:backend cfg))
            (throw (ex-info "node-app requires :backend :cljs" {})))
        manifest (production-manifest (json/read-str (slurp "package.json")))
        lockfile (io/file "package-lock.json")
        cli (io/file "node_modules/shadow-cljs/cli/runner.js")
        output (io/file (:cljs-output cfg))
        _ (when-not (.isFile lockfile)
            (throw (ex-info "ClojureScript packaging needs package-lock.json. Run npm install and commit the lockfile." {})))
        _ (when-not (.isFile cli)
            (throw (ex-info "Install shadow-cljs before packaging: npm ci" {})))
        distribution (node-distribution cfg)
        node (io/file distribution "bin/node")
        npm (io/file distribution "lib/node_modules/npm/bin/npm-cli.js")
        env {"PATH" (str (io/file distribution "bin") java.io.File/pathSeparator (System/getenv "PATH"))
             "npm_config_nodedir" (.getPath distribution)}
        app (io/file (:target cfg) "cljs-app")
        runtime (io/file (:target cfg) "node-runtime")]
    (println "[clj-gpui] shadow-cljs release" (:cljs-build cfg))
    (when (.exists output) (io/delete-file output))
    (sh! [(.getPath node) (.getPath cli) "release" (as-str (:cljs-build cfg))] {:env env})
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
    (copy-file lockfile (io/file app "package-lock.json"))
    (println "[clj-gpui] installing production npm dependencies")
    (sh! (into [(.getPath node) (.getPath npm) "ci" "--omit=dev" "--engine-strict" "--no-audit" "--no-fund"]
               (:npm-args cfg))
         {:dir app :env env})
    (chmod-exec (copy-file node (io/file runtime "bin/node")))
    (copy-file (io/file distribution "LICENSE") (io/file runtime "LICENSE"))
    (assoc cfg :node-app app :runtime runtime)))

(defn- prepare-package
  [opts]
  (let [cfg (load-config opts)]
    (if (= :cljs (:backend cfg))
      (-> cfg node-app host)
      (-> cfg uberjar host jre))))

(defn- copy-payload
  "Copy the backend payload into macOS Resources or a Linux runtime root."
  [cfg ^java.io.File root macos?]
  (if (= :cljs (:backend cfg))
    (sh! ["cp" "-R" (.getPath ^java.io.File (:node-app cfg)) (.getPath (io/file root "app"))] {})
    (copy-file (:jar cfg) (io/file (if macos? root (mkdirp (io/file root "lib")))
                                   (str (:name cfg) ".jar"))))
  (sh! ["cp" "-R" (.getPath ^java.io.File (:runtime cfg)) (.getPath (io/file root "runtime"))] {}))
