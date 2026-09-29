# Application templates

Copy one of these directories next to your clj-gpui checkout:

- [`clj/`](clj/): JVM Clojure, nREPL and source reloading.
- [`cljs/`](cljs/): ClojureScript, shadow-cljs, Bun and npm packages.

Both include native macOS (`.app`) and Linux (AppImage + `.deb`) packaging.
The default dependency assumes `../clj-gpui`; edit `deps.edn` for another
checkout location or a pinned Git dependency. When running a template directly
inside this repository, use `:local/root "../.."` instead.
