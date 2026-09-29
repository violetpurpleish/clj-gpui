(ns gpui.package
  "Reusable packaging for clj-gpui applications.

  Application config lives in `gpui.edn` at the project root:

    {:name \"cljdu\"
     :version \"0.1.0\"
     :main cljdu.app/app
     :id \"com.example.cljdu\"
     :icon \"resources/icon.png\"
     :title \"cljdu\"
     :description \"A native disk usage browser\"}

  Then, with a `:build` alias whose `:ns-default` is `gpui.package`
  and tools.build as `:extra-deps` (so the project still sees clj-gpui):

    clj -X:build package

  That command is native-only: macOS produces a `.app`, Linux produces
  an AppImage and a `.deb`. It never cross-compiles.

  ClojureScript apps use :backend :cljs, :cljs-build :app and
  :cljs-output \"target/app.js\" instead of :main. This packages a shadow-cljs
  release with Bun and locked production npm dependencies, without tools.build
  or a JVM in the shipped app. See templates/cljs/.

  LICENSE and NOTICE at the application repo root are copied into the
  package. Extra files can be listed as `:license-files` in `gpui.edn`.
  `:basis-aliases` selects deps.edn aliases for the packaged classpath, e.g.
  platform-specific native libraries; it does not include build tooling."
  (:require [clojure.data.json :as json]
            [clojure.edn :as edn]
            [clojure.java.io :as io]
            [clojure.string :as str]
            [gpui.host :as host]
            [gpui.package-launch :as launch])
  (:import [java.lang ProcessBuilder$Redirect]
           [java.util ArrayList]))

(set! *warn-on-reflection* true)

(def launcher-script launch/launcher-script)

(load "package_build")
(load "package_bun")
(load "package_native")
