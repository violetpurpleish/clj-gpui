(ns gpui.test-runner
  (:require [clojure.test :as t]
            [gpui.boolean-test]
            [gpui.core-test]
            [gpui.dev-test]
            [gpui.package-test]
            [gpui.platform-test]
            [gpui.prod-test]
            [gpui.ratom-test]
            [gpui.runtime-test]
            [gpui.theme-test]
            [gpui.ui-test]))

(defn -main [& _]
  (let [{:keys [fail error]} (t/run-all-tests #"gpui\..*-test")]
    (System/exit (if (pos? (+ fail error)) 1 0))))
