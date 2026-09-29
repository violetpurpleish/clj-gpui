(ns gpui.cljs-runtime-test
  (:require [cljs.test :refer-macros [deftest is async]]
            [gpui.cljs-test-guard]
            [gpui.platform :as platform]
            [gpui.ratom :as r]
            [gpui.runtime :as runtime]
            [gpui.theme :as theme]
            [gpui.ui :as ui]
            ["dayjs" :as dayjs]
            ["node:fs" :as fs]
            ["node:os" :as os]
            ["node:path" :as path]))

(deftest real-atoms-and-npm
  (let [state (r/atom 0 :meta {:source :test} :validator number?)]
    (is (instance? cljs.core/Atom state))
    (is (r/atom? state))
    (is (not (r/atom? (atom 0))))
    (is (= {:source :test} (meta state)))
    (is (= 1 (swap! state inc)))
    (is (= "2026-09-29" (.format (dayjs "2026-09-29") "YYYY-MM-DD")))
    (is (string? (os/platform)))))

(deftest boolean-validation-and-error-recovery
  (let [tree (runtime/export-tree (ui/button "nil flags" {:disabled nil :focus-ring nil}))]
    (is (false? (:disabled tree)))
    (is (contains? tree :focus-ring))
    (is (nil? (:focus-ring tree))))
  (let [tree (runtime/export-tree (ui/window (ui/button "bad" {:disabled "false"})))
        text (pr-str tree)]
    (is (re-find #"ClojureScript error" text))
    (is (re-find #"children\[0\].disabled" text)))
  (is (= "Recovered" (:text (runtime/export-tree (ui/label "Recovered"))))))

(deftest generations-and-native-batches
  (let [calls (atom [])
        tree #(ui/vstack (ui/button "first" (fn [] (swap! calls conj :first)))
                         (ui/input "" {:on-submit (fn [x] (swap! calls conj x))}))
        first-tree (runtime/export-tree (tree))
        first-id (get-in first-tree [:children 0 :on-click])
        submit-id (get-in first-tree [:children 1 :on-submit])]
    (is (:ok (runtime/invoke-callback-batch! [{:id first-id} {:id submit-id :value nil}])))
    (is (= [:first nil] @calls))
    (let [next-tree (runtime/export-tree (tree))]
      (is (not= first-id (get-in next-tree [:children 0 :on-click])))
      (is (false? (:ok (runtime/invoke-callback! first-id)))))))

(deftest reactive-renders-are-coalesced-and-deferred
  (async done
         ((fn []
            (let [messages (atom [])
                  state (r/atom 0)]
              (runtime/bind-connection! {:send #(swap! messages conj %)})
              (runtime/install-render-hook!)
              (dotimes [_ 5] (swap! state inc))
              (js/setTimeout
               (fn []
                 (is (= [{:op "request-render"}] @messages))
                 (reset! messages [])
                 (let [id (:on-click (runtime/export-tree (ui/button "inc" #(swap! state inc))))]
                   (runtime/handle {:op "callback" :callback-id id :defer-render true :id 1})
                   (js/setTimeout
                    (fn []
                      (is (= 6 @state))
                      (is (not-any? #(= "request-render" (:op %)) @messages))
                      (runtime/bind-connection! nil)
                      (done)) 40))) 40))))))

(deftest providers-can-await-other-callbacks-and-refresh
  (async done
         ((fn []
            (let [resolve* (atom nil)
                  provider (fn [_] (js/Promise. (fn [resolve _] (reset! resolve* resolve))))
                  editor #(ui/editor "" {:id "async" :lsp {:hover %}})
                  id (get-in (runtime/export-tree (editor provider)) [:lsp :hover])
                  pending (runtime/invoke-provider! id {:text "hello"})]
              (is (= id (get-in (runtime/export-tree (editor (fn [_] {:version :new}))) [:lsp :hover])))
              (@resolve* {:version :old})
              (-> pending
                  (.then (fn [result]
                           (is (= {:ok true :value {:version "old"}} result))
                           (runtime/invoke-provider! id {})))
                  (.then (fn [result]
                           (is (= {:ok true :value {:version "new"}} result))
                           (runtime/export-tree (ui/label "removed"))
                           (is (false? (:ok (runtime/invoke-provider! id {}))))
                           (done)))
                  (.catch (fn [error] (is false (str error)) (done)))))))))

(deftest in-flight-responses-cannot-cross-connections
  (async done
         ((fn []
            (let [resolve* (atom nil)
                  old-messages (atom [])
                  new-messages (atom [])
                  tree (runtime/export-tree
                        (ui/editor "" {:id "slow" :lsp {:hover (fn [_] (js/Promise. (fn [r _] (reset! resolve* r))))}}))]
              (runtime/bind-connection! {:send #(swap! old-messages conj %)})
              (runtime/handle {:op "provider" :id 7 :provider-id (get-in tree [:lsp :hover])})
              (runtime/bind-connection! {:send #(swap! new-messages conj %)})
              (@resolve* {:contents "old session"})
              (js/setTimeout
               (fn []
                 (is (empty? @new-messages))
                 (is (empty? @old-messages))
                 (runtime/bind-connection! nil)
                 (done)) 10))))))

(deftest platform-and-preview-roundtrip
  (async done
         ((fn []
            (let [messages (atom [])
                  picked (atom nil)]
              (runtime/bind-connection! {:send #(swap! messages conj %)})
              (let [id (platform/pick-directory #(reset! picked %))]
                (runtime/handle {:op "directory-picked" :request-id id :path "/tmp/👩‍💻"})
                (is (= {:path "/tmp/👩‍💻"} @picked)))
              (let [result (runtime/preview-png)
                    id (:request-id (last @messages))
                    png (apply str (repeat 40 "x"))]
                (runtime/handle {:op "preview-captured" :request-id id :png png})
                (-> result
                    (.then (fn [value]
                             (is (= png value))
                             (let [pending (runtime/preview-png)]
                               (runtime/bind-connection! nil)
                               pending)))
                    (.then (fn [value] (is (nil? value)) (done)))
                    (.catch (fn [error] (is false (str error)) (done))))))))))

(deftest theme-file-roundtrip
  (let [dir (fs/mkdtempSync (path/join (os/tmpdir) "clj-gpui-theme-"))
        theme-set {:name "CLJS Test" :themes [{:name "CLJS Dark" :mode :dark :colors {:primary "#123456"}}]}]
    (try
      (theme/register! theme-set)
      (let [file (theme/write-json "CLJS Test" (path/join dir "nested" "theme.json"))]
        (is (= (js->clj (js/JSON.parse (theme/json-str "CLJS Test")))
               (js->clj (js/JSON.parse (fs/readFileSync file "utf8"))))))
      (finally
        (theme/unregister! "CLJS Test")
        (fs/rmSync dir #js {:recursive true :force true})))))

(deftest async-callbacks-preserve-order-and-report-rejections
  (async done
         ((fn []
            (let [events (atom [])
                  tree (runtime/export-tree
                        (ui/vstack
                         (ui/button "async" (fn [] (js/Promise. (fn [resolve _]
                                                                  (js/setTimeout #(do (swap! events conj :first) (resolve nil)) 5)))))
                         (ui/button "next" #(swap! events conj :second))))
                  calls (mapv (fn [button] {:id (:on-click button)}) (:children tree))]
              (-> (runtime/invoke-callback-batch! calls)
                  (.then (fn [result]
                           (is (:ok result))
                           (is (= [:first :second] @events))
                           (let [messages (atom [])
                                 id (:on-click (runtime/export-tree (ui/button "fail" #(js/Promise.reject (js/Error. "async failure")))))]
                             (runtime/bind-connection! {:send #(swap! messages conj %)})
                             (-> (runtime/handle {:op "callback" :id 10 :callback-id id})
                                 (.then (fn []
                                          (is (false? (:ok (last @messages))))
                                          (is (re-find #"async failure" (:error (last @messages))))
                                          (runtime/bind-connection! nil)))))))
                  (.then (fn [] (done)))
                  (.catch (fn [error] (is false (str error)) (done)))))))))

(deftest production-chrome-includes-render-errors
  (runtime/set-production-mode! true)
  (try
    (is (= "app" (:chrome (runtime/export-tree (ui/window {:chrome :dev})))))
    (is (= "app" (:chrome (runtime/export-tree (fn [] (throw (js/Error. "render failed")))))))
    (finally (runtime/set-production-mode! false))))
